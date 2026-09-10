# Adaptive Resource Governor

## Inputs

The governor observes cgroup v2 CPU quota and throttling, effective parallelism, memory current/high/max, swap policy, filesystem free and reserved bytes, I/O latency, pressure stall information, queue depth, deadlines, lane budgets, and remote-worker health and cost.

## Controls

It adjusts admission, weighted-fair permits, compute parallelism, I/O concurrency, batch and group-commit size, Execution Segment fusion, cache size, Artifact spill thresholds, prefetch, compression level, trace detail within policy, local-versus-remote placement, and worker idle shutdown.

It never changes side-effect ordering, idempotency requirements, Capability Grants, Output Contracts, or Published Revision semantics to gain speed.

## Profiles

### Eco

- CPU quota: 0.5 logical core sustained.
- Memory limit: 500 MiB.
- Managed disk: 10 GiB.
- Goal: complete the 100,000-lightweight-Activation benchmark without OOM, disk exhaustion, state loss, or starvation; duration is measured and optimized, not pre-claimed.

### Standard comparison

- CPU quota: 2 logical cores.
- Memory and disk held equal for comparative tests unless a test states otherwise.
- Goal: exploit independent graph branches and Runs with monotonic throughput improvement over Eco while preserving interactive latency and deterministic observable results.

## Fast path

The compiler may constant-fold, remove unreachable work, memoize eligible pure results, and combine compatible logical nodes into Execution Segments. Canvas, Causal Trace, per-node errors, breakpoints, pinned data, and replay remain logically per node. Any optimization that changes observable behavior fails conformance.

## Acceptance evidence

Benchmarks run under cgroup-enforced 0.5, 1, and 2 CPU quotas and publish wall time, CPU time, throttled time, peak RSS, bytes read and written, database commits, trace volume, and correctness hashes. I/O-bound and CPU-bound suites are reported separately so remote waiting cannot disguise compute cost.
