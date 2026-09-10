---
status: accepted
---
# Orchestrate HTTP through policy-governed adapters

Keep ordinary HTTP requests on a lightweight native Rust adapter and make HTTP Orchestrator an explicit meta-node that selects native, compatibility, proxy, browser-backed, cache, or fallback adapters under policy. Adapter changes require classified failures and preserve idempotency, cost, latency, robots constraints, capability checks, and trace evidence; non-idempotent writes cannot migrate or retry without an idempotency key or approval.
