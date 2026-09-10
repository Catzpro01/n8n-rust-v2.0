---
status: accepted
---
# Validate side-effect-free agent output contracts

Every AI Agent result must satisfy one Output Contract for text, JSON Schema, multipart stream, Artifact, or typed error before reaching downstream Connections. Bounded repair is permitted within Agent Policy, while delivery to Gmail, GitHub, databases, or other systems remains an explicit downstream node so parsing and hidden side effects cannot share retry semantics.
