# 15: Import the first n8n 2.39.0 compatibility subset

**What to build:** A sanitized owner-authored n8n 2.39.0 workflow in the first supported subset imports into a Draft, preserves exact external identities and safe unknowns, reports compatibility honestly, and round-trips without copied implementation inputs.

**Blocked by:** 05: Publish and roll back a Manual Trigger revision

**Status:** ready-for-agent

- [ ] The private oracle is pinned by release/image/package digest, environment, locale, timezone, edition, and reset procedure before fixture collection.
- [ ] Fixture provenance uses only public documentation/specifications, owner-authored exports, and black-box documented surfaces; no n8n source/tests/Enterprise files/internal REST reverse-engineering/assets/copy enter implementation.
- [ ] A fail-closed sanitizer rejects credential names/IDs/values, authentication headers/cookies, personal endpoints/data, unexpected opaque fields, and large binary/base64 content.
- [ ] Import preserves user Node Instance names, exact external type and `typeVersion`, safe parameters/unknown fields, connections/settings, and the 2.39.0 Compatibility Profile.
- [ ] Native primary names remain distinct while visible Compatibility Aliases and exact fixture evidence are available in details.
- [ ] The Compatibility Report classifies every imported feature as Native equivalent, Delegated compatible, Preserved opaque, Adapted with declared difference, Unsupported, or Rejected unsafe.
- [ ] Unsupported/unsafe required behavior blocks publication; no node version, expression, ordering, capability, or credential is silently changed.
- [ ] Canonical comparison preserves semantic array/item/port/type/null/missing/linking order and normalizes only case-declared volatile fields.
- [ ] The supported first subset covers workflow JSON preservation, Items/linking, safe expressions, branching, deterministic merge, and execution-order setting diagnostics where represented.
- [ ] Round-trip, malformed import, secret-scanner, and field-level black-box differential results are committed as independent evidence.
