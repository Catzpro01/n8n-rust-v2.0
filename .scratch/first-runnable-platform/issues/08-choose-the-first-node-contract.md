# Choose the first Node Contract

Type: grilling
Status: resolved
Blocked by: 05, 07

## Question

What is the smallest versioned Node Contract and Apache-licensed SDK interface that supports the first native nodes, capabilities, resource budgets, side-effect semantics, typed ports, and future compatibility adapters without exposing engine internals?


## Answer

Publish the minimal behavior-first contract, host boundary, six-node proof set, and staged ecosystem test policy in [ADR-0054](../../../docs/adr/0054-publish-a-minimal-node-contract-and-staged-conformance-matrix.md).

The canonical `v1alpha1` JSON Node Contract locks identity/version/digest and declares Configuration Schema, gradual Port Schemas, one of four bounded Activation Shapes, Effect Class and replay facts, requested capabilities, Resource Budgets, typed Node Outcomes, and explicit compatibility mappings. Unknown normative fields fail; namespaced extensions round-trip without authority. Promote to `v1` only after Eco, malformed/budget/backpressure/cancellation, Artifact, compatibility, and cross-SDK vectors pass.

Node Contract behavior is separate from Rust/WASM/JavaScript/Python/MCP/remote Node Implementations and from Execution Lane choice. Implementations receive only a capability-limited Activation Context—validated input/config, backpressured port output, Artifact streams, cancellation/deadline, controlled clock/randomness, safe trace events, and approved grants/Secret Leases—never SQLite, scheduler, vault, filesystem paths, or engine internals. The specification, fixtures, and Rust SDK are Apache-2.0; third-party implementations retain their own licenses.

The first proof set is Manual Trigger, Generate Items, Edit Fields, If, Merge, and Summarize/Output Digest. These six Pure deterministic nodes prove the exact 100,000-Activation vertical journey but are not a catalog limit.

At the owner's explicit request, compatibility expansion includes a staged Catalog Conformance Matrix for every inventoried n8n 2.39.0 built-in node/typeVersion and every installed or curated community-package version. Tests use many minimal per-node/operation workflows and representative real workflows—not one unsafe mega-workflow—and cover import round trip, compile/reporting, mocked behavior, owner-authored black-box differential behavior, opt-in disposable live accounts, and resilience/failure cases. Community packages are separately installed, scanned, isolated, and may receive Certified Compatible only for an exact tested version/digest; untested packages remain visibly Unverified. Full catalog coverage proceeds after the first runnable rather than reverting to a big-bang milestone.

The owner accepted all recommended contract policies and chose the staged certified matrix on 2026-09-11 after confirming that the six nodes are only the first milestone and that broad built-in/community compatibility remains the final target.
