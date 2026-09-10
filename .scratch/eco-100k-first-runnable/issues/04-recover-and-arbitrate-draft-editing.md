# 04: Recover and arbitrate Draft editing

**What to build:** Editing remains loss-aware across reloads, disconnections, and two browser sessions through durable undo/redo, a single Draft Lease, graceful takeover, and an encrypted Recovery Copy.

**Blocked by:** 03: Create the first Node Contract and durable Draft

**Status:** closed

- [x] Exactly one Editor Session can mutate a Mutable Draft; another session is read-only and sees holder/expiry state.
- [x] Heartbeat, expiry, voluntary release, approved takeover, and grace-period takeover are externally exercised without simultaneous writers.
- [x] Duplicate command IDs are idempotent and stale base Draft Versions return a structured conflict without partial mutation.
- [x] Undo and redo survive daemon/browser reload and append semantic commands instead of rewriting history.
- [x] Materialized Draft snapshots and bounded history compaction preserve current state and required undo behavior.
- [x] Unacknowledged browser commands are retained in an encrypted expiring Recovery Copy and cleared by logout/expiry policy.
- [x] Reconnect auto-replays only against the exact base Draft Version; changed authority/state creates an explicit recovery fork and diff.
- [x] Closing or reloading with unacknowledged work displays an honest saving/offline/conflict state.
- [x] A two-tab browser test covers edit, disconnect, takeover, stale return, fork, and loss-free reconciliation.


## Implementation decisions (`/ask-matt`)

- A renewable Draft Lease defaults to 30 seconds, editor heartbeat targets 10 seconds, and a requested takeover becomes claimable after a 10-second grace. Test-only environment values may shorten these clocks.
- Lease authority is fenced by a daemon-owned generation and opaque per-tab Editor Session identity. Status reveals a human label and expiry, never another tab's session identity.
- Materialize versioned snapshots every 16 accepted commands and retain the latest four. Retain at most 64 detailed history events and a durable 32-operation undo/redo horizon; compaction never changes current Draft state.
- Recovery Copy is browser-only best-effort ciphertext: AES-256-GCM under a non-extractable WebCrypto key stored by IndexedDB structured clone. Its logical expiry cannot exceed the authenticated session expiry; logout, the expiry timer, and the next startup purge it. It is never independently publishable authority.
- Reconciliation auto-replays only with an exact base Draft Version and current Lease. An already-acknowledged command returns its durable receipt. Every authority/version mismatch creates an explicit durable recovery fork and semantic diff for Owner-directed application.
- Use pinned Playwright with real Chromium pages for the two-tab acceptance. Browser tooling remains development-only and is excluded from the production bundle.


## Closure evidence

- `make test` passes Rust unit tests, all eight external Python acceptances, and the real Chromium two-tab journey.
- Strict workspace/all-target Clippy, Rust formatting, frontend typecheck/build, `npm audit`, and `cargo audit --deny warnings` pass.
- Chromium was run three consecutive times after stabilizing command-completion waits and copied-tab identity coverage.
- The final stripped production binary is 6,397,824 bytes; the complete bundle is 6,599,793 bytes and contains no browser, Playwright runtime, Node.js, Python, Rust toolchain, source, or source-map file.
- SBOM/license metadata covers 159 locked components; Playwright and Playwright Core are marked excluded development dependencies.
- Native systemd smoke passes with 28,135,424-byte RSS, 0.0000 idle CPU cores, 0 swap, one resident daemon, and hardening exposure 1.6.
