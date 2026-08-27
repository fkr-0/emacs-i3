use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const CONFIG_ENV: &str = "EMACS_I3_CONFIG";
pub const SOCKET_ENV: &str = "EMACS_I3_SOCKET";
pub const TIMEOUT_ENV: &str = "EMACS_I3_TIMEOUT_MS";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub socket: Option<PathBuf>,
    pub timeout_ms: u64,
    pub emacs_classes: Vec<String>,
    pub emacs_name_prefixes: Vec<String>,
    pub tabbed_horizontal_focus: bool,
    pub aliases: BTreeMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            socket: None,
            timeout_ms: 250,
            emacs_classes: vec!["Emacs".to_owned()],
            emacs_name_prefixes: vec!["emacs: ".to_owned()],
            tabbed_horizontal_focus: true,
            aliases: BTreeMap::new(),
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        if self.timeout_ms == 0 {
            return Err(anyhow!("timeout_ms must be greater than zero"));
        }
        if self.emacs_classes.iter().any(|value| value.is_empty()) {
            return Err(anyhow!("emacs_classes cannot contain an empty value"));
        }
        if self
            .emacs_name_prefixes
            .iter()
            .any(|value| value.is_empty())
        {
            return Err(anyhow!("emacs_name_prefixes cannot contain an empty value"));
        }
        Ok(())
    }

    pub fn expand_alias(&self, command: &str) -> Result<String> {
        let mut value = command.to_owned();
        let mut seen = BTreeSet::new();
        while let Some(next) = self.aliases.get(&value) {
            if !seen.insert(value.clone()) {
                return Err(anyhow!("command alias cycle detected at {:?}", value));
            }
            value = next.clone();
        }
        Ok(value)
    }
}

#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub config: Config,
    pub path: Option<PathBuf>,
    pub loaded: bool,
}

pub fn load(cli_path: Option<&Path>) -> Result<LoadedConfig> {
    let env_path = env::var_os(CONFIG_ENV).map(PathBuf::from);
    let explicit_path = cli_path.map(PathBuf::from).or(env_path);
    let path = explicit_path.clone().or_else(default_config_path);

    let (config, loaded) = match path.as_deref() {
        Some(path) if path.exists() => {
            let text = fs::read_to_string(path)
                .with_context(|| format!("failed to read config {}", path.display()))?;
            let config = toml::from_str::<Config>(&text)
                .with_context(|| format!("failed to parse config {}", path.display()))?;
            (config, true)
        }
        Some(path) if explicit_path.is_some() => {
            return Err(anyhow!("config file does not exist: {}", path.display()));
        }
        _ => (Config::default(), false),
    };

    config.validate()?;
    Ok(LoadedConfig {
        config,
        path,
        loaded,
    })
}

pub fn default_config_path() -> Option<PathBuf> {
    if let Some(root) = env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(root).join("emacs-i3/config.toml"));
    }
    env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".config/emacs-i3/config.toml"))
}

pub fn resolve_socket(cli_socket: Option<&Path>, config: &Config) -> Option<PathBuf> {
    cli_socket
        .map(PathBuf::from)
        .or_else(|| env::var_os(SOCKET_ENV).map(PathBuf::from))
        .or_else(|| config.socket.clone())
        .or_else(|| {
            env::var_os("XDG_RUNTIME_DIR")
                .map(PathBuf::from)
                .map(|root| root.join("emacs/server"))
        })
}

pub fn resolve_timeout(cli_timeout_ms: Option<u64>, config: &Config) -> Result<u64> {
    let value = match cli_timeout_ms {
        Some(value) => value,
        None => match env::var(TIMEOUT_ENV) {
            Ok(value) => value
                .parse::<u64>()
                .with_context(|| format!("{TIMEOUT_ENV} must be a positive integer"))?,
            Err(env::VarError::NotPresent) => config.timeout_ms,
            Err(error) => return Err(error).context(format!("failed to read {TIMEOUT_ENV}")),
        },
    };
    if value == 0 {
        return Err(anyhow!("timeout must be greater than zero milliseconds"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_preserve_legacy_detection_and_fallback() {
        let config = Config::default();
        assert_eq!(config.emacs_classes, ["Emacs"]);
        assert_eq!(config.emacs_name_prefixes, ["emacs: "]);
        assert!(config.tabbed_horizontal_focus);
        assert_eq!(config.timeout_ms, 250);
    }

    #[test]
    fn aliases_expand_transitively_and_cycles_fail_closed() {
        let mut config = Config::default();
        config
            .aliases
            .insert("focus west".to_owned(), "focus left".to_owned());
        config
            .aliases
            .insert("go west".to_owned(), "focus west".to_owned());
        assert_eq!(config.expand_alias("go west").unwrap(), "focus left");

        config.aliases.insert("a".to_owned(), "b".to_owned());
        config.aliases.insert("b".to_owned(), "a".to_owned());
        assert!(config.expand_alias("a").is_err());
    }

    #[test]
    fn unknown_config_fields_are_rejected() {
        let error = toml::from_str::<Config>("mystery = true").unwrap_err();
        assert!(error.to_string().contains("unknown field"));
    }
}
