# 12: Recover Eco 100K after an ungraceful daemon kill

**What to build:** A real process kill after speculative progress resumes the exact Eco Run from its latest Durable Checkpoint, performs bounded replay, avoids duplicate committed output, and converges on the uninterrupted digest.

**Blocked by:** 11: Summarize the exact Eco 100K Run

**Status:** ready-for-agent

- [ ] The acceptance harness observes speculative progress ahead of a known durable checkpoint and sends an ungraceful process kill, not graceful shutdown.
- [ ] Systemd restarts the daemon and startup loads the same revision, pinned plan, checkpoint, Artifact references, digest state, and logical resume cursor.
- [ ] Incomplete post-checkpoint work is discarded/replayed within the declared bound; no partial checkpoint rows become durable.
- [ ] Committed outputs, counters, trace sequence, and Artifact references are not duplicated.
- [ ] Recovered execution completes exactly 100,000 logical Activations with the same branch counts and digest as an uninterrupted control run.
- [ ] UI/SSE visibly transitions through disconnected/recovering/replaying/running/terminal states and reports replay window.
- [ ] Multiple injected kill points around checkpoint and Artifact boundaries are covered through installed-process tests.
- [ ] SQLite WAL/FULL, commit count, restart time, replay work, trace volume, disk I/O, and correctness evidence are captured.
- [ ] Recovery target under ordinary daemon crash is measured against the accepted under-one-minute objective.
