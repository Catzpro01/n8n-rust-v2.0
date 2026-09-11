# Generate Items and Owner Artifacts

## Operator summary

`canopy.native/generate-items@0.1.0` expands one input into ordered logical Envelopes. It is a Pure, deterministic native node. For ordinal `i`, the logical item is:

```json
{"index": "i", "value": "start + i * step", "data": "configured JSON"}
```

`start` and `step` are signed 64-bit integers and every operation is checked. The Eco acceptance configuration is `count=49998`, `start=0`, `step=1`, and `data=null`; it emits values and indices 0 through 49,997.

## Hard bounds and backpressure

| Boundary | Hard value |
|---|---:|
| Generated items | 50,000 |
| Inline canonical `data` | 4 KiB |
| Physical micro-batch | 64 Envelopes or 256 KiB |
| Output-emitter queue | 256 Envelopes and 4 MiB |
| Logical output per Generate Activation | 64 MiB |
| Artifact plaintext | 16 MiB |
| Newly durable unique Artifact content per Generate Activation | 32 MiB |
| Authenticated encryption chunk | 64 KiB |
| Browser preview | 64 KiB |
| Progressive checkpoint | first of 1,024 outcomes, 1 MiB, 250 ms, or a control/terminal barrier |

The process uses four bounded physical batch slots. A full channel blocks the producer cooperatively. Saturation records backpressure and does not become a node failure. Logical byte accounting and the correctness chain use the original item facade, so inline versus Artifact representation cannot change the logical digest.

## Progressive durability and restart

`run_generation_progress` retains the durable next ordinal, logical bytes, stream digest, backpressure evidence, and optional Artifact reference. The first generation checkpoint commits the Manual Trigger Activation and its trace outcome in the same transaction. Later checkpoints advance only monotonically. On restart the deterministic session begins at the durable cursor; only a speculative suffix can replay. The terminal transaction commits the Generate Activation, closes its output, and lets a previously committed cancellation win.

Temporary Artifact storage errors create a durable `suspended` view backed by `run_suspensions`. A successful daemon restart runs Artifact startup reconciliation before Run admission, which is the revalidation barrier before suspended work is queued again. Invalid hard bounds, arithmetic overflow, an unauthorized retained Artifact reference, and malformed data are permanent failures.

## Artifact storage and identity

The public `artifact_id` is random and opaque. It is not a path or a content address. Within the Owner namespace, a random wrapped namespace seed derives independent BLAKE3 keys for keyed deduplication and object-key wrapping. Each unique object uses a random data-encryption key. Metadata and 64 KiB content chunks use XChaCha20-Poly1305 with domain-separated associated data.

Durability order is:

1. create a same-filesystem staging operation and five-minute lease;
2. stream and bound plaintext while refreshing the lease;
3. compute Owner-keyed BLAKE3 identity and plaintext digest;
4. encrypt authenticated chunks to staging and synchronize the file;
5. atomically hard-link immutable content into the object directory and synchronize the directory;
6. commit the Artifact index and retained authorized reference in SQLite.

Startup reconciliation runs before Run admission. Interrupted staging and unindexed placed objects move to quarantine, with both source and destination directories synchronized. Runtime maintenance takes the Artifact mutation barrier, moves only expired staging leases to quarantine, and leaves active leases untouched. Safe orphans remain for at least 24 hours. Suspect integrity evidence receives no automatic deletion deadline. Cleanup does not delete live referenced content.

## Same-origin Owner API

All reads require an authorized retained Owner reference. A denied, unknown, or unauthenticated Artifact returns the same non-disclosing `403 artifact_reference_denied` problem.

- `POST /api/v1/artifacts` — streaming upload and finalize; media type comes from `Content-Type`; maximum 16 MiB; requires Owner origin, session, and CSRF headers.
- `GET /api/v1/artifacts/{artifact_id}` — safe metadata only.
- `GET /api/v1/artifacts/{artifact_id}/preview?bytes=N` — authenticated preview, where `1 <= N <= 65536`.
- `GET /api/v1/artifacts/{artifact_id}/content` — fully verified content streamed through a two-chunk bounded HTTP mailbox after a complete integrity pass.
- `GET /api/v1/artifacts/{artifact_id}/content` with `Range: bytes=start-end` — one inclusive authenticated range and `206 Content-Range`; only the requested plaintext window is retained.

Reads verify the physical ciphertext digest, every authentication tag, encrypted metadata, and the plaintext digest before releasing bytes. Integrity failure moves the object to quarantine. No response exposes object paths, staging names, nonces, wrapped keys, deduplication identities, or cross-Owner equality.

The editor fetches metadata and at most 65,536 preview bytes only after the Owner selects **Verify and preview**. It never downloads all generated Items to render Run progress.

## Acceptance evidence — 2026-09-11

The final pre-commit `make test` gate passed with 4 Node Contract tests, 27 daemon unit tests, four browser scenarios, and 13 Python public-seam acceptance tests. The generation trace acceptance checks that every progressive checkpoint range begins at the previous durable cursor. Cancellation-versus-Artifact-preparation, bounded preview/range buffers, full HTTP content streaming, the shared read/quarantine/cleanup barrier, expired staging cleanup, live-reference preservation, and indefinite integrity-evidence quarantine have dedicated regressions.

Debug-profile exact Eco evidence:

```text
generate-eco=passed items=49998 logical_bytes=2027698 backpressure_events=781 rss_baseline=29855744 rss_peak=33128448 rss_delta=3272704 queue_items=256 queue_bytes=4194304
generate-restart=passed durable_cursor_before=1664 terminal_count=49998 activations=2 unique_checkpoints=61
generate-suspension=passed durable_state=suspended revalidated_state=succeeded
```

BLAKE3 is exact-pinned at 1.8.7 and declared `CC0-1.0 OR Apache-2.0` by the package metadata included in the release license inventory. `cargo-audit 0.22.2 --deny warnings` scanned all 153 locked Rust dependencies against 1,243 RustSec advisories with no finding. A 32 MiB combined keyed/unkeyed debug-profile measurement completed in 36,662 microseconds (872.83 MiB/s) on the verification VPS.

Browser and visual evidence:

```text
generate-ui=passed lazy-preview-bytes=4 axe-serious=0 desktop-visual=passed mobile-visual=passed mobile-overflow=0
```

Approved visual baselines are `editor/tests/baselines/generate-progress.desktop.png` and `editor/tests/baselines/generate-progress.mobile.png`. Final clean-commit binary identity, checksums, SBOM, and resource profile are emitted by `make release` into `out/tracer-bundle/release/release-manifest.json`; `make release-test` and `scripts/test-systemd-smoke.sh out/tracer-bundle` are the mandatory post-commit gates.
