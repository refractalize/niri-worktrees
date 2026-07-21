use crate::cli;
use crate::errors::{message, AppError, CommandError, Result};
use crate::git;
use crate::github;
use crate::models::{BranchRow, GitWorktree, PullRequestRow, Repo, WorktreeMapping, WorktreeRow};
use crate::niri::{self, NiriClient, SocketNiriClient};
use crate::output;
use crate::paths::normalize_path;
use crate::process::{split_program, CommandRunner, RealCommandRunner};
use crate::store::{self, FileStore, Store};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub struct RealEnv {
    pub store: FileStore,
    pub runner: RealCommandRunner,
    pub niri: SocketNiriClient,
}

impl RealEnv {
    pub fn new() -> Self {
        Self {
            store: FileStore,
            runner: RealCommandRunner,
            niri: SocketNiriClient,
        }
    }
}

pub trait Env {
    fn store(&self) -> &dyn Store;
    fn runner(&self) -> &dyn CommandRunner;
    fn niri(&self) -> &dyn NiriClient;
}

impl Env for RealEnv {
    fn store(&self) -> &dyn Store {
        &self.store
    }

    fn runner(&self) -> &dyn CommandRunner {
        &self.runner
    }

    fn niri(&self) -> &dyn NiriClient {
        &self.niri
    }
}

pub fn dispatch(command: cli::Command, env: &dyn Env) -> Result<()> {
    match command {
        cli::Command::ListWorktrees(args) => cmd_list_worktrees(args, env),
        cli::Command::ListBranches(args) => cmd_list_branches(args, env),
        cli::Command::ListPullRequests(args) => cmd_list_pull_requests(args, env),
        cli::Command::SetWorkspace(args) => cmd_set_workspace(args, env),
        cli::Command::UnsetWorkspace(args) => cmd_unset_workspace(args, env),
        cli::Command::GetWorktree(args) => cmd_get_worktree(args, env),
        cli::Command::FocusWorktree(args) => {
            focus_worktree(env, normalize_path(args.worktree))?;
            Ok(())
        }
        cli::Command::CreateWorktree(args) => cmd_create_worktree(args, env),
        cli::Command::CreateBranch(args) => cmd_create_branch(args, env),
        cli::Command::RemoveWorktree(args) => cmd_remove_worktree(args, env),
        cli::Command::SetRepo(args) => cmd_set_repo(args, env),
        cli::Command::SetRepoSetup(args) => cmd_set_repo_command(args, env, RepoCommandKind::Setup),
        cli::Command::SetRepoTeardown(args) => cmd_set_repo_command(args, env, RepoCommandKind::Teardown),
        cli::Command::RemoveRepo(args) => cmd_remove_repo(args, env),
        cli::Command::ListRepos(args) => cmd_list_repos(args, env),
    }
}

pub fn print_command_error(err: &CommandError, json: bool) {
    if json {
        eprintln!(
            "{}",
            serde_json::to_string_pretty(&err.to_json()).unwrap_or_else(|_| err.message.clone())
        );
    } else {
        eprintln!("{}", err.message);
    }
}

fn cmd_list_worktrees(args: cli::ListWorktrees, env: &dyn Env) -> Result<()> {
    let workspaces = env.niri().workspaces()?;
    let windows = env.niri().windows()?;
    let repos = repos_for_arg(env, args.repo.as_deref())?;
    let mut git_rows = vec![];
    for repo in repos {
        git_rows.extend(git::worktrees(env.runner(), &repo)?);
    }
    let rows = worktree_rows(env.store(), git_rows, &workspaces, &windows)?;
    if args.json {
        output::print_json("worktrees", rows)
    } else {
        output::print_worktrees(&rows);
        Ok(())
    }
}

fn cmd_list_branches(args: cli::ListBranches, env: &dyn Env) -> Result<()> {
    let repos = repos_for_arg(env, args.repo.as_deref())?;
    let workspace_ids = workspace_ids_by_worktree(env.store())?;
    let mut rows = vec![];
    for repo in repos {
        rows.extend(branches_for_repo(env.runner(), &repo, args.local, args.remote, &workspace_ids)?);
    }
    if args.json {
        output::print_json("branches", rows)
    } else {
        output::print_branches(&rows);
        Ok(())
    }
}

