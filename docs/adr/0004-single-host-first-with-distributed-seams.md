---
status: accepted
---
# Start as a single-host system with distributed seams

The first deployment is a low-memory single-host system for the current 2-vCPU VPS, using one Rust daemon, SQLite WAL, a content-addressed local Artifact store, and systemd. Storage, queueing, and Artifact interfaces must admit future Postgres, NATS, and S3-compatible adapters without changing Workflow definitions.
