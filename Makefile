# SPDX-License-Identifier: AGPL-3.0-or-later

NODE_HOME ?= $(HOME)/.local/node-v22.19.0-linux-x64
export PATH := $(HOME)/.cargo/bin:$(NODE_HOME)/bin:$(PATH)

.PHONY: editor check test build release release-test

editor:
	cd editor && npm ci && npm run typecheck && npm run build

check: editor
	cargo fmt --all -- --check
	cargo check --workspace --locked

test: editor
	cargo test --workspace --locked
	python3 -m unittest tests/acceptance/test_daemon_shell.py

release-test: release
	python3 -m unittest tests/acceptance/test_release_bundle.py

build: editor
	cargo build --workspace --locked

release:
	./scripts/build-release.sh
