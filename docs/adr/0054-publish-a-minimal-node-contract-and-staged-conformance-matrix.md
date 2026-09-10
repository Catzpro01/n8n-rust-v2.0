---
status: accepted
---
# Publish a minimal Node Contract and staged conformance matrix

## Context

The first runnable journey needs six deterministic Native Nodes, typed ports, safe expressions, bounded streaming, compile-time validation, per-node trace evidence, and exact replay. The same foundation must later support eight Node Forms, four Execution Lanes, n8n 2.39.0 imports, user-supplied community nodes, MCP, Connector Factory output, AI-generated nodes, and progressive Rust promotion without exposing engine internals or forcing every runtime into the resident daemon.

A contract that describes only the first six implementations would create a dead end. A contract that attempts to model every future runtime API would become speculative. The first public contract therefore describes observable node behavior, authority, and budgets while keeping executable bindings and engine/storage mechanics separate.

## Decision

### Versioned canonical contract

Publish the Node Contract schemas, canonical conformance fixtures, and Rust SDK under Apache-2.0. Begin implementation under an explicitly unstable `v1alpha1` contract namespace. Promote it to `v1` only after the Eco vertical slice, malformed/budget/cancellation tests, canonical round trips, and at least one non-native binding prove the semantics.

The canonical lock form is deterministic JSON validated by a published meta-schema and hashed with an algorithm-tagged digest. Human authoring tools may generate it, but Workflow Revisions pin the exact canonical identity, semantic version, and digest through a Node Contract Lock. Compatible-looking updates do not float into Published Revisions; every update requires validation, migration preview, permission/resource/behavior diff, and publication of a new revision.

Unknown normative fields fail validation rather than being ignored. Forward-compatible metadata is permitted only in a namespaced extension bag that round-trips without gaining authority. Display metadata is non-semantic and uses original product assets/copy or independently licensed service marks.

### Minimal required sections

A `v1alpha1` Node Contract contains only the following behavioral sections:

1. **identity** — namespace, stable name, semantic version, contract/API version, digest inputs, publisher/license references;
2. **configuration** — declarative Configuration Schema, defaults, expression eligibility, validation, secret-reference fields, and non-executable editor hints;
3. **ports** — stable named input/output identities, Port Schemas, cardinality, required/optional rules, and output multiplicity;
4. **activation** — Activation Shape, ordering/streaming rules, and bounded batch/barrier requirements;
5. **effects** — Effect Class plus determinism, idempotency, retry, reconciliation, compensation, and approval facts;
6. **capabilities** — requested network, credential-field, Artifact, time/random, process, model/tool, or other named host facilities;
7. **resources** — default and hard Resource Budgets for wall/CPU time, memory, input/output count and bytes, Artifact bytes, and concurrency;
8. **outcomes** — allowed typed Node Outcomes and stable original error-code namespace;
9. **compatibility** — external identities/version mappings and a safe opaque preservation area, without claiming equivalence by name alone.

Configuration and data schemas use a documented interoperable JSON Schema subset plus separate declarative UI hints. New workflows use precise Port Schemas where known; an explicit dynamic JSON Item schema preserves the n8n item facade when static shape is unavailable. Missing versus null, scalar type, array order, output port, binary Artifact metadata, and item-link provenance remain semantically significant.

### Activation shapes and backpressure

Support only four initial Activation Shapes:

- **Source** — originates captured/deterministic input;
- **Per-Item Stream** — consumes one Envelope at a time and may emit bounded zero, one, or many outputs;
- **Bounded Batch** — consumes an explicitly limited collection;
- **Barrier/Reducer** — waits for declared inputs/completion and emits merged or reduced output.

This is enough for triggers, transforms, routing, merge, and aggregation without materializing every item in RAM. Compatibility implementations that require full item collections may use Bounded Batch only under explicit materialization/spill limits. The output emitter is backpressured and enforces port, item, byte, Artifact, and cancellation limits.

### Contract versus implementation

A Node Contract states behavior; a Node Implementation is a separately identified executable binding that claims the contract and passes its conformance fixtures. Rust Native, WASM, JavaScript/Python worker, MCP, remote service, or generated implementations may satisfy the same contract. Node Form remains an authoring choice, and the compiler independently selects a valid Execution Lane.

Do not promise a stable Rust binary ABI. The stable interoperability surface is the versioned schema, logical activation message model, Artifact/secret handle semantics, typed outcomes, and canonical fixtures. The Rust SDK provides semver-controlled source-level types/builders; later WASM, TypeScript, and Python bindings are generated or implemented against the same public model.

Promoted in-daemon Native Nodes may use static dispatch for performance but obey the same logical Activation Context and fixtures. Custom native code first runs isolated according to the existing promotion policy.

