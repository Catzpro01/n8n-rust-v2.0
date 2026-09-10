# 04: Recover and arbitrate Draft editing

**What to build:** Editing remains loss-aware across reloads, disconnections, and two browser sessions through durable undo/redo, a single Draft Lease, graceful takeover, and an encrypted Recovery Copy.

**Blocked by:** 03: Create the first Node Contract and durable Draft

**Status:** ready-for-agent

- [ ] Exactly one Editor Session can mutate a Mutable Draft; another session is read-only and sees holder/expiry state.
- [ ] Heartbeat, expiry, voluntary release, approved takeover, and grace-period takeover are externally exercised without simultaneous writers.
- [ ] Duplicate command IDs are idempotent and stale base Draft Versions return a structured conflict without partial mutation.
- [ ] Undo and redo survive daemon/browser reload and append semantic commands instead of rewriting history.
- [ ] Materialized Draft snapshots and bounded history compaction preserve current state and required undo behavior.
- [ ] Unacknowledged browser commands are retained in an encrypted expiring Recovery Copy and cleared by logout/expiry policy.
- [ ] Reconnect auto-replays only against the exact base Draft Version; changed authority/state creates an explicit recovery fork and diff.
- [ ] Closing or reloading with unacknowledged work displays an honest saving/offline/conflict state.
- [ ] A two-tab browser test covers edit, disconnect, takeover, stale return, fork, and loss-free reconciliation.