fn cmd_list_pull_requests(args: cli::ListPullRequests, env: &dyn Env) -> Result<()> {
    let mut repos = repos_for_arg(env, args.repo.as_deref())?;
    if args.repo.is_none() {
        let had_repos = !repos.is_empty();
        repos.retain(|repo| repo.list_pull_requests);
        if had_repos && repos.is_empty() {
            eprintln!(
                "Warning: no repos have --list-pull-requests enabled. Run `niri-worktrees set-repo --repo <repo> --list-pull-requests true` to enable it."
            );
        }
    }
    let workspace_ids = workspace_ids_by_worktree(env.store())?;
    let runner = env.runner();
    let results: Vec<Result<Vec<PullRequestRow>>> = std::thread::scope(|scope| {
        let handles: Vec<_> = repos
            .iter()
            .map(|repo| scope.spawn(|| pull_requests_for_repo(runner, repo, &workspace_ids)))
            .collect();
        handles.into_iter().map(|handle| handle.join().unwrap()).collect()
    });
    let mut rows = vec![];
    for result in results {
        rows.extend(result?);
    }
    if args.json {
        output::print_json("pull_requests", rows)
    } else {
        output::print_pull_requests(&rows);
        Ok(())
    }
}

fn cmd_set_workspace(args: cli::SetWorkspace, env: &dyn Env) -> Result<()> {
    let worktree = if let Some(path) = args.worktree {
        normalize_path(path)
    } else {
        current_git_worktree_root(env.runner())?
    };
    let workspace_id = if let Some(id) = args.workspace_id {
        id
    } else {
        niri::focused_workspace(env.niri())?
            .get("id")
            .and_then(Value::as_u64)
            .ok_or_else(|| AppError::Message("Focused Niri workspace does not have an id".to_string()))?
    };
    store::set_worktree_mapping(env.store(), worktree, workspace_id)
}

fn cmd_unset_workspace(args: cli::UnsetWorkspace, env: &dyn Env) -> Result<()> {
    let mut worktree = args.worktree.map(normalize_path);
    if worktree.is_none() && args.workspace_id.is_none() {
        worktree = Some(current_git_worktree_root(env.runner())?);
    }
    store::unset_worktree_mapping(env.store(), worktree.as_deref(), args.workspace_id)
}

fn cmd_get_worktree(args: cli::GetWorktree, env: &dyn Env) -> Result<()> {
    let workspace_id = if let Some(id) = args.workspace_id {
        id
    } else {
        niri::focused_workspace(env.niri())?
            .get("id")
            .and_then(Value::as_u64)
            .ok_or_else(|| AppError::Message("Focused Niri workspace does not have an id".to_string()))?
    };
    let Some(mapping) = env
        .store()
        .load_worktrees()?
        .into_iter()
        .find(|entry| entry.workspace_id == workspace_id && entry.path.is_dir())
    else {
        return message(format!("No worktree is stored for workspace {workspace_id}"));
    };
    println!("{}", mapping.path.display());
    Ok(())
}

fn cmd_create_worktree(args: cli::CreateWorktree, env: &dyn Env) -> Result<()> {
    let repo_path = normalize_path(args.repo);
    let repo = stored_repo_for_path(env.store(), &repo_path)?
        .ok_or_else(|| AppError::Message(format!("No repo is stored for {}", repo_path.display())))?;
    if !git::local_branch_exists(env.runner(), &repo_path, &args.branch) {
        return message(format!("Local branch {} does not exist", args.branch));
    }
    let worktree = worktree_path_for_repo_branch(env.runner(), &repo, &args.branch);
    if worktree.exists() {
        return message(format!("Cannot create worktree because {} already exists", worktree.display()));
    }
    git::create_existing_branch_worktree(env.runner(), &repo_path, &args.branch, &worktree)?;
    focus_worktree(env, worktree)
}