### Minimal Activation Context

A Node Implementation receives no database, scheduler, vault, filesystem path, HTTP router, process-global state, or internal engine handle. The capability-limited Activation Context exposes only:

- validated immutable configuration and input reader;
- backpressured output emitter by stable port identity;
- Artifact readers/writers and typed references;
- cancellation and deadline observation;
- logical clock and seeded deterministic randomness;
- safe structured trace/event emission subject to redaction and limits;
- approved Capability Grant and short-lived Secret Lease handles;
- attempt, Run, Node Instance, and provenance identities required by the contract.

Ambient system time, randomness, environment variables, network, filesystem, thread/process spawning, and credential access are unavailable unless declared and granted through a suitable Execution Lane. Real nondeterministic sources become explicit capability/effect facts and their observations enter Causal Trace.

### Effects, budgets, and outcomes

Classify effects as Pure, External Read, External Write, or Orchestration. A contract must state determinism, idempotency/key strategy, retryable conditions, ambiguous-result reconciliation, optional compensation, and approval requirements. The first six contracts are Pure and deterministically replayable.

A Workflow may lower a Resource Budget freely. Raising network/credential scope, side-effect authority, memory, CPU, wall time, output, Artifact, or concurrency limits requires a visible reviewed grant and can force a safer Execution Lane. Inline limits are accounting/admission promises for reviewed Native Nodes; isolated lanes additionally enforce limits through process/cgroup/runtime mechanisms.

Node Outcomes are structured Success-by-port, Retryable Failure, Permanent Failure, Durable Suspension, Uncertain Outcome, and Cancelled results with stable codes and safe evidence. Panics, arbitrary process exits, and free-form strings are adapter inputs to classify, never the public outcome contract.

### First proof set, not final catalog

Implement these six original Native Node Definitions first:

1. **Manual Trigger** — Source using captured invocation input;
2. **Generate Items** — deterministic bounded expansion;
3. **Edit Fields** — safe fixed/expression projection and mutation;
4. **If** — deterministic typed two-port routing;
5. **Merge** — declared multi-input Barrier behavior for the first compatible modes;
6. **Summarize / Output Digest** — deterministic reduction, counters, and digest.

Together they exercise source, expansion, transform, expressions, branching, fan-in, reduction, multiple ports, item linking, bounded streaming, checkpointing, trace, and correctness hashing. The fixture is calibrated to exactly 100,000 logical Activations; the six-node count is only the first vertical milestone and never a catalog limit.

### Staged full-catalog conformance

Maintain a Catalog Conformance Matrix for the frozen n8n 2.39.0 profile and every installed or curated community-package version. Build the baseline inventory from allowed public documentation and black-box reference-UI/export observations, never upstream implementation source, tests, Enterprise files, internal `/rest` reverse-engineering, or copied assets.

For every inventoried built-in node identity and observed `typeVersion`, record import/export preservation, configuration and operation-family coverage, execution status, required capabilities/credentials, fixture hashes, and one of Native equivalent, Delegated compatible, Preserved opaque, Adapted with declared difference, Unsupported, or Rejected unsafe. Full catalog parity is a staged milestone, not a condition that blocks the first runnable six-node journey.

A single workflow containing every node is not a valid oracle. Use a corpus of minimal node/operation workflows plus representative end-to-end compositions:

1. catalog/schema/package load and canonical import round trip;
2. compile, Compatibility Report, permission, and budget diagnostics;
3. deterministic mocked/local-service behavior cases;
4. owner-authored black-box differential cases against pinned n8n 2.39.0;
5. opt-in live service tests using disposable/sandbox accounts and no committed secrets;
6. timeout, rate-limit, retry, cancellation, crash/restart, malformed data, and uncertain-outcome cases.

Community Node Packages are always user-supplied or owner-curated, separately installed under their own licenses, scanned and permission-reviewed, and executed first in an isolated compatibility worker. Grant **Certified Compatible** only to an exact package version/digest after its required matrix tests pass. Unknown packages may be sandboxed as Unverified; they never receive compatibility, trust, or production authority merely from popularity or successful installation. Packages that depend on unsupported private n8n internals remain explicitly unsupported or require an independently specified adapter.

## Consequences

The first Node Contract is small enough to implement deeply but already describes authority, effects, streaming, retries, and portability rather than Rust internals. Native code keeps a static fast path while compatibility workers and future SDKs share fixture-backed semantics.

Exact pinning and staged certification add migration and test work, but prevent silent node drift and false promises that arbitrary community code cannot fail. All known n8n 2.39.0 built-ins and every installed/curated community package receive visible matrix status over time, while the project retains runnable vertical milestones instead of waiting for an unbounded ecosystem before shipping evidence.
