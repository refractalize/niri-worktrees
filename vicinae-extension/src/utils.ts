import { exec } from 'child_process';
import { homedir } from 'os';
import { promisify } from 'util';
import { showToast, Toast, confirmAlert, Alert } from '@vicinae/api';

export const NIRI_WORKTREES = `${homedir()}/.local/bin/niri-worktrees`;

export async function fetchRepoDisplayNames(): Promise<Map<string, string>> {
  const { stdout } = await execAsync(`${NIRI_WORKTREES} list-repos --json`);
  const data = JSON.parse(stdout) as { repos: Array<{ path: string; repo_origin?: string | null }> };
  const map = new Map<string, string>();
  for (const repo of data.repos) {
    map.set(repo.path, repoDisplayName(repo.path, repo.repo_origin ?? null));
  }
  return map;
}

export function repoDisplayName(repoPath: string, origin: string | null): string {
  if (!origin) return repoBasename(repoPath);

  let url = origin.endsWith('.git') ? origin.slice(0, -4) : origin;

  // SSH: git@github.com:org/repo
  if (!url.startsWith('http') && url.includes(':')) {
    url = url.slice(url.indexOf(':') + 1);
  } else if (url.includes('://')) {
    // HTTPS: https://github.com/org/repo
    url = url.slice(url.indexOf('://') + 3);
    const slash = url.indexOf('/');
    url = slash !== -1 ? url.slice(slash + 1) : url;
  }

  const parts = url.split('/').filter(Boolean);
  if (parts.length >= 2) return `${parts[parts.length - 2]}/${parts[parts.length - 1]}`;
  if (parts.length === 1) return parts[0] ?? repoBasename(repoPath);
  return repoBasename(repoPath);
}

export const execAsync = promisify(exec);

export function stripRemotePrefix(remote: string): string {
  const slash = remote.indexOf('/');
  return slash !== -1 ? remote.slice(slash + 1) : remote;
}

export function repoBasename(repoPath: string): string {
  return repoPath.split('/').filter(Boolean).pop() ?? repoPath;
}

export function errorMessage(error: unknown): string {
  return error instanceof Error
    ? error.message
    : typeof error === 'object' && error !== null && 'stderr' in error
      ? String((error as { stderr: unknown }).stderr)
      : String(error);
}

export function showError(title: string, error: unknown) {
  showToast({ style: Toast.Style.Failure, title, message: errorMessage(error) });
}

export async function withProgressToast(
  progressTitle: string,
  successTitle: string,
  failureTitle: string,
  action: () => Promise<void>
): Promise<boolean> {
  const toast = await showToast({ style: Toast.Style.Animated, title: progressTitle });
  try {
    await action();
    toast.style = Toast.Style.Success;
    toast.title = successTitle;
    return true;
  } catch (error) {
    toast.style = Toast.Style.Failure;
    toast.title = failureTitle;
    toast.message = errorMessage(error);
    return false;
  }
}

const REMOVE_ERROR_TITLES: Record<string, string> = {
  workspace_has_windows: 'Workspace still has open windows',
  worktree_dirty: 'Worktree has uncommitted changes',
  teardown_failed: 'Teardown command failed',
  git_worktree_remove_failed: 'git worktree remove failed',
};

function parseRemoveError(error: unknown): { title: string; message: string } {
  const stderr =
    typeof error === 'object' && error !== null && 'stderr' in error
      ? String((error as { stderr: unknown }).stderr)
      : null;

  if (stderr) {
    try {
      const parsed = JSON.parse(stderr) as { error?: { code?: string; message?: string } };
      const code = parsed.error?.code ?? '';
      const message = parsed.error?.message ?? stderr;
      const title = REMOVE_ERROR_TITLES[code] ?? 'Failed to remove worktree';
      return { title, message };
    } catch {
      return { title: 'Failed to remove worktree', message: stderr };
    }
  }

  return {
    title: 'Failed to remove worktree',
    message: error instanceof Error ? error.message : String(error),
  };
}

export async function removeWorktree(path: string): Promise<boolean> {
  const confirmed = await confirmAlert({
    title: 'Remove Worktree',
    message: `Remove worktree at ${path}?\n\nThe workspace must be empty and the worktree must be clean.`,
    primaryAction: { title: 'Remove', style: Alert.ActionStyle.Destructive },
  });
  if (!confirmed) return false;

  const toast = await showToast({ style: Toast.Style.Animated, title: 'Removing worktree…' });
  try {
    await execAsync(`${NIRI_WORKTREES} remove-worktree --json --worktree ${JSON.stringify(path)}`);
    toast.style = Toast.Style.Success;
    toast.title = 'Worktree removed';
    return true;
  } catch (error) {
    const { title, message } = parseRemoveError(error);
    toast.style = Toast.Style.Failure;
    toast.title = title;
    toast.message = message;
    return false;
  }
}
