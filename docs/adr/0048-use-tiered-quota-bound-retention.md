---
status: accepted
---
# Use tiered quota-bound retention

To operate inside 10 GiB, successful raw payloads default to 24 hours, successful metadata to 14 days, error and audit evidence to 90 days, and Artifacts to quota-aware LRU unless pinned, with compression and content addressing throughout. Workflows may override retention with a pre-Run storage estimate, while admission control and reserved recovery space prevent data growth from endangering the instance.
