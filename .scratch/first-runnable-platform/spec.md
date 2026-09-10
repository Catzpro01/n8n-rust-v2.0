# Spec: First Runnable Independent Workflow Platform — Eco 100K

Type: spec
Status: ready-for-agent
Blocked by: none
Source: First Runnable Independent Platform Wayfinder, all ten tickets resolved

## Problem Statement

The Owner needs a self-hosted workflow automation platform that feels familiar to an n8n user while remaining an independently named and implemented product. The existing product must not be copied: server/editor source, Enterprise files, tests, icons, assets, copy, or distinctive trade dress are forbidden implementation inputs. Compatibility against frozen n8n 2.39.0 behavior must instead be demonstrated through public documentation, owner-authored exports, black-box observations, and independently written conformance fixtures.

The first release must prove an end-to-end product, not a collection of horizontal framework pieces. The Owner must be able to edit a real six-node Workflow, recover autosaved work, publish an immutable revision, execute exactly 100,000 logical Activations, inspect progress and evidence, survive an ungraceful daemon restart, verify a deterministic Output Digest, and roll back. A separate 100,000-Node-Instance fixture must prove that the editor and compiler scale without creating one DOM element or one runtime task per node.

The default installation has an unusually strict Eco Resource Profile: one always-resident Rust daemon, embedded SQLite, embedded static editor assets, no resident Node.js/Python/browser/model runtime, no more than 500 MiB RAM, sustained 0.5 logical CPU, and no more than 10 GiB of managed disk. It must still finish the 100,000-Activation fixture, and additional cores must safely reduce duration without changing Workflow semantics, logical ordering, or correctness evidence.

The result must also establish trustworthy seams for future compatibility nodes, remote workers, eight Node Forms, four Execution Lanes, AI Agent composition, Agent Blueprints, Skill/MCP visualization, native and external Model Routes, Workflow Hub, Skill Hub, scraping, connectors, and full catalog conformance. Those later capabilities must not be prebuilt into or burden the first runnable bundle.

## Solution

Deliver one clean-room vertical slice named **Eco 100K**. A hardened Rust daemon serves an original TypeScript/Preact editor, owns the authoritative Mutable Draft, compiles exact signed Published Revisions into pinned Execution Plans, schedules bounded work, persists Durable Checkpoints and logical Causal Trace in SQLite WAL/FULL, streams large values through encrypted content-addressed Artifacts, and exposes one versioned browser/client contract.

The editor uses the accepted hybrid interaction design: Structure Deck for group-first macro navigation, Atlas Canvas for viewport-culled detail, Command Map for global search/jump, semantic zoom, virtualized lists, a collapsible inspector, and accessible non-Canvas paths for critical actions. It loads one compact topology snapshot and lazy-loads detailed configuration, schemas, and trace records.

The Eco Workflow contains six original Pure deterministic Native Nodes: Manual Trigger, Generate Items, Edit Fields, If, Merge, and Summarize. Summarize exposes the Output Digest operation used by the fixture. Their Apache-2.0 `v1alpha1` Node Contracts exercise Source, Per-Item Stream, Barrier/Reducer, typed routing, gradual Port Schemas, safe expressions, backpressure, item linking, reduction, and deterministic digest behavior without internet or credentials.

The primary acceptance seam is the browser-visible Owner journey through the versioned daemon interface, real process lifecycle, real SQLite/filesystem state, and real cgroup controls. One harness drives install, edit, publish, run, kill, restart, inspect, verify, rollback, and resource reporting. Pure compiler/engine and contract conformance tests supplement this seam where exhaustive state coverage cannot be obtained economically through the browser.

Native systemd packaging is the default. The same signed binary is also published in an optional minimal OCI image tested with Podman and Docker. Signed Current/Previous Release Slots, a protected Recovery Reserve, complete Recovery Sets, Quarantine Mode, an offline Recovery Kit, and Restore Drills make update and recovery behavior part of the product rather than operator folklore.

## User Stories

