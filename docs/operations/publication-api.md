# Deterministic publication API

`compile_workflow` is the pure compiler (ADR 0007): one Workflow document, the exact Node Contract Locks, one Compatibility Profile, and one versioned policy in; a canonical `canopy.execution-plan/1` Execution Plan or structured diagnostics out. It performs no I/O, so identical inputs always produce identical plan bytes and identical `canopy.compiler/1` digests. A Plan may carry warnings; only `severity: "error"` diagnostics fail compilation. Designated warnings (`require_acknowledgement: true`) block publication until their codes are explicitly acknowledged.

Authenticated Owners use, per workflow `id`:

- `GET /api/v1/workflows/{id}/compile` — preview: `status` is `ok`, `warnings`, or `failed`, with diagnostics and, when compilable, the plan identity (`plan_format`, `compiler_algorithm`, `plan_digest`, node/connection counts).
- `POST /api/v1/workflows/{id}/publish` — body `{editor_session_id, lease_generation, base_draft_version, acknowledged_warnings[]}`. The caller must hold the current Draft Lease; the base must equal the current Draft Version. Publication re-reads the authoritative document inside one immediate write transaction (pending Draft Commands are applied synchronously, so nothing can land between read and store), compiles, and stores the immutable revision plus its evidence and an `hmac-sha256:<hex>` signature derived from the Owner master key. Problems: `423 draft_lease_required`, `409 stale_draft_version` (with `current_draft_version`), `422 compilation_failed` (with `diagnostics`), `409 publication_warnings_unacknowledged` (with `required_acknowledgements`).
- `GET /api/v1/workflows/{id}/publication` — `current_revision` plus every revision in ascending order, each flagged `current` or `superseded`.
- `GET /api/v1/workflows/{id}/revisions/{n}` — the stored record: document, digests, the pinned plan verbatim, contract locks, Compatibility Profile, signature, and publication evidence.
- `POST /api/v1/workflows/{id}/rollback` — moves `current_revision` to the preceding stored revision; `409 no_preceding_revision` when none exists. No revision is ever edited or deleted (ADR 0026).
- `GET /api/v1/workflows/{id}/diff` — the Mutable Draft versus the current Published Revision: added/removed/modified Node Instances, added/removed Connections, and changed workflow fields.

Published Revisions are insert-only rows in `published_revisions`; the plan is pinned with its algorithm-tagged digests and plan format identity, and is served verbatim across restarts without recompilation. The editor surfaces both states — Mutable Draft and Published Revision — with the visual diff, per-publication warning acknowledgement, and rollback through this public client seam.
