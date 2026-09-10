# AI Agent Node

## Typed subports

- Engine: exactly one primary Agent Engine and an explicit ordered fallback policy.
- Memory: zero or more ordered Memory Layers.
- Skill: zero or more locked Skill Sets.
- MCP/Tool: zero or more tools governed by local policy.
- Policy: exactly one Agent Policy, with a secure default supplied when omitted.
- Output: exactly one Output Contract.

## Engine adapters

The common interface admits built-in planning/execution, model-provider adapters, local inference, MCP, A2A, and isolated CLI/process agents. Adapter differences remain visible in capabilities, lifecycle, session handling, cost, and trace records.

## Safety invariants

No engine, memory layer, skill, or tool receives a credential merely because it is connected. Capability Grants and Secret Leases are explicit. Output validation has no external side effect. Budget exhaustion produces a typed resumable state rather than an unbounded loop.
