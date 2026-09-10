# 08: Transform items with Edit Fields and the safe expression VM

**What to build:** The Owner can configure Edit Fields with fixed values and supported expressions, preview diagnostics, and deterministically transform every generated item while preserving types and item linking.

**Blocked by:** 07: Stream Generate Items through bounded Envelopes and Artifacts

**Status:** ready-for-agent

- [ ] Edit Fields declares one Per-Item Stream input/output, Pure deterministic effects, bounded expansion, and no capabilities.
- [ ] Its declarative form distinguishes fixed values from expressions and supports the exact Eco transformation fields.
- [ ] The Rust expression VM compiles the accepted common subset and produces deterministic typed results using logical context only.
- [ ] Tests preserve missing versus null, strings versus numbers/booleans, nested arrays/objects, Unicode, escaping, and item-link provenance.
- [ ] Unsupported JavaScript semantics are identified before publication and classified for future delegation rather than evaluated in the daemon.
- [ ] Syntax, type, missing-node/item, resource-limit, and runtime errors use stable original structured codes and safe evidence.
- [ ] Logical clock and seeded randomness are controlled; ambient time/random/environment access is unavailable.
- [ ] Backpressure, cancellation, output limits, and forced Artifact spill remain effective across the transform.
- [ ] Repeated and restart/replay executions produce byte-for-byte canonical logical outputs and the same trace facts.