1. As the Owner, I want one documented native systemd installation path, so that I can install the platform on a small Linux VPS without Docker.
2. As the Owner, I want an optional OCI image containing the same release binary, so that I can choose Podman or Docker without receiving a different product.
3. As the Owner, I want the default installation to contain no Rust, Node.js, or Python toolchain, so that production remains small and has less attack surface.
4. As the Owner, I want the daemon to start as an unprivileged service identity, so that a server vulnerability does not automatically become root access.
5. As the Owner, I want the service to enforce the Eco CPU, memory, swap, task, and disk policy, so that the workflow platform cannot consume the whole host.
6. As the Owner, I want the editor and administration surface to bind to private control access by default, so that high-authority functions are not accidentally public.
7. As the Owner, I want only explicitly published ingress routes to be public, so that webhooks do not expose the editor or recovery controls.
8. As the Owner, I want HTTPS with reloadable certificate material, so that browser and webhook traffic is encrypted without embedding secrets in the release.
9. As the Owner, I want a first-run Owner identity rather than an anonymous administrator, so that audit actors and later multi-user seams are real from the beginning.
10. As the Owner, I want a Recovery Kit created during installation, so that encrypted state can still be opened after the VPS is lost.
11. As the Owner, I want the product to warn until the Recovery Kit is stored outside the VPS, so that I do not mistake local encryption for recoverability.
12. As the Owner, I want release identity, version, and build provenance visible in the UI and diagnostics, so that I know exactly what is running.
13. As the Owner, I want a health/readiness view, so that I can distinguish a running process from a usable installation.
14. As the Owner, I want the daemon to restart automatically after an ordinary crash, so that transient faults do not require manual repair.
15. As the Owner, I want an original product name, logo, visual system, and wording, so that my platform is not presented as an official n8n product.
16. As the Owner, I want to open the independent editor in a current desktop browser, so that no desktop application is required.
17. As the Owner, I want Chrome, Edge, Firefox, and Safari desktop support within the declared evergreen window, so that the editor is not tied to one browser engine.
18. As the Owner, I want to create an Eco 100K Workflow from a built-in template, so that the first complete journey is immediately reproducible.
19. As the Owner, I want to add the six first Native Nodes from a searchable catalog, so that the fixture uses ordinary editor behavior rather than hidden setup.
20. As the Owner, I want to connect named typed ports visually, so that invalid graph connections are found before execution.
21. As the Owner, I want to configure node fields through declarative forms, so that node-supplied UI code is not required.
22. As the Owner, I want fixed values and supported expressions to be visually distinguishable, so that configuration intent is clear.
23. As the Owner, I want expression diagnostics before publication, so that unsupported JavaScript behavior is not discovered during a production Run.
24. As the Owner, I want to move, group, collapse, and annotate Node Instances, so that large Workflows remain understandable.
25. As the Owner, I want Groups to remain organizational only, so that moving a node cannot silently change Run semantics.
26. As the Owner, I want Command Map search across every Node Instance, so that I can jump directly to a node without panning across a large canvas.
27. As the Owner, I want Structure Deck navigation, so that I can understand the Workflow at group and macro level.
28. As the Owner, I want semantic zoom, so that the canvas shows groups, compact nodes, or detailed local Connections at the appropriate scale.
29. As the Owner, I want the inspector and Structure Deck to collapse independently, so that I can prioritize canvas space or detailed configuration.
30. As the Owner, I want selection, viewport, open panels, and search query to remain Editor Session state, so that transient navigation does not create a new Workflow Revision.
31. As the Owner, I want authored layout, Groups, and annotations included in the Workflow Revision, so that a published Workflow remains understandable when reopened.
32. As the Owner, I want autosave acknowledgements tied to a Draft Version, so that I know which changes are durable.
33. As the Owner, I want undo and redo to survive reload, so that autosave does not destroy editing history.
34. As the Owner, I want only one Editor Session to hold the Draft Lease, so that two tabs cannot silently overwrite each other.
35. As the Owner, I want another tab to request graceful takeover, so that a crashed or forgotten editor does not permanently lock the Draft.
36. As the Owner, I want unacknowledged work retained as an encrypted expiring Recovery Copy, so that a browser crash does not automatically lose recent editing.
37. As the Owner, I want Recovery Copy replay only against the exact base Draft Version, so that reconnect does not silently merge incompatible edits.
38. As the Owner, I want conflicting recovered work offered as a fork and diff, so that I choose what to retain.
39. As the Owner, I want keyboard and screen-reader access to critical graph operations, so that Canvas is not the only usable interface.
40. As the Owner, I want visible save, offline, conflict, compile, and publish states, so that the editor never pretends uncertain work is safe.
41. As the Owner, I want publication to flush all Draft Commands and verify the exact Draft Version, so that a stale or partial document cannot be published.
42. As the Owner, I want complete deterministic compilation before publication, so that a Published Revision is runnable and reviewable.
43. As the Owner, I want designated warnings to require acknowledgement and errors to block publication, so that risk cannot be hidden.
44. As the Owner, I want publication to create an immutable signed Published Revision, so that production Runs cannot change underneath me.
45. As the Owner, I want a visual difference between the Mutable Draft and last Published Revision, so that I understand what will change.
46. As the Owner, I want to roll back to the preceding Published Revision without deleting history, so that a bad change is reversible.
47. As the Owner, I want the six first nodes to have familiar functional names, so that the editor is approachable to an n8n user.
48. As the Owner, I want imported n8n identity and `typeVersion` visible as a Compatibility Alias/detail, so that familiarity does not erase provenance.
49. As the Owner, I want native and imported identities kept separate, so that a compatibility claim never implies official endorsement or copied implementation.
50. As the Owner, I want a Compatibility Report before an imported Workflow can publish, so that Native, Delegated, Preserved, Adapted, Unsupported, and Rejected behavior is explicit.
51. As the Owner, I want to start the Eco Run from the Published Revision, so that the execution target is immutable.
52. As the Owner, I want Run Admission to return success only after the Run is durable, so that an accepted trigger cannot disappear from a memory queue.
53. As the Owner, I want exactly 100,000 logical Activations in the Eco fixture, so that benchmark comparisons use the same work.
54. As the Owner, I want additional CPU cores to reduce duration automatically when safe, so that stronger hardware is used without changing the Workflow.
55. As the Owner, I want the same Logical Order and Output Digest at 0.5 CPU and multiple cores, so that speed does not redefine correctness.
56. As the Owner, I want live progress and Durable Checkpoint progress shown separately, so that I understand what may replay after a crash.
57. As the Owner, I want effective CPU quota, CPU time, wall time, throttling, RSS, memory peak, disk I/O, commit count, trace volume, and disk use visible, so that efficiency claims are measurable.
58. As the Owner, I want queue depth, backpressure, spill, and suspension state visible, so that adaptive slowdown is understandable.
59. As the Owner, I want the control plane to remain responsive under workload pressure, so that I can inspect or cancel a Run before the host becomes unusable.
60. As the Owner, I want the Resource Governor to reduce concurrency before memory or disk exhaustion, so that safety is preferred to an OOM crash.
61. As the Owner, I want a Run to enter Durable Suspension near a hard resource limit, so that temporary pressure does not discard completed work.
62. As the Owner, I want new ingress rejected before admission when bounded queues are full, so that overload is explicit rather than lossy.
63. As the Owner, I want weighted-fair scheduling across admitted Runs, so that one giant Run cannot starve small interactive work.
64. As the Owner, I want cooperative durable cancellation, so that the engine stops safely without pretending external effects were reversed.
65. As the Owner, I want an unhandled node failure to fail the Run unless an explicit route or continue policy exists, so that failure behavior is predictable.
66. As the Owner, I want ambiguous external effects represented as Uncertain Outcome, so that a timeout cannot silently cause duplicate writes.
67. As the Owner, I want the Eco Run to survive an ungraceful process kill, so that durability is proved under a real fault rather than graceful shutdown.
68. As the Owner, I want restart to resume from the latest Durable Checkpoint with bounded replay, so that throughput does not require a commit per Activation.
69. As the Owner, I want restart to avoid duplicating committed output, so that recovery does not corrupt results.
70. As the Owner, I want the recovered Run to produce the same Output Digest as the uninterrupted Run, so that replay correctness is objectively verified.
71. As the Owner, I want to inspect any retained logical Activation in Causal Trace, so that I can understand data flow and decisions.
72. As the Owner, I want Causal Trace to link input provenance, output references, routes, checkpoints, retries, resource use, and errors, so that failures are explainable.
73. As the Owner, I want Execution Segment metrics in addition to per-node evidence, so that physical optimization remains observable.
74. As the Owner, I want large values shown as authorized Artifact references and streamed previews, so that the browser does not materialize bulk data.
75. As the Owner, I want recent full trace and pinned evidence retained under a visible policy, so that important Runs remain auditable.
76. As the Owner, I want eligible terminal evidence compacted before disk danger, so that the 10 GiB profile remains sustainable.
77. As the Owner, I want Evidence Pins to prevent compaction or expiry, so that selected proof is not removed automatically.
78. As the Owner, I want no credential values in logs, trace, Artifacts, browser responses, exports, or recovery manifests, so that observability does not become exfiltration.
79. As the Owner, I want the 100,000-Node-Instance fixture to compile successfully, so that graph size is not limited by the first six-node demo.
80. As the Owner, I want search and direct jump across 100,000 Node Instances, so that large graphs remain navigable.
81. As the Owner, I want group selection and movement on the large fixture, so that organizational operations do not require per-node DOM state.
82. As the Owner, I want viewport rendering to stay bounded as graph size grows, so that 100,000 nodes do not create 100,000 DOM elements.
83. As the Owner, I want input and navigation to remain responsive while indexes build in a worker, so that large-document load does not freeze the page.
84. As a clean-room compatibility operator, I want the n8n 2.39.0 oracle pinned by immutable image/package digest, so that observations do not drift with upstream releases.
85. As a clean-room compatibility operator, I want every fixture to record public sources, reference environment, author, sanitizer, hashes, and normalization rules, so that evidence provenance is reviewable.
86. As a clean-room compatibility operator, I want a fail-closed sanitizer for credentials, headers, personal data, and opaque large values, so that fixtures are safe to commit.
87. As a clean-room compatibility operator, I want object-key normalization without erasing array order, null/missing, scalar type, output port, item order, or item-link meaning, so that comparisons remain semantically honest.
88. As a clean-room compatibility operator, I want one minimal behavior per fixture, so that failures identify a specific compatibility gap.
89. As a clean-room compatibility operator, I want the first native mappings tested against owner-authored black-box exports, so that compatibility claims have evidence.
90. As a clean-room compatibility operator, I want exact English product error text excluded from conformance unless it is a wire contract, so that the platform uses original copy.
91. As a Node SDK developer, I want the Node Contract schema and Rust SDK under Apache-2.0, so that extensions can be adopted broadly.
92. As a Node SDK developer, I want canonical fixtures and algorithm-tagged digests, so that independent implementations can prove the same contract.
93. As a Node SDK developer, I want configuration, ports, Activation Shape, effects, capabilities, budgets, and outcomes declared without engine internals, so that another runtime can implement the behavior.
94. As a Node SDK developer, I want a dynamic JSON Item Port Schema when static shape is unavailable, so that n8n-compatible data remains representable.
95. As a Node SDK developer, I want unknown normative contract fields rejected and namespaced extensions preserved, so that forward evolution is explicit.
96. As a Node SDK developer, I want only a capability-limited Activation Context, so that node code cannot reach SQLite, the scheduler, vault internals, or arbitrary filesystem paths.
97. As a Node SDK developer, I want backpressured input/output and Artifact streams, so that my node cannot accidentally force unbounded materialization.
98. As a Node SDK developer, I want logical clock and seeded randomness, so that Pure nodes can remain deterministic during replay.
99. As a Node SDK developer, I want structured Node Outcomes, so that retry, failure, suspension, uncertainty, and cancellation are not inferred from strings or panics.
100. As a release operator, I want signed Release Manifests with digests, provenance, SBOM, migrations, and compatibility ranges, so that an update is authenticated before installation.
101. As a release operator, I want Current and Previous immutable Release Slots, so that a failed candidate can roll back quickly.
102. As a release operator, I want update activation to require Owner approval or an enabled maintenance window, so that production does not change unexpectedly.
103. As a release operator, I want a verified pre-upgrade Recovery Set before migration, so that database change has a known rollback point.
104. As a release operator, I want admission and public side effects closed during candidate migration/readiness, so that automatic rollback cannot discard new production work.
105. As a release operator, I want incompatible active Execution Plans to block an update, so that suspended Runs are not recompiled to new semantics.
106. As a release operator, I want a protected Recovery Reserve, so that backup and rollback remain possible when ordinary state grows.
107. As the Owner, I want local Recovery Sets and periodic Restore Drills, so that database corruption can be recovered without guessing.
108. As the Owner, I want suspected corruption to enter Quarantine Mode, so that automatic writes or triggers do not worsen evidence.
109. As the Owner, I want diagnosis and estimated loss before approving a restore, so that a stale backup does not silently replace recoverable state.
110. As the Owner, I want encrypted off-site backup status shown separately from local backup status, so that host-loss readiness is truthful.
111. As the Owner, I want the installation marked Disaster-Recovery Ready only after an off-site Recovery Set and Recovery Kit pass checks, so that a green badge represents tested recovery.
112. As the Owner, I want GPU use Off by default and Safe Auto available only in Advanced Settings, so that optional acceleration cannot silently add heavy runtime or semantic drift.
113. As the Owner, I want the first runnable to reserve clear adapter and lock seams for Agent Blueprints and external Agent Engines, so that later AI composition does not require replacing the engine.
114. As the Owner, I want future Agent Blueprint, 9Router-style Model Route, Skill/MCP visualization, and Hub work excluded from this implementation milestone, so that those goals remain visible without turning the first release into a big bang.

