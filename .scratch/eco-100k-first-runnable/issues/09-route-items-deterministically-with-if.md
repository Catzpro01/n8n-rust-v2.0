# 09: Route items deterministically with If

**What to build:** If routes every transformed item to exactly one named true/false output with stable ordering, counts, provenance, diagnostics, and Causal Trace.

**Blocked by:** 08: Transform items with Edit Fields and the safe expression VM

**Status:** ready-for-agent

- [ ] If declares one typed/dynamic-item input, two stable named outputs, Per-Item Stream Activation Shape, Pure effects, and no capabilities.
- [ ] The editor configures the frozen Eco predicate using declarative typed conditions and supported expressions.
- [ ] Every input item reaches exactly one output; no item is duplicated, dropped, or sent to both branches.
- [ ] True/false item order is deterministic across repeated runs, restarts, and resource profiles.
- [ ] Missing/null/type/coercion boundary cases and AND/OR composition used by the first contract are covered by conformance tests.
- [ ] Invalid Port Schema, predicate, or output configuration blocks publication with structured diagnostics.
- [ ] Branch output, count, item-link provenance, and logical route decision are inspectable in Causal Trace without retaining all payloads in browser memory.
- [ ] Cancellation, queue pressure, and downstream backpressure do not violate exactly-one-branch behavior.
