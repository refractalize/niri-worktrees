.PHONY: install test

install:
	cargo build --release
	install -Dm755 target/release/niri-worktrees $(HOME)/.local/bin/niri-worktrees

test:
	cargo test

