---
status: superseded by ADR-0050
---
# Build a private hybrid n8n derivative

For personal self-hosted use, freeze behavioral and import compatibility against n8n 2.39.0, retain all required n8n copyright and license notices, and permit adapting the mature editor and public workflow contracts while replacing the execution architecture with a new Rust core. A clean-room rewrite would delay feature parity substantially, while a direct line-by-line port would preserve the constraints the new engine is intended to remove.