## Implementation Decisions

### 1. Product, legal, and release boundary

- Build an independently named clean-room product. Familiar workflow concepts and factual compatibility labels are allowed; n8n source, tests, Enterprise files, editor assets, icons, copy, and distinctive presentation are forbidden inputs.
- Use the frozen n8n 2.39.0 Compatibility Profile identified by release tag and commit in the accepted research. Current online documentation is supporting evidence only until confirmed against the private frozen oracle.
- Independently authored daemon/editor code is AGPL-3.0-or-later. Node SDKs, protocol/package schemas, clients, fixtures intended as public interoperability material, and examples are Apache-2.0. Brand assets are separately reserved. Third-party components retain their own licenses/notices.
- The first runnable is a production-shaped vertical slice, not full n8n parity and not a prototype merge. Prototype branches remain evidence only.
- Native primary Node Definition identities are separate from imported identities. Preserve the exact user-assigned name, external type, `typeVersion`, safe configuration, and Compatibility Profile. Show a visible Compatibility Alias and fixture-backed status without implying endorsement.

### 2. Primary external test seam

- Prefer one highest stable seam: a harness controlling the installed daemon, browser/client API, process lifecycle, filesystem state, and cgroup. The harness performs the entire Eco journey and produces one evidence bundle.
- The same versioned daemon interface used by the editor is the API seam for setup and assertions; the test must not write SQLite tables or internal files to create success conditions.
- Real process termination, restart, WAL recovery, and Artifact verification are mandatory. Graceful shutdown is not a substitute for the fault case.
- The separate 100,000-Node-Instance browser fixture uses the same document import/load, compiler, topology snapshot, search, grouping, and renderer surfaces.
- Pure compiler/engine tests and Node Contract conformance tests are secondary seams justified by combinatorial state coverage. They do not replace the installed journey.

