# 07: Stream Generate Items through bounded Envelopes and Artifacts

**What to build:** Generate Items can emit the Eco item stream through bounded Envelopes, backpressure, and encrypted content-addressed Artifact spill without materializing the whole stream in RAM.

**Blocked by:** 06: Run and trace Manual Trigger durably

**Status:** complete (shared understanding confirmed and implementation authorized 2026-09-11; pre-commit acceptance gates passed 2026-09-11)

- [x] Generate Items has a locked Pure deterministic Per-Item/expansion contract with declarative count/start/step configuration and hard output budgets.
- [x] For the Eco configuration it emits exactly 49,998 ordered items from one trigger input while preserving provenance.
- [x] Engine queues and output emitters apply backpressure and remain bounded while a slow consumer is exercised.
- [x] Large configured values and forced-spill cases stream to encrypted algorithm-tagged content-addressed Artifacts in the Owner namespace.
- [x] Artifact staging, sync, atomic placement, SQLite reference commit, deduplication, authorization, and safe orphan cleanup follow the accepted crash-safe order.
- [x] No credential or Secret Lease value can enter Artifact content or metadata through this node.
- [x] The editor shows generated progress, output count, spill/backpressure state, and authorized lazy Artifact detail without loading all items.
- [x] Fault tests cover cancellation, output/item/byte budget exceeded, digest mismatch, staging interruption, and reference denial.
- [x] RSS and queue evidence demonstrate that memory does not grow linearly with 49,998 output items.

Completion evidence and the operator-facing API/recovery contract are recorded in `docs/operations/generate-items-artifacts.md`. The pre-commit public seam emitted exactly 49,998 Items with a 3,272,704-byte RSS delta while the advertised bound remained 256 Envelopes / 4 MiB. Progressive trace ranges were contiguous from each prior durable cursor, and restart acceptance resumed without duplicate logical Activations. Full `make test`, strict Clippy, RustSec audit, browser accessibility, and visual-regression gates passed. Clean-commit release inspection, release Generate/Artifact acceptance, and native systemd smoke are mandatory post-commit gates.


## Shared-understanding gate

The Owner invoked `/ask-matt` on 2026-09-11. Ticket 07 is being sharpened through bounded `/grill-with-docs` decision rounds inside this Wayfinder-produced implementation ticket. The Owner explicitly confirmed decisions 07-A through 07-I and authorized production implementation on 2026-09-11. No production implementation began before that confirmation.

## Confirmed owner decisions — round 1 (2026-09-11)

### 07-A — Structured deterministic range item

Generate Items has declarative signed-integer `count`, `start`, and `step` fields plus optional fixed JSON `data`. For ordinal `i`, its logical item facade is `{index: i, value: start + i * step, data: configured_data}`. Keeping generated fields outside `data` removes merge/collision policy. Arithmetic is checked, array order and JSON scalar distinctions remain significant, and provenance separately links every item to the input Envelope, Generate Activation, output port, and ordinal. The Eco fixture uses `count=49,998`, with the exact start/step values frozen before implementation completion.

### 07-B — Per-item logical Envelopes with bounded physical micro-batches

Every generated item is one independently ordered logical Envelope with its own provenance and byte accounting. The transport may pack adjacent Envelopes into bounded physical micro-batches for CPU efficiency, but a batch is not a logical item, cannot change ordering/digest/item linking, and cannot become an unbounded materialized collection. Admission is governed by both item count and owned/serialized bytes. The producer blocks cooperatively at the output-emitter seam under backpressure rather than growing memory or bypassing cancellation.

### 07-C — Dual Owner-scoped Artifact identity and chunked envelope encryption

The public/client identity is an opaque `artifact_id`, never a storage path or raw cross-namespace content address. Inside the Owner Artifact Namespace, a keyed BLAKE3 identity supplies deterministic same-owner deduplication without exposing cross-owner equality. Encrypted authenticated metadata retains the algorithm-tagged plaintext digest used to verify successful reads, while a separate physical digest verifies stored ciphertext.

Each Artifact receives a random data-encryption key wrapped under the existing master-key custody model. Content is encrypted as independently authenticated XChaCha20-Poly1305 chunks with domain-separated associated data binding format version, Owner namespace, Artifact identity, chunk ordinal, and declared length. This reuses the pinned cipher family already present in the daemon; BLAKE3 must be exact-version pinned, licensed, audited, and measured before production use.

### 07-D — Operation leases plus safe orphan quarantine

Artifact staging records a bounded operation lease and remains on the target filesystem. The durability order is stream/limit/digest/encrypt into staging, synchronize file data and metadata, atomically place the immutable object without overwriting a different object, synchronize the parent directory, and only then allow the dedicated SQLite writer to commit a live authorized reference. A committed reference may never point to an object that was not made durable.

Startup reconciliation runs before Run Admission while no Artifact writer is active. During normal service, cleanup runs only behind an Artifact maintenance barrier and respects operation leases plus a quarantine age; it never races rename/reference commit. Interrupted staging or a final unreferenced object is a safe orphan, while digest mismatch/corrupt objects remain quarantined evidence rather than becoming readable content. Exact lease/quarantine durations and disk-pressure behavior are deferred to round 2.

