# Workflow Automation

This context describes the language used to define, execute, and extend personal automation workflows at any practical graph size.

## Language

**Workflow**:
A user-authored graph of Node Instances and Connections that expresses an automation.
_Avoid_: Flow, pipeline, automation graph

**Workflow Revision**:
An immutable snapshot of a Workflow used as the sole definition for a Run; it includes authored structural layout but excludes transient Editor Session navigation state.
_Avoid_: Workflow version, saved workflow

**Node Definition**:
A reusable contract describing a node's configuration, ports, capabilities, and execution behavior.
_Avoid_: Plugin, node type, component

**Node Instance**:
A configured occurrence of a Node Definition inside a Workflow Revision.
_Avoid_: Node when the distinction from Node Definition matters

**Connection**:
A directed route from an output port of one Node Instance to an input port of another.
_Avoid_: Edge, wire

**Run**:
One durable execution of exactly one Workflow Revision through exactly one pinned Execution Plan.
_Avoid_: Execution, job

**Run Admission**:
The budgeted durable acceptance of a trigger as a Queued Run; a trigger rejected before admission is not a Run.
_Avoid_: Enqueue when durability has not been established

**Logical Order**:
The deterministic ordering of Activation outcomes and Causal Trace evidence independent of concurrent completion timing.
_Avoid_: Wall-clock order

**Activation**:
One scheduled attempt to process a Node Instance within a Run for a particular input envelope.
_Avoid_: Node execution, task

**Envelope**:
The bounded metadata and references passed between Activations; large values are represented by Artifact references rather than embedded bytes.
_Avoid_: Item when referring to the internal runtime representation

**Artifact**:
Content-addressed run data stored outside an Envelope and read as a stream.
_Avoid_: Binary data, blob, payload file

**Artifact Namespace**:
The authorization, encryption-key, and deduplication boundary within which equal Artifact content may share storage.
_Avoid_: Bucket when referring to the security boundary

**Native Node**:
A Node Definition executed by the Rust runtime without a language compatibility layer.
_Avoid_: Built-in node

**Compatibility Node**:
A Node Definition executed by an isolated compatibility worker to preserve behavior of an existing n8n node.
_Avoid_: Legacy node, JavaScript node

**Compensation Workflow**:
A Workflow invoked to counteract completed side effects after a Run can no longer proceed safely.
_Avoid_: Rollback workflow

**Execution Plan**:
The validated, immutable scheduling representation compiled from one Workflow Revision and pinned unchanged for the life of each Run.
_Avoid_: Compiled workflow, runtime graph

**Compatibility Report**:
A pre-activation account of which imported behavior is supported, adapted, blocked, or delegated to Compatibility Nodes.
_Avoid_: Import warnings

**Remediation Patch**:
A reversible proposed change that includes evidence, confidence, expected effects, and a rollback path.
_Avoid_: Auto-fix when approval is still required

**Node Form**:
One of the eight user-facing ways to define or obtain a Node Definition: JavaScript Script, Python Script, WASM Component, Rust Native, External Process, AI-Generated Node, Sub-workflow Node, or MCP Tool Node.
_Avoid_: Security tier, runtime tier

**Execution Lane**:
The resource and isolation class selected for an Activation independently of how its Node Definition was authored.
_Avoid_: Runtime language, node tier

**Inline Native Lane**:
The lowest-overhead Execution Lane for trusted built-in behavior that requires no external runtime.
_Avoid_: Simple mode

**WASM Micro Lane**:
A short-lived capability-limited Execution Lane for portable custom computation.
_Avoid_: Embedded plugin

**Isolated Runtime Lane**:
An on-demand Execution Lane for language runtimes and arbitrary processes that must not share the daemon's trust domain.
_Avoid_: Compatibility mode

**Heavy Orchestrator Lane**:
A separately budgeted Execution Lane for browsers, scrapers, local models, and other resource-intensive systems.
_Avoid_: Heavy node

**Node Contract**:
The versioned reviewable declaration of a Node Definition's inputs, outputs, configuration, capabilities, side effects, determinism, idempotency, and resource budget.
_Avoid_: Plugin manifest, node schema

**Node Contract Lock**:
The exact Node Contract identity, version, and digest pinned by a Workflow Revision so its behavior cannot change silently.
_Avoid_: Version range, latest node

**Port Schema**:
The declared shape and cardinality accepted or emitted by a named Node Contract port, including an explicit dynamic-item form when static typing is not possible.
_Avoid_: Rust type, UI field type

**Activation Shape**:
The Node Contract declaration of whether work originates data, processes one Envelope, consumes a bounded batch, or waits at a barrier/reducer.
_Avoid_: Execution Lane, runtime language

