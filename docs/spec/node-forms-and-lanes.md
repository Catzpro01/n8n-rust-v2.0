# Node Forms and Execution Lanes

## Eight Node Forms

1. JavaScript Script — source entered directly, without a build step.
2. Python Script — source entered directly, with environments managed by the system.
3. WASM Component — hot-loadable capability-based component.
4. Rust Native — locally built and trusted native implementation.
5. External Process — any language or executable behind an isolated process contract.
6. AI-Generated Node — description-to-contract, tests, implementation, and install workflow.
7. Sub-workflow Node — a Workflow Revision exposed as a reusable Node Definition.
8. MCP Tool Node — a discovered MCP tool exposed as a Node Definition.

## Lane selection

- Inline Native: built-in Rust nodes and compiled sub-workflow scheduling.
- WASM Micro: capability-limited WASM and suitable lightweight script implementations.
- Isolated Runtime: Node.js/npm, full CPython, custom native libraries, and arbitrary executables.
- Heavy Orchestrator: browser automation, scraping stacks, local models, and external agent harnesses.

Selection is automatic at compile time, visible before publication, and manually restrictable. A Node Form may map to more than one lane based on requested capabilities, but promotion to a more privileged lane requires explicit approval.
