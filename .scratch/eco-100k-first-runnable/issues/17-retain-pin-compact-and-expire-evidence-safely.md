# 17: Retain, pin, compact, and expire evidence safely

**What to build:** The Owner can see storage estimates, retain recent full evidence, pin important evidence, compact/expire eligible terminal data, and preserve Recovery Reserve without deleting active or required state.

**Blocked by:** 13: Govern bounded work and scale across cgroup CPU profiles

**Status:** ready-for-agent

- [ ] The accepted tiered defaults for successful raw payloads, successful metadata, error/audit evidence, and quota-aware Artifacts are represented in visible configurable Retention Profiles.
- [ ] Pre-Run storage estimates and current live/trace/Artifact/recovery-reserve usage are visible.
- [ ] Evidence Pins prevent selected Causal Trace and Artifact roots from compaction, expiry, or ordinary quota GC.
- [ ] Eligible terminal Run compaction retains revision/plan identity, terminal state, digest, checkpoint/hash-chain evidence, counters, failures/retries, and required Artifact roots.
- [ ] Artifact GC removes only verified unreferenced/unpinned objects after safety age and never a committed live reference.
- [ ] Approaching Recovery Reserve triggers safe compaction/expiry/orphan cleanup and then admission reduction/suspension before consuming protected bytes.
- [ ] Active/suspended Runs, Recovery Sets, Current/Previous releases, and pinned evidence are never silently deleted.
- [ ] Retention/compaction operations are bounded, resumable after restart, and represented in trace/audit evidence.
- [ ] Disk-nearly-full tests verify SQLite/checkpoint/control-plane health and no data corruption.
