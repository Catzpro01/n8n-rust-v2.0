---
status: accepted
---
# Keep only Rust resident and select the cheapest execution lane

The always-resident daemon contains only the Rust control and native data planes. The compiler assigns each Activation to the cheapest valid Execution Lane: Inline Native, WASM Micro, Isolated Runtime, or Heavy Orchestrator; optional Node.js, CPython, browser, agent, and process workers start only when selected and stop after idle. This preserves broad language compatibility without charging simple workflows the memory and attack surface of every supported runtime.

The eight user-visible Node Forms are authoring and integration choices, not eight resident runtimes or an ordered security hierarchy. JavaScript and Python nodes may use a micro sandbox when their requested capabilities fit, otherwise they are promoted visibly to an isolated worker; no silent capability escalation is allowed.
