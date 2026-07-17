use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorktreeMapping {
    pub path: PathBuf,
    pub workspace_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorktreeStore {
    #[serde(default)]
    pub worktrees: Vec<WorktreeMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Repo {
    pub path: PathBuf,
    #[serde(default)]
    pub bare: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teardown: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepoStore {
    #[serde(default)]
    pub repos: Vec<Repo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitWorktree {
    pub path: PathBuf,
    pub repo: PathBuf,
    pub local_branch: Option<String>,
    pub remote_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BranchRow {
    pub local_branch: Option<String>,
    pub remote_branch: Option<String>,
    pub repo: PathBuf,
    pub worktree: Option<PathBuf>,
    pub workspace_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullRequestRow {
    pub pr_number: Option<u64>,
    pub status: Option<String>,
    pub local_branch: Option<String>,
    pub remote_branch: Option<String>,
    pub repo: PathBuf,
    pub worktree: Option<PathBuf>,
    pub workspace_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeRow {
    pub path: PathBuf,
    pub repo: PathBuf,
    pub local_branch: Option<String>,
    pub remote_branch: Option<String>,
    pub workspace: Option<serde_json::Value>,
    pub windows: Vec<serde_json::Value>,
    #[serde(skip)]
    pub is_focused: bool,
    #[serde(skip)]
    pub focus_timestamp: Option<(i64, i64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub head_ref_name: String,
    pub pr_number: Option<u64>,
    pub status: Option<String>,
}
