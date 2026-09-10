# Initial performance contract

## Constrained-host SLO

The Rust daemon—engine, API, scheduler, and embedded SQLite, excluding the user's browser and optional external or compatibility workers—must complete one Run containing 100,000 lightweight Activations with:

- concurrency: 1;
- maximum input payload: 1 KiB per Activation;
- large or binary values represented as streamed Artifacts;
- peak resident set size: at most 500 MiB under a systemd MemoryMax of 500 MiB;
- sustained CPU quota: at most 50% of one logical core;
- managed persistent footprint: at most 10 GiB;
- no loss of durable Run state after a forced process restart.

This is a repeatable baseline for the always-resident Rust daemon and Inline Native Lane. Optional WASM, Node.js, CPython, browser, LLM, scraper, agent, or external-process workers have separate declared budgets and are not a loophole: the UI and Run record must display their measured peak usage. It is not a claim that arbitrary payloads or external tools can fit within 500 MiB.

## Required evidence

The repository must contain a deterministic workflow generator, benchmark runner, peak-RSS capture, restart/fault-injection test, and a checked result for the deployment VPS.

## Adaptive CPU evidence

The same correctness fixture runs under cgroup-enforced 0.5, 1, and 2 logical-core quotas. Results report wall time separately from CPU time and must show monotonic throughput scaling when the workload has parallel CPU work. Eco-profile completion is mandatory; equal speed to the two-core profile is not claimed for CPU-bound work unless optimization or remote placement removes equivalent local computation.
