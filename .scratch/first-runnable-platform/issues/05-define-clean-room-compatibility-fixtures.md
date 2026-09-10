# Define clean-room compatibility fixtures

Type: research
Status: resolved
Blocked by: none

## Question

What clean-room process and fixture sources can establish n8n 2.39.0 workflow JSON, expression, webhook, and core-node compatibility without copying implementation, Enterprise code, protected assets, or distinctive product presentation?


## Answer

Freeze a private n8n 2.39.0 black-box oracle and derive only sanitized, owner-authored, behavior-level fixtures from allowed public documentation, public API/CLI/webhook surfaces, and owner-created exports. The cited process and matrix are [Research: clean-room compatibility fixtures for n8n 2.39.0](../../../docs/discovery/clean-room-compatibility-fixtures.md).

The exact baseline is release tag `n8n@2.39.0`, signed commit `1620ec42cab26e2936e8fd4c1c011c5d210e3a95`; GitHub marked it pre-release, but the explicit Compatibility Profile stays pinned. Record an immutable reference image/package digest, environment, timezone, locale, and edition state. Current online docs can drift and are provisional until confirmed against that oracle.

Separate reference observation from implementation. The reference operator may create original workflows and observe documented external behavior; the clean-room implementer receives only sanitized workflow JSON, deterministic inputs, minimal normalized expected facts, provenance, and independently worded assertions. n8n implementation source, upstream tests, Enterprise `.ee.` files, internal `/rest` reverse-engineering, templates, screenshots, icons, CSS, and product copy are forbidden inputs.

Fixture families cover workflow JSON/round trips, item and linking semantics, expressions, webhooks, deterministic core nodes, v1 versus imported legacy v0 execution order, and import diagnostics. Canonicalization sorts object keys but preserves semantic ordering and type distinctions; only declared volatile fields are normalized. Credential names/IDs, cURL headers, personal data, and binary payloads pass a fail-closed sanitizer before commit.

Every import yields evidence-backed status per feature: Native equivalent, Delegated compatible, Preserved opaque, Adapted with declared difference, Unsupported, or Rejected unsafe. Adding a later Compatibility Profile never mutates 2.39.0 evidence.
