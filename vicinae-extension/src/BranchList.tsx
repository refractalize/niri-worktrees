import { List, Icon, ActionPanel, Action, closeMainWindow, Color } from '@vicinae/api';
import { useEffect, useState } from 'react';
import type { Branch } from './types';
import {
  execAsync,
  fetchRepoDisplayNames,
  showError,
  stripRemotePrefix,
  withProgressToast,
  NIRI_WORKTREES,
} from './utils';

async function fetchBranches(pr: boolean): Promise<Branch[]> {
  const flag = pr ? ' --pr' : '';
  const { stdout } = await execAsync(`${NIRI_WORKTREES} list-branches --json${flag}`);
  const data = JSON.parse(stdout) as { branches: Branch[] };
  return data.branches;
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

function groupByRepo(branches: Branch[]): Map<string, Branch[]> {
  const map = new Map<string, Branch[]>();
  for (const branch of branches) {
    const group = map.get(branch.repo) ?? [];
    group.push(branch);
    map.set(branch.repo, group);
  }
  return map;
}

interface BranchListProps {
  pr?: boolean;
  navigationTitle: string;
  emptyTitle: string;
  emptyDescription: string;
}

export default function BranchList({ pr = false, navigationTitle, emptyTitle, emptyDescription }: BranchListProps) {
  const [branches, setBranches] = useState<Branch[]>([]);
  const [repoNames, setRepoNames] = useState<Map<string, string>>(new Map());
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    Promise.all([fetchBranches(pr), fetchRepoDisplayNames()])
      .then(([bs, names]) => {
        setBranches(bs);
        setRepoNames(names);
      })
      .catch((error) => showError('Failed to load branches', error))
      .finally(() => setLoading(false));
  }, [pr]);

  const grouped = groupByRepo(branches);

  return (
    <List isLoading={loading} navigationTitle={navigationTitle}>
      {!loading && branches.length === 0 && (
        <List.EmptyView icon={Icon.Git} title={emptyTitle} description={emptyDescription} />
      )}
      {[...grouped.entries()].map(([repo, repoBranches]) => (
        <List.Section key={repo} title={repoNames.get(repo) ?? repo} subtitle={repo}>
          {repoBranches.map((branch) => {
            const displayName = branch.local_branch ?? branch.remote_branch ?? '(unknown)';
            const remoteName = branch.remote_branch ?? '';

            let actionTitle: string;
            let actionIcon: Icon;
            let runAction: () => Promise<boolean>;
            if (branch.worktree !== null) {
              actionTitle = 'Focus Worktree';
              actionIcon = Icon.Eye;
              runAction = () => focusWorktree(branch.worktree!);
            } else if (branch.local_branch !== null) {
              actionTitle = 'Create Worktree & Focus';
              actionIcon = Icon.Plus;
              runAction = () => createWorktree(repo, branch.local_branch!);
            } else {
              actionTitle = 'Create Branch & Focus';
              actionIcon = Icon.Plus;
              const localBranch = stripRemotePrefix(branch.remote_branch ?? '');
              runAction = () => createBranchFromRemote(repo, localBranch, branch.remote_branch ?? '');
            }

            return (
              <List.Item
                key={`${repo}:${displayName}`}
                title={displayName}
                subtitle={branch.local_branch !== null ? remoteName : ''}
                icon={Icon.Git}
                accessories={[
                  branch.workspace_id !== null
                    ? { tag: { value: String(branch.workspace_id), color: Color.Green } }
                    : { tag: { value: 'no workspace', color: Color.SecondaryText } },
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
