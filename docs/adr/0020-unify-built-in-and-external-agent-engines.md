---
status: accepted
---
# Unify built-in and external agent engines

One Agent Engine interface covers the built-in engine, OpenAI-compatible and local models, MCP or A2A remote agents, and isolated CLI/process adapters such as Hermes, OpenClaw, OpenCode, and Claude. One engine handles a turn and fallback activates only on classified failure, keeping lifecycle, tool authority, cost, and output behavior observable across otherwise different harnesses.
