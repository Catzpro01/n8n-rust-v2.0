# Wayfinder Map: First Runnable Independent Platform

## Destination

Reach a clear, implementation-ready route to a first runnable clean-room release: one Rust-only production daemon that serves an independent editor, saves and publishes a small workflow, executes it durably, and proves the 100,000-lightweight-Activation Eco benchmark under 500 MiB RAM, 0.5 CPU, and a 10 GiB managed footprint. The output is ready to collapse through `/to-spec`, then `/to-tickets`.

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

## Not yet specified

- Exact technology and interaction architecture for an independent high-scale browser editor.
- Exact Rust module interfaces and storage transaction boundaries for the first vertical slice.
- Measured feasibility and bottlenecks of the 100,000-Activation Eco benchmark.
- Exact Node Contract and SDK surface for the first native nodes.
- Reproducible production bundle, updater, and rollback details under the 10 GiB profile.
- The first complete feature slice and its external test seams.

## Out of scope

- Full n8n node-catalog parity, AI Agent, Scrape Orchestrator, and Workflow Hub implementation: these remain product goals but receive later maps after the runnable foundation exists.
- Rewriting Chromium, Firefox, hosted scraping providers, or third-party agent products: Rust protocol adapters and remote execution are the boundary.
- Public launch or commercial distribution: requires specialist legal review and a later release map.
