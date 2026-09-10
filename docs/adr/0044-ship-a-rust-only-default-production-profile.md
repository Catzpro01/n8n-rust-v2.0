---
status: accepted
---
# Ship a Rust-only default production profile

Build and compatibility tooling may use Rust, Node.js, Python, and other toolchains, but the installed default production profile contains one Rust daemon, embedded SQLite, static editor assets, and optional signed WASM components; frontend JavaScript executes in the user's browser rather than as a server runtime. Non-Rust local workers are absent and stopped by default and may start only for an explicitly selected Node Form, preserving a measurable Rust-only steady state.
