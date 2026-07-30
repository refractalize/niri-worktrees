.PHONY: install install-vicinae-extension test

install:
	cargo build --release
	install -Dm755 target/release/niri-worktrees $(HOME)/.local/bin/niri-worktrees

install-vicinae-extension:
	npm --prefix vicinae-extension ci
	npm --prefix vicinae-extension run build

test:
	cargo test