### 3. Production system shape

- Run one modular Rust daemon as the only always-resident product server in the default profile. The daemon contains HTTP/HTTPS routing, static editor serving, identity/control, Draft handling, compiler, deterministic Run engine, Resource Governor/scheduler, SQLite storage adapter, filesystem Artifact adapter, Causal Trace queries, and maintenance subcommands.
- Use a manually configured Tokio runtime: one core worker in the Eco profile; more workers/permits only when effective cgroup CPU allows. Cap blocking work and keep SQLite off Tokio core workers.
- Use Axum/Tower HTTP with selected features, rustls with one measured crypto provider, Serde JSON, Rusqlite with bundled SQLite and required backup/hooks/limits features, BLAKE3 for content identity, and `tracing` JSON. Exact locked versions and feature flags are release inputs.
- Do not add an ORM, database server, message broker, container runtime requirement, OpenTelemetry collector, plugin host, or local language runtime to the first bundle.
- Keep code as a modular monolith initially. Extract crates/services only after measured isolation or reuse needs; domain contracts must not expose SQL, filesystem paths, Tokio types, or HTTP framework types.

### 4. Editor technology and interaction

- Build the shell, Structure Deck, Command Map, inspector, forms, diagnostics, and bounded accessibility surfaces in TypeScript/Preact. Node tooling is build-only.
- Render graph detail imperatively with Canvas 2D. Do not use a DOM node per Node Instance, React Flow, a required WebGL path, or a required Rust/WASM UI.
- Construct packed topology, spatial, adjacency, and search indexes in a Web Worker in chunks. Canvas renders only the active semantic zoom and viewport. Detailed local Connections are culled; macro/group context remains visible.
- Keep one mostly immutable packed topology base plus sparse mutation/selection overlays. Rebase overlays under a bounded policy rather than rewriting the full graph for each edit.
- Load the complete compact topology for local search/jump. Load detailed node configuration, Configuration Schema, and Causal Trace data lazily in bounded batches.
- Independently virtualize Structure Deck rows, search results, inspector content, diagnostics, connection lists, and accessibility lists. DOM count must scale with viewport/visible lists, not graph size.
- Target the latest two Chrome/Edge/Firefox desktop lines and current Safari desktop. Mobile may view/monitor but full editing is outside this release.
- Critical operations must have tested keyboard and screen-reader equivalents through non-Canvas surfaces, targeting WCAG 2.2 AA.

### 5. Authoritative Draft and publication model

- The daemon is the authority for every Mutable Draft. Browser storage is an encrypted, expiring best-effort Recovery Copy, never an independently publishable Draft.
- Mutations are semantic idempotent Draft Commands with command identity and base Draft Version, not generic JSON Patch. The daemon accepts a command/batch atomically and returns the new version, affected identities, delta, and diagnostics.
- Persist undo/redo as semantic command history. Undo/redo append commands; they do not rewrite accepted history. Periodically materialize snapshots and compact eligible old Draft history after publication.
- Grant one renewable Draft Lease to one Editor Session. Other sessions are read-only. Takeover uses notification/grace, approval or expiry. Unacknowledged commands remain Recovery Copy material.
- Replay a Recovery Copy automatically only if its base Draft Version remains exact. Otherwise create an explicit recovery fork and diff.
- A Workflow Revision includes executable graph structure, stable Node Instance/Connection/Group identities, authored layout, Groups, annotations, settings, Compatibility Profile, Node Contract Locks, and safe opaque import metadata. It excludes selection, viewport, open panels, and search query.
- Groups are organizational and never implicitly alter execution, retries, resources, capabilities, or output.
- Publication flushes pending commands, verifies the exact Draft Version and lease holder, performs complete deterministic compilation, blocks on errors, requires acknowledgement for designated warnings, and atomically stores/signs the immutable Published Revision and pinned Execution Plan.
- Rollback creates or selects a new current Draft/publication from a preceding immutable revision; it never mutates old revisions.

### 6. Browser-to-daemon contract

- Use same-origin HTTPS and a versioned capability handshake. Embedded editor and daemon normally match, but incompatible clients must receive an explicit structured error.
- Use versioned JSON for identity/session operations, Draft manifest/details, Draft Commands, lease/takeover, compile diagnostics, publication, Run admission/cancellation/status, trace queries, Artifact authorization, and structured problem responses.
- Use reconnectable server-sent events with cursor and gap/resync behavior for Draft status, compilation, Run/resource progress, and durable event notifications. HTTP command responses—not SSE—are commit acknowledgement.
- Use one documented versioned little-endian binary format for packed topology. Validate magic, schema version, section bounds/lengths, and digest before exposing arrays to renderer/index code.
- Keep event subscriber buffers bounded. A slow or disconnected browser cannot block engine/checkpoint progress.
- Do not add WebSockets until a real bidirectional/collaboration requirement exists.

### 7. Compiler and Execution Plan

- Implement compilation as a deterministic pure transformation of one Workflow Revision, exact Node Contract Locks, Compatibility Profile, and explicit policy into either a canonical Execution Plan or structured diagnostics.
- The compiler performs no database, network, filesystem, clock, random, or executor I/O.
- Validate graph identities, ports, cardinality, configuration, expressions, explicit cycles/loop constructs, capabilities, effects, budgets, and compatibility status.
- Include immutable plan format/compiler ABI identity, revision digest, contract/catalog locks, logical graph, scheduling dependencies, lane eligibility, resource/effect metadata, and deterministic segment candidates in the plan.
- Draft compilation may cache/incrementally update diagnostics. Publication always executes a full compile.
- Each Run pins one Execution Plan unchanged. Upgrade or resume never recompiles an active/suspended Run silently.

### 8. First Node Contract and six-node fixture

