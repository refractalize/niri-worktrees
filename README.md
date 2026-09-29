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

## Test

```sh
mise run test
```
