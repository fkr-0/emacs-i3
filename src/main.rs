mod emacs;

use anyhow::{anyhow, Context, Result};
use clap::{App, AppSettings, Arg};
use emacs::EmacsClient;
use i3ipc::reply::{Node, NodeLayout};
use i3ipc::I3Connection;
use std::env;

fn main() -> Result<()> {
    let matches = App::new("emacs-i3")
        .setting(AppSettings::TrailingVarArg)
        .version(env!("CARGO_PKG_VERSION"))
        .author("Jos van Bakel <jos@codeaddict.org>")
        .about("Emacs i3 integration")
        .arg(
            Arg::with_name("emacs")
                .short("e")
                .long("emacs")
                .help("Override command to send to Emacs")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("command")
                .multiple(true)
                .help("Command to send to i3 and Emacs (unless overriden)"),
        )
        .get_matches();

    let command_args = matches
        .values_of("command")
        .unwrap_or_default()
        .collect::<Vec<_>>();
    let i3_command = normalize_command(&command_args)?;

    let emacs_command = if let Some(emacs_arg) = matches.value_of("emacs") {
        emacs_arg.to_owned()
    } else {
        i3_command.clone()
    };

    let emacs_socket_path = env::var("XDG_RUNTIME_DIR")
        .map(|dir| dir + "/emacs/server")
        .ok();

    let mut i3 = I3Connection::connect().context("failed to connect to i3 IPC socket")?;
    let tree = i3.get_tree().context("failed to read i3 tree")?;
    let node = find_focused(&tree);

    let mut to_i3 = true;

    if node.map(is_emacs).unwrap_or(false) {
        if let Some(socket_path) = emacs_socket_path {
            let mut emacs = EmacsClient::new(&socket_path);
            match emacs.eval(&emacs_i3_command(&emacs_command)) {
                Ok(value) => to_i3 = emacs_value_is_nil(&value),
                Err(error) => {
                    eprintln!("emacs command failed, falling back to i3: {}", error);
                }
            }
        } else {
            eprintln!("XDG_RUNTIME_DIR is not set, falling back to i3");
        }
    }

    if to_i3 {
        let i3_command = fallback_i3_command(&i3_command, &tree);
        let response = i3
            .run_command(&i3_command)
            .with_context(|| format!("failed to send i3 command '{}'", i3_command))?;
        let mut errors = Vec::new();
        for outcome in response.outcomes {
            if let Some(msg) = outcome.error {
                errors.push(msg);
            }
        }
        if !errors.is_empty() {
            return Err(anyhow!(
                "i3 command '{}' failed: {}",
                i3_command,
                errors.join("; ")
            ));
        }
    }

    Ok(())
}

fn normalize_command(args: &[&str]) -> Result<String> {
    if args.is_empty() {
        return Err(anyhow!("missing i3 command"));
    }

    Ok(args.join(" "))
}

fn fallback_i3_command(command: &str, tree: &Node) -> String {
    if focused_inside_tablike_container(tree) {
        match command {
            "focus left" => return "focus prev".to_owned(),
            "focus right" => return "focus next".to_owned(),
            _ => {}
        }
    }

    command.to_owned()
}

fn emacs_value_is_nil(value: &str) -> bool {
    value.trim_end() == "nil"
}

fn focused_inside_tablike_container(node: &Node) -> bool {
    node.nodes.iter().any(|child| {
        (matches!(node.layout, NodeLayout::Tabbed | NodeLayout::Stacked) && subtree_focused(child))
            || focused_inside_tablike_container(child)
    }) || node.floating_nodes.iter().any(focused_inside_tablike_container)
}

fn subtree_focused(node: &Node) -> bool {
    node.focused
        || node.nodes.iter().any(subtree_focused)
        || node.floating_nodes.iter().any(subtree_focused)
}

/// Find the focused window in the tree.
fn find_focused(node: &Node) -> Option<&Node> {
    if node.focused {
        Some(node)
    } else {
        node.nodes.iter().find_map(find_focused)
    }
}

/// Determine if the node in question is an Emacs window.
fn is_emacs(node: &Node) -> bool {
    if let Some(props) = node.window_properties.as_ref() {
        if let Some(class) = props.get(&i3ipc::reply::WindowProperty::Class) {
            return class == "Emacs";
        }
    }

    node.name.as_ref().unwrap().starts_with("emacs: ")
}

/// Format the command to an expression to be run in Emacs.
fn emacs_i3_command(command: &str) -> String {
    let escaped_command = command.replace("\"", "\\\"");
    format!("(my/emacs-i3-command \"{}\")", escaped_command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_preserves_i3_focus_direction() {
        assert_eq!(normalize_command(&["focus", "left"]).unwrap(), "focus left");
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

        assert_eq!(fallback_i3_command("focus left", &tree), "focus prev");
        assert_eq!(fallback_i3_command("focus right", &tree), "focus next");
        assert_eq!(fallback_i3_command("focus up", &tree), "focus up");
    }

    #[test]
    fn split_container_preserves_spatial_focus() {
        let tree = node(
            NodeLayout::SplitH,
            false,
            vec![node(NodeLayout::SplitH, true, vec![])],
        );

        assert_eq!(fallback_i3_command("focus left", &tree), "focus left");
        assert_eq!(fallback_i3_command("focus right", &tree), "focus right");
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
        }
    }
}
