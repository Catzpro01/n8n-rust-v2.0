---
status: accepted
---
# Separate the deterministic engine from transactional storage

## Context

The first production journey must compile and execute 100,000 deterministic logical Activations, retain per-node Causal Trace evidence, recover after an ungraceful daemon restart, stream large values as Artifacts, and remain usable inside 500 MiB RAM, sustained 0.5 CPU, and 10 GiB managed disk. The Eco kernel prototype demonstrated that SQLite FULL checkpoint frequency dominates this lightweight workload and that bounded grouped commits recover deterministically with low RSS.

The implementation also needs room for remote-first workers, multiple Execution Lanes, durable suspension, retries, and later storage evolution without creating speculative microservices, generic repositories, or a task/actor for every Node Instance.

## Decision

### One modular Rust daemon

Build a modular monolith with explicit in-process boundaries for domain types, compiler, deterministic Run engine, scheduler/Resource Governor, SQLite storage, filesystem Artifact storage, HTTP/API, and observability. Keep these as modules in the first implementation and extract crates or services only when measured isolation, reuse, or deployment requirements justify it.

Do not expose database rows, SQL connections, filesystem paths, Tokio tasks, or transport-specific types as domain contracts. Do not introduce an ORM, a generic multi-database abstraction, event sourcing for every domain object, or interfaces for future systems before an implemented use case needs them.

### Pure deterministic compiler

The compiler accepts one immutable Workflow Revision, locked Node Contracts, a Compatibility Profile, and explicit compilation policy. It returns either a canonical immutable Execution Plan or structured diagnostics. Compilation performs no HTTP, SQLite, filesystem, clock, random, or executor I/O.

Drafts may use incremental compilation for feedback, but publication always runs a complete deterministic compile. A Published Revision records the revision digest, Execution Plan hash and format, compiler ABI/version, locked Node Contract identities, and Compatibility Profile. A Run pins exactly one Execution Plan for its lifetime and is never silently recompiled after daemon upgrade or resume. An update must prove it can read every active/suspended plan or defer the update/migrate through an explicit reviewed path.

### Deterministic engine and governed scheduler

Represent Run semantics as a deterministic state machine. It consumes the pinned Execution Plan, durable resume state, commands, timer/trigger events, and typed Activation outcomes; it produces decisions such as ready Activation attempts, Durable Checkpoint candidates, suspension, failure, cancellation, completion, and Causal Trace facts. It does not execute SQL, network calls, or Artifact filesystem operations.

The scheduler is a separate policy layer. It applies Resource Governor permits, bounded weighted-fair Run selection, lane limits, deadlines, backpressure, and Execution Segment optimizations. Independent work may complete concurrently, but Logical Order, visible outputs, trace semantics, and correctness digest remain deterministic across CPU profiles. Ordering-sensitive side effects are serialized as required by their Node Contract.

Never allocate one operating-system thread, Tokio task, or actor per Node Instance/Activation. Ready work, executor results, storage commands, public ingress, and event subscribers all use count- and byte-bounded queues. A lagging status client receives an explicit gap/resync instruction rather than blocking Run progress.

### Durable admission and checkpoint boundary

A trigger becomes a Run only after a Run Admission transaction durably creates a Queued Run tied to its immutable revision and plan. Already admitted Runs are never discarded because memory queues fill. Once admission limits are reached, new external ingress is rejected before acceptance with a retryable overload response; internal sources retain their source-specific durable missed/delayed state.

A dedicated SQLite writer thread owns all write connections. A very small bounded set of read-only connections serves snapshots/queries. Critical WAL, FULL synchronous, foreign-key, timeout, limit, version, and compile-option settings are set and read back at startup.

The principal write interface is a typed `commit_checkpoint`-style operation, not arbitrary repository calls. In one SQLite transaction it advances durable Run state and resume cursor, Logical Order, digest/counters, Activation outcomes and provenance, retry/failure/cancellation state, Artifact references, and retained logical Causal Trace. Batching is bounded by item count, bytes, elapsed time, progress latency, and maximum replay work chosen by the Resource Governor; the prototype batch size is evidence, not a constant.

The UI distinguishes Speculative Progress from the latest Durable Checkpoint. After interruption, work after that checkpoint may be deterministically replayed. Only committed progress is promised to survive a daemon or host crash.

Cancellation is a durable cooperative request: stop admitting new Activations, ask in-flight work to stop, checkpoint settled facts, and record work or external effects that cannot be cancelled. An unhandled Activation failure fails the Run and stops new admission unless an explicit Node Contract/workflow failure route or continue-on-error policy says otherwise.

