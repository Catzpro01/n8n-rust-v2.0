# Prototype the Eco execution kernel

Type: prototype
Status: resolved
Blocked by: 01

## Question

Can a throwaway Rust execution kernel compile and execute 100,000 lightweight logical Activations with durable batched SQLite evidence under cgroup limits of 500 MiB and 0.5 CPU, and which costs dominate wall time, RSS, and disk I/O?


## Answer

Yes. A deliberately throwaway Rust/rusqlite kernel on branch `prototype/eco-execution-kernel` at commit `594c557d77b480025413ce6239cfaea6ce4e6547` compiled and completed 100,000 deterministic logical Activations in transient cgroup v2 services. The prototype and raw primary evidence stay off `master`; its compact report is `prototypes/eco-execution-kernel/results/summary.md` on that commit.

### Validated result

- Eco full-evidence run (`CPUQuota=50%`, `MemoryMax=500M`, swap disabled, SQLite WAL and `synchronous=FULL`): 8.35 s wall, 334.3 ms cgroup CPU, no measured throttling, 6,400 KiB process peak RSS, 16,276 KiB cgroup memory peak, 4,874,240-byte database, 25,816 filesystem output blocks, 98 checkpoint commits, and exactly 100,000 trace rows.
- Correctness: 49,218 values followed route zero, 50,782 followed route one, and the deterministic output digest was `8223a1edda007787`.
- Crash recovery: the process aborted with exit 134 after computing Activation 50,123 inside the uncommitted batch beginning at 49,152. Restart read the durable sequence and digest at 49,152, replayed from there, and converged on exactly 100,000 trace rows and the same digest as the uninterrupted run. No partial-batch rows survived.
- Managed footprint is feasible at this scale: full per-Activation evidence occupied about 4.87 MB; summary-only evidence occupied 12,288 bytes. Both are far below 10 GiB, although production retention still needs explicit limits.

### Dominant cost and decision

SQLite durable commit/fsync cadence dominates this lightweight workload, not the transform loop, CPU quota, or resident memory. With otherwise equivalent FULL-durability evidence, 1,563 checkpoint commits at batch 64 took 99.02 s, 98 commits at batch 1,024 took 8.35 s, and 13 commits at batch 8,192 took 1.71 s. A summary-only run still took 5.62 s for 98 FULL commits despite using only 83 ms cgroup CPU and writing no trace rows. Raising the CPU quota from 50% to 200% did not materially improve the FULL run. `synchronous=NORMAL` reduced wall time but weakens power-loss durability and is rejected as the default.

Carry forward this production direction:

1. Keep SQLite WAL with FULL durability for acknowledged checkpoints.
2. Group checkpoint commits with bounded item-count and elapsed-time thresholds chosen by the Resource Governor; do not hard-code the prototype's batch size.
3. Atomically commit resume sequence, deterministic digest state, route counters, and retained trace evidence. On restart, discard incomplete work and replay only from the last committed sequence.
4. Bound the replay window and event retention separately. Larger groups improve throughput but enlarge crash redo and progress-reporting latency.
5. Treat logical per-Activation evidence as a retention/profile choice; it is affordable for this fixture but must not force unbounded production growth.
6. Preserve semantics across resource profiles. Parallelism can increase for independent work, but committed ordering and correctness hashes remain deterministic.

The measurements establish feasibility and identify the transaction boundary as the next engine/storage design seam. They do not validate the production scheduler, concurrent Runs, Artifact spilling, API, or editor, all of which remain later decisions.
