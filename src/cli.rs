use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "niri-worktrees")]
#[command(about = "Track git worktree directories and their Niri workspace ids.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    ListWorktrees(ListWorktrees),
    ListBranches(ListBranches),
    ListPullRequests(ListPullRequests),
    SetWorkspace(SetWorkspace),
    UnsetWorkspace(UnsetWorkspace),
    FocusWorktree(FocusWorktree),
    FocusBranch(FocusBranch),
    CreateBranch(CreateBranch),
    RemoveWorktree(RemoveWorktree),
    OpenTerminal(OpenTerminal),
    SetRepo(SetRepo),
    RemoveRepo(RemoveRepo),
    ListRepos(ListRepos),
}

#[derive(Debug, Args)]
pub struct ListWorktrees {
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub repo: Option<String>,
}

#[derive(Debug, Args)]
pub struct ListBranches {
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub repo: Option<String>,
    #[arg(long, conflicts_with = "remote")]
    pub local: bool,
    #[arg(long, conflicts_with = "local")]
    pub remote: bool,
}

#[derive(Debug, Args)]
pub struct ListPullRequests {
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub repo: Option<String>,
}

#[derive(Debug, Args)]
pub struct SetWorkspace {
    #[arg(long)]
    pub worktree: Option<String>,
    #[arg(long)]
    pub workspace_id: Option<u64>,
}

#[derive(Debug, Args)]
pub struct UnsetWorkspace {
    #[arg(long)]
    pub worktree: Option<String>,
    #[arg(long)]
    pub workspace_id: Option<u64>,
}

#[derive(Debug, Args)]
pub struct FocusWorktree {
    pub worktree: String,
}

#[derive(Debug, Args)]
pub struct FocusBranch {
    #[arg(long)]
    pub repo: Option<String>,
    #[arg(long)]
    pub add_worktree: bool,
    pub branch: String,
}

#[derive(Debug, Args)]
pub struct CreateBranch {
    #[arg(long)]
    pub repo: Option<String>,
    pub branch: String,
}

#[derive(Debug, Args)]
pub struct RemoveWorktree {
    #[arg(long)]
    pub json: bool,
    #[arg(long, required_unless_present = "workspace_id", conflicts_with = "workspace_id")]
    pub worktree: Option<String>,
    #[arg(long, required_unless_present = "worktree", conflicts_with = "worktree")]
    pub workspace_id: Option<u64>,
}

#[derive(Debug, Args)]
pub struct OpenTerminal {
    #[arg(long)]
    pub workspace_id: Option<u64>,
    #[arg(last = true)]
    pub command: Vec<String>,
}

#[derive(Debug, Args)]
pub struct SetRepo {
    #[arg(long)]
    pub repo: Option<String>,
    #[arg(long)]
    pub setup: Option<String>,
    #[arg(long)]
    pub teardown: Option<String>,
    #[arg(long)]
    pub bare: Option<BoolArg>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BoolArg {
    True,
    False,
}

impl From<BoolArg> for bool {
    fn from(value: BoolArg) -> Self {
        matches!(value, BoolArg::True)
    }
}

#[derive(Debug, Args)]
pub struct RemoveRepo {
    #[arg(long)]
    pub repo: Option<String>,
}

#[derive(Debug, Args)]
pub struct ListRepos {
    #[arg(long)]
    pub json: bool,
}

