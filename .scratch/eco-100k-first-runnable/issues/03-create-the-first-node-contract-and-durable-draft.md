# 03: Create the first Node Contract and durable Draft

**What to build:** The Owner can create a Workflow, add and configure a Manual Trigger from the native catalog, save it through semantic Draft Commands, reload it from SQLite, and inspect its exact versioned Node Contract.

**Blocked by:** 02: Establish the Owner and recovery root

**Status:** resolved

- [x] An Apache-2.0 `v1alpha1` Node Contract meta-schema, canonical JSON/digest rules, conformance fixture format, and Rust SDK surface are published.
- [x] The Manual Trigger contract declares identity, Configuration Schema, ports, Source Activation Shape, Pure effect, no capabilities, Resource Budget, typed outcomes, and compatibility metadata.
- [x] Unknown normative contract fields fail; namespaced non-authoritative extensions round-trip; exact contract version/digest becomes a Node Contract Lock.
- [x] The editor catalog renders the Manual Trigger using declarative original metadata and adds it to a new Workflow without node-supplied UI code.
- [x] Semantic Draft Commands carry idempotent command identity and base Draft Version; accepted commands return new version, delta, affected identities, and diagnostics.
- [x] Workflow, Node Instance, Connection-ready identity, layout, annotation, settings, and safe compatibility metadata persist through a restart and reload.
- [x] Transient viewport, selection, panels, and search query do not change the Draft Version.
- [x] The API and editor never expose SQL rows, storage paths, engine handles, or vault internals.
- [x] External tests create, edit, reload, and validate the one-node Draft only through the browser/client contract.


## Answer

Published the Apache-2.0 `v1alpha1` interoperability surface as a strict meta-schema, canonical JSON and algorithm-tagged digest implementation, exact Node Contract Lock type, independent conformance fixture format, fixture, and Rust SDK crate. The first native contract is `canopy.native/manual-trigger@0.1.0`: a deterministic Pure Source with no capabilities, bounded resources, typed outcomes, strict configuration, original declarative editor metadata, and visible compatibility metadata. The release bundle carries the contract and SDK materials at the paths recorded in its manifest.

The authenticated daemon now owns normalized Mutable Drafts in FULL-synchronous WAL SQLite. Semantic `add_node`, `configure_node`, and Workflow annotation commands require a unique command identity and exact base Draft Version. Accepted acknowledgements include the new version, semantic delta, affected identities, and diagnostics; command receipts survive restart for durable idempotency. Exact Node Contract Locks are enforced, stale bases conflict without mutation, malformed configuration is rejected, and an immediate write transaction serializes command commits.

The Preact editor renders its native catalog from build-generated declarative contract metadata. After Owner sign-in, its catalog action creates a Workflow and adds Manual Trigger through the same public client contract used by tests; no node-supplied executable UI is loaded. Workflow, Node Instance identity/configuration/layout/annotation, Connection collection, settings, compatibility metadata, and command receipts survive daemon restart. Viewport, selection, panel, and search state are explicitly transient and do not advance Draft Version.

Verification on the target VPS (2026-09-11):

- The test-first tracer initially failed intentionally with `404` at `GET /api/v1/catalog` before production implementation.
- Four Node Contract SDK conformance tests passed, including recursive canonicalization, strict required/unknown normative fields, namespaced extension round-trip, and the exact published Manual Trigger fixture/lock.
- Seven external daemon tests passed. Ticket 03's client-only journey checks the embedded editor routes, declarative catalog digest, contract facts, Workflow creation, add/configure acknowledgements, durable duplicate-command replay, exact-lock and configuration rejection, stale conflict, transient editor state, restart/reload, persisted authored metadata, and internal-data exclusion.
- The same journey passed against the stripped release binary. Cargo formatting/tests and Clippy with warnings denied, TypeScript typecheck, ShellCheck, npm audit with zero vulnerabilities, bundle checksum/runtime-only inspection, and JSON material review passed.
- Native systemd install/start/restart/uninstall smoke passed at 27,541,504 bytes RSS, 0.0000 idle CPU cores, and hardening exposure 1.6 (`OK`); explicit test cleanup left no service, process, state, or master key.
- Final verified implementation commit was `c73c93c25d3ac20340f61d53612d320505aa643a`; its release manifest identified that exact clean commit and a 6,045,568-byte binary.

Review reached fixed point at `c73c93c25d3ac20340f61d53612d320505aa643a`. Standards review tightened nested normative-field validation, included the contract API version in exact locks, reused the established mutation credential boundary, serialized Draft writers, and corrected the packaged SDK directory contract. Spec review found no remaining missing, incorrect, or out-of-scope behavior.
