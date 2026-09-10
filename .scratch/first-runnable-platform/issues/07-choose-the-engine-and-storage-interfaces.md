# Choose the engine and storage interfaces

Type: grilling
Status: resolved
Blocked by: 02, 04

## Question

Where should the external and internal seams sit among workflow compilation, scheduling, durable Run state, Causal Trace, SQLite transactions, Envelopes, and Artifacts so the first implementation is deep, testable, and replaceable without speculative interfaces?


## Answer

Adopt the modular-monolith seams and invariants in [ADR-0053](../../../docs/adr/0053-separate-the-deterministic-engine-from-transactional-storage.md).

A pure deterministic compiler converts one Workflow Revision plus locked Node Contracts and Compatibility Profile into a pinned immutable Execution Plan. A deterministic Run state machine consumes commands/outcomes and emits scheduling/checkpoint decisions without I/O; a separate Resource-Governed scheduler supplies bounded weighted-fair concurrency and may fuse physical Execution Segments while preserving Logical Order and per-node evidence.

One dedicated SQLite writer exposes typed use-case transactions, especially durable Run Admission, side-effect attempt intent/outcome, and an aggregate Durable Checkpoint. A checkpoint atomically advances resume state, logical order, digest/counters, outcomes/provenance, retry/failure/cancellation, Artifact references, and retained Causal Trace. The browser distinguishes Speculative Progress; crash recovery replays only the bounded window after the last checkpoint.

Envelopes stay bounded and refer to streamed encrypted content-addressed Artifacts. Objects become durable before SQLite may reference them, making failed writes safe orphans rather than dangling committed references. Initial deduplication is Owner-namespace-wide; credentials never enter Artifact storage.

The engine admits Runs durably under bounded count/byte budgets, reports retryable overload before admission, reduces concurrency/spills/backpressures under resource pressure, and durably suspends before hard limits. It records Uncertain Outcome rather than blindly retrying ambiguous external effects. Tiered retention preserves active/recent and pinned evidence while safely compacting eligible terminal trace data.

The owner explicitly accepted bounded replay, visible uncertain side effects, fail-by-default unhandled errors, tiered retention, deterministic parallelism, durable bounded fairness, adaptive slowdown/suspension, Owner-wide encrypted Artifact deduplication, publish-pinned plans that survive upgrades, cooperative cancellation, and logical-plus-physical observability on 2026-09-11. After a plain-language explanation that “slowdown” means reducing concurrency/intake and spilling or suspending only under pressure—not sleeping nodes or degrading correctness—the owner confirmed ticket 07.
