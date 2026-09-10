# 03: Create the first Node Contract and durable Draft

**What to build:** The Owner can create a Workflow, add and configure a Manual Trigger from the native catalog, save it through semantic Draft Commands, reload it from SQLite, and inspect its exact versioned Node Contract.

**Blocked by:** 02: Establish the Owner and recovery root

**Status:** ready-for-agent

- [ ] An Apache-2.0 `v1alpha1` Node Contract meta-schema, canonical JSON/digest rules, conformance fixture format, and Rust SDK surface are published.
- [ ] The Manual Trigger contract declares identity, Configuration Schema, ports, Source Activation Shape, Pure effect, no capabilities, Resource Budget, typed outcomes, and compatibility metadata.
- [ ] Unknown normative contract fields fail; namespaced non-authoritative extensions round-trip; exact contract version/digest becomes a Node Contract Lock.
- [ ] The editor catalog renders the Manual Trigger using declarative original metadata and adds it to a new Workflow without node-supplied UI code.
- [ ] Semantic Draft Commands carry idempotent command identity and base Draft Version; accepted commands return new version, delta, affected identities, and diagnostics.
- [ ] Workflow, Node Instance, Connection-ready identity, layout, annotation, settings, and safe compatibility metadata persist through a restart and reload.
- [ ] Transient viewport, selection, panels, and search query do not change the Draft Version.
- [ ] The API and editor never expose SQL rows, storage paths, engine handles, or vault internals.
- [ ] External tests create, edit, reload, and validate the one-node Draft only through the browser/client contract.
