# 05: Publish and roll back a Manual Trigger revision

**What to build:** The Owner can compile the exact Draft, review diagnostics and differences, publish a signed immutable Manual Trigger revision with a pinned Execution Plan, and roll back without mutating history.

**Blocked by:** 04: Recover and arbitrate Draft editing

**Status:** completed (2026-09-11)

- [x] The compiler is a deterministic pure transformation of one Workflow Revision, exact Node Contract Locks, Compatibility Profile, and policy into a plan or structured diagnostics.
- [x] Compilation validates graph identities, configuration, ports, expressions, capabilities, effects, budgets, and compatibility status without I/O.
- [x] Publish flushes pending Draft Commands, verifies lease holder and exact Draft Version, and blocks stale/invalid/error states.
- [x] Designated warnings require explicit acknowledgement and are captured in publication evidence.
- [x] Published Revision and Execution Plan record algorithm-tagged digests, compiler/plan format identity, contract locks, and Compatibility Profile and remain immutable after restart.
- [x] The editor shows a visual Draft-versus-published difference and distinct Mutable Draft/Published Revision states.
- [x] Rollback selects/creates new current work from the preceding revision without editing or deleting either immutable revision.
- [x] A plan remains pinned and readable across restart; no run-time or upgrade path silently recompiles it.
- [x] Publish and rollback are driven through the public client seam and verified against signed identities rather than direct database inspection.

## Confirmed implementation decisions (`/ask-matt`, 2026-09-11)

### 05-A — Canonical identity and signature

Published Revision and Execution Plan payloads use RFC 8785 JSON Canonicalization Scheme bytes. Every identity is algorithm-tagged (`jcs-rfc8785`, `sha256`, and `ed25519-rfc8032`) rather than relying on an implicit serializer or cryptographic default.

A dedicated randomly generated Ed25519 publication key signs a domain-separated canonical publication envelope. Its private seed is encrypted at rest under the existing daemon master key with a dedicated key-wrapping context; it is not derived directly from or reused as the master key. The envelope records key ID, public key, signature algorithm, canonicalization algorithm, payload digests, compiler identity, plan-format identity, Compatibility Profile, and exact Node Contract Locks. Old public keys remain available so key rotation cannot make existing revisions unverifiable. The public client seam verifies signatures and digests independently of direct database access.

### 05-B — Preview, full compile, and atomic publication

Compile Preview runs the complete pure deterministic compiler against one exact Draft snapshot and returns structured diagnostics plus a compile-input digest. The editor disables Publish until its command/recovery queue is empty.

Publish supplies the exact Draft Version, Editor Session identity, Lease generation, compile-input digest, and required diagnostic acknowledgements. The daemon takes an exact snapshot and performs a fresh full compile outside the SQLite write transaction. A short immediate transaction then rechecks the live holder and generation, unchanged Draft Version and digest, acknowledgement set, and compiler result before atomically inserting the immutable Revision, immutable pinned Plan, publication evidence, and current-publication event. Any mismatch returns a structured stale/conflict result and inserts nothing.

### 05-C — Designated warning acknowledgement

Diagnostics have stable code, severity, subject identity, structured arguments, `requires_ack`, and a deterministic fingerprint. Errors always block. Only policy-designated warnings require acknowledgement; ordinary warnings remain visible without producing habitual confirmation clicks.

An acknowledgement is valid only for the exact compile-input digest and diagnostic fingerprint and is stored in publication evidence. Any relevant Draft, contract, profile, policy, or compiler-input change invalidates it. Ticket 05 exercises the path with a designated warning that the Manual Trigger output is not connected or consumed.

### 05-D — Non-destructive rollback

Rollback first verifies the target Revision signature, payload digest, pinned Plan digest, and same-Workflow lineage. It does not recompile or duplicate the target Revision. A transaction appends a signed rollback/current-publication event and changes the current Published Revision projection to the selected preceding immutable Revision.

Newer Published Revisions, Plans, signatures, and publication evidence remain untouched and readable. The Mutable Draft also remains intact so unfinished repair work is not silently discarded; restoring a published snapshot into Draft is a separate explicit operation. The editor shows current-published, newer-history, and Mutable-Draft differences distinctly.

## Decision references

- RFC 8785 defines invariant canonical JSON suitable for repeatable hashing and signatures: <https://datatracker.ietf.org/doc/html/rfc8785>.
- RFC 8032 defines Ed25519 signing/verification and test vectors: <https://www.rfc-editor.org/rfc/rfc8032>.
- NIST FIPS 186-5 includes EdDSA/Ed25519 and deterministic signature generation: <https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.186-5.pdf>.
- The selected Rust implementation must be exact-version pinned, license-recorded, vulnerability-audited, and checked against the repository Rust MSRV before production use; current API reference: <https://docs.rs/ed25519-dalek/latest/ed25519_dalek/struct.SigningKey.html>.
- SQLite documents trigger-side `RAISE()` support used by later implementation tests to reject accidental mutation of immutable rows: <https://www.sqlite.org/lang_createtrigger.html#the_raise_function>.


## Implementation verification (2026-09-11)

- `cargo +1.85.1 fmt --all -- --check`, workspace Clippy with `-D warnings`, and all Rust tests pass.
- The RFC 8785 sample and tagged SHA-256 fixture pass; pure compiler tests cover deterministic output plus graph identities, configuration, ports/cardinality, expressions, capabilities, effects, budgets, and Compatibility Profile failures.
- `make test` passes 10 Rust tests, both browser journeys, and 9 daemon/public API acceptance tests.
- The publication browser journey passes keyboard operation, warning acknowledgement, two immutable revisions, non-destructive rollback, 390 px overflow, exact reload screenshot comparison, and axe-core with zero serious/critical violations.
- `test_publication_rollback.py` independently recomputes JCS digests, publication-evidence linkage, key IDs, and RFC 8032 Ed25519 verification from HTTP responses only; debug and release-bundle runs both pass across daemon restart.
- `cargo-audit 0.22.1` scans 149 locked Cargo dependencies against 1,243 RustSec advisories with no vulnerability finding; `npm audit --audit-level=high` reports zero vulnerabilities.
- Exact dependency metadata records `serde_jcs 0.2.0` (MIT OR Apache-2.0, Rust 1.85), `ed25519-dalek 3.0.0` (BSD-3-Clause, Rust 1.85), and test-only excluded `axe-core 4.13.0` (MPL-2.0).
- Release bundle tests and checksum verification pass; the stripped release binary is 7,167,968 bytes.
- Native systemd smoke passes with 28,942,336 RSS bytes, 0.0000 idle CPU cores, and hardening exposure 1.6 under the existing 500 MiB / 0.5 CPU limits.
- Verification format, key custody, atomicity, rollback, and no-silent-recompile rules are documented in `docs/publication-signatures.md`.
