# 06: Run and trace Manual Trigger durably

**What to build:** The Owner can durably admit and run the published Manual Trigger, watch reconnectable progress, cancel safely, and inspect one logical Activation and its checkpointed Causal Trace.

**Blocked by:** 05: Publish and roll back a Manual Trigger revision

**Status:** completed (2026-09-11)

- [x] Run Admission commits a Queued Run tied to exactly one immutable revision/plan before returning success.
- [x] A deterministic Run state machine and governed bounded scheduler execute one Manual Trigger Activation without performing storage/network I/O inside the state machine.
- [x] One dedicated SQLite writer commits an aggregate Durable Checkpoint containing resume state, Logical Order, outcome, digest/counters, provenance, and retained trace.
- [x] Status and SSE distinguish queued, live/speculative, durable, terminal, and reconnect/resync behavior; HTTP response is command commit truth.
- [x] Causal Trace exposes Run, revision/plan, Activation attempt/outcome, input/output, checkpoint, timing, and safe resource facts.
- [x] A durable cooperative cancellation request stops new work, settles/checkpoints facts, and reaches a visible terminal result.
- [x] Ready work, executor results, database commands, and SSE subscribers are count- and byte-bounded.
- [x] Service restart after a completed run preserves terminal state, digest, trace, and revision identity.
- [x] Browser-visible start, status, reconnect, cancel, trace, and completed-run tests use no internal database setup.

## Shared-understanding gate

The Owner explicitly confirmed decisions 06-A through 06-E on 2026-09-11 and authorized Ticket 06 implementation. No production implementation began before that confirmation.

## Confirmed owner decisions (2026-09-11)

### 06-A — Run Admission identity and exact publication target

Use strict exact-target admission. The client supplies a durable `run_request_id` plus the `publication_event_id`, `revision_id`, and `plan_digest` it observed. One SQLite transaction verifies that exact publication is still current and admissible, fingerprints the request, and inserts the Run already bound to that immutable revision and pinned plan in `queued` state before HTTP success.

An identical retry returns the same Run and does not enqueue duplicate work. Reusing the request identity with different content returns HTTP 409. If the current publication no longer matches the expected event/revision/plan, admission returns HTTP 409 without creating a Run. The compact request-identity fact remains with retained Run identity so a later retry cannot create a second Run merely because detailed evidence was compacted.

### 06-B — Bounded SSE replay, gap, and resync

Use a bounded hybrid stream. Durable checkpoint notifications have a retained SQLite cursor. Live/speculative updates have a bounded in-memory sequence scoped by a daemon boot epoch. SSE event IDs are opaque cursors carrying enough run, durable, epoch, and live position identity for validation; reconnect consumes standard `Last-Event-ID`.

Replay only when the requested suffix is still wholly available. A cursor older than retained history, a lagging subscriber, an unknown cursor, or a changed boot epoch produces an explicit `gap` followed by a current `resync` snapshot rather than pretending continuity. Subscriber/ring pressure never blocks Run execution; a lagging stream is resynchronized or disconnected for reconnect. HTTP command responses and durable status remain commit truth, while live fields are explicitly labelled speculative.

### 06-C — Adaptive checkpoint policy with hard replay bounds

Use balanced adaptive grouped checkpoints under SQLite WAL plus `synchronous=FULL`. Commit at the first of 1,024 completed logical outcomes, 1 MiB of serialized checkpoint data, 250 ms from the first uncheckpointed outcome, or a control/terminal barrier. Admission and cancellation are immediate durable barriers. The Resource Governor may lower these thresholds under measured pressure or progress needs but may not exceed them.

The durable resume cursor therefore bounds replay to at most 1,024 outcomes and 1 MiB of retained checkpoint input, apart from one currently executing cooperative Activation. Checkpoint boundaries and replay are physical evidence and cannot alter Logical Order, visible output, counters, or the algorithm-tagged correctness digest. Ticket 06's single successful Manual Trigger Activation commits its terminal aggregate checkpoint immediately.

### 06-D — Durable cancellation and terminal race

Use first-durable-commit-wins semantics. Cancellation has its own idempotent `cancellation_request_id`; HTTP success means the cancellation request is durable. If cancellation commits before any terminal checkpoint, the scheduler stops dispatching new work, in-flight work observes a cooperative token at safe points, known facts are settled, and the Run reaches immutable terminal `cancelled`. A speculative result arriving after that cancellation barrier may be retained only as labelled physical/late-result evidence; it cannot publish successful logical output or change the terminal result.

