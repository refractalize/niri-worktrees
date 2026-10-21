# niri-worktrees

## Build and install

Build the release binary:

```sh
mise run build
```

From the repository root, build the release binary and install it to
`~/.local/bin`:

```sh
mise run install
```

Ensure `~/.local/bin` is included in your `PATH`, then verify the installation:

```sh
niri-worktrees --help
```

## Install the Vicinae extension

Install the `niri-worktrees` binary first, then install the extension dependencies
and build the extension:

```sh
mise run install-vicinae-extension
```

The build installs the extension into Vicinae's user extension directory. If the
Niri Worktrees commands do not appear immediately, restart Vicinae.

## Install the DankMaterialShell bar widget

The widget shows the worktree on the focused Niri workspace, how many commits
it is ahead of `origin/main` (or the origin default branch), whether it has
uncommitted changes, and, when the branch has a pull request, its number,
state, checks, and unresolved review threads. It is hidden on workspaces without a worktree.

```sh
mise run install-dms-plugin
dms ipc plugin-scan scan
```

Then enable **Niri Worktrees** in DMS Settings > Plugins and add it to a bar
section. Pull requests are only looked up for repositories with
`--list-pull-requests true`. Click the widget to open the pull request in the
browser; right-click to refresh.

DMS usually runs without your shell's `PATH`, so the widget adds
`~/.local/bin:~/.local/share/mise/shims` when running `niri-worktrees` and
`gh`. Change "Extra PATH directories" in the plugin settings if they live
elsewhere.

The widget runs `niri-worktrees worktree-status --json`, which can also be used
directly:

```sh
niri-worktrees worktree-status                  # focused workspace
niri-worktrees worktree-status --workspace-id 3 --json
niri-worktrees worktree-status --no-pull-request
```

## Test

```sh
mise run test
```

## Protect local files from worktree removal

Configure exact paths relative to each worktree root:

```sh
niri-worktrees set-repo --repo /path/to/repo \
  --protected-path mise.local.toml \
  --protected-path .envrc
```

Passing `--protected-path` replaces the repository's entire list; repeat it for
multiple paths. Omitting it preserves the existing list. To clear the list:

```sh
niri-worktrees set-repo --repo /path/to/repo --clear-protected-paths
```

The list is stored locally as `protected_paths` in
`${XDG_DATA_HOME:-~/.local/share}/niri-worktrees/repos.json` and is included in
`list-repos --json`. It applies to all worktrees of that stored repository.
Existing repositories default to an empty list.

Removal stops if any listed path exists, even when Git ignores it. Files,
directories, and symlinks (including dangling symlinks) count. Paths are literal,
not glob patterns; absolute paths and `..` components are rejected. File contents
are never read or logged. Invalid configuration and filesystem inspection errors
also block removal.

The CLI checks before running teardown and again immediately before Git removal.
Move or remove the protected files yourself, or change the configured list, before
retrying. Vicinae displays the CLI error. Direct `git worktree remove` bypasses
this application-level protection, and the checks are not atomic with deletion.
