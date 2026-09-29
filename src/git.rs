use crate::errors::{message, AppError, Result};
use crate::models::{GitWorktree, Repo};
use crate::paths::normalize_path;
use crate::process::CommandRunner;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

fn git(runner: &dyn CommandRunner, repo: &Path, args: &[&str]) -> Result<crate::process::CmdOutput> {
    let mut full = vec!["-C".to_string(), repo.display().to_string()];
    full.extend(args.iter().map(|arg| (*arg).to_string()));
    runner
        .run("git", &full, None)
        .map_err(|e| AppError::Message(format!("git was not found or failed to start: {e}")))
}

pub fn is_git_repository(runner: &dyn CommandRunner, path: &Path) -> bool {
    git(runner, path, &["rev-parse", "--git-dir"])
        .map(|out| out.status == 0)
        .unwrap_or(false)
}

pub fn is_bare_repository(runner: &dyn CommandRunner, path: &Path) -> bool {
    git(runner, path, &["rev-parse", "--is-bare-repository"])
        .map(|out| out.status == 0 && out.stdout.trim() == "true")
        .unwrap_or(false)
}

pub fn common_dir(runner: &dyn CommandRunner, path: &Path) -> Option<PathBuf> {
    let out = git(runner, path, &["rev-parse", "--path-format=absolute", "--git-common-dir"]).ok()?;
    if out.status != 0 {
        return None;
    }
    let stdout = out.stdout.trim();
    (!stdout.is_empty()).then(|| normalize_path(stdout))
}

pub fn worktree_root(runner: &dyn CommandRunner, path: &Path) -> Option<PathBuf> {
    let out = git(runner, path, &["rev-parse", "--show-toplevel"]).ok()?;
    if out.status != 0 {
        return None;
    }
    let stdout = out.stdout.trim();
    (!stdout.is_empty()).then(|| normalize_path(stdout))
}

pub fn default_repo_path(runner: &dyn CommandRunner) -> Result<PathBuf> {
    let cwd = std::env::current_dir()
        .map_err(|e| AppError::Message(format!("Could not get current directory: {e}")))?;
    let root = worktree_root(runner, &cwd);
    let common = common_dir(runner, &cwd);

    if let Some(root) = root {
        if let Some(common) = common {
            if common.file_name().and_then(|s| s.to_str()) == Some(".git") {
                return Ok(common.parent().unwrap_or(&root).to_path_buf());
            }
        }
        return Ok(root);
    }

    let out = git(
        runner,
        &cwd,
        &["rev-parse", "--is-bare-repository", "--path-format=absolute", "--git-dir"],
    )?;
    let lines: Vec<&str> = out.stdout.lines().collect();
    if out.status == 0 && lines.len() >= 2 && lines[0] == "true" {
        return Ok(normalize_path(lines[1]));
    }

    message("No --repo was provided and the current directory is not inside a git repository")
}

pub fn branch_names(
    runner: &dyn CommandRunner,
    repo: &Path,
    local: bool,
    remote: bool,
) -> Result<Vec<String>> {
    let mut args = vec!["for-each-ref", "--format=%(refname:short)"];
    if local {
        args.push("refs/heads");
    }
    if remote {
        args.push("refs/remotes");
    }
    let out = git(runner, repo, &args)?;
    if out.status != 0 {
        return message(format!(
            "Could not list branches for {}{}",
            repo.display(),
            suffix_stderr(&out.stderr)
        ));
    }
    Ok(out
        .stdout
        .lines()
        .filter(|branch| !branch.ends_with("/HEAD"))
        .filter(|branch| !remote || branch.contains('/'))
        .map(ToOwned::to_owned)
        .collect())
}

pub fn branch_upstream(runner: &dyn CommandRunner, repo: &Path, branch: &str) -> Option<String> {
    let refname = format!("refs/heads/{branch}");
    let out = git(runner, repo, &["for-each-ref", "--format=%(upstream:short)", &refname]).ok()?;
    if out.status != 0 {
        return None;
    }
    let stdout = out.stdout.trim();
    (!stdout.is_empty()).then(|| stdout.to_string())
}

