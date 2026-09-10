# Default retention under the 10 GiB profile

- Successful raw payloads: 24 hours.
- Successful metadata and compact Causal Traces: 14 days.
- Error traces and security audit evidence: 90 days.
- Artifacts: content-addressed, compressed, quota-aware LRU unless pinned.
- Local recovery: reserved space and rotation independent from ordinary Artifact eviction.

Per-workflow overrides require a storage estimate. Admission control reserves enough space to checkpoint and recover a Run before accepting it.