**Node Outcome**:
A typed Activation result such as Success, Retryable Failure, Permanent Failure, Durable Suspension, Uncertain Outcome, or Cancelled.
_Avoid_: Free-form error, process exit code

**Effect Class**:
The Node Contract classification of externally visible behavior as Pure, External Read, External Write, or Orchestration, together with its retry and reconciliation facts.
_Avoid_: HTTP method, permission level

**Activation Context**:
The capability-limited host surface through which a Node Implementation reads input, emits output, streams Artifacts, observes cancellation/deadline, and uses granted time, randomness, tracing, or Secret Leases.
_Avoid_: Engine handle, application context

**Compatibility Mapping**:
An explicit fixture-backed translation between an external node identity/configuration and a locked Node Contract or delegated compatibility implementation.
_Avoid_: Alias when behavior has not been proven

**Node Implementation**:
A separately versioned executable binding that claims one Node Contract and must pass its conformance fixtures in an eligible Execution Lane.
_Avoid_: Node Contract, Node Form

**Community Node Package**:
A user-supplied or owner-curated separately installed distribution of external Node Implementations governed by its own license and trust evidence.
_Avoid_: Built-in dependency, automatically trusted plugin

**Catalog Conformance Matrix**:
The Compatibility-Profile and package-version-pinned evidence inventory for node import, configuration, execution, permissions, failures, and compatibility status.
_Avoid_: Supported nodes list without fixture evidence

**Certified Compatible**:
A status granted only to an exact external node or package version and digest after its required conformance evidence passes.
_Avoid_: Compatible by package name or popularity

**Configuration Schema**:
The declarative parameters, validation rules, defaults, and editor hints for configuring a Node Instance without executing node-supplied UI code.
_Avoid_: Custom settings component

**Resource Budget**:
The declared default and hard bounds for an Activation's time, CPU, memory, output, Artifact, and concurrency consumption.
_Avoid_: Resource request when referring to enforced limits

**Capability Grant**:
An explicit, least-privilege authorization for one Node Instance to use a named host facility or credential scope during an Activation.
_Avoid_: Permission when referring to runtime authority

**Secret Lease**:
A short-lived, scoped delivery of selected credential fields to one authorized Activation.
_Avoid_: Mounted secret, environment secret

**Promotion**:
The reviewed transition of a proven custom Native Node from an isolated worker into the trusted built-in set.
_Avoid_: Installation, enabling

**Public Gateway**:
The internet-facing ingress that exposes only explicitly published webhook, form, OAuth callback, MCP, and health routes.
_Avoid_: Public server, public UI

**Compatibility Profile**:
A versioned statement of supported behavior against one frozen n8n release, backed by compatibility tests and migration rules.
_Avoid_: n8n version, compatibility mode

**Agent Engine**:
The selected reasoning and action runtime for one AI Agent turn, whether built in or reached through a model, MCP, A2A, CLI, or process adapter.
_Avoid_: Model when the runtime performs more than inference

**Memory Layer**:
One independently governed source and sink of retained agent context with a defined scope, retrieval policy, write policy, provenance, and budget.
_Avoid_: Memory provider

**Memory Stack**:
An ordered set of Memory Layers consulted under one retrieval and token policy.
_Avoid_: Combined memory

**Skill Set**:
A version-locked collection of instructions and resources granted to an AI Agent under a declared scope and capability policy.
_Avoid_: Prompt pack, skill folder

**Output Contract**:
The typed, side-effect-free shape an AI Agent result must satisfy before it can enter the main workflow.
_Avoid_: Output parser

**Agent Policy**:
The enforceable limits and approval rules governing an AI Agent's models, tools, memory, costs, recursion, and side effects.
_Avoid_: System prompt, guardrail prompt

**Mutable Draft**:
The editable, autosaved working form of a Workflow that cannot be used for a production Run.
_Avoid_: Development workflow

**Draft Version**:
The monotonic identity of a Mutable Draft state after an accepted Draft Command; unlike a Workflow Revision, it is not publishable or immutable.
_Avoid_: Draft Revision, autosave Revision

**Draft Command**:
A durable, ordered intent to change a Mutable Draft, accepted only against its declared base Draft Version.
_Avoid_: Patch, frontend event

**Editor Session**:
One connected browser context that can observe a Mutable Draft and may hold its Draft Lease.
_Avoid_: Tab, frontend client

**Draft Lease**:
Time-bounded exclusive authority for one Editor Session to change a Mutable Draft; other Editor Sessions can observe, request takeover, or fork.
_Avoid_: File lock, workflow lock

**Recovery Copy**:
Best-effort Draft changes retained by an Editor Session but not yet accepted by the authoritative daemon; it must be reconciled before publish.
_Avoid_: Offline Draft, local Workflow