#[derive(Debug, Default)]
struct LocalBranchInfo {
    upstream: Option<String>,
    commit_timestamp: Option<i64>,
}

fn local_branch_info(runner: &dyn CommandRunner, repo: &Path) -> HashMap<String, LocalBranchInfo> {
    let out = match git(
        runner,
        repo,
        &[
            "for-each-ref",
            "--format=%(refname:short)%09%(upstream:short)%09%(committerdate:unix)",
            "refs/heads",
        ],
    ) {
        Ok(out) if out.status == 0 => out,
        _ => return HashMap::new(),
    };
    out.stdout
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(3, '\t');
            let branch = fields.next()?;
            let upstream = fields
                .next()
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            let commit_timestamp = fields.next().and_then(|value| value.parse().ok());
            Some((
                branch.to_string(),
                LocalBranchInfo {
                    upstream,
                    commit_timestamp,
                },
            ))
        })
        .collect()
}

pub fn branch_upstreams(runner: &dyn CommandRunner, repo: &Path) -> HashMap<String, String> {
    local_branch_info(runner, repo)
        .into_iter()
        .filter_map(|(branch, info)| info.upstream.map(|upstream| (branch, upstream)))
        .collect()
}

pub fn remote_url(runner: &dyn CommandRunner, repo: &Path, remote: &str) -> Option<String> {
    let out = git(runner, repo, &["remote", "get-url", remote]).ok()?;
    if out.status != 0 {
        return None;
    }
    let stdout = out.stdout.trim();
    (!stdout.is_empty()).then(|| stdout.to_string())
}

pub fn worktrees(runner: &dyn CommandRunner, repo: &Repo) -> Result<Vec<GitWorktree>> {
    let out = git(runner, &repo.path, &["worktree", "list", "--porcelain"])?;
    if out.status != 0 {
        return message(format!(
            "Could not list worktrees for {}{}",
            repo.path.display(),
            suffix_stderr(&out.stderr)
        ));
    }
    let branch_info = local_branch_info(runner, &repo.path);
    let merged_branches = branches_merged_into_default(runner, &repo.path);
    let mut rows = parse_worktree_list(&out.stdout, &repo.path, |branch| {
        branch_info
            .get(branch)
            .map(|info| (info.upstream.clone(), info.commit_timestamp))
            .unwrap_or_default()
    });
    for row in &mut rows {
        row.is_merged_into_default_branch = row.local_branch.as_ref().and_then(|branch| {
            merged_branches.as_ref().and_then(|info| info.status(branch))
        });
    }
    if is_bare_repository(runner, &repo.path) {
        rows.retain(|row| row.path != normalize_path(&repo.path));
    }
    Ok(rows)
}

pub fn worktree_branches(
    runner: &dyn CommandRunner,
    repo: &Path,
) -> Result<HashMap<String, (PathBuf, Option<String>)>> {
    let out = git(runner, repo, &["worktree", "list", "--porcelain"])?;
    if out.status != 0 {
        return message(format!(
            "Could not list worktrees for {}{}",
            repo.display(),
            suffix_stderr(&out.stderr)
        ));
    }
    let branch_info = local_branch_info(runner, repo);
    let rows = parse_worktree_list(&out.stdout, repo, |branch| {
        branch_info
            .get(branch)
            .map(|info| (info.upstream.clone(), info.commit_timestamp))
            .unwrap_or_default()
    });
    Ok(rows
        .into_iter()
        .filter_map(|row| row.local_branch.map(|branch| (branch, (row.path, row.remote_branch))))
        .collect())
}

pub fn local_branch_exists(runner: &dyn CommandRunner, repo: &Path, branch: &str) -> bool {
    let refname = format!("refs/heads/{branch}");
    git(runner, repo, &["show-ref", "--verify", "--quiet", &refname])
        .map(|out| out.status == 0)
        .unwrap_or(false)
}