fn cmd_create_branch(args: cli::CreateBranch, env: &dyn Env) -> Result<()> {
    let repo_path = repo_arg_or_default(env.runner(), args.repo.as_deref())?;
    let repo = stored_repo_for_path(env.store(), &repo_path)?
        .ok_or_else(|| AppError::Message(format!("No repo is stored for {}", repo_path.display())))?;
    if args.branch.contains('/') {
        return message("New branch names for create-branch must be local branch names");
    }
    if git::local_branch_exists(env.runner(), &repo_path, &args.branch) {
        return message(format!("Local branch {} already exists", args.branch));
    }
    let worktree = worktree_path_for_repo_branch(env.runner(), &repo, &args.branch);
    if worktree.exists() {
        return message(format!("Cannot create worktree because {} already exists", worktree.display()));
    }
    let from_branch = match args.from_branch {
        Some(branch) => branch,
        None => git::default_origin_branch(env.runner(), &repo_path)?,
    };
    git::create_branch_worktree(
        env.runner(),
        &repo_path,
        &args.branch,
        &from_branch,
        &worktree,
    )?;
    git::unset_current_branch_upstream(env.runner(), &worktree)?;
    focus_worktree(env, worktree.clone())?;
    run_setup(env.runner(), &repo, &worktree)
}

fn cmd_remove_worktree(args: cli::RemoveWorktree, env: &dyn Env) -> Result<()> {
    remove_worktree(args, env).map_err(AppError::Command)
}

fn cmd_set_repo(args: cli::SetRepo, env: &dyn Env) -> Result<()> {
    let repo_path = repo_arg_or_default(env.runner(), args.repo.as_deref())?;
    if !git::is_git_repository(env.runner(), &repo_path) {
        return message(format!("{} is not a git repository", repo_path.display()));
    }
    let mut repos = env.store().load_repos()?;
    if let Some(repo) = repos.iter_mut().find(|repo| repo.path == repo_path) {
        if let Some(list_pull_requests) = args.list_pull_requests {
            repo.list_pull_requests = bool::from(list_pull_requests);
        }
    } else {
        repos.push(Repo {
            path: repo_path,
            list_pull_requests: args.list_pull_requests.map(bool::from).unwrap_or(false),
            setup: None,
            teardown: None,
        });
    }
    env.store().save_repos(&repos)
}

enum RepoCommandKind {
    Setup,
    Teardown,
}

fn cmd_set_repo_command(args: cli::SetRepoCommand, env: &dyn Env, kind: RepoCommandKind) -> Result<()> {
    let repo_path = normalize_path(args.repo);
    let mut repos = env.store().load_repos()?;
    let Some(repo) = repos.iter_mut().find(|repo| repo.path == repo_path) else {
        return message(format!("No repo is stored for {}", repo_path.display()));
    };
    let command = (!args.command.is_empty()).then_some(args.command);
    match kind {
        RepoCommandKind::Setup => repo.setup = command,
        RepoCommandKind::Teardown => repo.teardown = command,
    }
    env.store().save_repos(&repos)
}

fn cmd_remove_repo(args: cli::RemoveRepo, env: &dyn Env) -> Result<()> {
    let repo_path = repo_arg_or_default(env.runner(), args.repo.as_deref())?;
    let mut repos = env.store().load_repos()?;
    repos.retain(|repo| repo.path != repo_path);
    env.store().save_repos(&repos)
}

fn cmd_list_repos(args: cli::ListRepos, env: &dyn Env) -> Result<()> {
    let rows: Vec<(Repo, Option<String>, bool)> = env
        .store()
        .load_repos()?
        .into_iter()
        .map(|repo| {
            let origin = git::remote_url(env.runner(), &repo.path, "origin");
            let bare = git::is_bare_repository(env.runner(), &repo.path);
            (repo, origin, bare)
        })
        .collect();
    if args.json {
        let repos: Vec<Value> = rows
            .into_iter()
            .map(|(repo, origin, bare)| {
                json!({
                    "path": repo.path,
                    "repo_origin": origin,
                    "bare": bare,
                    "list_pull_requests": repo.list_pull_requests,
                    "setup": repo.setup,
                    "teardown": repo.teardown,
                })
            })
            .collect();
        output::print_json("repos", repos)
    } else {
        output::print_repos(&rows);
        Ok(())
    }
}

