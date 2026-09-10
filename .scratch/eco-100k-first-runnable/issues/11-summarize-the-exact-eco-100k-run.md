# 11: Summarize the exact Eco 100K Run

**What to build:** Summarize consumes the merged stream, emits count/branch counters and a frozen algorithm-tagged Output Digest, and completes the full six-node Eco journey with exactly 100,000 logical Activations.

**Blocked by:** 10: Merge closed branch streams without unbounded memory

**Status:** ready-for-agent

- [ ] Summarize declares Barrier/Reducer Activation Shape, Pure deterministic effects, one output, bounded state/spill policy, and the Output Digest operation.
- [ ] The fixture equation is enforced: 1 Manual Trigger + 1 Generate Items + 49,998 Edit Fields + 49,998 If + 1 Merge + 1 Summarize = 100,000 Activations.
- [ ] An independent fixture definition freezes input range, transform, predicate, merge order, canonicalization, branch/total counts, digest algorithm, and expected digest before completion is claimed.
- [ ] Summarize verifies total and branch counters and emits a small typed summary without materializing the merged payload in browser memory.
- [ ] The browser shows exact activation progress, durable checkpoint progress, branch counts, elapsed wall/CPU time, and Output Digest.
- [ ] Causal Trace can locate any retained logical Activation and connect it to the final summary/provenance.
- [ ] Repeated runs and varied safe scheduler interleavings produce identical counts, logical ordering, and digest.
- [ ] A full browser/API test creates, publishes, runs, inspects, and rolls back the six-node Workflow without internal setup shortcuts.
