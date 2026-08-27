mod command;
mod config;
mod emacs;

use anyhow::{Result, anyhow};
use clap::{CommandFactory, Parser, ValueEnum};
use clap_complete::{generate, shells};
use command::{Direction, Operation, ParsedCommand};
use config::{Config, LoadedConfig};
use emacs::EmacsClient;
use i3ipc::I3Connection;
use i3ipc::reply::{Node, NodeLayout, WindowProperty};
use serde::Serialize;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(
    name = "emacs-i3",
    version,
    author = "Jos van Bakel <jos@codeaddict.org>",
    about = "Emacs i3 integration"
)]
struct Cli {
    /// Override command sent to Emacs while retaining the original i3 fallback.
    #[arg(short = 'e', long)]
    emacs: Option<String>,

    /// Override the Emacs server Unix socket path.
    #[arg(long, value_name = "PATH")]
    socket: Option<PathBuf>,

    /// Override Emacs IPC connect/read/write timeout in milliseconds.
    #[arg(long, value_name = "MS")]
    timeout_ms: Option<u64>,

    /// Load configuration from this TOML file.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Explain routing decisions on stderr. Repeat for configuration details.
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Inspect configuration, socket discovery, i3 connectivity, and focus without acting.
    #[arg(long)]
    diagnose: bool,

    /// Emit diagnostic output as JSON (requires --diagnose).
    #[arg(long, requires = "diagnose")]
    json: bool,

    /// Print the merged on-disk/default configuration as TOML and exit.
    #[arg(long)]
    print_effective_config: bool,

    /// Generate shell completion source and exit.
    #[arg(long, value_enum, value_name = "SHELL")]
    generate_completion: Option<CompletionShell>,

    /// i3 command to offer to Emacs and then, when unhandled, to i3.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    command: Vec<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CompletionShell {
    Bash,
    Elvish,
    Fish,
    Powershell,
    Zsh,
}

#[derive(Debug)]
struct RuntimeConfig {
    loaded: LoadedConfig,
    socket_path: Option<PathBuf>,
    timeout_ms: u64,
}

#[derive(Debug, Serialize)]
struct DiagnosticReport {
    version: &'static str,
    config_path: Option<String>,
    config_loaded: bool,
    socket_path: Option<String>,
    socket_exists: bool,
    timeout_ms: u64,
    i3_connected: bool,
    i3_error: Option<String>,
    focused_node_id: Option<i64>,
    focused_node_name: Option<String>,
    focused_is_emacs: Option<bool>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let runtime = resolve_runtime(&cli)?;

    if cli.print_effective_config {
        ensure_no_command(&cli, "--print-effective-config")?;
        let mut effective = runtime.loaded.config.clone();
        effective.socket = runtime.socket_path.clone();
        effective.timeout_ms = runtime.timeout_ms;
        print!("{}", toml::to_string_pretty(&effective)?);
        return Ok(());
    }
    if let Some(shell) = cli.generate_completion {
        ensure_no_command(&cli, "--generate-completion")?;
        write_completion(shell);
        return Ok(());
    }
    if cli.diagnose {
        ensure_no_command(&cli, "--diagnose")?;
        return run_diagnostics(&runtime, cli.json);
    }

    let command = normalize_command(&cli.command)?;
    let command = runtime.loaded.config.expand_alias(&command)?;
    let parsed = ParsedCommand::parse(&command);
    let emacs_command = cli.emacs.as_deref().unwrap_or(&command);

    verbose_config(&cli, &runtime);

    let mut i3 = connect_i3()?;
    let tree = read_i3_tree(&mut i3)?;
    let node = find_focused(&tree);
    let focused_is_emacs = node
        .map(|node| is_emacs(node, &runtime.loaded.config))
        .unwrap_or(false);

    if cli.verbose > 0 {
        eprintln!(
            "route: command={:?} operation={:?} focused_id={:?} focused_is_emacs={}",
            command,
            parsed.operation,
            node.map(|node| node.id),
            focused_is_emacs
        );
    }

