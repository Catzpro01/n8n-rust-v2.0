# SPDX-License-Identifier: AGPL-3.0-or-later

NODE_HOME ?= $(HOME)/.local/node-v22.19.0-linux-x64
export PATH := $(HOME)/.cargo/bin:$(NODE_HOME)/bin:$(PATH)

.PHONY: editor browser-test-deps check test build release release-test

editor:
	cd editor && npm ci && npm run typecheck && npm run build

browser-test-deps:
	cd editor && npx playwright install --with-deps chromium

check: editor
	cargo fmt --all -- --check
	cargo check --workspace --locked

test: editor
	cargo test --workspace --locked
	cargo build --workspace --locked
	cd editor && npx playwright install chromium && npm run test:browser
	python3 -m unittest tests/acceptance/test_daemon_shell.py tests/acceptance/test_owner_recovery.py tests/acceptance/test_durable_draft.py tests/acceptance/test_editing_recovery.py tests/acceptance/test_publication_rollback.py tests/acceptance/test_run_manual_trigger.py tests/acceptance/test_generate_items_artifacts.py

release-test: release
	python3 -m unittest tests/acceptance/test_release_bundle.py
	WORKFLOWD_BIN=out/tracer-bundle/usr/bin/workflowd python3 -m unittest tests/acceptance/test_run_manual_trigger.py

build: editor
	cargo build --workspace --locked

release:
	./scripts/build-release.sh