- Publish canonical deterministic JSON `v1alpha1` Node Contracts plus meta-schema, canonical fixtures, and Rust SDK under Apache-2.0. Promote to `v1` only after the stated conformance gates and at least one non-native binding exercise the model.
- Required contract sections are identity/version/digest; Configuration Schema and declarative editor hints; stable named Port Schemas/cardinality; Activation Shape; Effect Class and determinism/idempotency/retry/reconciliation facts; capability requests; default/hard Resource Budgets; allowed typed Node Outcomes; and compatibility mappings/extensions.
- Reject unknown normative fields. Preserve only namespaced non-authoritative extensions.
- Support Source, Per-Item Stream, Bounded Batch, and Barrier/Reducer Activation Shapes. Compatibility materialization is always bounded/spillable.
- Separate Node Contract behavior from Node Implementation binding and Execution Lane. Do not promise a stable Rust binary ABI; stabilize schemas, semantics, messages/handles, outcomes, and fixtures.
- Restrict Activation Context to validated configuration/input, backpressured output by stable port, Artifact streams, cancellation/deadline, logical clock/seeded randomness, safe trace events, and approved Capability Grant/Secret Lease handles. No engine/database/vault/filesystem internals are exposed.
- All six first Node Implementations are Pure, deterministic, replayable, credential-free, network-free Native Nodes in the Inline Native Lane.
- Manual Trigger is a Source Activation that emits one captured deterministic invocation Envelope.
- Generate Items consumes that Envelope once and emits exactly 49,998 ordered items for the Eco fixture from declared deterministic range fields.
- Edit Fields executes one Per-Item Stream Activation per generated item, preserves item linking, and derives the fields required by the route and digest using fixed values and the safe expression subset.
- If executes one Per-Item Stream Activation per item, evaluates one deterministic typed predicate, and sends every item to exactly one named true/false output.
- Merge executes one Barrier/Reducer Activation after both branch streams close. Its first mode appends/spools both branch streams in declared deterministic input order without unbounded memory.
- Summarize executes one Barrier/Reducer Activation, verifies total and branch counts, and emits the algorithm-tagged Output Digest over canonical logical items and provenance.
- The activation equation is 1 Manual Trigger + 1 Generate Items + 49,998 Edit Fields + 49,998 If + 1 Merge + 1 Summarize = exactly 100,000 logical Activations.
- Freeze the fixture predicate, canonicalization, branch counts, and expected digest in independently generated acceptance data before implementation completion. The value must not be copied from n8n or derived from production implementation output alone.

### 9. Deterministic Run engine and scheduler

- Implement Run semantics as a deterministic state machine consuming the pinned plan, durable resume state, commands/events, timers, and typed Activation outcomes. It emits scheduling, attempt, checkpoint, trace, suspension, failure, cancellation, and completion decisions without performing I/O.
- Keep scheduling policy separate. The Resource Governor applies bounded weighted-fair Run selection, priorities/deadlines, lane and destination limits, backpressure, and adaptive concurrency.
- Never create one thread, Tokio task, or actor per Node Instance or Activation. Keep ready work, executor results, database commands, public ingress, and event subscribers count- and byte-bounded.
- Additional cores may execute independent work concurrently. Commit outcomes and trace in deterministic Logical Order. Serialize ordering-sensitive work according to the Node Contract.
- A trigger becomes a Run only after durable Run Admission. Reject new external ingress with retryable overload before acceptance when limits are full; never drop an admitted Run from memory pressure.
- Default unhandled Activation failure stops new admissions for that Run and transitions it to Failed after settled checkpointing. Continue-on-error or failure routing must be explicit.
- Cancellation is durable/cooperative: stop new Activations, request in-flight cancellation, checkpoint settled facts, and record work/effects that cannot be cancelled.
- For side-effecting future nodes, commit attempt identity/idempotency/reconciliation intent before dispatch when required. A crash after external acceptance but before durable outcome may create Uncertain Outcome. Retry automatically only with declared safety.

### 10. SQLite durability and recovery

- Use bundled approved SQLite, WAL mode, `synchronous=FULL`, foreign keys, busy timeout, and runtime limits. Set and read back critical pragmas on each relevant connection; fail closed on mismatch.
- Assert approved SQLite runtime version and compile options at startup. Do not rely only on crate version.
- One dedicated writer thread owns writes. A very small bounded read-only connection set serves snapshots/queries.
- Concentrate Run persistence in typed use-case transactions rather than exposing arbitrary repositories. The principal Durable Checkpoint transaction atomically advances resume state, Logical Order, correctness digest/counters, outcomes/provenance, retry/failure/cancellation state, Artifact references, and retained logical Causal Trace.
- Bound checkpoint grouping by item count, bytes, elapsed time, progress latency, and maximum replay work selected by the Resource Governor. Do not hard-code the prototype batch size.
- UI and API distinguish Speculative Progress after the last checkpoint. After an ungraceful restart, replay only the bounded window and retain exactly one committed logical result.
- Startup loads nonterminal Runs and pinned plans, verifies supported formats and referenced Artifacts, handles incomplete attempts, restores timers/admission, and resumes with bounded replay.
- Schedule WAL checkpoints and online backups deliberately; retain companion WAL/SHM semantics during live operation.

### 11. Envelopes and Artifact storage

- Keep Envelopes bounded: typed small values, metadata, provenance, and Artifact references. Preserve the n8n-style JSON Item facade at compatibility boundaries.
- Spill large JSON, binary, streams, and retained bulk output to Artifact storage without fully materializing them in scheduler queues or browser memory.
- Use streaming algorithm-tagged content digests and envelope encryption inside the Owner Artifact Namespace. Deduplicate only within the current Owner trust domain. Credentials and Secret Leases are never Artifacts.
- Write Artifact content through size limits and digest/encryption into same-filesystem staging; flush/synchronize; atomically place the immutable object; only then permit SQLite to commit a live reference.
- Treat a staged/final unreferenced object as a safe orphan. Never permit a committed database reference to point at an object that was not made durable.
- Authorize every Artifact reference/read. Never expose storage paths as API identity.
- Inline/spill thresholds may adapt for resource efficiency but cannot change logical output, item linking, or Output Digest.

### 12. Causal Trace, metrics, and retention