**Published Revision**:
A signed Workflow Revision approved for production Runs under one Compatibility Profile.
_Avoid_: Active workflow

**Causal Trace**:
The ordered evidence connecting triggers, Activations, data references, decisions, retries, resource use, and errors within a Run.
_Avoid_: Execution log

**Durable Checkpoint**:
An atomically persisted boundary from which a Run can recover with bounded deterministic replay after interruption.
_Avoid_: Progress update, autosave

**Speculative Progress**:
Completed Run work after the latest Durable Checkpoint that may be replayed if interruption occurs before it is committed.
_Avoid_: Durable progress

**Uncertain Outcome**:
The recorded state of a side-effecting Activation when an external system may have acted but success or failure cannot be proven safely.
_Avoid_: Failure when the external effect may have succeeded

**Cancellation Request**:
A durable intent to stop admitting new Activations and cooperatively stop in-flight work without claiming to reverse completed or uncertain external effects.
_Avoid_: Kill when work may have escaped the process

**Retention Profile**:
The policy that selects full, compacted, expired, or pinned Causal Trace and Artifact evidence under a storage budget.
_Avoid_: Log level

**Evidence Pin**:
An explicit hold that prevents selected Causal Trace or Artifact evidence from being compacted or expired.
_Avoid_: Favorite, archive

**Durable Suspension**:
A persisted Run state that consumes no active execution slot while awaiting time, callback, event, or approval.
_Avoid_: Sleeping task, waiting thread

**HTTP Adapter**:
A selectable implementation of an outbound request path, such as native HTTP, a compatibility client, a proxy provider, or browser-backed fetch.
_Avoid_: HTTP engine, scraper when no extraction is involved

**HTTP Orchestrator**:
A policy-governed Node Definition that selects and may safely fall back among HTTP Adapters while preserving request identity, idempotency, cost, and audit evidence.
_Avoid_: Smart HTTP Request

**Connector Definition**:
A versioned collection of Node Definitions and credential contracts generated from or maintained against an external system interface.
_Avoid_: Integration package

**Crawl Plan**:
The approved scope, frontier rules, budgets, compliance policy, extraction contract, and adapter policy for one scraping Run.
_Avoid_: Scrape job, crawler config

**URL Frontier**:
The durable, deduplicated set of discovered request candidates and their crawl relationships within a Crawl Plan.
_Avoid_: URL queue

**Scrape Adapter**:
A local or remote implementation capable of fetching, rendering, crawling, or extracting web content under a Crawl Plan.
_Avoid_: Scraper when referring to the selectable implementation

**Scrape Result Bundle**:
The typed result for one fetched resource, combining selected Artifacts, normalized content, extracted records, links, provenance, and confidence.
_Avoid_: Scraped page

**Workflow Package**:
A declarative, versioned distribution containing workflow definitions, requirements, evidence, and provenance but no secret values or implicitly executable installer.
_Avoid_: Template when the distribution includes more than a workflow graph

**Skill Package**:
A versioned distribution of a Skill Set with source lock, scope, capability requirements, tests, license, evidence, and provenance.
_Avoid_: Prompt download, unpinned skill folder

**Workflow Hub**:
The built-in discovery, review, installation, and update experience for Workflow Packages indexed from Hub Sources.
_Avoid_: Unreviewed template gallery

**Skill Hub**:
The built-in discovery, review, installation, and update experience for Skill Packages indexed from Hub Sources.
_Avoid_: Automatic prompt installer

**Hub Source**:
A configured catalog or repository origin whose Workflow Package and Skill Package metadata is indexed locally.
_Avoid_: Marketplace server

**Trust Evidence**:
Verifiable facts about a Workflow Package's identity, provenance, dependencies, tests, capabilities, maintenance, and sandbox results.
_Avoid_: Trust score when referring to the underlying facts

**Rust Promotion Track**:
The measured migration path by which a remote or isolated adapter gains a behavior-compatible Native Node implementation and becomes the preferred execution path.
_Avoid_: Rewrite when only the orchestration adapter changes language

**Execution Segment**:
A compiled group of compatible logical Activations that can execute together while preserving each Node Instance's observable output, error, and trace semantics.
_Avoid_: Fused node, physical node

**Resource Governor**:
The policy module that observes effective CPU, memory, disk, I/O, and pressure limits and continuously adjusts admission, concurrency, batching, caching, spill, and remote placement.
_Avoid_: Autoscaler when no new machine is being created

**Resource Profile**:
A named target envelope for CPU, memory, disk, latency, and throughput against which a deployment and benchmark are evaluated.
_Avoid_: Server size