    let mut to_i3 = true;
    if focused_is_emacs {
        match runtime.socket_path.as_deref() {
            Some(socket_path) => {
                let mut emacs =
                    EmacsClient::new(socket_path, Duration::from_millis(runtime.timeout_ms));
                match emacs.eval(&emacs_i3_command(emacs_command)) {
                    Ok(value) => {
                        to_i3 = emacs_value_is_nil(&value);
                        if cli.verbose > 0 {
                            eprintln!("route: emacs_result={:?} handled={}", value, !to_i3);
                        }
                    }
                    Err(error) => {
                        eprintln!("emacs command failed, falling back to i3: {error}");
                    }
                }
            }
            None => eprintln!(
                "no Emacs server socket could be resolved, falling back to i3; use --socket or {}",
                config::SOCKET_ENV
            ),
        }
    }

    if to_i3 {
        let i3_command = fallback_i3_command(&parsed, &tree, &runtime.loaded.config);
        if cli.verbose > 0 {
            eprintln!("route: i3_command={i3_command:?}");
        }
        run_i3_command(&mut i3, &i3_command)?;
    }

    Ok(())
}

fn resolve_runtime(cli: &Cli) -> Result<RuntimeConfig> {
    let loaded = config::load(cli.config.as_deref())?;
    let socket_path = config::resolve_socket(cli.socket.as_deref(), &loaded.config);
    let timeout_ms = config::resolve_timeout(cli.timeout_ms, &loaded.config)?;
    Ok(RuntimeConfig {
        loaded,
        socket_path,
        timeout_ms,
    })
}

fn ensure_no_command(cli: &Cli, mode: &str) -> Result<()> {
    if cli.command.is_empty() && cli.emacs.is_none() {
        Ok(())
    } else {
        Err(anyhow!("{mode} cannot be combined with a window command"))
    }
}

fn write_completion(shell: CompletionShell) {
    let mut command = Cli::command();
    let name = command.get_name().to_owned();
    let mut stdout = io::stdout();
    match shell {
        CompletionShell::Bash => generate(shells::Bash, &mut command, name, &mut stdout),
        CompletionShell::Elvish => generate(shells::Elvish, &mut command, name, &mut stdout),
        CompletionShell::Fish => generate(shells::Fish, &mut command, name, &mut stdout),
        CompletionShell::Powershell => {
            generate(shells::PowerShell, &mut command, name, &mut stdout)
        }
        CompletionShell::Zsh => generate(shells::Zsh, &mut command, name, &mut stdout),
    }
}

fn run_diagnostics(runtime: &RuntimeConfig, json: bool) -> Result<()> {
    let mut report = DiagnosticReport {
        version: env!("CARGO_PKG_VERSION"),
        config_path: runtime.loaded.path.as_deref().map(path_to_display_string),
        config_loaded: runtime.loaded.loaded,
        socket_path: runtime.socket_path.as_deref().map(path_to_display_string),
        socket_exists: runtime
            .socket_path
            .as_deref()
            .map(Path::exists)
            .unwrap_or(false),
        timeout_ms: runtime.timeout_ms,
        i3_connected: false,
        i3_error: None,
        focused_node_id: None,
        focused_node_name: None,
        focused_is_emacs: None,
    };

    match I3Connection::connect() {
        Ok(mut i3) => match i3.get_tree() {
            Ok(tree) => {
                report.i3_connected = true;
                if let Some(node) = find_focused(&tree) {
                    report.focused_node_id = Some(node.id);
                    report.focused_node_name = node.name.clone();
                    report.focused_is_emacs = Some(is_emacs(node, &runtime.loaded.config));
                }
            }
            Err(error) => report.i3_error = Some(format!("failed to read i3 tree: {error:?}")),
        },
        Err(error) => report.i3_error = Some(format!("failed to connect to i3: {error:?}")),
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("emacs-i3 diagnostics");
        println!("  version:          {}", report.version);
        println!(
            "  config:           {}{}",
            report.config_path.as_deref().unwrap_or("<none>"),
            if report.config_loaded {
                " (loaded)"
            } else {
                " (defaults)"
            }
        );
        println!(
            "  Emacs socket:     {}{}",
            report.socket_path.as_deref().unwrap_or("<unresolved>"),
            if report.socket_exists {
                " (exists)"
            } else {
                ""
            }
        );
        println!("  IPC timeout:      {} ms", report.timeout_ms);
        println!("  i3 connected:     {}", report.i3_connected);
        println!("  focused node:     {:?}", report.focused_node_id);
        println!("  focused is Emacs: {:?}", report.focused_is_emacs);
        if let Some(error) = &report.i3_error {
            println!("  i3 error:         {error}");
        }
    }
    Ok(())
}

