# 13: Govern bounded work and scale across cgroup CPU profiles

**What to build:** The Resource Governor completes Eco 100K at 0.5 CPU, uses more cores safely, bounds admission/queues, treats multiple Runs fairly, and slows or suspends visibly before resource failure.

**Blocked by:** 12: Recover Eco 100K after an ungraceful daemon kill

**Status:** ready-for-agent

- [ ] Direct cgroup-v2 parsing reports CPU quota/usage/throttling, memory current/peak/events, I/O/pids/pressure when available and explicit unavailable states otherwise.
- [ ] The installed Eco test enforces 50% CPU, 500 MiB MemoryMax, no swap, task limits, and the managed-disk policy and still completes correctly.
- [ ] Ready/executor/storage/event queues enforce count and byte bounds; overload rejects external ingress before durable admission with retry guidance.
- [ ] Multiple admitted Runs demonstrate weighted fairness and that one large Run cannot starve a small interactive Run.
- [ ] Memory, I/O, queue, and disk pressure reduce safe concurrency/cache/batch choices, apply backpressure/spill, and enter Durable Suspension before hard failure.
- [ ] A reserved control-plane budget keeps status, health, trace, and cancellation responsive under pressure.
- [ ] Additional CPU profiles automatically increase eligible concurrency and reduce duration when work permits without changing plan, counts, Logical Order, or digest.
- [ ] The benchmark manifest records wall/CPU/throttle/RSS/memory/disk-I/O/commit/trace/queue/suspension/correctness evidence.
- [ ] Advanced Accelerator Policy is Off by default; Safe Auto can report no eligible accelerator cleanly without probing/starting heavy GPU runtimes in the default bundle.
