# Run observability and replay

Every Run produces a Causal Trace relating trigger receipt, revision, plan, Activations, retries, suspensions, Envelopes, Artifacts, policy decisions, Secret Lease identifiers, resource usage, external calls, output validation, and errors. Sensitive values are redacted before persistence.

Replay starts from a selected Activation using recorded references or user-provided fixtures. External writes are mocked by default; an approved live replay receives new idempotency and audit context so it cannot masquerade as the original attempt.
