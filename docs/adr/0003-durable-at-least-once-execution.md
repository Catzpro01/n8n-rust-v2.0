---
status: accepted
---
# Use durable at-least-once execution semantics

Runs use durable at-least-once scheduling with checkpoints, idempotency keys, deduplication, classified retries, timeouts, circuit breakers, and optional compensation workflows. Claiming universal exactly-once behavior would be false for arbitrary external side effects; exactly-once state transitions may still be provided inside storage adapters that support them.
