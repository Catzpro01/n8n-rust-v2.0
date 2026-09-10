---
status: accepted
---
# Adapt execution to the effective resource envelope

The Resource Governor reads cgroup quotas and live CPU, memory, disk, and I/O pressure and adjusts execution without changing Workflow semantics. At 0.5 CPU it uses aggressive Execution Segment fusion, cooperative single-lane compute, batched persistence, bounded cache, spill, and remote placement to complete 100,000 lightweight Activations within the 500 MiB and 10 GiB profile at lower throughput; when two or more cores are available it increases parallel branches, runs, decoding, and worker permits while preserving fairness and durability.

The system aims for the fastest measured result inside each Resource Profile, not physically impossible equal CPU-bound performance across unequal compute quotas. I/O-bound workflows may perform similarly at 0.5 and two cores, while CPU-bound workflows must scale with available compute or explicit remote offload.