pub fn remote_branch_exists(runner: &dyn CommandRunner, repo: &Path, branch: &str) -> bool {
    let refname = format!("refs/remotes/{branch}");
    git(runner, repo, &["show-ref", "--verify", "--quiet", &refname])
        .map(|out| out.status == 0)
        .unwrap_or(false)
}

pub fn default_origin_branch(runner: &dyn CommandRunner, repo: &Path) -> Result<String> {
    let out = git(runner, repo, &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])?;
    if out.status == 0 {
        let branch = out.stdout.trim();
        if !branch.is_empty() {
            return Ok(branch.to_string());
        }
    }
    for branch in ["origin/main", "origin/master"] {
        if remote_branch_exists(runner, repo, branch) {
            return Ok(branch.to_string());
        }
    }
    message(format!("Could not determine origin default branch for {}", repo.display()))
}

pub struct DefaultMergeInfo {
    local_branch: String,
    merged_branches: HashSet<String>,
}

impl DefaultMergeInfo {
    pub fn status(&self, branch: &str) -> Option<bool> {
        if branch == self.local_branch || branch == format!("origin/{}", self.local_branch) {
            None
        } else {
            Some(self.merged_branches.contains(branch))
        }
    }
}

pub fn branches_merged_into_default(
    runner: &dyn CommandRunner,
    repo: &Path,
) -> Option<DefaultMergeInfo> {
    for (base, fallback_local_branch) in [
        ("refs/remotes/origin/HEAD", None),
        ("origin/main", Some("main")),
        ("origin/master", Some("master")),
    ] {
        let merged = format!("--merged={base}");
        let out = git(
            runner,
            repo,
            &[
                "for-each-ref",
                "--format=%(refname:short)%09%(symref:short)",
                &merged,
                "refs/heads",
                "refs/remotes",
            ],
        )
        .ok()?;
        if out.status == 0 {
            let mut local_branch = fallback_local_branch.map(ToOwned::to_owned);
            let mut merged_branches = HashSet::new();
            for line in out.stdout.lines() {
                let (branch, symref) = line.split_once('\t').unwrap_or((line, ""));
                if branch == "origin/HEAD" {
                    local_branch = symref.strip_prefix("origin/").map(ToOwned::to_owned);
                } else {
                    merged_branches.insert(branch.to_string());
                }
            }
            if let Some(local_branch) = local_branch {
                return Some(DefaultMergeInfo {
                    local_branch,
                    merged_branches,
                });
            }
        }
    }
    None
}

pub fn create_branch_worktree(
    runner: &dyn CommandRunner,
    repo: &Path,
    new_branch: &str,
    from_branch: &str,
    worktree: &Path,
) -> Result<()> {
    let args = vec![
        "-C".to_string(),
        repo.display().to_string(),
        "worktree".to_string(),
        "add".to_string(),
        worktree.display().to_string(),
        "-b".to_string(),
        new_branch.to_string(),
        from_branch.to_string(),
    ];
    let out = runner
        .run("git", &args, None)
        .map_err(|e| AppError::Message(format!("git was not found or failed to start: {e}")))?;
    if out.status != 0 {
        return message(format!(
            "Could not create worktree {} for {} from {}{}",
            worktree.display(),
            new_branch,
            from_branch,
            suffix_stderr(&out.stderr)
        ));
    }
    Ok(())
}

pub fn create_existing_branch_worktree(
    runner: &dyn CommandRunner,
    repo: &Path,
    branch: &str,
    worktree: &Path,
) -> Result<()> {
    let args = vec![
        "-C".to_string(),
        repo.display().to_string(),
        "worktree".to_string(),
        "add".to_string(),
        worktree.display().to_string(),
        branch.to_string(),
    ];
    let out = runner
        .run("git", &args, None)
        .map_err(|e| AppError::Message(format!("git was not found or failed to start: {e}")))?;
    if out.status != 0 {
        return message(format!(
            "Could not create worktree {} for {}{}",
            worktree.display(),
            branch,
            suffix_stderr(&out.stderr)
        ));
    }
    Ok(())
}

