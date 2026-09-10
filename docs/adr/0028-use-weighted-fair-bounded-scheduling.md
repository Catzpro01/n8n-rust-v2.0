---
status: accepted
---
# Use weighted-fair bounded scheduling

Scheduling uses priorities and deadlines with quotas per Workflow and Execution Lane, bounded queues, spill-to-disk, backpressure, destination and credential rate limits, and admission control. This prevents a giant Run from starving interactive webhooks or smaller workflows and avoids treating available memory as an unbounded queue.
