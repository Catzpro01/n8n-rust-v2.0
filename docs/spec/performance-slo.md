# Initial performance contract

## Memory SLO

The Rust daemon—engine, API, scheduler, and embedded SQLite, excluding the user's browser and optional external or compatibility workers—must complete one Run containing 100,000 lightweight Activations with:

- concurrency: 1;
- maximum input payload: 1 KiB per Activation;
- large or binary values represented as streamed Artifacts;
- peak resident set size: at most 500 MiB;
- no loss of durable Run state after a forced process restart.

This is a repeatable baseline, not a claim that arbitrary payloads, browser engines, LLMs, or external processes can fit within 500 MiB.

## Required evidence

The repository must contain a deterministic workflow generator, benchmark runner, peak-RSS capture, restart/fault-injection test, and a checked result for the deployment VPS.