pub fn unset_current_branch_upstream(runner: &dyn CommandRunner, worktree: &Path) -> Result<()> {
    let upstream = git(
        runner,
        worktree,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    )?;
    if upstream.status != 0 {
        return Ok(());
    }
    let out = git(runner, worktree, &["branch", "--unset-upstream"])?;
    if out.status != 0 {
        return message(format!(
            "Could not unset upstream for branch in {}{}",
            worktree.display(),
            suffix_stderr(&out.stderr)
        ));
    }
    Ok(())
}

pub fn status_porcelain(runner: &dyn CommandRunner, worktree: &Path) -> Result<String> {
    let out = git(runner, worktree, &["status", "--porcelain"])?;
    if out.status != 0 {
        return message(format!(
            "Could not check worktree status for {}{}",
            worktree.display(),
            suffix_stderr(&out.stderr)
        ));
    }
    Ok(out.stdout.trim().to_string())
}

pub fn remove_worktree(runner: &dyn CommandRunner, repo: &Path, worktree: &Path) -> Result<crate::process::CmdOutput> {
    git(runner, repo, &["worktree", "remove", &worktree.display().to_string()])
}

pub fn parse_worktree_list<F>(text: &str, repo: &Path, branch_info: F) -> Vec<GitWorktree>
where
    F: Fn(&str) -> (Option<String>, Option<i64>),
{
    let mut rows = Vec::new();
    let mut current: Option<GitWorktree> = None;
    for line in text.lines() {
        if line.is_empty() {
            if let Some(row) = current.take() {
                rows.push(row);
            }
            continue;
        }
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        match key {
            "worktree" => {
                current = Some(GitWorktree {
                    path: normalize_path(value),
                    repo: repo.to_path_buf(),
                    local_branch: None,
                    remote_branch: None,
                    commit_timestamp: None,
                    is_merged_into_default_branch: None,
                });
            }
            "branch" if value.starts_with("refs/heads/") => {
                if let Some(row) = &mut current {
                    let branch = value.trim_start_matches("refs/heads/");
                    let (upstream, commit_timestamp) = branch_info(branch);
                    row.local_branch = Some(branch.to_string());
                    row.remote_branch = upstream;
                    row.commit_timestamp = commit_timestamp;
                }
            }
            _ => {}
        }
    }
    if let Some(row) = current {
        rows.push(row);
    }
    rows
}

fn suffix_stderr(stderr: &str) -> String {
    let stderr = stderr.trim();
    if stderr.is_empty() {
        String::new()
    } else {
        format!(": {stderr}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_status_excludes_local_and_remote_default_branches() {
        let info = DefaultMergeInfo {
            local_branch: "main".into(),
            merged_branches: ["main", "origin/main", "feature", "origin/feature"]
                .into_iter().map(String::from).collect(),
        };
        assert_eq!(info.status("main"), None);
        assert_eq!(info.status("origin/main"), None);
        assert_eq!(info.status("feature"), Some(true));
        assert_eq!(info.status("origin/feature"), Some(true));
        assert_eq!(info.status("unmerged"), Some(false));
    }

    #[test]
    fn parses_porcelain_worktree_list() {
        let rows = parse_worktree_list(
            "worktree /repo\nbranch refs/heads/main\n\nworktree /repo/feat\nbranch refs/heads/feat\n",
            Path::new("/repo"),
            |branch| {
                (
                    (branch == "feat").then(|| "origin/feat".to_string()),
                    (branch == "feat").then_some(123),
                )
            },
        );
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].local_branch.as_deref(), Some("feat"));
        assert_eq!(rows[1].remote_branch.as_deref(), Some("origin/feat"));
        assert_eq!(rows[1].commit_timestamp, Some(123));
    }

    #[test]
    fn remote_branch_filter_requires_branch_name() {
        let branches = ["origin", "origin/HEAD", "origin/main", "upstream/topic"];
        let filtered: Vec<&str> = branches
            .into_iter()
            .filter(|branch| !branch.ends_with("/HEAD"))
            .filter(|branch| branch.contains('/'))
            .collect();
        assert_eq!(filtered, ["origin/main", "upstream/topic"]);
    }
}
