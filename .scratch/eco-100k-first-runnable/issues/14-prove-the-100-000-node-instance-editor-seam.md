# 14: Prove the 100,000-Node-Instance editor seam

**What to build:** A deterministic 100,000-Node-Instance Workflow compiles and remains searchable, navigable, selectable, groupable, movable, inspectable, and renderable without graph-linear DOM.

**Blocked by:** 05: Publish and roll back a Manual Trigger revision

**Status:** ready-for-agent

- [ ] The fixture contains exactly 100,000 stable Node Instances and a deterministic connected topology with group/search/type distributions and frozen correctness hash.
- [ ] The production compiler validates the full document and the daemon returns a versioned packed topology snapshot with checked magic/version/section bounds/digest.
- [ ] A Web Worker builds spatial, adjacency, and search indexes incrementally with visible progress and cancellation.
- [ ] Command Map search and direct jump find frozen expected matches and focus the correct Node Instance.
- [ ] Structure Deck supports group-first navigation, selection, movement, collapse, and a 512-node grouping operation without graph-linear UI state.
- [ ] Canvas 2D renders viewport/semantic-zoom detail while inspector/configuration data loads lazily in bounded batches.
- [ ] App-owned DOM/list counts remain bounded by viewport/visible rows at 100,000 nodes; no hidden/detached 100,000-element tree exists.
- [ ] Packed bytes, worker/main-thread timings, responsiveness, memory, draw/culled counts, overlays, and DOM counts are recorded as release evidence.
- [ ] The accepted prototype observations are used as sanity prior art only; production completion requires release-candidate browser evidence.
