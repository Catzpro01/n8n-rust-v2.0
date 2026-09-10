# Research the minimal Rust production stack

Type: research
Status: resolved
Blocked by: none

## Question

Which maintained Rust libraries and deployment techniques best satisfy one-binary HTTPS/API/static serving, async execution, SQLite durability, content-addressed storage, structured tracing, cryptography, cgroup observation, and minimal binary/RSS constraints?


## Answer

Use a narrow, feature-gated Tokio/Axum/Rusqlite stack rather than an ORM, general plugin host, or resident observability platform. The cited primary-source research is [Research: minimal Rust production stack](../../../docs/discovery/minimal-rust-production-stack.md).

The recommended first-release lines are Tokio with an explicitly sized runtime; Axum/Tower HTTP; rustls with exactly one measured crypto provider; embedded prebuilt assets; Serde JSON; Rusqlite with bundled/pinned SQLite plus backup/hooks/limits; BLAKE3 Artifacts; `tracing` JSON; RustCrypto Argon2id, XChaCha20-Poly1305, `secrecy`/`zeroize`, and Ed25519; direct cgroup v2/PSI parsing; and a hardened systemd system service.

The Eco runtime has one Tokio core worker, bounded permits, a low blocking-thread ceiling, and a dedicated SQLite writer thread. It must read back WAL/FULL and other critical pragmas, assert an approved SQLite version, and retain the bounded group-checkpoint design proven by the Eco prototype. Missing cgroup controllers are reported as unavailable, not zero.

The installed target contains one stripped daemon with embedded SQLite/editor assets, config/notices/SBOM, and systemd units—no Rust, Node.js, Python, source tree, package manager, or build cache. Exact crate features and TLS provider remain subject to a tracer-daemon gate measuring binary size, RSS, startup, advisories, HTTPS/static/SSE behavior, SQLite crash/backup/disk-full behavior, Artifact atomicity, hardening, and the cgroup benchmark.
