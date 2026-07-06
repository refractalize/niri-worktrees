use crate::errors::{message, Result};
use std::env;
use std::path::{Path, PathBuf};

const STORE_DIR_NAME: &str = "niri-worktrees";
const STORE_FILE_NAME: &str = "worktrees.json";
const REPOS_FILE_NAME: &str = "repos.json";
const CONFIG_FILE_NAME: &str = "config.toml";

pub fn normalize_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let path = if path.starts_with("~") {
        if let Some(home) = home_dir() {
            if path == Path::new("~") {
                home
            } else if let Ok(stripped) = path.strip_prefix("~") {
                home.join(stripped)
            } else {
                path.to_path_buf()
            }
        } else {
            path.to_path_buf()
        }
    } else {
        path.to_path_buf()
    };

    if path.is_absolute() {
        path
    } else {
        env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(path)
    }
}

pub fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

pub fn config_path() -> PathBuf {
    if let Some(config) = env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(config).join(STORE_DIR_NAME).join(CONFIG_FILE_NAME)
    } else {
        home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config")
            .join(STORE_DIR_NAME)
            .join(CONFIG_FILE_NAME)
    }
}

pub fn runtime_store_path() -> Result<PathBuf> {
    let runtime = env::var_os("XDG_RUNTIME_DIR");
    let Some(runtime) = runtime else {
        return message("XDG_RUNTIME_DIR is not set");
    };
    Ok(PathBuf::from(runtime).join(STORE_DIR_NAME).join(STORE_FILE_NAME))
}

pub fn repos_store_path() -> PathBuf {
    if let Some(data) = env::var_os("XDG_DATA_HOME") {
        PathBuf::from(data).join(STORE_DIR_NAME).join(REPOS_FILE_NAME)
    } else {
        home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".local")
            .join("share")
            .join(STORE_DIR_NAME)
            .join(REPOS_FILE_NAME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_path_uses_xdg_config_home() {
        env::set_var("XDG_CONFIG_HOME", "/tmp/config-test");
        assert_eq!(
            config_path(),
            PathBuf::from("/tmp/config-test/niri-worktrees/config.toml")
        );
        env::remove_var("XDG_CONFIG_HOME");
    }
}

