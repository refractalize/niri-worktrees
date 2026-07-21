import { List, Icon, ActionPanel, Action, closeMainWindow, Color } from '@vicinae/api';
import { useEffect, useState } from 'react';
import type { Worktree } from './types';
import {
  execAsync,
  fetchRepoDisplayNames,
  removeWorktree,
  showError,
  withProgressToast,
  NIRI_WORKTREES,
} from './utils';

async function fetchWorktrees(): Promise<Worktree[]> {
  const { stdout } = await execAsync(`${NIRI_WORKTREES} list-worktrees --json`);
  const data = JSON.parse(stdout) as { worktrees: Worktree[] };
  return data.worktrees;
}

async function focusWorktree(path: string): Promise<boolean> {
  return withProgressToast('Focusing worktree…', 'Worktree focused', 'Failed to focus worktree', async () => {
    await execAsync(`${NIRI_WORKTREES} focus-worktree ${JSON.stringify(path)}`);
  });
}

function latestFocusMs(windows: Worktree['windows']): number | null {
  let latest = -Infinity;
  for (const w of windows) {
    if (w.focus_timestamp) {
      const ts = w.focus_timestamp.secs * 1000 + w.focus_timestamp.nanos / 1_000_000;
      if (ts > latest) latest = ts;
    }
  }
  return isFinite(latest) ? latest : null;
}

function focusAgoText(windowsLatestMs: number, globalLatestMs: number): string {
  const diffSec = Math.floor((globalLatestMs - windowsLatestMs) / 1000);
  if (diffSec < 60) return `${diffSec}s ago`;
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHr = Math.floor(diffMin / 60);
  return `${diffHr}h ago`;
}

export default function Worktrees() {
  const [worktrees, setWorktrees] = useState<Worktree[]>([]);
  const [repoNames, setRepoNames] = useState<Map<string, string>>(new Map());
  const [loading, setLoading] = useState(true);

  const load = () => {
    setLoading(true);
    Promise.all([fetchWorktrees(), fetchRepoDisplayNames()])
      .then(([wts, names]) => {
        setWorktrees(wts);
        setRepoNames(names);
      })
      .catch((error) => showError('Failed to load worktrees', error))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    load();
  }, []);

  return (
    <List isLoading={loading} navigationTitle="Worktrees">
      {!loading && worktrees.length === 0 && (
        <List.EmptyView
          icon={Icon.CodeBlock}
          title="No Worktrees"
          description="No active worktree–workspace mappings found."
        />
      )}
      {(() => {
        const globalLatestMs = worktrees.reduce<number>((max, wt) => {
          const ts = latestFocusMs(wt.windows);
          return ts !== null && ts > max ? ts : max;
        }, -Infinity);

        return worktrees.map((wt) => {
        const basename = wt.path.split('/').pop() ?? wt.path;
        const branchDiffersFromBasename = wt.local_branch !== null && wt.local_branch !== basename;
        const title = branchDiffersFromBasename ? `${basename} • ${wt.local_branch}` : basename;
        const subtitle = wt.repo ? (repoNames.get(wt.repo) ?? wt.repo) : '';
        const windowCount = wt.windows.length;
        const isUrgent = wt.windows.some((w) => w.is_urgent);
        const wtLatestMs = latestFocusMs(wt.windows);
        const lastFocused = wtLatestMs !== null && isFinite(globalLatestMs)
          ? focusAgoText(wtLatestMs, globalLatestMs)
          : null;

        return (
          <List.Item
            key={wt.path}
            title={title}
            subtitle={subtitle}
            icon={isUrgent ? { source: Icon.Exclamationmark, tintColor: Color.Red } : Icon.CodeBlock}
            accessories={[
              lastFocused !== null ? { text: lastFocused, icon: Icon.Clock } : {},
              windowCount > 0
                ? {
                    text: `${windowCount} window${windowCount !== 1 ? 's' : ''}`,
                    icon: Icon.Wind,
                  }
                : {},
              wt.workspace !== null
                ? { tag: { value: String(wt.workspace.id), color: Color.Blue } }
                : { tag: { value: 'no workspace', color: Color.SecondaryText } },
            ]}
            actions={
              <ActionPanel>
                <Action
                  title="Focus Worktree"
                  icon={Icon.Eye}
                  onAction={async () => {
                    const ok = await focusWorktree(wt.path);
                    if (ok) closeMainWindow({ clearRootSearch: true });
                  }}
                />
                <Action
                  title="Remove Worktree"
                  icon={Icon.Trash}
                  style={Action.Style.Destructive}
                  shortcut={{ modifiers: ['ctrl'], key: 'x' }}
                  onAction={async () => {
                    const ok = await removeWorktree(wt.path);
                    if (ok) load();
                  }}
                />
                <Action.CopyToClipboard
                  title="Copy Path"
                  content={wt.path}
                  shortcut={{ modifiers: ['ctrl'], key: 'c' }}
                />
              </ActionPanel>
            }
          />
        );
        });
      })()}
    </List>
  );
}
