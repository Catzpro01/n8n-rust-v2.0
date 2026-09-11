<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Durable Manual Trigger Runs

This document freezes the first public Run contract implemented by Ticket 06. It covers one published, pure, deterministic Manual Trigger Activation. Later node tickets extend the same engine, scheduler, checkpoint, and trace seams; they must not silently weaken this contract.

## Authority and boundaries

A **Run** executes exactly one immutable Workflow Revision through exactly one pinned Execution Plan. The pure Run engine receives the pinned plan and captured invocation as values and performs no storage, network, or clock I/O. A central governed scheduler owns ready/result flow and one native executor thread. A dedicated Run SQLite writer serializes admission, cancellation, and aggregate checkpoint transactions.

HTTP command responses and the SQLite projection are commit truth. SSE is a bounded notification and observation path, never a commit acknowledgement.

## Public owner API

All routes require the existing Owner session. Mutating routes additionally require the existing origin and CSRF checks.

| Method | Route | Purpose |
| --- | --- | --- |
| `POST` | `/api/v1/workflows/{workflow_id}/runs` | Durably admit an exact published target. |
| `GET` | `/api/v1/runs/{run_id}` | Read durable and optional live/speculative status. |
| `POST` | `/api/v1/runs/{run_id}/cancel` | Durably request cooperative cancellation. |
| `GET` | `/api/v1/runs/{run_id}/trace` | Read and independently re-verify retained Causal Trace/checkpoint hashes. |
| `GET` | `/api/v1/runs/{run_id}/events` | Observe the bounded SSE stream. |

Run Admission accepts:

```json
{
  "run_request_id": "run-request-client-uuid",
  "publication_event_id": "event-observed-by-the-client",
  "revision_id": "revision-observed-by-the-client",
  "plan_digest": "sha256:...",
  "captured_invocation": { "manual": true }
}
```

The captured invocation is RFC 8785/JCS canonical JSON capped at 8 KiB within the existing 16 KiB HTTP body limit. Obvious credential-bearing field names such as `authorization`, `cookie`, `password`, `secret`, `token`, and `private_key` are rejected rather than retained in Run evidence.

### Exact-target and retry behavior

One immediate SQLite transaction:

1. checks whether `run_request_id` already has a retained request fingerprint;
2. verifies the current publication event, revision, revision digest, plan identity, and plan digest;
3. re-hashes the stored pinned plan and verifies its revision link;
4. enforces the nonterminal Run allowance;
5. inserts the Run, admission trace fact, and Queued Durable Checkpoint atomically.

Only then can HTTP return `201 Created`. An identical retry returns the same Run with `200 OK`. Reusing the request identity for different content returns `409 request_identity_conflict`. A changed event/revision/plan returns `409 stale_publication_target` and creates no Run, so the request identity remains usable with corrected content.

## Status model

The status representation deliberately separates two clocks:

- `durable`: state, checkpoint sequence, Logical Order, counters/digest, terminal fact, and update time already committed by SQLite;
- `live`: an optional boot-epoch-scoped observation that can be speculative and can disappear after restart or bounded cache eviction.

Durable states are:

| State | Terminal | Meaning |
| --- | --- | --- |
| `queued` | no | Run Admission and its resume point are committed; no logical outcome is claimed. |
| `cancel_requested` | no | Cancellation won the writer race; new dispatch is blocked while known in-flight facts settle. |
| `succeeded` | yes | Complete output, counters, digest, Activation, trace, and terminal checkpoint committed. |
| `failed` | yes | A typed permanent failure and incomplete correctness evidence committed. |
| `cancelled` | yes | Cancellation settled with incomplete correctness evidence; it is never presented as success. |

The first Manual Trigger Run uses a short 250 ms governed admission window. This makes durable Queued state observable and allows a just-issued Cancellation Request to reach the writer before pure work is dispatched. It is scheduling policy, not a sleep inside the deterministic engine.

## Deterministic Activation and correctness

The engine accepts only the already-pinned one-node native plan whose locked contract is the pure deterministic Manual Trigger, whose configuration is `capture_mode=manual`, and whose output port is `invocation`. It never recompiles a plan.

A successful Run emits exactly one logical Activation at Logical Order 1 and returns the bounded captured invocation on the `invocation` port. The algorithm-tagged correctness digest is SHA-256 over a JCS canonical logical-outcome envelope containing revision digest, plan digest, Node Instance identity, outcome, port, and output. It excludes Run identity, wall time, thread completion order, and checkpoint placement, so equal logical runs produce equal correctness digests.

## Durable Checkpoints

The scalable checkpoint policy commits at the first of:

- 1,024 completed logical outcomes;
- 1 MiB of serialized checkpoint data;
- 250 ms from the first uncheckpointed outcome; or
- an admission, cancellation, suspension/failure, or terminal control barrier.