fn repos_for_arg(env: &dyn Env, repo_arg: Option<&str>) -> Result<Vec<Repo>> {
    let repos = env.store().load_repos()?;
    if let Some(repo) = repo_arg {
        let repo_path = repo_arg_or_default(env.runner(), Some(repo))?;
        let repos: Vec<Repo> = repos.into_iter().filter(|repo| repo.path == repo_path).collect();
        if repos.is_empty() {
            return message(format!("No repo is stored for {}", repo_path.display()));
        }
        Ok(repos)
    } else {
        Ok(repos)
    }
}

fn repo_arg_or_default(runner: &dyn CommandRunner, repo: Option<&str>) -> Result<PathBuf> {
    if let Some(repo) = repo {
        Ok(normalize_path(repo))
    } else {
        git::default_repo_path(runner)
    }
}

fn current_git_worktree_root(runner: &dyn CommandRunner) -> Result<PathBuf> {
    let cwd = std::env::current_dir()
        .map_err(|e| AppError::Message(format!("Could not get current directory: {e}")))?;
    git::worktree_root(runner, &cwd).ok_or_else(|| {
        AppError::Message(
            "No --worktree was provided and the current directory is not inside a git worktree"
                .to_string(),
        )
    })
}

fn stored_repo_for_path(store: &dyn Store, path: &Path) -> Result<Option<Repo>> {
    Ok(store.load_repos()?.into_iter().find(|repo| repo.path == path))
}

fn stored_repo_for_worktree(env: &dyn Env, worktree: &Path) -> Result<Option<Repo>> {
    let Some(worktree_common) = git::common_dir(env.runner(), worktree) else {
        return Ok(None);
    };
    for repo in env.store().load_repos()? {
        if git::common_dir(env.runner(), &repo.path) == Some(worktree_common.clone()) {
            return Ok(Some(repo));
        }
    }
    Ok(None)
}

fn workspace_ids_by_worktree(store: &dyn Store) -> Result<HashMap<PathBuf, u64>> {
    Ok(store
        .load_worktrees()?
        .into_iter()
        .filter(|entry| entry.path.is_dir())
        .map(|entry| (entry.path, entry.workspace_id))
        .collect())
}