fn verbose_config(cli: &Cli, runtime: &RuntimeConfig) {
    if cli.verbose < 2 {
        return;
    }
    eprintln!(
        "config: path={:?} loaded={} socket={:?} timeout_ms={} aliases={}",
        runtime.loaded.path,
        runtime.loaded.loaded,
        runtime.socket_path,
        runtime.timeout_ms,
        runtime.loaded.config.aliases.len()
    );
}

fn connect_i3() -> Result<I3Connection> {
    I3Connection::connect()
        .map_err(|error| anyhow!("failed to connect to i3 IPC socket: {error:?}"))
}

fn read_i3_tree(i3: &mut I3Connection) -> Result<Node> {
    i3.get_tree()
        .map_err(|error| anyhow!("failed to read i3 tree: {error:?}"))
}

fn run_i3_command(i3: &mut I3Connection, command: &str) -> Result<()> {
    let response = i3
        .run_command(command)
        .map_err(|error| anyhow!("failed to send i3 command {command:?}: {error:?}"))?;
    let errors = response
        .outcomes
        .into_iter()
        .filter_map(|outcome| outcome.error)
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(
            "i3 command '{}' failed: {}",
            command,
            errors.join("; ")
        ))
    }
}

fn normalize_command(args: &[String]) -> Result<String> {
    if args.is_empty() {
        return Err(anyhow!("missing i3 command"));
    }
    Ok(args.join(" "))
}

fn fallback_i3_command(command: &ParsedCommand, tree: &Node, config: &Config) -> String {
    if config.tabbed_horizontal_focus && focused_inside_tablike_container(tree) {
        match command.operation {
            Operation::Focus(Direction::Left) => return "focus prev".to_owned(),
            Operation::Focus(Direction::Right) => return "focus next".to_owned(),
            _ => {}
        }
    }
    command.raw.clone()
}

fn emacs_value_is_nil(value: &str) -> bool {
    value.trim_end() == "nil"
}

fn focused_inside_tablike_container(node: &Node) -> bool {
    node.nodes.iter().any(|child| {
        (matches!(node.layout, NodeLayout::Tabbed | NodeLayout::Stacked) && subtree_focused(child))
            || focused_inside_tablike_container(child)
    }) || node
        .floating_nodes
        .iter()
        .any(focused_inside_tablike_container)
}

fn subtree_focused(node: &Node) -> bool {
    node.focused
        || node.nodes.iter().any(subtree_focused)
        || node.floating_nodes.iter().any(subtree_focused)
}

fn find_focused(node: &Node) -> Option<&Node> {
    if node.focused {
        Some(node)
    } else {
        node.nodes
            .iter()
            .find_map(find_focused)
            .or_else(|| node.floating_nodes.iter().find_map(find_focused))
    }
}

fn is_emacs(node: &Node, config: &Config) -> bool {
    if let Some(props) = node.window_properties.as_ref() {
        if let Some(class) = props.get(&WindowProperty::Class) {
            if config
                .emacs_classes
                .iter()
                .any(|expected| class == expected)
            {
                return true;
            }
        }
    }
    node.name
        .as_deref()
        .map(|name| {
            config
                .emacs_name_prefixes
                .iter()
                .any(|prefix| name.starts_with(prefix))
        })
        .unwrap_or(false)
}

fn emacs_i3_command(command: &str) -> String {
    let escaped_command = escape_elisp_string(command);
    format!("(my/emacs-i3-command \"{escaped_command}\")")
}

fn escape_elisp_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character => escaped.push(character),
        }
    }
    escaped
}

