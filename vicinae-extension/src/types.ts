export interface NiriWorkspace {
  id: number;
  idx: number;
  name: string | null;
  output: string;
  is_urgent: boolean;
  is_active: boolean;
  is_focused: boolean;
  active_window_id: number | null;
}

export interface NiriWindow {
  id: number;
  title: string;
  app_id: string;
  workspace_id: number;
  is_focused: boolean;
  is_urgent: boolean;
  focus_timestamp: { secs: number; nanos: number } | null;
}

export interface Worktree {
  path: string;
  repo: string | null;
  local_branch: string | null;
  remote_branch: string | null;
  workspace: NiriWorkspace | null;
  windows: NiriWindow[];
}

export interface Branch {
  local_branch: string | null;
  remote_branch: string | null;
  repo: string;
  worktree: string | null;
  workspace_id: number | null;
}

export interface PullRequest {
  pr_number: number | null;
  status: string;
  unresolved_review_comments: number | null;
  total_review_comments: number | null;
  checks_status: string | null;
  local_branch: string | null;
  remote_branch: string | null;
  repo: string;
  worktree: string | null;
  workspace_id: number | null;
}

export interface RepoInfo {
  path: string;
  repo_origin: string | null;
  bare: boolean;
  setup?: string;
  teardown?: string;
}
