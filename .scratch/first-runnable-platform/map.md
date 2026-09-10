# Wayfinder Map: First Runnable Independent Platform

## Destination

Reach a clear, implementation-ready route to a first runnable clean-room release: one Rust-only production daemon that serves an independent editor, saves and publishes a small workflow, executes it durably, and proves the 100,000-lightweight-Activation Eco benchmark under 500 MiB RAM, 0.5 CPU, and a 10 GiB managed footprint. The output is ready to collapse through `/to-spec`, then `/to-tickets`.

**Route status: specification published with `ready-for-agent`; ready for `/to-tickets` (all 10 Wayfinder tickets resolved).**

## Published specification

- [First Runnable Independent Workflow Platform — Eco 100K](spec.md): one browser/API/process/cgroup acceptance seam, 114 user stories, implementation decisions, testing decisions, exclusions, and recovery/compatibility constraints; status `ready-for-agent`.

## Notes

- Existing decisions live in `CONTEXT.md` and `docs/adr/`; do not reopen them silently.
- `docs/legal/clean-room-policy.md` is mandatory.
- Server/editor default license: AGPL-3.0-or-later. SDK/protocol/schema license: Apache-2.0.
- Use `/grilling` plus `/domain-modeling` for HITL decisions, `/research` for primary-source reading, and `/prototype` for runnable unknowns.
- Prototype code answers questions; it is not production implementation.
- Resource claims require cgroup-enforced evidence on the target VPS.

## Decisions so far

- [Clean-room product and licensing](../../docs/adr/0050-build-an-independent-clean-room-editor-and-platform.md): build an independently named editor and platform; do not copy n8n code or trade dress.
- [Constrained Rust-only deployment](../../docs/adr/0044-ship-a-rust-only-default-production-profile.md): the default server application is one Rust daemon under the Eco envelope.
- [Adaptive resources](../../docs/adr/0049-adapt-execution-to-the-effective-resource-envelope.md): execution adapts from 0.5 to multiple cores while preserving behavior.
- [Durable execution](../../docs/adr/0003-durable-at-least-once-execution.md): use checkpoints, idempotency, deduplication, retry classification, and compensation.
- [Compiled structured graph](../../docs/adr/0007-compile-immutable-revisions-into-structured-plans.md): compile immutable revisions and constrain cycles through explicit constructs.
- [Streaming data](../../docs/adr/0008-stream-envelopes-and-spill-artifacts.md): bounded Envelopes and content-addressed Artifacts replace unbounded materialization.

- [Define the first runnable vertical slice](issues/01-define-the-first-runnable-slice.md): Eco 100K proves an independent six-node editor-to-durable-Run journey, crash recovery, trace inspection, rollback, and separate 100,000-Activation and 100,000-Node-Instance fixtures.
- [Prototype the Eco execution kernel](issues/02-prototype-the-eco-execution-kernel.md): the constrained Rust/SQLite kernel is feasible; FULL commit cadence dominates, and bounded group checkpoints recover deterministically after an ungraceful abort.
- [Prototype the virtualized independent editor](issues/03-prototype-the-virtualized-editor.md): use group-first macro navigation, a viewport-culled Canvas 2D detail renderer, command search/jump, semantic zoom, packed graph state, and sparse UI overlays.
- [Research the minimal Rust production stack](issues/04-research-the-minimal-rust-production-stack.md): use a feature-gated Tokio/Axum/Rusqlite/rustls stack, dedicated SQLite writer, direct cgroup parsing, embedded assets, and a hardened one-daemon systemd bundle.
- [Define clean-room compatibility fixtures](issues/05-define-clean-room-compatibility-fixtures.md): pin a private 2.39.0 black-box oracle and use sanitized owner-authored exports, public docs, deterministic observations, provenance, and evidence-backed import classifications.
- [Choose the independent editor interface](issues/06-choose-the-editor-interface.md): use connected TypeScript/Preact, imperative virtualized Canvas 2D, durable versioned Draft Commands, a single graceful-takeover Draft Lease, compact topology plus lazy detail, accessible bounded DOM surfaces, and versioned JSON/SSE/binary Rust contracts.
- [Choose the engine and storage interfaces](issues/07-choose-the-engine-and-storage-interfaces.md): separate a pure compiler and deterministic Run state machine from the governed scheduler, aggregate SQLite checkpoint writer, streamed encrypted Artifact store, and bounded external use-case seams.
- [Choose the first Node Contract](issues/08-choose-the-first-node-contract.md): publish an Apache-2.0 behavior-first contract and narrow Activation Context, prove it with six deterministic Native Nodes, then expand through a version-pinned Catalog Conformance Matrix for built-ins and installed/curated community packages.
- [Choose the production bundle and recovery path](issues/09-choose-the-production-bundle-and-recovery-path.md): ship a signed systemd-first one-binary bundle with optional identical OCI form, Current/Previous release slots, protected Recovery Reserve, complete local/off-site Recovery Sets, offline Recovery Kit, restore drills, Quarantine Mode, and opt-in Safe Auto accelerators.
- [Confirm the route is ready for a spec](issues/10-confirm-the-route-is-ready-for-a-spec.md): one coherent browser/API/process/cgroup acceptance seam covers editor-to-publish-to-100K durable Run, crash recovery, trace/digest/rollback, and the separate 100K-node virtualization fixture; native node names retain visible exact n8n compatibility aliases.

## Intentionally deferred to specification

- Concrete Rust type/module names, SQLite table layouts, wire byte offsets, queue/threshold constants, and filesystem paths.
- Exact `v1alpha1` Node Contract schema fields and conformance vectors within the accepted ADR boundaries.
- Measured disk partition values, backup schedules/provider credentials, and release-signing implementation within the accepted recovery policy.

## Out of scope

- Full n8n node-catalog parity, AI Agent, Scrape Orchestrator, Workflow Hub, and Skill Hub implementation: these remain product goals, including a fixture-backed Catalog Conformance Matrix, Play-Store-like discovery, visual Agent Blueprints showing effective Engine/Model Route/Memory/Skill/MCP/Policy/Output at Global/Project/Workflow/Node scope, adapters for Hermes/OpenClaw/OpenCode/Claude Code/MiroFish/Antigravity and later engines, and native Rust plus external 9Router-compatible model routing, but receive later maps after the runnable foundation exists.
- Rewriting Chromium, Firefox, hosted scraping providers, or third-party agent products: Rust protocol adapters and remote execution are the boundary.
- Public launch or commercial distribution: requires specialist legal review and a later release map.
