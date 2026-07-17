use crate::errors::{AppError, Result};
use crate::models::{Repo, RepoStore, WorktreeMapping, WorktreeStore};
use crate::paths::{normalize_path, repos_store_path, runtime_store_path};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub trait Store {
    fn load_worktrees(&self) -> Result<Vec<WorktreeMapping>>;
    fn save_worktrees(&self, worktrees: &[WorktreeMapping]) -> Result<()>;
    fn load_repos(&self) -> Result<Vec<Repo>>;
    fn save_repos(&self, repos: &[Repo]) -> Result<()>;
}

#[derive(Debug, Default)]
pub struct FileStore;

impl Store for FileStore {
    fn load_worktrees(&self) -> Result<Vec<WorktreeMapping>> {
        let path = runtime_store_path()?;
        if !path.exists() {
            return Ok(vec![]);
        }
        let value = read_json(&path)?;
        let mut out = vec![];
        if let Some(entries) = value.get("worktrees").and_then(Value::as_array) {
            for entry in entries {
                if let Some(mapping) = normalize_mapping(entry) {
                    out.push(mapping);
                }
            }
        }
        Ok(out)
    }

    fn save_worktrees(&self, worktrees: &[WorktreeMapping]) -> Result<()> {
        let path = runtime_store_path()?;
        write_json(&path, &WorktreeStore { worktrees: worktrees.to_vec() })
    }

    fn load_repos(&self) -> Result<Vec<Repo>> {
        let path = repos_store_path();
        if !path.exists() {
            let legacy = self.load_legacy_repos()?;
            if !legacy.is_empty() {
                self.save_repos(&legacy)?;
            }
            return Ok(legacy);
        }
        let value = read_json(&path)?;
        let mut out = vec![];
        if let Some(entries) = value.get("repos").and_then(Value::as_array) {
            for entry in entries {
                if let Some(repo) = normalize_repo(entry) {
                    out.push(repo);
                }
            }
        }
        Ok(out)
    }

    fn save_repos(&self, repos: &[Repo]) -> Result<()> {
        let path = repos_store_path();
        write_json(&path, &RepoStore { repos: repos.to_vec() })
    }
}

impl FileStore {
    fn load_legacy_repos(&self) -> Result<Vec<Repo>> {
        let path = runtime_store_path()?;
        if !path.exists() {
            return Ok(vec![]);
        }
        let value = read_json(&path)?;
        let mut out = vec![];
        if let Some(entries) = value.get("repos").and_then(Value::as_array) {
            for entry in entries {
                if let Some(repo) = normalize_repo(entry) {
                    out.push(repo);
                }
            }
        }
        Ok(out)
    }
}

fn read_json(path: &Path) -> Result<Value> {
    let text = std::fs::read_to_string(path).map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| AppError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| AppError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|s| s.to_str()).unwrap_or("json")
    ));
    let text = serde_json::to_string_pretty(value)
        .map_err(|source| AppError::Json { path: path.to_path_buf(), source })?
        + "\n";
    std::fs::write(&tmp, text).map_err(|source| AppError::Io {
        path: tmp.clone(),
        source,
    })?;
    std::fs::rename(&tmp, path).map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn normalize_mapping(value: &Value) -> Option<WorktreeMapping> {
    let obj = value.as_object()?;
    let path = obj
        .get("path")
        .or_else(|| obj.get("worktree_path"))?
        .as_str()?;
    let workspace_id = obj
        .get("workspace_id")
        .or_else(|| obj.get("niri_workspace_id"))?
        .as_u64()?;
    Some(WorktreeMapping {
        path: normalize_path(path),
        workspace_id,
    })
}

fn normalize_repo(value: &Value) -> Option<Repo> {
    let obj = value.as_object()?;
    let path = obj.get("path")?.as_str()?;
    Some(Repo {
        path: normalize_path(path),
        bare: obj.get("bare").and_then(Value::as_bool).unwrap_or(false),
        setup: obj.get("setup").and_then(command_array),
        teardown: obj.get("teardown").and_then(command_array),
    })
}

fn command_array(value: &Value) -> Option<Vec<String>> {
    value
        .as_array()?
        .iter()
        .map(|value| value.as_str().map(ToOwned::to_owned))
        .collect()
}

pub fn set_worktree_mapping(store: &dyn Store, path: PathBuf, workspace_id: u64) -> Result<()> {
    let mut worktrees = store.load_worktrees()?;
    worktrees.retain(|entry| entry.path != path && entry.workspace_id != workspace_id);
    worktrees.push(WorktreeMapping { path, workspace_id });
    store.save_worktrees(&worktrees)
}

pub fn unset_worktree_mapping(
    store: &dyn Store,
    path: Option<&Path>,
    workspace_id: Option<u64>,
) -> Result<()> {
    let mut worktrees = store.load_worktrees()?;
    worktrees.retain(|entry| {
        let path_matches = path.is_none_or(|path| entry.path == path);
        let workspace_matches = workspace_id.is_none_or(|id| entry.workspace_id == id);
        !(path_matches && workspace_matches)
    });
    store.save_worktrees(&worktrees)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct MemoryStore {
        worktrees: RefCell<Vec<WorktreeMapping>>,
    }

    impl Store for MemoryStore {
        fn load_worktrees(&self) -> Result<Vec<WorktreeMapping>> {
            Ok(self.worktrees.borrow().clone())
        }

        fn save_worktrees(&self, worktrees: &[WorktreeMapping]) -> Result<()> {
            self.worktrees.replace(worktrees.to_vec());
            Ok(())
        }

        fn load_repos(&self) -> Result<Vec<Repo>> {
            Ok(vec![])
        }

        fn save_repos(&self, _repos: &[Repo]) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn normalizes_legacy_mapping_keys() {
        let value = serde_json::json!({"worktree_path": "/tmp/a", "niri_workspace_id": 4});
        assert_eq!(normalize_mapping(&value).unwrap().workspace_id, 4);
    }

    #[test]
    fn normalizes_repo_defaults_bare_false() {
        let value = serde_json::json!({"path": "/tmp/repo"});
        assert!(!normalize_repo(&value).unwrap().bare);
    }

    #[test]
    fn setting_mapping_replaces_existing_path_and_workspace() {
        let store = MemoryStore {
            worktrees: RefCell::new(vec![
                WorktreeMapping {
                    path: PathBuf::from("/repo/a"),
                    workspace_id: 1,
                },
                WorktreeMapping {
                    path: PathBuf::from("/repo/b"),
                    workspace_id: 2,
                },
                WorktreeMapping {
                    path: PathBuf::from("/repo/c"),
                    workspace_id: 3,
                },
            ]),
        };

        set_worktree_mapping(&store, PathBuf::from("/repo/c"), 2).unwrap();

        assert_eq!(
            store.worktrees.borrow().as_slice(),
            [
                WorktreeMapping {
                    path: PathBuf::from("/repo/a"),
                    workspace_id: 1,
                },
                WorktreeMapping {
                    path: PathBuf::from("/repo/c"),
                    workspace_id: 2,
                },
            ]
        );
    }
}