## Confirmed owner decisions — round 2 (2026-09-11)

### 07-E — Balanced concrete Eco bounds and accounting

Generate Items accepts at most 50,000 outputs, so the exact 49,998-item fixture remains below the hard ceiling. `start` and `step` are signed 64-bit integers and every `start + index * step` operation is checked before execution. The frozen Eco values are `count=49,998`, `start=0`, `step=1`, and `data=null`, producing indices/values 0 through 49,997.

Canonical inline `data` is capped at 4 KiB. The Envelope transport is capped at 256 logical Envelopes and 4 MiB, while one physical micro-batch stops at the first of 64 Envelopes or 256 KiB. Artifact plaintext is processed in 64 KiB authenticated chunks; one Artifact is capped at 16 MiB, one Generate Activation at 64 MiB of expanded logical output and 32 MiB of newly durable unique Artifact content, and one authorized browser preview at 64 KiB. Logical bytes are counted before deduplication or compression, while Artifact quota separately counts newly durable encrypted storage, so repeated content cannot bypass the output budget.

`storage_mode=auto|artifact` is a physical policy hint: `artifact` forces spill for conformance/fault testing, while `auto` spills above the effective inline threshold. The Resource Governor may lower inline, batch, queue utilization, preview, or concurrency limits under pressure but may not raise contract hard limits. Inline versus Artifact representation is physical evidence and cannot change logical Items, provenance, ordering, or correctness digests.

### 07-F — Progressively checkpointed streaming output

Generate Items emits Envelopes immediately through the bounded output-emitter seam. The durable generation cursor, contiguous emitted range, Artifact roots/references, downstream committed outcomes when present, Logical Order, counters/digest state, and safe trace summary advance together in the existing aggregate checkpoint transaction. Checkpoints remain bounded by the accepted first-of 1,024 committed logical outcomes, 1 MiB, 250 ms, or control/terminal barrier policy; output bytes and the generation cursor are also included so a slow stream cannot create an unbounded unrecorded prefix.

A restart regenerates only the deterministic suffix after the durable cursor and never republishes an already committed downstream outcome. Speculative Envelopes after the cursor may replay and remain explicitly non-authoritative. Cancellation stops generation/output at cooperative item/chunk boundaries, settles already committed facts, and records incomplete correctness. A terminal Generate outcome closes its output port explicitly; a failure closes it with a typed failure after settled facts rather than pretending a truncated stream is complete.

### 07-G — Same-origin authorized Artifact stream API

The existing Owner session, origin, CSRF, and bounded-request policies authorize Artifact upload creation/finalization and reference mutation. Large configuration content uses a streaming upload/finalize path instead of a giant Draft-command JSON body. Reads authorize every metadata, preview, range, and content request against the Owner namespace and retained reference; denial returns 403 without disclosing existence, digest, size, media type, or storage details.

The API exposes only opaque `artifact_id`, format/content algorithms, safe media type, logical size, authorized digest evidence, reference/pin state, and bounded integrity/provenance facts. It never exposes filesystem paths, wrapped keys, nonces, ciphertext layout, staging names, or cross-owner equality. Content/range reads authenticate and decrypt complete chunks before releasing plaintext. The editor lazy-loads metadata and at most a 64 KiB preview on explicit Owner action; it never loads all generated Items or a full large Artifact merely to show Run progress.

### 07-H — Typed pressure and failure outcomes

Normal queue saturation applies cooperative backpressure and records bounded queue/wait evidence; it is not a node failure. Temporary disk/resource pressure that prevents safe staging, sync, checkpoint, or control-plane reserve enters Durable Suspension and can resume only after revalidation. Authored hard count/item/logical-byte/Artifact limits, checked-arithmetic overflow, malformed configuration, and individually oversized values produce typed Permanent Failure because waiting cannot make them valid.

Cancellation produces Cancelled and prevents new output. Plaintext/ciphertext digest failure, authentication-tag failure, truncation, wrong namespace/key, staging/placement invariant failure, or a committed-reference integrity failure produces typed failure, quarantines the suspect object/evidence, and never releases unverified plaintext. Unauthorized Artifact reads return indistinguishable 403 responses; execution-time reference denial becomes a typed permanent node failure without leaking metadata.

### 07-I — Namespace keys, staging leases, and quarantine defaults

Each Owner Artifact Namespace receives a random namespace seed wrapped by the existing master-key custody mechanism. Domain-separated BLAKE3 key derivation produces independent deduplication and Artifact-key-wrapping keys, so one key is never reused for both purposes. Master-key rotation rewraps the namespace seed without re-encrypting Artifact content; object data keys remain independently random and can be rewrapped without changing plaintext identity.

A staging operation lease lasts five minutes and is refreshed while chunks, sync, placement, or reference commit are active. Startup reconciliation occurs before admission. Runtime reconciliation first quiesces Artifact mutation behind its maintenance barrier. Safe unreferenced objects remain quarantined for at least 24 hours before quota-aware deletion; active leases, live references, pins, and suspect corruption evidence are not removed. If expired safe-orphan cleanup cannot restore the required disk/recovery reserve, the system suspends/rejects work rather than deleting young or referenced content.
