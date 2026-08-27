use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_emacs-i3"))
}

#[test]
fn help_is_available_without_an_i3_session() {
    let output = command().arg("--help").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Emacs i3 integration"));
    assert!(stdout.contains("--emacs"));
}

#[test]
fn version_comes_from_cargo_package_metadata() {
    let output = command().arg("--version").output().unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        concat!("emacs-i3 ", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn effective_config_loads_explicit_toml_without_i3() {
    let directory = temp_directory();
    let config = directory.join("config.toml");
    fs::write(
        &config,
        r#"
timeout_ms = 91
emacs_classes = ["MyEmacs"]
tabbed_horizontal_focus = false

[aliases]
"go west" = "focus left"
"#,
    )
    .unwrap();

    let output = command()
        .args([
            "--config",
            config.to_str().unwrap(),
            "--print-effective-config",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("timeout_ms = 91"));
    assert!(stdout.contains("MyEmacs"));
    assert!(stdout.contains("tabbed_horizontal_focus = false"));
    assert!(stdout.contains("go west"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn effective_config_respects_cli_env_config_precedence() {
    let directory = temp_directory();
    let config = directory.join("config.toml");
    let config_socket = directory.join("config.sock");
    let env_socket = directory.join("env.sock");
    let cli_socket = directory.join("cli.sock");
    fs::write(
        &config,
        format!(
            "socket = {:?}\ntimeout_ms = 300\n",
            config_socket.to_string_lossy()
        ),
    )
    .unwrap();

    let output = command()
        .args([
            "--config",
            config.to_str().unwrap(),
            "--socket",
            cli_socket.to_str().unwrap(),
            "--timeout-ms",
            "100",
            "--print-effective-config",
        ])
        .env("EMACS_I3_SOCKET", &env_socket)
        .env("EMACS_I3_TIMEOUT_MS", "200")
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    let effective: toml::Value = toml::from_slice(&output.stdout).unwrap();
    assert_eq!(
        effective["socket"].as_str().unwrap(),
        cli_socket.to_str().unwrap()
    );
    assert_eq!(effective["timeout_ms"].as_integer().unwrap(), 100);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn diagnostic_json_is_read_only_and_reports_socket_env() {
    let directory = temp_directory();
    let socket = directory.join("missing-emacs.sock");
    let output = command()
        .args(["--diagnose", "--json"])
        .env("EMACS_I3_SOCKET", &socket)
        .env("I3SOCK", directory.join("missing-i3.sock"))
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["socket_path"], socket.to_string_lossy().as_ref());
    assert_eq!(report["socket_exists"], false);
    assert_eq!(report["i3_connected"], false);
    assert!(report["i3_error"].as_str().unwrap().contains("i3"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn completion_generation_does_not_require_i3() {
    let output = command()
        .args(["--generate-completion", "bash"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("emacs-i3"));
    assert!(stdout.contains("--diagnose"));
}

#[test]
fn explicit_missing_config_fails_before_i3() {
    let directory = temp_directory();
    let missing = directory.join("missing.toml");
    let output = command()
        .args([
            "--config",
            missing.to_str().unwrap(),
            "--print-effective-config",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("config file does not exist")
    );
    fs::remove_dir_all(directory).unwrap();
}

fn temp_directory() -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "emacs-i3-cli-test-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    directory
}

#[test]
fn missing_command_fails_before_i3_ipc_is_needed() {
    let output = command().output().unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("missing i3 command")
    );
}
