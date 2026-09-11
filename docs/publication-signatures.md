# Signed publication and pinned plan verification

Ticket 05 publishes immutable Workflow Revisions and Execution Plans. This document defines the v1alpha1 verification contract exposed by the authenticated public client API.

## Identities

| Purpose | Identity |
| --- | --- |
| Canonical JSON | `jcs-rfc8785` |
| Digest | `sha256:<lowercase hex>` |
| Signature | `ed25519-rfc8032` |
| Revision payload | `canopy.workflow-revision+jcs/v1alpha1` |
| Compiler ABI | `canopy.compiler/v1alpha1` |
| Plan format | `canopy.plan+jcs/v1alpha1` |
| Publication event | `canopy.publication-event/v1alpha1` |
| Compatibility Profile | `canopy.native/v1alpha1` |

All payload digests are SHA-256 over RFC 8785 JCS bytes. A signature is Ed25519 over:

```text
UTF8("canopy-publication-event-v1") || 0x00 || JCS(event.envelope)
```

The envelope includes the target Revision and Plan digests, publication-evidence digest, compile-input digest, compiler and plan identities, Compatibility Profile, exact Node Contract Locks, event lineage, and signing-key identity. The `signature` object repeats the public identity and carries an unpadded base64url signature.

The key ID is independently derivable as:

```text
"ed25519:" || lowercase_hex(SHA256(raw_32_byte_public_key))
```

## Public client verification

A client retrieving `GET /api/v1/workflows/{workflow}/revisions/{revision}` must:

1. JCS-canonicalize `revision.payload` and compare its tagged SHA-256 digest with `revision.digest`.
2. JCS-canonicalize `plan.payload` and compare its digest with `plan.digest`.
3. Verify that the plan's `revision_digest`, contract locks, Compatibility Profile, compiler ABI, and format match the Revision and evidence.
4. Recompute the publication-evidence digest over the evidence identity fields.
5. Verify every envelope linkage and algorithm identity.
6. Derive the key ID from the response's raw public key.
7. Verify the Ed25519 signature over the domain-separated JCS envelope.

`GET /api/v1/workflows/{workflow}/publication` returns the signed current-publication event. After rollback this is the signed rollback event, while the current and newer immutable Revision summaries remain distinct.

The Python acceptance client in `tests/acceptance/test_publication_rollback.py` implements JCS hashing and RFC 8032 verification independently. It uses only HTTP responses and never inspects the database.

## Key custody

The daemon generates a dedicated random 32-byte Ed25519 seed on first publication. It does not derive the seed from, or reuse, the daemon master key. At rest, the seed is encrypted with XChaCha20-Poly1305 under the master key and this dedicated associated-data context:

```text
UTF8("canopy:publication-signing-key-wrap:v1") || 0x00 || UTF8(key_id)
```

Signing-key rows are append-only. Old public keys remain queryable through the immutable signed records, allowing later rotation without invalidating old signatures.

## Compilation and atomicity

Compile Preview and Publish each run the full pure compiler against one exact Draft snapshot. Publish performs its full compile before the immediate SQLite transaction. Inside the short transaction it rechecks Editor Session authority, Lease generation, Draft Version, compile-input digest, compiler success, and the exact required acknowledgement set before inserting the Revision, Plan, evidence, event, and current projection.

The database rejects updates and deletes to signing keys, Revisions, Plans, evidence, and publication events with immutability triggers. Only the current-publication projection is mutable.

Rollback verifies the signed target Revision, payload digest, pinned Plan digest, same-Workflow lineage, and preceding sequence. It appends a signed rollback event and updates the current projection. It does not compile, copy a Revision, delete newer history, or replace the Mutable Draft.

No read, rollback, startup, or execution-plan loading path invokes the compiler. The stored plan is the execution identity; a future incompatible plan format must fail explicitly rather than silently recompile.

## Exact dependencies

- `serde_jcs = 0.2.0` — MIT OR Apache-2.0; declares Rust 1.85; RFC 8785 serialization is checked against the RFC sample.
- `ed25519-dalek = 3.0.0` — BSD-3-Clause; declares Rust 1.85; built without default features and only with `alloc`, `signature`, and `zeroize`.
- `axe-core = 4.13.0` — MPL-2.0; exact-pinned, development-only browser accessibility audit engine; excluded from the production runtime.

The workspace pins Rust 1.85.1. Cargo and npm lockfiles, package checksums, license expressions, and dependency scopes are emitted in release metadata.