If `succeeded`, `failed`, or `cancelled` was already durably committed, a later cancellation returns an explicit `already_terminal` response with the terminal snapshot and does not rewrite history. A queued Run can record cancellation and terminal `cancelled` atomically without starting an Activation. A cancelled Run retains partial counters/digest facts as incomplete evidence and can never be presented as successful completion.

### 06-E — Concrete Eco queue and byte limits

Use the balanced Eco profile. Admit at most 64 nonterminal Runs and keep at most 16 hot Run contexts in memory. Across hot Runs, the ready-work queue is capped at 1,024 entries and 4 MiB; the executor-result queue at 256 entries and 16 MiB; and the dedicated-writer command queue at 32 entries and 8 MiB. Each hot Run's live SSE ring is capped at 256 events and 1 MiB. Admit at most 32 SSE subscribers, each with a mailbox capped at 64 events and 256 KiB. Ticket 06 Manual Trigger admission accepts at most 8 KiB of canonical inline captured-invocation JSON inside the existing 16 KiB HTTP body cap.

Every queue accounts for both entries and owned/serialized bytes before admission. The scheduler reserves downstream capacity before dispatch, applies backpressure or suspends admission/work when a work/result/storage limit is reached, and never waits for SSE consumers. An individually oversized value is rejected safely in Ticket 06; later Artifact work may replace large inline values with bounded references without changing logical semantics. At these maxima, hot queued payload is bounded to roughly 52 MiB before container/index overhead, leaving most of the 500 MiB installation budget for the daemon, SQLite, traces, reads, and safety margin.


## Implementation verification (2026-09-11)

- `cargo +1.85.1 fmt --all -- --check`, workspace Clippy over all targets with `-D warnings`, all 17 Rust tests, and `git diff --check` pass.
- `make test` passes all three public browser journeys and all 10 daemon/public API acceptance tests. The Run browser reports `run-browser=passed sse-gap-resync=passed axe-serious=0 reload-markup-diff=0 mobile-overflow=0`.
- The deterministic engine tests prove the pinned plan is never recompiled, plan mutation is rejected, equal inputs produce one stable logical outcome/digest, and cancellation is a pure transition input without engine storage/network access.
- Public-seam Run acceptance proves queued-before-dispatch admission, identical retry without duplication even after a newer publication becomes current, changed-content and stale exact-target HTTP 409 behavior without consuming the request identity, and equal correctness digests across distinct Run IDs.
- The same acceptance journey proves ordered snapshot/live/terminal observation, retained suffix replay through `Last-Event-ID`, explicit gap/resync for an unknown cursor and after a boot-epoch change, durable queued cancellation, oversized invocation rejection, sensitive-field rejection, trace hash verification, and exact terminal status/digest/trace/revision/plan reconstruction after restart.
- The implementation has one wake-driven governed scheduler, one native executor, and one dedicated Run SQLite writer; it creates no task/thread/actor per Run, Node Instance, or Activation. Idle scheduling blocks on a wake channel instead of polling.
- Count and byte limits are executable constants, returned as status/trace evidence, and asserted at the public seam: 64 nonterminal Runs, 16 hot execution contexts, 1,024/4 MiB ready, 256/16 MiB results, 32/8 MiB writer commands, 256/1 MiB per cached live ring, 32 subscribers with 64/256 KiB mailboxes, and 8 KiB canonical inline invocation. Frames are individually bounded so 64 frames cannot exceed the mailbox byte limit.
- SQLite Run connections explicitly preserve WAL, `synchronous=FULL`, foreign keys, one atomic immediate writer transaction per admission/control/checkpoint barrier, immutable terminal guards, and immutable retained Activation/trace/checkpoint rows.
- `cargo audit` scans 150 locked Cargo dependencies against 1,243 RustSec advisories with no vulnerability finding; `npm audit --audit-level=moderate` reports zero vulnerabilities.
- `make release-test` passes bundle/checksum inspection and the full Run acceptance journey against the stripped release binary; the installed artifact remains far below the 10 GiB disk ceiling.
- Native systemd smoke passes with 30,273,536 RSS bytes, 0.0000 idle CPU cores, and hardening exposure 1.6 under the existing 500 MiB / 0.5 CPU limits.
- The stable API, truth boundaries, state model, retry semantics, checkpoint/cancellation policy, SSE cursor behavior, concrete Eco limits, and trace-safety rules are documented in `docs/run-durability.md`.
