use crate::errors::{AppError, Result};
use crate::paths::config_path;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub terminal_command: TerminalCommand,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum TerminalCommand {
    String(String),
    Args(Vec<String>),
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path();
        if !path.exists() {
            return Err(AppError::Message(format!(
                "Config file does not exist: {}",
                path.display()
            )));
        }
        let text = std::fs::read_to_string(&path).map_err(|source| AppError::Io {
            path: path.clone(),
            source,
        })?;
        toml::from_str(&text).map_err(|source| AppError::Toml { path, source })
    }

    pub fn terminal_argv(&self) -> Result<Vec<String>> {
        let argv = match &self.terminal_command {
            TerminalCommand::String(value) => shlex::split(value).ok_or_else(|| {
                AppError::Message(format!(
                    "{} must define 'terminal-command' as a shell-style string",
                    config_path().display()
                ))
            })?,
            TerminalCommand::Args(args) => args.clone(),
        };

        if argv.is_empty() {
            return Err(AppError::Message(format!(
                "{} has an empty 'terminal-command'",
                config_path().display()
            )));
        }

        Ok(argv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_array_command() {
        let cfg: Config = toml::from_str(r#"terminal-command = ["ghostty", "+new-window"]"#).unwrap();
        assert_eq!(cfg.terminal_argv().unwrap(), ["ghostty", "+new-window"]);
    }

    #[test]
    fn parses_string_command() {
        let cfg: Config = toml::from_str(r#"terminal-command = "ghostty +new-window""#).unwrap();
        assert_eq!(cfg.terminal_argv().unwrap(), ["ghostty", "+new-window"]);
    }
}

