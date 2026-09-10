# 06: Run and trace Manual Trigger durably

**What to build:** The Owner can durably admit and run the published Manual Trigger, watch reconnectable progress, cancel safely, and inspect one logical Activation and its checkpointed Causal Trace.

**Blocked by:** 05: Publish and roll back a Manual Trigger revision

**Status:** ready-for-agent

- [ ] Run Admission commits a Queued Run tied to exactly one immutable revision/plan before returning success.
- [ ] A deterministic Run state machine and governed bounded scheduler execute one Manual Trigger Activation without performing storage/network I/O inside the state machine.
- [ ] One dedicated SQLite writer commits an aggregate Durable Checkpoint containing resume state, Logical Order, outcome, digest/counters, provenance, and retained trace.
- [ ] Status and SSE distinguish queued, live/speculative, durable, terminal, and reconnect/resync behavior; HTTP response is command commit truth.
- [ ] Causal Trace exposes Run, revision/plan, Activation attempt/outcome, input/output, checkpoint, timing, and safe resource facts.
- [ ] A durable cooperative cancellation request stops new work, settles/checkpoints facts, and reaches a visible terminal result.
- [ ] Ready work, executor results, database commands, and SSE subscribers are count- and byte-bounded.
- [ ] Service restart after a completed run preserves terminal state, digest, trace, and revision identity.
- [ ] Browser-visible start, status, reconnect, cancel, trace, and completed-run tests use no internal database setup.