- Store stable logical trace facts for Run, checkpoint, Activation attempt/outcome, input/output provenance, output port, route, retry, cancellation, failure, suspension, and Artifact reference.
- Store physical Execution Segment, queue, worker/lane, cgroup resource, disk I/O, commit, and throttle measurements as an additional layer.
- Segment fusion may reduce physical work but cannot remove per-Node Instance output/error/retry/provenance observability.
- Default retention follows the accepted tiered quota-bound policy: recent successful raw evidence, longer metadata, longer error/audit evidence, and quota-aware Artifact retention unless pinned. The specification implementation must expose configured values and storage estimates.
- Compaction of terminal Runs retains revision/plan identity, terminal state, Output Digest, checkpoint/hash-chain evidence, counters, failures/retries, required Artifact roots, and Evidence Pins.
- Never persist credentials, authorization headers, cookies, workflow secret values, private model reasoning, or unbounded payloads in trace.
- cgroup v2 observation discovers the effective process cgroup and parses keyed CPU, memory, I/O, pids, and pressure files. Unknown fields are tolerated. Missing controllers are reported unavailable, never fabricated as zero.

### 13. Resource adaptation and benchmark evidence

- Enforce the Eco benchmark with `CPUQuota=50%`, `MemoryMax=500M`, swap disabled, explicit task limits, I/O accounting where available, and a 10 GiB managed storage policy.
- The Run must complete at 0.5 CPU even if slower. Resource pressure may reduce concurrency/cache, change safe batch decisions, stream/spill, reject new admission, or durably suspend. It may not alter semantics or silently delete evidence.
- Reserve a small control-plane budget for editor/API health, status, cancellation, and recovery.
- Repeat the same correctness fixture under larger CPU allowances. More available cores must increase eligible worker/branch/Run permits automatically and must not change branch counts, item ordering, trace meaning, or digest.
- Capture wall time, process/cgroup CPU, quota periods, throttled periods/time, RSS/process peak, cgroup memory peak/events, disk bytes/I/O stats where available, SQLite commit count, database/Artifact footprint, trace rows/bytes, queue/backpressure events, restart point, replay window, and digest.
- Treat prototype measurements as feasibility/prior art, not production pass numbers. Production evidence must be generated by the release candidate on the target VPS.

### 14. 100,000-Node-Instance document fixture

- Use a deterministic synthetic Workflow Revision with exactly 100,000 Node Instances and a connected topology sufficient to exercise packed columns, adjacency, groups, search, selection, and local Connections.
- Compile the entire document through the production compiler and return a versioned packed topology snapshot through the production client seam.
- Prove global Command Map search, direct jump, Structure Deck navigation, group-first macro view, viewport selection, multi-selection/grouping, node movement, inspector detail fetch, and semantic zoom.
- Assert that Canvas owns graph drawing and that app-owned rendered DOM/list surfaces remain bounded by visible viewport/list windows rather than total node count. No assertion may pass by hiding an unbounded detached DOM tree.
- Build indexes in a worker/chunks with progress and cancellation. Keep main-thread controls responsive during load/search/group operations.
- Record compile, transfer, parse/index, search, jump, selection/group, render, memory, packed bytes, overlay size, and DOM-count evidence. Retain the accepted prototype hash/size/search observations as prior-art sanity data, not a hard production performance claim.

### 15. Minimum n8n 2.39.0 compatibility seam

- Maintain a private immutable reference environment identified by exact image/package digest, release commit, runtime, locale, timezone, edition, and relevant environment settings.
- Use only public documentation/API specifications, owner-authored exported Workflows, and black-box UI/API/CLI/webhook observations. Keep reference operation separate from clean-room implementation.
- Store one deterministic fixture per behavior with provenance manifest, sanitized Workflow JSON, input, minimal normalized expected facts, explicit volatile-field rules, and correctness digest.
- Fail closed during sanitization of credential names/IDs, authentication headers/cookies, OAuth data, webhook secrets, hostnames/personal values, pinned production payloads, and unexpected binary/base64 values.
- For the first release, prove workflow JSON import/preservation/reporting for the supported native-equivalent subset, item arrays and linking, safe expression subset, typed branching, deterministic merge behavior, execution order setting handling where represented, and unsupported node/expression diagnostics.
- Preserve safe unknown fields in an opaque extension area without executing them. Never silently upgrade `typeVersion`, expressions, execution order, permissions, or credentials.
- Produce a machine/browser Compatibility Report classifying every imported feature as Native equivalent, Delegated compatible, Preserved opaque, Adapted with declared difference, Unsupported, or Rejected unsafe.
- Full Catalog Conformance Matrix expansion is outside this release, but the fixture layout and report status vocabulary must not require redesign.

### 16. Identity, capabilities, and security

- Provide one Owner account in the first deployment while retaining explicit actor, project membership, role, service account, credential ownership, and audit seams.
- Use memory-hard Argon2id password hashing with calibrated parameters and upgrade-on-login capability; use secure same-origin session cookies, CSRF/origin protections, request/body limits, and strict sensitive-header redaction.
- Separate private control routing from Public Gateway routing in the application. Public routes never expose Draft, credential, update, backup, restore, or Recovery Kit operations.
- Keep credential values in an envelope-encrypted vault. Wrap per-credential data keys with a rotatable master key supplied through systemd/container credentials. Activations receive only scoped short-lived Secret Leases.
- Deny outbound network by default. No first-six Node Contract requests egress, process, real time/random, credential, or external side-effect capabilities.
- Emit original structured diagnostics with stable codes and correlation identity. Do not copy n8n prose.
- Apply CSP and browser security headers compatible with embedded assets and same-origin API/SSE. Do not require external CDN scripts/styles/fonts for the editor.

### 17. Production bundle, update, and recovery

