use crate::models::{BranchRow, PullRequestRow, Repo, WorktreeRow};
use comfy_table::{presets::UTF8_BORDERS_ONLY, Cell, Table};
use serde_json::json;

pub fn print_json<T: serde::Serialize>(key: &str, value: T) -> crate::errors::Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({ key: value }))
            .map_err(|e| crate::errors::AppError::Message(format!("Could not serialize JSON: {e}")))?
    );
    Ok(())
}

pub fn print_worktrees(rows: &[WorktreeRow]) {
    let mut table = table();
    table.set_header(["Worktree", "Local Branch", "Remote Branch", "Repo", "Workspace ID", "Windows"]);
    for row in rows {
        let workspace_id = row
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.get("id"))
            .and_then(|id| id.as_u64())
            .map(|id| id.to_string())
            .unwrap_or_default();
        let windows = row
            .workspace
            .as_ref()
            .map(|_| row.windows.len().to_string())
            .unwrap_or_default();
        table.add_row([
            row.path.display().to_string(),
            row.local_branch.clone().unwrap_or_default(),
            row.remote_branch.clone().unwrap_or_default(),
            row.repo.display().to_string(),
            workspace_id,
            windows,
        ]);
    }
    println!("{table}");
}

pub fn print_branches(rows: &[BranchRow]) {
    let mut table = table();
    table.set_header(["Local Branch", "Remote Branch", "Repo", "Worktree", "Workspace ID"]);
    for row in rows {
        table.add_row([
            row.local_branch.clone().unwrap_or_default(),
            row.remote_branch.clone().unwrap_or_default(),
            row.repo.display().to_string(),
            row.worktree.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
            row.workspace_id.map(|id| id.to_string()).unwrap_or_default(),
        ]);
    }
    println!("{table}");
}

pub fn print_pull_requests(rows: &[PullRequestRow]) {
    let mut table = table();
    table.set_header(["PR", "Status", "Local Branch", "Remote Branch", "Repo", "Worktree", "Workspace ID"]);
    for row in rows {
        table.add_row([
            row.pr_number.map(|id| id.to_string()).unwrap_or_default(),
            row.status.clone().unwrap_or_default(),
            row.local_branch.clone().unwrap_or_default(),
            row.remote_branch.clone().unwrap_or_default(),
            row.repo.display().to_string(),
            row.worktree.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
            row.workspace_id.map(|id| id.to_string()).unwrap_or_default(),
        ]);
    }
    println!("{table}");
}

pub fn print_repos(rows: &[(Repo, Option<String>)]) {
    let mut table = table();
    table.set_header(["Repo", "Repo Origin", "Bare", "Setup", "Teardown"]);
    for (repo, origin) in rows {
        table.add_row(vec![
            Cell::new(repo.path.display().to_string()),
            Cell::new(origin.clone().unwrap_or_default()),
            Cell::new(repo.bare.to_string()),
            Cell::new(repo.setup.clone().unwrap_or_default()),
            Cell::new(repo.teardown.clone().unwrap_or_default()),
        ]);
    }
    println!("{table}");
}

fn table() -> Table {
    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table
}