fn worktree_rows(
    store: &dyn Store,
    git_rows: Vec<GitWorktree>,
    workspaces: &[Value],
    windows: &[Value],
) -> Result<Vec<WorktreeRow>> {
    let latest = niri::latest_focus_timestamps_by_workspace(windows);
    let workspaces_by_id: HashMap<u64, Value> = workspaces
        .iter()
        .filter_map(|w| w.get("id").and_then(Value::as_u64).map(|id| (id, w.clone())))
        .collect();
    let mut windows_by_workspace: HashMap<u64, Vec<Value>> = HashMap::new();
    for window in windows {
        if let Some(id) = window.get("workspace_id").and_then(Value::as_u64) {
            windows_by_workspace.entry(id).or_default().push(window.clone());
        }
    }
    let stored: HashMap<PathBuf, WorktreeMapping> = store
        .load_worktrees()?
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect();
    let mut rows: Vec<WorktreeRow> = git_rows
        .into_iter()
        .map(|row| {
            let workspace_id = stored.get(&row.path).map(|entry| entry.workspace_id);
            let workspace = workspace_id.and_then(|id| workspaces_by_id.get(&id).cloned());
            let windows = if workspace.is_some() {
                workspace_id
                    .and_then(|id| windows_by_workspace.get(&id).cloned())
                    .unwrap_or_default()
            } else {
                vec![]
            };
            let is_focused = workspace
                .as_ref()
                .and_then(|w| w.get("is_focused"))
                .and_then(Value::as_bool)
                == Some(true);
            let focus_timestamp = workspace_id.and_then(|id| latest.get(&id).copied());
            WorktreeRow {
                path: row.path,
                repo: row.repo,
                local_branch: row.local_branch,
                remote_branch: row.remote_branch,
                workspace,
                windows,
                is_focused,
                focus_timestamp,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        (
            b.is_focused,
            b.focus_timestamp.is_some(),
            b.focus_timestamp,
            &b.repo,
            &b.path,
        )
            .cmp(&(
                a.is_focused,
                a.focus_timestamp.is_some(),
                a.focus_timestamp,
                &a.repo,
                &a.path,
            ))
    });
    Ok(rows)
}

fn branches_for_repo(
    runner: &dyn CommandRunner,
    repo: &Repo,
    local_only: bool,
    remote_only: bool,
    workspace_ids: &HashMap<PathBuf, u64>,
) -> Result<Vec<BranchRow>> {
    let local_branches = git::branch_names(runner, &repo.path, true, false)?;
    let remote_branches: HashSet<String> =
        git::branch_names(runner, &repo.path, false, true)?.into_iter().collect();
    let worktrees = git::worktree_branches(runner, &repo.path)?;
    let upstreams = git::branch_upstreams(runner, &repo.path);
    let mut rows = vec![];
    let mut paired_remotes = HashSet::new();

    if !remote_only {
        for branch in &local_branches {
            let upstream = upstreams.get(branch).cloned();
            let remote_branch = upstream.filter(|upstream| remote_branches.contains(upstream));
            if let Some(remote_branch) = &remote_branch {
                paired_remotes.insert(remote_branch.clone());
            }
            let worktree = worktree_for_branch(branch, &worktrees);
            rows.push(BranchRow {
                local_branch: Some(branch.clone()),
                remote_branch,
                repo: repo.path.clone(),
                workspace_id: worktree.as_ref().and_then(|path| workspace_ids.get(path).copied()),
                worktree,
            });
        }
    }

    if remote_only {
        for branch in &local_branches {
            let Some(upstream) = upstreams.get(branch).cloned() else {
                continue;
            };
            if !remote_branches.contains(&upstream) {
                continue;
            }
            paired_remotes.insert(upstream.clone());
            let worktree = worktree_for_branch(branch, &worktrees);
            rows.push(BranchRow {
                local_branch: Some(branch.clone()),
                remote_branch: Some(upstream),
                repo: repo.path.clone(),
                workspace_id: worktree.as_ref().and_then(|path| workspace_ids.get(path).copied()),
                worktree,
            });
        }
    }

    if !local_only {
        let mut remotes: Vec<_> = remote_branches.into_iter().collect();
        remotes.sort();
        for branch in remotes {
            if paired_remotes.contains(&branch) {
                continue;
            }
            rows.push(BranchRow {
                local_branch: None,
                remote_branch: Some(branch),
                repo: repo.path.clone(),
                worktree: None,
                workspace_id: None,
            });
        }
    }
    Ok(rows)
}

fn pull_requests_for_repo(
    runner: &dyn CommandRunner,
    repo: &Repo,
    workspace_ids: &HashMap<PathBuf, u64>,
) -> Result<Vec<PullRequestRow>> {
    let (branches, prs) = std::thread::scope(|scope| {
        let branches_handle = scope.spawn(|| branches_for_repo(runner, repo, false, false, workspace_ids));
        let prs_handle = scope.spawn(|| github::pull_requests(runner, &repo.path));
        (branches_handle.join().unwrap(), prs_handle.join().unwrap())
    });
    let branches = branches?;
    let prs = prs?;
    let mut rows = vec![];
    for pr in prs {
        let branch_row = branch_row_for_pr(&pr.head_ref_name, &branches);
        let branch_row = match branch_row {
            Some(row) if row.worktree.is_some() || pr.status.as_deref() == Some("OPEN") => row.clone(),
            Some(_) => continue,
            None if pr.status.as_deref() == Some("OPEN") => BranchRow {
                local_branch: Some(pr.head_ref_name.clone()),
                remote_branch: None,
                repo: repo.path.clone(),
                worktree: None,
                workspace_id: None,
            },
            None => continue,
        };
        rows.push(PullRequestRow {
            pr_number: pr.pr_number,
            status: pr.status,
            unresolved_review_comments: pr.unresolved_review_comments,
            total_review_comments: pr.total_review_comments,
            checks_status: pr.checks_status,
            local_branch: branch_row.local_branch,
            remote_branch: branch_row.remote_branch,
            repo: branch_row.repo,
            worktree: branch_row.worktree,
            workspace_id: branch_row.workspace_id,
        });
    }
    Ok(rows)
}

fn branch_row_for_pr<'a>(head: &str, rows: &'a [BranchRow]) -> Option<&'a BranchRow> {
    rows.iter().find(|row| {
        row.local_branch.as_deref() == Some(head)
            || row
                .remote_branch
                .as_deref()
                .map(branch_pr_name)
                .as_deref()
                == Some(head)
    })
}