- Install immutable root-owned Current and Previous Release Slots and create temporary Staging only during update. Keep configuration/state outside release directories.
- Authenticate a Release Manifest through an Owner-approved Release Trust Root. Verify bundle/image digest, provenance, SBOM/notices, migration list, required Recovery Reserve, anti-rollback state, and database/plan/contract ranges.
- Permit unprivileged background download/verification, but require Owner approval or an enabled maintenance window for activation. A root-owned one-shot helper verifies again before pointer/migration changes.
- Before migration, stop Run Admission and public side effects, settle/checkpoint or suspend work, verify active plan compatibility and space, and create a complete verified pre-upgrade Recovery Set.
- Start the candidate without public traffic. Verify database/migration, Artifact/vault metadata, embedded editor/API, plan loading, and a synthetic no-side-effect execution before opening traffic.
- If pre-traffic activation fails, restore the known pre-upgrade Recovery Set and Previous automatically. After new traffic begins, rollback that could discard new data requires explicit recovery approval.
- Protect a Recovery Reserve from ordinary workload growth. State/retention cleanup yields before checkpoint, backup, staging, or rollback safety is endangered.
- A Recovery Set covers consistent SQLite state/migrations, every referenced immutable Artifact root, encrypted vault metadata, redacted configuration/instance identity, and exact release/plan/contract/profile/component identities.
- Verify every backup manifest. Run periodic full Restore Drills in an isolated non-executing environment with network triggers and side effects disabled.
- Local Recovery Sets are required for first runnable. Off-site encrypted deduplicating backup is optional until a destination is configured; the UI must state that local-only is not Disaster-Recovery Ready.
- Use Quarantine Mode for suspected ordinary corruption: block writes/admission/triggers/side effects, preserve suspect evidence, diagnose non-destructively, present verified recovery choices and estimated loss, and require Owner approval.
- Target recovery objectives: ordinary crash under one minute; failed pre-traffic update under five minutes; approved local restore under thirty minutes with approximately one-hour target RPO; host-loss rebuild under four hours with approximately 24-hour target RPO once off-site is active. Report these as targets dependent on infrastructure, not unconditional guarantees.
- GPU/accelerator policy exists in Advanced Settings with Off default and Safe Auto opt-in, but no GPU worker, CUDA/ROCm stack, or model weight is required in this release.

### 18. Deliberately preserved future seams

- Keep Node Form independent from Execution Lane. The first six are Rust Native on Inline Native, but contracts/locks/outcomes/Artifact handles can later support WASM, isolated process/language, MCP, and Heavy Orchestrator bindings.
- Keep optional component identities and locks representable in release/recovery manifests without installing their runtimes.
- Keep Agent Engine, Model Route, Memory Stack, Skill Set, MCP/Tool, Agent Policy, Output Contract, and Agent Blueprint domain identities available for later maps. Do not implement their runtime/UI in this slice.
- Keep Hub package and trust evidence concepts available without implementing discovery, installation, ratings, or update catalogs.
- Keep project/role/tenant security boundaries explicit while shipping only one Owner.

## Testing Decisions

### Testing philosophy

- Test externally visible behavior at the highest stable seam. Prefer one installed browser/API/process/cgroup journey over separate tests that know internal modules.
- Assert domain invariants, durable state transitions, observable output, protocol behavior, resource evidence, and recovery results—not private function call order, SQL statement shape, or renderer internals.
- Use lower-level tests only for pure deterministic compiler/engine state spaces, parser/canonicalizer security, cryptographic formats, and fault cases that would be prohibitively slow or ambiguous through the browser.
- Every resource/performance claim needs cgroup-enforced release-candidate evidence from the target VPS. Local developer timing is sanity evidence only.
- Every compatibility claim needs frozen-profile provenance and a field-level normalized diff. Popularity, package names, and successful import alone are not compatibility proof.

### Primary installed acceptance test

- Build a release Production Bundle from a locked dependency graph and install it into an empty target state using the native systemd path.
- Verify only the expected binary/runtime data, service definitions, configuration, release metadata, and state directories are installed; verify no toolchains or frontend source/build tree remain.
- Establish the Owner, Recovery Kit acknowledgement, private editor access, release identity, and health/readiness.
- Drive the Eco editor journey through the same versioned client contract used by the browser: template creation, node editing/connection/configuration/search/grouping, autosave acknowledgement, lease/reload/undo, compile diagnostics, publication, and immutable revision verification.
- Start the Run and verify durable admission. Capture progress and resource telemetry.
- Kill the daemon ungracefully after observed Speculative Progress is ahead of the latest Durable Checkpoint. Restart through systemd and verify bounded replay and no duplicate committed output.
- Wait for terminal success; assert exactly 100,000 logical Activations, frozen branch/total counts, expected algorithm-tagged digest, revision/plan identities, and trace/checkpoint/resource evidence.
- Inspect trace through public query surfaces, pin evidence, and roll back to the previous Published Revision.
- Export one machine-readable benchmark/evidence manifest containing environment, release, cgroup, metrics, correctness, restart, storage, and test hashes.

### Document-scale browser acceptance test

- Load the deterministic 100,000-Node-Instance fixture through production document/compiler/topology APIs.
- Assert all node/connection identities compile and packed snapshot validation succeeds.
- Exercise global search, direct jump, group navigation, selection, grouping, movement, semantic zoom, inspector lazy-load, and accessible list alternatives.
- Measure worker progress, main-thread responsiveness, memory, topology bytes, sparse overlays, Canvas draw counts, and bounded DOM/list counts.
- Compare DOM/list counts at smaller and 100,000-node fixtures to prove they remain viewport-bounded rather than graph-linear.
- Run on the declared evergreen browser matrix, with full interaction tests on primary browsers and smoke/accessibility tests on the remaining supported engines.

### Pure compiler and engine tests

- Use table/property tests for canonical compilation, stable plan/digest output, graph validation, port/cardinality errors, safe expression compilation, explicit cycles, compatibility status, resource/effect checks, and deterministic segment candidates.
- Re-run the same plan under varied completion interleavings and resource permits; assert the same Logical Order, branch counts, terminal state, and digest.
- Generate restart points before, during, and after checkpoint boundaries; assert bounded replay and invariant preservation.
- Cover queued, running, suspending, suspended, cancelling, cancelled, succeeded, failed, and uncertain-attempt transitions.
- Cover bounded queue saturation, weighted fairness, admission rejection, cancellation, unavailable cgroup controllers, disk pressure, and control-plane reserve.

### SQLite and Artifact fault tests

- Use real temporary SQLite databases with WAL/FULL, not a fake repository.
- Inject process abort, disk-full/nearly-full, write failure, busy/lock, interrupted backup, WAL growth/checkpoint, migration failure, corrupt pages, incompatible schema, and unsupported SQLite version/pragma conditions.
- Verify atomically committed checkpoint aggregates, read-back of critical pragmas, backup/restore consistency, and Quarantine Mode.
- Inject Artifact failures before sync, after sync/before rename, after rename/before SQLite commit, and after reference commit. Assert only safe orphans or valid references result.
- Test encrypted chunk boundaries, digest mismatch, truncation, wrong key/namespace, authorization denial, deduplication, garbage collection, and pinned retention.

