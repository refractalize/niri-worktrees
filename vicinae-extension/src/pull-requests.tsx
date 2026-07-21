import { List, Icon, ActionPanel, Action, closeMainWindow, Color } from '@vicinae/api';
import { useEffect, useState } from 'react';
import type { PullRequest } from './types';
import {
  execAsync,
  fetchRepoDisplayNames,
  removeWorktree,
  showError,
  stripRemotePrefix,
  withProgressToast,
  NIRI_WORKTREES,
} from './utils';

async function fetchPullRequests(): Promise<PullRequest[]> {
  const { stdout } = await execAsync(`${NIRI_WORKTREES} list-pull-requests --json`);
  const data = JSON.parse(stdout) as { pull_requests: PullRequest[] };
  return data.pull_requests;
}

async function focusWorktree(worktree: string): Promise<boolean> {
  return withProgressToast('Focusing worktree…', 'Worktree focused', 'Failed to focus worktree', async () => {
    await execAsync(`${NIRI_WORKTREES} focus-worktree ${JSON.stringify(worktree)}`);
  });
}

async function createWorktree(repo: string, localBranch: string): Promise<boolean> {
  return withProgressToast('Creating worktree…', 'Worktree created', 'Failed to create worktree', async () => {
    await execAsync(
      `${NIRI_WORKTREES} create-worktree --repo ${JSON.stringify(repo)} ${JSON.stringify(localBranch)}`
    );
  });
}

async function createBranchFromRemote(
  repo: string,
  localBranch: string,
  fromRef: string
): Promise<boolean> {
  return withProgressToast('Creating branch…', 'Branch created', 'Failed to create branch', async () => {
    await execAsync(
      `${NIRI_WORKTREES} create-branch --repo ${JSON.stringify(repo)} --from ${JSON.stringify(fromRef)} ${JSON.stringify(localBranch)}`
    );
  });
}

function groupByRepo(prs: PullRequest[]): Map<string, PullRequest[]> {
  const map = new Map<string, PullRequest[]>();
  for (const pr of prs) {
    const group = map.get(pr.repo) ?? [];
    group.push(pr);
    map.set(pr.repo, group);
  }
  return map;
}

function statusTag(status: string): { value: string; color: Color } {
  switch (status.toUpperCase()) {
    case 'OPEN':
      return { value: 'Open', color: Color.Green };
    case 'MERGED':
      return { value: 'Merged', color: Color.Purple };
    case 'CLOSED':
      return { value: 'Closed', color: Color.Red };
    default:
      return { value: status, color: Color.SecondaryText };
  }
}

function checksTag(status: string): { value: string; color: Color } {
  switch (status.toUpperCase()) {
    case 'SUCCESS':
      return { value: 'Checks Passing', color: Color.Green };
    case 'FAILURE':
    case 'ERROR':
      return { value: 'Checks Failing', color: Color.Red };
    case 'PENDING':
    case 'EXPECTED':
      return { value: 'Checks Pending', color: Color.Yellow };
    default:
      return { value: status, color: Color.SecondaryText };
  }
}

export default function PullRequests() {
  const [prs, setPrs] = useState<PullRequest[]>([]);
  const [repoNames, setRepoNames] = useState<Map<string, string>>(new Map());
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    Promise.all([fetchPullRequests(), fetchRepoDisplayNames()])
      .then(([fetchedPrs, names]) => {
        setPrs(fetchedPrs);
        setRepoNames(names);
      })
      .catch((error) => showError('Failed to load pull requests', error))
      .finally(() => setLoading(false));
  }, []);

  const grouped = groupByRepo(prs);

  return (
    <List isLoading={loading} navigationTitle="Pull Requests">
      {!loading && prs.length === 0 && (
        <List.EmptyView
          icon={Icon.Git}
          title="No Pull Requests"
          description="No open pull requests found in stored repositories."
        />
      )}
      {[...grouped.entries()].map(([repo, repoPrs]) => (
        <List.Section key={repo} title={repoNames.get(repo) ?? repo} subtitle={repo}>
          {repoPrs.map((pr) => {
            const displayName = pr.local_branch ?? pr.remote_branch ?? '(unknown)';
            const tag = statusTag(pr.status);

            let actionTitle: string;
            let actionIcon: Icon;
            let runAction: () => Promise<boolean>;
            if (pr.worktree !== null) {
              actionTitle = 'Focus Worktree';
              actionIcon = Icon.Eye;
              runAction = () => focusWorktree(pr.worktree!);
            } else if (pr.local_branch !== null) {
              actionTitle = 'Create Worktree & Focus';
              actionIcon = Icon.Plus;
              runAction = () => createWorktree(repo, pr.local_branch!);
            } else {
              actionTitle = 'Create Branch & Focus';
              actionIcon = Icon.Plus;
              const localBranch = stripRemotePrefix(pr.remote_branch ?? '');
              runAction = () => createBranchFromRemote(repo, localBranch, pr.remote_branch ?? '');
            }

            return (
              <List.Item
                key={`${repo}:${displayName}`}
                title={pr.pr_number !== null ? `#${pr.pr_number} ${displayName}` : displayName}
                subtitle={pr.remote_branch ?? ''}
                icon={Icon.Git}
                accessories={[
                  { tag },
                  pr.checks_status !== null ? { tag: checksTag(pr.checks_status) } : {},
                  pr.unresolved_review_comments !== null && pr.unresolved_review_comments > 0
                    ? {
                        tag: {
                          value: `${pr.unresolved_review_comments} unresolved`,
                          color: Color.Orange,
                        },
                      }
                    : {},
                  pr.workspace_id !== null
                    ? { tag: { value: String(pr.workspace_id), color: Color.Blue } }
                    : {},
                ]}
                actions={
                  <ActionPanel>
                    <Action
                      title={actionTitle}
                      icon={actionIcon}
                      onAction={async () => {
                        const ok = await runAction();
                        if (ok) closeMainWindow({ clearRootSearch: true });
                      }}
                    />
                    {pr.worktree !== null && (
                      <Action
                        title="Remove Worktree"
                        icon={Icon.Trash}
                        style={Action.Style.Destructive}
                        shortcut={{ modifiers: ['ctrl'], key: 'x' }}
                        onAction={async () => {
                          const ok = await removeWorktree(pr.worktree!);
                          if (ok) setPrs((prev) => prev.filter((p) => p.worktree !== pr.worktree));
                        }}
                      />
                    )}
                    <Action.CopyToClipboard
                      title="Copy Branch Name"
                      content={displayName}
                      shortcut={{ modifiers: ['ctrl'], key: 'c' }}
                    />
                  </ActionPanel>
                }
              />
            );
          })}
        </List.Section>
      ))}
    </List>
  );
}