fn worktree_for_branch(
    branch: &str,
    worktrees: &HashMap<String, (PathBuf, Option<String>)>,
) -> Option<PathBuf> {
    if !branch.contains('/') {
        return worktrees.get(branch).map(|(path, _)| path.clone());
    }
    worktrees
        .values()
        .find(|(_, upstream)| upstream.as_deref() == Some(branch))
        .map(|(path, _)| path.clone())
}

fn branch_pr_name(branch: &str) -> String {
    branch
        .split_once('/')
        .map(|(_, name)| name.to_string())
        .unwrap_or_else(|| branch.to_string())
}

fn focus_worktree(env: &dyn Env, worktree: PathBuf) -> Result<()> {
    let matching = env
        .store()
        .load_worktrees()?
        .into_iter()
        .find(|entry| entry.path == worktree);
    if let Some(mapping) = matching {
        if niri::find_workspace_by_id(env.niri(), mapping.workspace_id)?.is_some() {
            niri::focus_workspace(env.niri(), mapping.workspace_id)?;
            return Ok(());
        }
    }
    let workspace = niri::focus_last_workspace_on_current_output(env.niri())?;
    let workspace_id = workspace
        .get("id")
        .and_then(Value::as_u64)
        .ok_or_else(|| AppError::Message("Newly focused Niri workspace does not have an id".to_string()))?;
    store::set_worktree_mapping(env.store(), worktree, workspace_id)
}

fn worktree_path_for_repo_branch(runner: &dyn CommandRunner, repo: &Repo, branch: &str) -> PathBuf {
    let base = if git::is_bare_repository(runner, &repo.path) {
        repo.path.clone()
    } else {
        repo.path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| repo.path.clone())
    };
    normalize_path(base.join(branch))
}

fn remove_worktree(args: cli::RemoveWorktree, env: &dyn Env) -> std::result::Result<(), CommandError> {
    let entry = if let Some(workspace_id) = args.workspace_id {
        env.store()
            .load_worktrees()
            .map_err(command_from_app)?
            .into_iter()
            .find(|entry| entry.workspace_id == workspace_id)
            .ok_or_else(|| {
                CommandError::new(
                    "worktree_not_stored",
                    format!("No worktree is stored for workspace {workspace_id}"),
                )
                .details(json!({ "workspace_id": workspace_id }))
                .json(args.json)
            })?
    } else {
        let worktree = normalize_path(args.worktree.as_deref().unwrap_or_default());
        env.store()
            .load_worktrees()
            .map_err(command_from_app)?
            .into_iter()
            .find(|entry| entry.path == worktree)
            .ok_or_else(|| {
                CommandError::new(
                    "worktree_not_stored",
                    format!("No workspace is stored for worktree {}", worktree.display()),
                )
                .details(json!({ "worktree": worktree }))
                .json(args.json)
            })?
    };

    if !entry.path.is_dir() {
        return Err(CommandError::new(
            "worktree_missing",
            format!("Worktree directory does not exist: {}", entry.path.display()),
        )
        .details(json!({"worktree": entry.path, "workspace_id": entry.workspace_id}))
        .json(args.json));
    }

    ensure_workspace_has_no_windows(env.niri(), entry.workspace_id, args.json)?;
    ensure_worktree_clean(env.runner(), &entry.path, args.json)?;
    let repo = stored_repo_for_worktree(env, &entry.path)
        .map_err(command_from_app)?
        .ok_or_else(|| {
            CommandError::new(
                "repo_not_stored",
                format!("No stored repo was found for worktree {}", entry.path.display()),
            )
            .details(json!({"worktree": entry.path, "workspace_id": entry.workspace_id}))
            .json(args.json)
        })?;
    run_teardown(env.runner(), &repo, &entry.path).map_err(|mut err| {
        err.json = args.json;
        err
    })?;
    let out = git::remove_worktree(env.runner(), &repo.path, &entry.path).map_err(command_from_app)?;
    if out.status != 0 {
        return Err(CommandError::new(
            "git_worktree_remove_failed",
            format!(
                "Could not remove worktree {}{}",
                entry.path.display(),
                if out.stderr.trim().is_empty() {
                    String::new()
                } else {
                    format!(": {}", out.stderr.trim())
                }
            ),
        )
        .exit_code(13)
        .details(json!({"worktree": entry.path, "repo": repo.path, "stderr": out.stderr.trim()}))
        .json(args.json));
    }
    let mut mappings = env.store().load_worktrees().map_err(command_from_app)?;
    mappings.retain(|mapping| mapping.path != entry.path);
    env.store()
        .save_worktrees(&mappings)
        .map_err(command_from_app)?;
    Ok(())
}

