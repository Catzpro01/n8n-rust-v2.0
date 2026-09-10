# Node Contract and durable Draft API

The Apache-2.0 `v1alpha1` Node Contract schema, fixture format, fixtures, and Rust SDK live under `sdk/node-contract/`, `contracts/`, and `crates/node-contract/`. Canonical JSON sorts object keys recursively, preserves array/scalar distinctions, emits compact UTF-8, and uses `sha256:<lowercase-hex>` locks. Unknown top-level normative fields fail; only namespaced extension-bag fields are non-authoritative and preserved.

Authenticated Owners use `GET /api/v1/catalog` and `GET /api/v1/node-contracts/{namespace}/{name}/{version}`. The first contract is the Pure Source-shaped `canopy.native/manual-trigger@0.1.0`, with no capabilities and bounded resources.

Create a Mutable Draft with `POST /api/v1/workflows`, then submit semantic commands to `POST /api/v1/workflows/{id}/draft-commands`. Every command supplies a unique `command_id` and exact `base_draft_version`. Repeating an accepted command returns its original acknowledgement; a different command with a stale base returns `stale_draft_version` and the authoritative version. Acknowledgements include delta, affected identities, and diagnostics.

Workflow annotation, settings, compatibility metadata, Node Instances, configuration, layout, and the Connection collection are durable. `POST /api/v1/workflows/{id}/editor-session` accepts viewport, selection, panels, and search navigation but deliberately stores none of them and does not advance Draft Version.
