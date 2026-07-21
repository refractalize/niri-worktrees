import { Form, ActionPanel, Action, popToRoot } from '@vicinae/api';
import { useEffect, useState } from 'react';
import type { Branch, RepoInfo, Worktree } from './types';
import { execAsync, repoDisplayName, stripRemotePrefix, withProgressToast, NIRI_WORKTREES } from './utils';

async function fetchRepos(): Promise<RepoInfo[]> {
  const { stdout } = await execAsync(`${NIRI_WORKTREES} list-repos --json`);
  const data = JSON.parse(stdout) as { repos: RepoInfo[] };
  return data.repos;
}

async function fetchTopWorktree(): Promise<Worktree | null> {
  const { stdout } = await execAsync(`${NIRI_WORKTREES} list-worktrees --json`);
  const data = JSON.parse(stdout) as { worktrees: Worktree[] };
  return data.worktrees[0] ?? null;
}

async function fetchBranchesForRepo(repo: string): Promise<Branch[]> {
  const { stdout } = await execAsync(
    `${NIRI_WORKTREES} list-branches --json --repo ${JSON.stringify(repo)}`
  );
  const data = JSON.parse(stdout) as { branches: Branch[] };
  return data.branches;
}

async function fetchDefaultRemoteBranch(repo: string): Promise<string | null> {
  try {
    const { stdout } = await execAsync(
      `git -C ${JSON.stringify(repo)} rev-parse --abbrev-ref origin/HEAD`
    );
    const name = stdout.trim();
    return name || null;
  } catch {
    return null;
  }
}

interface BranchSets {
  local: Set<string>;
  remote: Set<string>;
  remoteFull: Set<string>;
}

function validateBranchName(
  value: string,
  branches: BranchSets | null
): string | undefined {
  if (!value) return 'Branch name is required';
  if (value.startsWith('.')) return 'Must not start with a dot';
  if (value.startsWith('-')) return 'Must not start with a hyphen';
  if (value.endsWith('.') || value.endsWith('/')) return 'Must not end with a dot or slash';

  // eslint-disable-next-line no-control-regex
  if (/[ ~^:?*[\\\x00-\x1f\x7f]/.test(value)) return 'Contains an invalid character';
  if (value.includes('..')) return 'Must not contain consecutive dots (..)';
  if (value.includes('@{')) return 'Must not contain @{';
  if (value.includes('//')) return 'Must not contain consecutive slashes';
  if (value.split('/').some((part) => part.endsWith('.lock'))) {
    return 'No path component may end with .lock';
  }

  if (branches !== null) {
    if (branches.local.has(value)) return 'A local branch with this name already exists';
    if (branches.remote.has(value)) return 'A remote branch with this name already exists on the remote';
  }

  return undefined;
}

export default function CreateBranch() {
  const [repos, setRepos] = useState<RepoInfo[]>([]);
  const [selectedRepo, setSelectedRepo] = useState<string>('');
  const [branches, setBranches] = useState<BranchSets | null>(null);
  const [loading, setLoading] = useState(true);
  const [branchValue, setBranchValue] = useState('');
  const [branchError, setBranchError] = useState<string | undefined>(undefined);
  const [fromBranch, setFromBranch] = useState('');
  const [defaultRemoteBranch, setDefaultRemoteBranch] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([fetchRepos(), fetchTopWorktree()])
      .then(([fetchedRepos, topWorktree]) => {
        const sorted = [...fetchedRepos].sort((a, b) =>
          repoDisplayName(a.path, a.repo_origin ?? null).localeCompare(
            repoDisplayName(b.path, b.repo_origin ?? null)
          )
        );
        setRepos(sorted);

        const defaultRepo =
          topWorktree?.repo != null ? topWorktree.repo : (sorted[0]?.path ?? '');
        setSelectedRepo(defaultRepo);
      })
      .catch(() => {})
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    if (!selectedRepo) return;
    setBranches(null);
    setFromBranch('');
    setDefaultRemoteBranch(null);
    fetchDefaultRemoteBranch(selectedRepo).then(setDefaultRemoteBranch);
    fetchBranchesForRepo(selectedRepo)
      .then((fetched) => {
        const local = new Set<string>();
        const remote = new Set<string>();
        const remoteFull = new Set<string>();
        for (const b of fetched) {
          if (b.local_branch) local.add(b.local_branch);
          if (b.remote_branch) {
            remote.add(stripRemotePrefix(b.remote_branch));
            remoteFull.add(b.remote_branch);
          }
        }
        setBranches({ local, remote, remoteFull });
      })
      .catch(() => setBranches({ local: new Set(), remote: new Set(), remoteFull: new Set() }));
  }, [selectedRepo]);

  const defaultStrippedName = defaultRemoteBranch ? stripRemotePrefix(defaultRemoteBranch) : null;

  function handleBranchChange(value: string) {
    setBranchValue(value);
    setBranchError(validateBranchName(value, branches));
  }

  async function handleSubmit(values: Form.Values) {
    const repo = values['repo'] as string;
    const branch = values['branch'] as string;
    const from = values['from'] as string;

    const error = validateBranchName(branch, branches);
    if (error) {
      setBranchError(error);
      return false;
    }

    const ok = await withProgressToast('Creating branch…', 'Branch created', 'Failed to create branch', async () => {
      const fromArg = from ? ` --from ${JSON.stringify(from)}` : '';
      await execAsync(
        `${NIRI_WORKTREES} create-branch --repo ${JSON.stringify(repo)}${fromArg} ${JSON.stringify(branch)}`
      );
    });
    if (ok) await popToRoot();
    return false;
  }

  return (
    <Form
      isLoading={loading}
      actions={
        <ActionPanel>
          <Action.SubmitForm title="Create Branch" onSubmit={handleSubmit} />
        </ActionPanel>
      }
    >
      <Form.Dropdown
        id="repo"
        title="Repository"
        value={selectedRepo}
        onChange={setSelectedRepo}
      >
        {repos.map((repo) => (
          <Form.Dropdown.Item
            key={repo.path}
            value={repo.path}
            title={repoDisplayName(repo.path, repo.repo_origin ?? null)}
          />
        ))}
      </Form.Dropdown>
      <Form.Dropdown
        id="from"
        title="Create From"
        value={fromBranch}
        onChange={setFromBranch}
      >
        <Form.Dropdown.Item value="" title={defaultRemoteBranch ?? 'Default'} />
        {[...(branches?.local ?? [])]
          .filter((name) => name !== defaultStrippedName)
          .sort()
          .map((name) => (
            <Form.Dropdown.Item key={`local/${name}`} value={name} title={name} />
          ))}
        {[...(branches?.remoteFull ?? [])]
          .filter(
            (full) => !branches?.local.has(stripRemotePrefix(full)) && full !== defaultRemoteBranch
          )
          .sort()
          .map((full) => (
            <Form.Dropdown.Item key={`remote/${full}`} value={full} title={full} />
          ))}
      </Form.Dropdown>
      <Form.TextField
        id="branch"
        title="Branch Name"
        placeholder="my-feature"
        value={branchValue}
        {...(branchError !== undefined ? { error: branchError } : {})}
        onChange={handleBranchChange}
      />
    </Form>
  );
}