The Resource Governor may lower but not exceed these thresholds. This bounds replay to 1,024 outcomes and 1 MiB, apart from one currently executing cooperative Activation. Ticket 06 has one Activation, so its terminal aggregate checkpoint commits immediately.

One terminal transaction atomically advances resume state, Logical Order, typed outcome, algorithm-tagged digest and counters, Activation input/output digests and bounded values, provenance, cancellation/failure state, Causal Trace events, hash-chain head, and the Run projection.

## Cancellation race

Cancellation carries a unique `cancellation_request_id`. HTTP success means the request is durable.

The first durable writer commit wins:

- cancellation before a terminal checkpoint stops new dispatch and converges on immutable `cancelled`;
- a speculative result that arrives after the cancellation barrier cannot publish successful logical output;
- a terminal checkpoint committed first is never rewritten, and cancellation returns `already_terminal` with that terminal snapshot;
- a queued Run can commit cancellation and terminal `cancelled` in one transaction without dispatching an Activation.

After restart, any retained `cancel_requested` state is finalized as `cancelled` before scheduling resumes.

## SSE cursor, replay, gap, and resync

SSE event types are `snapshot`, `live`, `durable`, `terminal`, `gap`, and `resync`. Each event carries an opaque ID containing Run, durable checkpoint, daemon boot epoch, and in-memory live position. Clients must not parse or manufacture this representation.

Browsers reconnect with the standard `Last-Event-ID` request header. The same-origin `cursor` query parameter is accepted as an explicit-client fallback. If the entire requested suffix remains in the bounded ring and fits one bounded subscriber mailbox, it is replayed. An unknown/old cursor, a suffix larger than that mailbox, a lagging subscriber whose mailbox was evicted, a cache eviction, or a changed daemon boot epoch produces an explicit `gap` followed by a current `resync` snapshot.

A slow SSE consumer never blocks Run execution. Its bounded sender is dropped; automatic reconnect either replays the retained suffix or receives gap/resync.

## Concrete Eco limits

Every hot queue admits by both entry count and owned/serialized bytes.

| Boundary | Count | Bytes |
| --- | ---: | ---: |
| Nonterminal Run Admission | 64 | bounded request rows; 8 KiB invocation each |
| Hot Run contexts | 16 | bounded by the queues/rings below |
| Ready work | 1,024 | 4 MiB |
| Executor results | 256 | 16 MiB |
| SQLite writer commands | 32 | 8 MiB |
| Live ring per cached hot Run | 256 | 1 MiB |
| SSE subscribers | 32 total | 64 events / 256 KiB per mailbox |

At most 16 live rings are cached. A no-subscriber ring may be evicted to admit a newly hot Run. Downstream result bytes are reserved before dispatch. Ready/result/writer pressure applies backpressure; subscriber pressure causes reconnect/resync instead.

## Causal Trace and safe physical evidence

Trace events and checkpoint snapshots are canonicalized with JCS and linked with tagged SHA-256 hashes. A trace read recomputes every event hash, previous-hash link, and checkpoint hash before returning `integrity_verified=true`.

The logical layer retains Run/revision/plan identity, Activation identity and attempt, Logical Order, typed outcome, bounded input/output, input/output digests, provenance, counters, correctness digest, and checkpoint relationship. The physical layer separately retains wall timing, elapsed microseconds, native lane, one executor-thread fact, queue profile, and checkpoint policy. It does not record credentials, cookies, authorization headers, private model reasoning, or unbounded payloads.

Terminal Run state, revision/plan identity, digest, Activation, checkpoints, and trace are SQLite facts and remain readable after daemon restart. Live boot-epoch state is intentionally not durable.

## Normative references

- [`docs/adr/0053-separate-the-deterministic-engine-from-transactional-storage.md`](adr/0053-separate-the-deterministic-engine-from-transactional-storage.md)
- [`docs/adr/0027-record-causal-traces-and-replay-side-effects-safely.md`](adr/0027-record-causal-traces-and-replay-side-effects-safely.md)
- [WHATWG HTML Living Standard: Server-sent events and `Last-Event-ID`](https://html.spec.whatwg.org/multipage/server-sent-events.html#the-last-event-id-header), accessed 2026-09-11
- [SQLite Write-Ahead Logging](https://www.sqlite.org/wal.html), accessed 2026-09-11
- [SQLite `PRAGMA synchronous`](https://www.sqlite.org/pragma.html#pragma_synchronous), accessed 2026-09-11
- [IETF HTTPAPI Idempotency-Key draft](https://datatracker.ietf.org/doc/html/draft-ietf-httpapi-idempotency-key-header-07), consulted for retry/fingerprint semantics; the public API keeps the identity in its JSON contract rather than claiming the draft header is an RFC