fn ensure_workspace_has_no_windows(
    niri: &dyn NiriClient,
    workspace_id: u64,
    json_output: bool,
) -> std::result::Result<(), CommandError> {
    let windows: Vec<Value> = niri
        .windows()
        .map_err(command_from_app)?
        .into_iter()
        .filter(|window| window.get("workspace_id").and_then(Value::as_u64) == Some(workspace_id))
        .collect();
    if windows.is_empty() {
        Ok(())
    } else {
        Err(CommandError::new(
            "workspace_has_windows",
            format!("Workspace {workspace_id} still has {} window(s)", windows.len()),
        )
        .exit_code(10)
        .details(json!({"workspace_id": workspace_id, "window_count": windows.len(), "windows": windows}))
        .json(json_output))
    }
}

fn ensure_worktree_clean(
    runner: &dyn CommandRunner,
    worktree: &Path,
    json_output: bool,
) -> std::result::Result<(), CommandError> {
    let status = git::status_porcelain(runner, worktree).map_err(command_from_app)?;
    if status.is_empty() {
        Ok(())
    } else {
        Err(CommandError::new(
            "worktree_dirty",
            format!("Worktree {} is not clean", worktree.display()),
        )
        .exit_code(11)
        .details(json!({"worktree": worktree, "status": status}))
        .json(json_output))
    }
}

fn run_setup(runner: &dyn CommandRunner, repo: &Repo, worktree: &Path) -> Result<()> {
    let Some(setup) = repo.setup.as_deref().filter(|command| !command.is_empty()) else {
        return Ok(());
    };
    let Some((program, rest)) = split_program(setup) else {
        return message("Setup command is empty");
    };
    let exit_code = runner
        .run_inherit(program, &rest, Some(worktree))
        .map_err(|e| AppError::Message(format!("Could not run setup command: {e}")))?;
    if exit_code != 0 {
        return Err(AppError::Command(
            CommandError::new(
                "setup_failed",
                format!("Setup command failed with exit code {exit_code}"),
            )
            .exit_code(14)
            .details(json!({"worktree": worktree, "exit_code": exit_code})),
        ));
    }
    Ok(())
}

fn run_teardown(
    runner: &dyn CommandRunner,
    repo: &Repo,
    worktree: &Path,
) -> std::result::Result<(), CommandError> {
    let Some(teardown) = repo.teardown.as_deref().filter(|command| !command.is_empty()) else {
        return Ok(());
    };
    let Some((program, rest)) = split_program(teardown) else {
        return Err(CommandError::new("teardown_failed", "Teardown command is empty")
            .exit_code(12)
            .details(json!({"worktree": worktree})));
    };
    let exit_code = runner.run_inherit(program, &rest, Some(worktree)).map_err(|e| {
        CommandError::new("teardown_failed", format!("Could not run teardown command: {e}"))
            .exit_code(12)
            .details(json!({"worktree": worktree}))
    })?;
    if exit_code != 0 {
        return Err(CommandError::new(
            "teardown_failed",
            format!("Teardown command failed with exit code {exit_code}"),
        )
        .exit_code(12)
        .details(json!({"worktree": worktree, "exit_code": exit_code})));
    }
    Ok(())
}

fn command_from_app(err: AppError) -> CommandError {
    CommandError::new("generic", err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_pr_name_strips_remote_prefix() {
        assert_eq!(branch_pr_name("origin/feature"), "feature");
        assert_eq!(branch_pr_name("feature"), "feature");
    }
}
