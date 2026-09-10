# Draft Lease, durable history, and browser Recovery Copy

Ticket 04 adds deterministic single-writer editing without turning the private Owner product into a collaboration system. The Rust daemon remains the only Mutable Draft authority.

## Lease contract

A browser tab generates an opaque Editor Session identity and registers a short human label. One Workflow has at most one live Lease row. Responses expose only role (`holder`, `read_only`, or `available`), generation, holder label, and server expiry—not another tab's opaque identity.

The default Lease is 30 seconds. Holder commands and heartbeats renew it; the editor heartbeats every 10 seconds. Every command carries both Editor Session identity and Lease generation. The daemon checks holder, generation, and expiry inside the same immediate SQLite transaction as the mutation, so an old holder or an old generation cannot commit.

A read-only tab can request takeover. The holder sees the request and may approve or decline. Approval transfers authority immediately. Otherwise the requester may claim after the default 10-second grace or after Lease expiry. The holder may release voluntarily. Test-only environment overrides are:

- `WORKFLOWD_DRAFT_LEASE_TTL_SECONDS`;
- `WORKFLOWD_DRAFT_TAKEOVER_GRACE_SECONDS`;
- `WORKFLOWD_DRAFT_SNAPSHOT_INTERVAL`;
- `WORKFLOWD_DRAFT_HISTORY_LIMIT`;
- `WORKFLOWD_DRAFT_UNDO_LIMIT`.

## Append-only undo and bounded materialization

A normal accepted command stores its semantic forward and inverse operation in a bounded durable undo stack. Undo and redo each append a new command receipt and history event, advance Draft Version, and apply the semantic inverse/forward operation. They never delete or rewrite an accepted receipt.

The defaults preserve 32 undoable operations, 64 detailed history events, a snapshot every 16 accepted commands, and the latest four materialized snapshots. The normalized current Draft and bounded undo/redo stacks are independent of compacted detail, so compaction cannot change current state or the advertised undo horizon.

## Encrypted browser Recovery Copy

Before sending a command, the editor writes the command as AES-256-GCM ciphertext to IndexedDB. The key is generated non-extractable by WebCrypto and stored as a structured-cloned `CryptoKey`; IndexedDB records retain only routing/expiry metadata, IV, and ciphertext. MDN documents both serializable `CryptoKey` storage and IndexedDB's structured-clone model:

- <https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto#storing_keys>
- <https://developer.mozilla.org/en-US/docs/Web/API/IndexedDB_API>

The expiry cannot exceed the authenticated Owner session. Acknowledgement deletes the copy. Logout clears all copies and the key even if the logout request itself fails; an expiry timer and the next editor startup also purge them. `beforeunload` warns while work is saving or unacknowledged. Visible states are `saving`, `saved`, `offline`, and `conflict`.

This encryption is defense against plaintext-at-rest disclosure and casual browser-storage inspection. It is not a defense against compromised same-origin JavaScript, an unlocked browser profile, or endpoint compromise: such code can ask the non-extractable key to decrypt. CSP/XSS defenses remain mandatory.

`sessionStorage` preserves a tab identity across reload, while a `BroadcastChannel` detects copied identities and regenerates a collision. MDN notes that session storage is tab-partitioned but may initially be copied from an opener: <https://developer.mozilla.org/en-US/docs/Web/API/Window/sessionStorage>.

## Reconciliation

`POST /api/v1/workflows/{id}/recovery/reconcile` has exactly three outcomes:

1. `already_acknowledged`: the daemon already has the durable command receipt, so the local copy can be deleted;
2. `auto_replayed`: base Draft Version, live holder identity, and Lease generation all match exactly;
3. `conflict_fork`: any version or authority change stores an explicit durable fork and semantic diff and returns HTTP 409.

A Lease holder inspects open forks and explicitly applies selected recovered work as a new semantic command against the current exact Draft Version. No path silently merges, drops, or publishes browser storage.

## Browser acceptance

The development-only Playwright test opens two real Chromium pages in one browser context, matching Playwright's documented multi-page model: <https://playwright.dev/docs/pages#multiple-pages>. On a fresh Ubuntu build host, provision it once with `make browser-test-deps`; normal `make test` then reuses the cached browser. It proves separate tab identities, read-only enforcement, takeover, offline ciphertext, reload warning, stale recovery fork/diff, explicit loss-free application, undo/reload/redo, and logout/expiry cleanup. Playwright and browser binaries are build/test tools only; neither is included in the production bundle.