### Node Contract and first-node conformance tests

- Validate the `v1alpha1` meta-schema, canonical serialization/digest, exact lock, unknown normative rejection, namespaced extension round trip, and migration preview.
- Run canonical vectors through Rust SDK types and at least one non-native adapter proof before declaring `v1`.
- For each first node, cover valid configuration, malformed configuration, port/type/cardinality errors, zero/one/many items, missing/null/type distinctions, Unicode/numeric boundaries, cancellation, budget exceeded, backpressure, Artifact spill, provenance, and deterministic replay.
- Verify no first node can access network, credentials, process execution, filesystem paths, ambient clock, or ambient randomness.
- Verify the exact 100,000-Activation equation and frozen digest independently from production implementation output.

### Compatibility tests

- Verify the reference runtime digest and environment before oracle collection.
- Test sanitizer positive/negative cases and secret scanners; reject any fixture containing credential identifiers/values, authorization material, personal endpoints, or unexpected opaque data.
- Test import/export preservation, exact external identity/typeVersion, native alias mapping, unknown safe field retention, unsupported behavior, safe expressions, Items/linking, branching/merge, and execution-order settings in the first supported subset.
- Compare canonical expected facts while preserving semantic ordering and type distinctions. Compare binary by byte digest/metadata and errors by original independent code/category rather than copied prose.
- Verify Compatibility Report classifications and that unresolved/delegated/unsafe features block publication when required.

### Security, accessibility, and packaging tests

- Test Owner bootstrap, password hashing parameter upgrade, session fixation/expiry/logout, CSRF/origin enforcement, private/public route separation, body/rate limits, sensitive-header/log redaction, and Artifact authorization.
- Test vault encryption envelopes, key rotation/read compatibility, missing/wrong system credential, Secret Lease scope/expiry, and Recovery Kit restore without persisting plaintext secrets.
- Run dependency/license/advisory/SBOM/provenance checks against the locked release.
- Test systemd sandbox/resource unit behavior and verify required exceptions. Test direct HTTPS and certificate reload.
- Run keyboard-only, screen-reader, focus, announcement, contrast, reduced-motion, and automated WCAG checks for all critical non-Canvas paths.
- Smoke the same signed binary under rootless/non-root Podman and Docker with read-only rootfs, one writable state volume, no engine socket, explicit resource limits, and matching backup/restore behavior.

### Update, backup, and recovery tests

- Test valid signed update, bad signature, corrupt digest, expired metadata, unauthorized downgrade, incompatible plan/schema, insufficient Recovery Reserve, missing Recovery Kit acknowledgement, and advisory policy failure.
- Test candidate migration/readiness failure before traffic; assert exact pre-upgrade Recovery Set restore and Previous-slot activation.
- Test that automatic rollback is refused after new production traffic/state unless explicit recovery is approved.
- Build complete local Recovery Sets, verify referenced Artifact roots and exact release/plan/contract versions, and run isolated Restore Drills without side effects.
- Test Quarantine Mode on suspected corruption and require explicit restore selection.
- When an off-site destination exists, test encrypted incremental backup, retention, restore to a clean host, Recovery Kit use, and Disaster-Recovery Ready status expiration when evidence becomes stale.

### Prior art

- The Eco execution-kernel prototype at its accepted commit proves 100,000 Activations, WAL/FULL grouped commits, cgroup operation, crash recovery, and deterministic digest feasibility; its timings and schema are not production contracts.
- The virtualized-editor prototype at its accepted commit proves 100,000-node packed topology, spatial search, grouping, and bounded Canvas/DOM techniques; its implementation is not merged into production.
- The stack and compatibility discovery notes provide primary-source constraints for Rust dependencies, SQLite durability, cgroup/systemd behavior, and clean-room n8n 2.39.0 fixtures.

## Out of Scope

- Full n8n built-in node-catalog parity or certification of arbitrary community packages.
- Production Node.js, Python, browser, local model, Hermes, OpenClaw, OpenCode, Claude Code, MiroFish, Antigravity, 9Router, or other external Agent Engine runtimes.
- AI Agent runtime, Agent Blueprint resolver/UI, Model Route implementation, Memory Stack, Skill execution, MCP client/server composition, or multi-agent councils.
- Workflow Hub, Skill Hub, package ratings, remote catalogs, one-click Internet package installation, or public marketplace operation.
- Gmail, GitHub, broad connector catalog, Connector Factory, external HTTP orchestration, OAuth providers, and production credential-bearing workflows.
- WASM Micro, Isolated Runtime, and Heavy Orchestrator execution implementations beyond the contracts/locks needed to preserve future seams.
- Scrape Orchestrator, crawling, browser automation, hosted scraping providers, or rewriting browser engines.
- Real-time multi-user collaboration, offline-first editing/CRDT merge, full mobile editing, organizations, billing, or public multi-tenant hosting.
- GPU execution, CUDA/ROCm packaging, local model weights, or automatic accelerator selection beyond storing/displaying the Off default policy seam.
- Fully automated ACME if a provisioned certificate or owner-managed proxy supplies HTTPS for the first installation.
- Mandatory off-site backup before Eco proof; local-only installations must remain visibly not Disaster-Recovery Ready.
- Public launch, commercial claims, trademark clearance, or dual licensing without later specialist legal and contribution review.
- Claims of superiority without repeatable scorecard evidence.

## Further Notes

- The Owner has explicitly confirmed the primary external acceptance seam and declared the Wayfinder route ready for this specification.
- The exact product brand name may be chosen separately; implementation must use a neutral original working identity and must not depend on n8n naming or assets.
- Prototype source remains on its prototype branches. Reuse decisions and behavior, not prototype code wholesale.
- The exact Rust type/module names, SQLite tables/indexes, endpoint paths, packed byte offsets, queue thresholds, checkpoint intervals, disk partition values, cryptographic record encoding, backup schedule/provider, and release-signing library are implementation-ticket decisions constrained by this spec and measured evidence.
- The frozen first compatibility baseline is n8n 2.39.0 even though release metadata identified it as a pre-release. Later profiles are additive and never mutate 2.39.0 evidence.
- The exposed SSH credential must be rotated outside this specification before relying on the host as a secure production environment.
- The next workflow step is `/to-tickets`; production implementation must not begin as one big task directly from this document.
