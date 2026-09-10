# 05: Publish and roll back a Manual Trigger revision

**What to build:** The Owner can compile the exact Draft, review diagnostics and differences, publish a signed immutable Manual Trigger revision with a pinned Execution Plan, and roll back without mutating history.

**Blocked by:** 04: Recover and arbitrate Draft editing

**Status:** ready-for-agent

- [ ] The compiler is a deterministic pure transformation of one Workflow Revision, exact Node Contract Locks, Compatibility Profile, and policy into a plan or structured diagnostics.
- [ ] Compilation validates graph identities, configuration, ports, expressions, capabilities, effects, budgets, and compatibility status without I/O.
- [ ] Publish flushes pending Draft Commands, verifies lease holder and exact Draft Version, and blocks stale/invalid/error states.
- [ ] Designated warnings require explicit acknowledgement and are captured in publication evidence.
- [ ] Published Revision and Execution Plan record algorithm-tagged digests, compiler/plan format identity, contract locks, and Compatibility Profile and remain immutable after restart.
- [ ] The editor shows a visual Draft-versus-published difference and distinct Mutable Draft/Published Revision states.
- [ ] Rollback selects/creates new current work from the preceding revision without editing or deleting either immutable revision.
- [ ] A plan remains pinned and readable across restart; no run-time or upgrade path silently recompiles it.
- [ ] Publish and rollback are driven through the public client seam and verified against signed identities rather than direct database inspection.
