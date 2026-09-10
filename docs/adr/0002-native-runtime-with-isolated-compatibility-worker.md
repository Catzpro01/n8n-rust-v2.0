---
status: accepted
---
# Prefer native nodes and isolate compatibility nodes

Rust-native nodes are the primary execution path, while existing TypeScript n8n nodes may run through an optional isolated compatibility worker that starts only when required. This preserves migration and feature coverage without coupling the core scheduler or its memory target to Node.js, at the cost of maintaining a clearly versioned compatibility seam.
