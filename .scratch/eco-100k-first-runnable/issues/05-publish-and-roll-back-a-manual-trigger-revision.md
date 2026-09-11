# 05: Publish and roll back a Manual Trigger revision

**What to build:** The Owner can compile the exact Draft, review diagnostics and differences, publish a signed immutable Manual Trigger revision with a pinned Execution Plan, and roll back without mutating history.

**Blocked by:** 04: Recover and arbitrate Draft editing

**Status:** claimed

- [ ] The compiler is a deterministic pure transformation of one Workflow Revision, exact Node Contract Locks, Compatibility Profile, and policy into a plan or structured diagnostics.
- [ ] Compilation validates graph identities, configuration, ports, expressions, capabilities, effects, budgets, and compatibility status without I/O.
- [ ] Publish flushes pending Draft Commands, verifies lease holder and exact Draft Version, and blocks stale/invalid/error states.
- [ ] Designated warnings require explicit acknowledgement and are captured in publication evidence.
- [ ] Published Revision and Execution Plan record algorithm-tagged digests, compiler/plan format identity, contract locks, and Compatibility Profile and remain immutable after restart.
- [ ] The editor shows a visual Draft-versus-published difference and distinct Mutable Draft/Published Revision states.
- [ ] Rollback selects/creates new current work from the preceding revision without editing or deleting either immutable revision.
- [ ] A plan remains pinned and readable across restart; no run-time or upgrade path silently recompiles it.
- [ ] Publish and rollback are driven through the public client seam and verified against signed identities rather than direct database inspection.
## Progress (2026-09-11) — implementation complete, verification pending

Implementation landed on `arena/01a08da0-n8n-rust-v2-0`:

- `crates/workflowd/src/revision.rs` — pure `compile_workflow` (`canopy.compiler/1` → `canopy.execution-plan/1`): validates graph identities, duplicate ids/names, connection endpoints, declared ports, cycles (explicit-constructs-only), configuration schema subset, expression markers, capabilities, effect classes, budgets against the Compatibility Profile policy, and contract lock identity and digest. Warnings carry `require_acknowledgement`; only `severity: "error"` diagnostics fail compilation. `RevisionService` implements `compile_preview`, `publish` (single IMMEDIATE transaction: lease-holder + generation check → exact Draft Version → compile → designated-warning gate → next revision number → canonical digests → HMAC signature → insert-only store), `rollback` (moves the `current_revision` pointer only), `publication`, `revision(n)` (stored plan verbatim), and `diff` (draft vs current published revision). Eight unit tests cover determinism, policy, ports, cycles, schema, and designated warnings.
- `crates/workflowd/src/security.rs` — `SecurityService::sign_revision_payload` (RFC 2104 HMAC-SHA256 under `sha256("canopy-revision-signing-v1\0" || master key)` over canonical JSON; output `hmac-sha256:<hex>`) and public `record_audit` (`revision.publish` / `revision.rollback`).
- `crates/workflowd/src/revision_http.rs` — six public routes under `/api/v1/workflows/{id}`: `GET compile`, `POST publish` (write auth), `GET publication`, `GET revisions/{n}`, `POST rollback` (write auth), `GET diff`. Structured problems: `409 stale_draft_version` (with `current_draft_version`), `409 publication_warnings_unacknowledged` (with `required_acknowledgements`), `423 draft_lease_required`, `422 compilation_failed` (with `diagnostics`), `409 no_preceding_revision`, `404`.
- `draft.rs` — `lease_holder_generation_matches` and `load_draft_document` shared with the publication path; `draft_http` auth helpers are now `pub(crate)`.
- `main.rs` / `app.rs` / `identity.rs` — module wiring, `AppState.revisions`, six routes, capabilities 11 → 13 (`deterministic-revision-compiler`, `signed-published-revisions`).
- Editor — publication section with distinct Mutable Draft / Published Revision states, compile + per-publication warning acknowledgement, visual added/removed/modified diff, revision history, and rollback (`main.tsx`, `editing-types.ts`, `styles.css`).
- Verification artifacts — `tests/acceptance/test_publication.py` (deterministic digests recomputed client-side, signature recomputed from the master key, stale/lease/error gates, undo-restored document reproducing the first digests, rollback chain with no history loss, restart proving pinned byte-identical plans); `editor/tests/publish.mjs` (Playwright compile → ack → publish → annotate → publish → rollback journey); Makefile test list; `test:browser` runs both browser journeys; `docs/operations/publication-api.md`.
- Verified inside the sandbox: frontend `tsc --noEmit` passes, esbuild production build passes, and the production bundle contains the new public seam markers.

Verification still required (no Rust toolchain in this sandbox; the VPS at 103.55.37.234 is unreachable from sandbox egress, which is domain-allowlisted): run `cargo fmt --all` (normalizes formatting, then `cargo fmt --all -- --check`), `make check`, `make test`, `npm audit`, and `cargo audit --deny warnings` on the VPS against this branch. Closure evidence is appended under `## Answer` once those pass.
