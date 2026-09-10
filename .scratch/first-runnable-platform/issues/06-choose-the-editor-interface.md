# Choose the independent editor interface

Type: grilling
Status: resolved
Blocked by: 03, 04

## Question

Which browser technology, graph document model, virtualization seam, and Rust client contract should the independent editor adopt based on the prototype and constrained production stack evidence?


## Answer

Adopt the connected TypeScript/Preact + Canvas 2D design and versioned hybrid Rust client contract recorded in [ADR-0052](../../../docs/adr/0052-use-a-connected-preact-canvas-editor-with-a-hybrid-contract.md). The decision incorporates the accepted 100,000-node B+A+C prototype and the constrained Rust stack research.

The Rust daemon is authoritative. It accepts durable semantic Draft Commands against an exact Draft Version, maintains durable undo/redo and materialized snapshots, grants one graceful-takeover Draft Lease, reconciles encrypted Recovery Copies by exact replay or explicit fork, compiles in Rust, and publishes only an exact atomically signed immutable revision. Groups remain organizational and transient Editor Session navigation state stays outside Workflow Revisions.

The browser loads all compact topology/index data but lazy-loads configuration/schema/trace detail. Preact owns bounded DOM surfaces; Canvas 2D and worker-built packed indexes own graph scale. All list surfaces remain virtualized, and critical operations have WCAG 2.2 AA-targeted non-Canvas paths.

The wire seam is versioned JSON for commands/manifests/errors, reconnectable SSE for status/progress, and a documented validated binary topology snapshot. WebSockets, WebGL, Rust/WASM UI, DOM-per-node, real-time collaboration, and offline-first CRDT behavior are intentionally deferred.

The owner confirmed this technical decision on 2026-09-11 provided that the product keeps familiar n8n-visible workflow interaction, import capability, and the previously requested Hub scope. Familiar functional conventions are permitted, but the implementation, name/logo, generic icons, visual system, wording, and distinctive treatment remain original. Third-party service marks must come from independent authorized sources, not n8n assets.

Workflow JSON and Node Instances remain import targets under the 2.39.0 Compatibility Profile. Proven native behavior runs in Rust; permitted user-supplied community packages may run in isolated compatibility workers; unsupported behavior is preserved/reported/delegated rather than silently changed. n8n implementation code or restrictively licensed built-in node source is never bundled as an import shortcut.

Workflow Hub and Skill Hub remain explicit later milestones with an in-product Play-Store-like search/review/install/update experience over locally indexed Hub Sources. Their lateness is sequencing, not removal from scope.
