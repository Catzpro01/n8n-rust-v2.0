---
status: accepted
---
# Use a connected Preact/Canvas editor with a versioned hybrid contract

## Context

The editor must remain responsive and accessible at 100,000 Node Instances, fit the one-daemon production profile, preserve familiar workflow behavior without copying n8n implementation or trade dress, survive browser/daemon interruption, and publish only daemon-validated immutable revisions. The accepted prototype combines a group-first Structure Deck, a viewport-culled Atlas Canvas, global Command Map search, semantic zoom, packed graph state, and sparse overlays.

## Decision

### Browser technology and visual boundary

Use TypeScript and Preact for the application shell, Structure Deck, Command Map, forms, inspector, diagnostics, and bounded state surfaces. Render the graph imperatively with Canvas 2D; use Web Workers for packed-snapshot parsing and bounded index/search construction. Do not use a DOM element per Node Instance, React Flow, Rust/WASM UI, WebGL, or a mandatory `OffscreenCanvas` path in the baseline. The Node toolchain exists only in the build stage, and compiled content-hashed assets are embedded in the Rust daemon.

The initial target is current desktop evergreen browsers: the latest two Chrome/Edge/Firefox lines and current Safari desktop. Mobile may monitor Runs but full graph editing is deferred.

The interaction model may use familiar functional workflow conventions—node cards, typed ports, Connections, drag/drop, pan/zoom, inspector, expressions, and run evidence—but the product has an original name, logo, visual system, icons, wording, layout treatment, and implementation. Service marks such as Gmail or GitHub are obtained independently from official/licensed sources; no n8n-bundled asset is copied.

### Authority, document, and editing model

The Rust daemon owns the authoritative normalized Mutable Draft. A Workflow Revision contains executable graph structure plus authored layout, Groups, and annotations, but excludes transient Editor Session state such as viewport, selection, open panels, and search query. Groups are organizational only and never implicitly affect scheduling, Execution Lane, retry, resource policy, or Run output.

Draft mutations are semantic, versioned Draft Commands rather than generic JSON Patch. Each idempotent command/batch declares a command ID and base Draft Version. The daemon accepts it atomically, durably advances the Draft Version, and returns affected identities, diagnostics, and a delta. Stable internal identities cover Node Instances, Connections, Groups, and semantic ports; compatibility identifiers and safe unknown import fields are preserved explicitly rather than becoming internal authority.

Persist command history so undo/redo survives reload and takeover; undo and redo append semantic commands instead of rewriting history. Materialize bounded snapshots and compact old Draft history after publish according to retention policy.

One Editor Session holds a renewable Draft Lease. Other sessions are read-only but can request graceful takeover; approval or expiry transfers the lease. Unacknowledged changes remain an encrypted, expiring Recovery Copy. Replay is automatic only when its base Draft Version is still exact; otherwise create a recovery fork and explicit diff rather than silently merging or dropping work.

Publishing is a strict barrier: the lease holder flushes commands, the daemon verifies the exact Draft Version, compiles and validates in Rust, requires explicit acknowledgement of designated warnings, and atomically creates and signs one immutable Published Revision. Errors or stale state block publish.

### Virtualization seam

Open a large Workflow with one versioned compact topology/index snapshot for all Node Instances and Connections, then fetch detailed configuration, Node Contract schema, and Causal Trace data lazily in batches. The accepted prototype demonstrated that its 100,000-node packed graph and spatial index occupies about 1.6 MiB, so complete compact topology gives local search/jump without loading 100,000 detailed objects or DOM elements.

Keep a mostly immutable packed base plus sparse mutation/selection overlays. Build bounded spatial, search, and adjacency indexes in a worker. Canvas draws only the current semantic zoom level and viewport; detailed local Connections are culled, macro/group context remains visible, and overlay growth triggers a controlled rebase. Structure Deck rows, command results, inspector content, diagnostics, and accessibility lists are independently virtualized.

Canvas is not the only interface. Critical graph operations must have WCAG 2.2 AA-targeted keyboard and screen-reader paths through Structure Deck, Command Map, inspector, and a Connection list, with tested focus and announcements.

### Rust client contract

Use a versioned hybrid contract:

- same-origin HTTPS and versioned JSON for capability manifests, Draft Commands, batch detail requests, leases/takeover, publication, and structured errors;
- reconnectable server-sent events with cursor/`Last-Event-ID` behavior for Draft status, compilation, and Run/resource progress;
- a documented versioned little-endian binary format for compiled packed topology, with magic, schema version, section offsets and lengths, bounds, and digest validation before use.

Commands are acknowledged by their HTTP response; SSE is not the source of command commit truth. A capability handshake rejects incompatible client/protocol combinations explicitly. WebSockets are deferred until a real bidirectional requirement such as collaboration exists. Final endpoint names, byte offsets, heartbeat intervals, dependency minor versions, and storage/security key mechanics belong to the implementation specification and measured security decisions, not this ADR.

## Consequences

The browser remains lightweight at ordinary and extreme graph sizes, while the Rust daemon controls validation, durability, conflicts, compilation, and publication. The hybrid wire format adds a small documented binary protocol and conformance tests, but avoids the parse/allocation cost of JSON topology at 100,000 nodes. Connected authority and a single-writer lease avoid CRDT complexity; they deliberately trade real-time co-editing for deterministic autosave, recovery, and publication in the initial private-owner product.
