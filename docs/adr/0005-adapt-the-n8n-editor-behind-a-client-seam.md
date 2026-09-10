---
status: superseded by ADR-0050
---
# Adapt the n8n editor behind a client seam

Adapt the n8n 2.39.0 Vue/TypeScript editor to preserve mature canvas and configuration behavior, but route every backend interaction through one versioned client interface implemented by the Rust server. Rebuilding the browser UI in Rust/WASM would not reduce server RSS and would make feature parity substantially slower.