### Side-effect attempt boundary

For side-effecting work, durably record attempt identity, policy, idempotency/reconciliation material, and intent before dispatch when the Node Contract requires it. Record the returned outcome in a later durable transaction. A crash or timeout between external acceptance and durable outcome can become an Uncertain Outcome.

Automatically retry only when the Node Contract proves the operation safe through idempotency, a stable idempotency key, or an explicit reconciliation protocol. Otherwise preserve evidence and require a visible owner/policy decision. Do not claim general exactly-once delivery across external systems.

The first runnable vertical slice implements deterministic Native Nodes and the same attempt/outcome seam; it does not need to implement a speculative remote-worker wire protocol.

### Envelope and Artifact boundary

An Envelope contains bounded typed values, metadata, logical provenance, and Artifact references. Large JSON, binary, streams, and retained bulk outputs spill to Artifact storage rather than being copied through scheduler queues or held fully in RAM. Inline/spill thresholds are governed implementation policy and must not change logical output or digest semantics.

Artifact storage is content-addressed, streamed, and envelope-encrypted within an Artifact Namespace. The initial private deployment deduplicates within the Owner trust domain; future project/tenant trust domains use separate namespace/key boundaries. Credentials and Secret Leases are never Artifacts.

Artifact creation follows a one-way crash-safe protocol:

1. stream plaintext through size limits and digest calculation into encrypted staging;
2. flush and durably synchronize the staged object;
3. atomically place the immutable object in its content-addressed namespace;
4. only then allow a SQLite transaction to create a live reference.

A failed database transaction can therefore leave only an unreferenced orphan, which startup/retention sweeps may remove after a safety age. A committed database reference must never point at an object that was not made durable first. Authorization is checked on every Artifact reference/read; storage paths are never API identities.

### Trace, retention, and resource pressure

Store logical per-Node Instance Causal Trace facts in the same checkpoint transaction as the state they explain. Store Execution Segment and physical resource metrics as an additional layer rather than replacing logical observability. Segment fusion must preserve each Node Instance's visible output, error, retry, provenance, and logical timing facts.

A tiered Retention Profile keeps full evidence for active and recent Runs, permits Evidence Pins, and compacts/expires eligible terminal evidence under age and quota policy. Compaction retains plan/revision identity, terminal state, correctness/output digest, checkpoint/hash-chain evidence, counts, failures/retries, and required Artifact roots. It never removes pinned evidence or data required by active/suspended Runs.

Under CPU, memory, I/O, queue, or disk pressure, first reduce governed concurrency/cache/batch choices, apply streaming backpressure, and spill bounded values. If a hard limit approaches, durably suspend resumable work and stop new admission rather than risk OOM, corrupt storage, or silently delete evidence. The editor, status, health, cancellation, and recovery control plane retains a reserved small budget. When pressure clears, the governor increases safe parallelism again.

### External application seam and testing

HTTP handlers invoke typed use-case commands/queries and never manipulate tables. Starting a Run returns success only after durable admission. Status reports durable and speculative progress separately. Cancellation is a durable command. Artifact content is streamed only after authorization. Reconnectable events are bounded views of state, not an alternate source of commit truth.

Use narrow test ports only at actual nondeterministic/side-effect boundaries: clock/ID source, Activation executor/Execution Lane, Artifact device adapter, and outer transport as needed. Exercise SQLite through temporary real databases and injected crash/disk/lock faults rather than building a fake generic repository. The pure compiler and engine receive exhaustive fixture, replay, property, and determinism tests.

Startup recovery loads nonterminal Runs with their pinned plans and latest checkpoints, verifies supported formats/digests and required Artifacts, sweeps aged staging orphans, classifies incomplete side-effect attempts, restores timers/admission, and resumes with bounded replay.

## Consequences

The first implementation can be deep end-to-end—compile, schedule, execute six deterministic Native Nodes, group FULL checkpoints, crash/recover, inspect logical trace, and verify the same digest—without prebuilding every worker, connector, Hub, or database backend. A single typed checkpoint aggregate and strict Artifact ordering concentrate durability invariants where they can be tested.

The architecture deliberately accepts bounded replay to avoid a FULL fsync per Activation, connected queue limits instead of unlimited acceptance, and honest Uncertain Outcomes instead of an impossible universal exactly-once promise. Additional cores improve safe throughput, but never redefine workflow semantics.