fn path_to_display_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_preserves_i3_focus_direction() {
        assert_eq!(
            normalize_command(&["focus".to_owned(), "left".to_owned()]).unwrap(),
            "focus left"
        );
    }

    #[test]
    fn command_rejects_empty_input() {
        assert!(normalize_command(&[]).is_err());
    }

    #[test]
    fn emacs_nil_allows_protocol_trailing_newline() {
        assert!(emacs_value_is_nil("nil"));
        assert!(emacs_value_is_nil("nil\n"));
        assert!(!emacs_value_is_nil("t\n"));
    }

    #[test]
    fn tabbed_container_maps_horizontal_focus_to_tab_order() {
        let tree = node(
            NodeLayout::SplitH,
            false,
            vec![node(
                NodeLayout::Tabbed,
                false,
                vec![node(NodeLayout::SplitH, true, vec![])],
            )],
        );
        let config = Config::default();

        assert_eq!(
            fallback_i3_command(&ParsedCommand::parse("focus left"), &tree, &config),
            "focus prev"
        );
        assert_eq!(
            fallback_i3_command(&ParsedCommand::parse("focus right"), &tree, &config),
            "focus next"
        );
        assert_eq!(
            fallback_i3_command(&ParsedCommand::parse("focus up"), &tree, &config),
            "focus up"
        );
    }

    #[test]
    fn tabbed_focus_rule_can_be_disabled() {
        let tree = node(
            NodeLayout::Tabbed,
            false,
            vec![node(NodeLayout::SplitH, true, vec![])],
        );
        let config = Config {
            tabbed_horizontal_focus: false,
            ..Config::default()
        };
        assert_eq!(
            fallback_i3_command(&ParsedCommand::parse("focus left"), &tree, &config),
            "focus left"
        );
    }

    #[test]
    fn split_container_preserves_spatial_focus() {
        let tree = node(
            NodeLayout::SplitH,
            false,
            vec![node(NodeLayout::SplitH, true, vec![])],
        );
        let config = Config::default();

        assert_eq!(
            fallback_i3_command(&ParsedCommand::parse("focus left"), &tree, &config),
            "focus left"
        );
        assert_eq!(
            fallback_i3_command(&ParsedCommand::parse("focus right"), &tree, &config),
            "focus right"
        );
    }

    #[test]
    fn focused_floating_window_is_found() {
        let mut tree = node(NodeLayout::SplitH, false, vec![]);
        tree.floating_nodes
            .push(node(NodeLayout::SplitH, true, vec![]));

        assert!(find_focused(&tree).unwrap().focused);
    }

    #[test]
    fn nameless_node_without_window_properties_is_not_emacs() {
        let node = node(NodeLayout::SplitH, true, vec![]);
        assert!(!is_emacs(&node, &Config::default()));
    }

    #[test]
    fn configured_emacs_class_and_legacy_name_are_detected() {
        let mut by_class = node(NodeLayout::SplitH, true, vec![]);
        by_class.window_properties = Some(
            [(WindowProperty::Class, "CustomEmacs".to_owned())]
                .iter()
                .cloned()
                .collect(),
        );
        let config = Config {
            emacs_classes: vec!["CustomEmacs".to_owned()],
            emacs_name_prefixes: vec!["editor: ".to_owned()],
            ..Config::default()
        };
        assert!(is_emacs(&by_class, &config));

        let mut by_name = node(NodeLayout::SplitH, true, vec![]);
        by_name.name = Some("editor: scratch".to_owned());
        assert!(is_emacs(&by_name, &config));
    }

    #[test]
    fn emacs_command_escapes_elisp_string_metacharacters() {
        assert_eq!(
            emacs_i3_command("exec echo \"a\\b\"\nnext"),
            "(my/emacs-i3-command \"exec echo \\\"a\\\\b\\\"\\nnext\")"
        );
    }

    fn node(layout: NodeLayout, focused: bool, nodes: Vec<Node>) -> Node {
        Node {
            focus: Vec::new(),
            nodes,
            floating_nodes: Vec::new(),
            id: 0,
            name: None,
            nodetype: i3ipc::reply::NodeType::Con,
            border: i3ipc::reply::NodeBorder::Normal,
            current_border_width: 0,
            layout,
            percent: None,
            rect: (0, 0, 0, 0),
            window_rect: (0, 0, 0, 0),
            deco_rect: (0, 0, 0, 0),
            geometry: (0, 0, 0, 0),
            window: None,
            window_properties: None,
            urgent: false,
            focused,
            marks: Vec::new(),
            sticky: false,
            fullscreen_mode: i3ipc::reply::NodeFullScreenMode::None,
            floating: i3ipc::reply::NodeFloating::AutoOff,
        }
    }
}
