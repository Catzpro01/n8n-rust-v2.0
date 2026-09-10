# 01: Boot the production-shaped daemon and editor shell

**What to build:** A clean build installs and starts one production-shaped Rust daemon that opens durable SQLite safely, serves an embedded original editor shell, reports release/health/resource identity, and runs through the default systemd path inside the idle Eco envelope.

**Blocked by:** None (can start immediately)

**Status:** claimed

- [ ] A locked Rust workspace and build-only TypeScript/Preact editor pipeline produce one stripped daemon with embedded content-hashed editor assets.
- [ ] An installed daemon serves the editor shell, versioned capability/release identity, liveness, and readiness through externally tested HTTPS/HTTP surfaces.
- [ ] Startup creates or opens SQLite, asserts an approved runtime, sets and reads back WAL, FULL synchronous, foreign keys, busy timeout, and configured limits, and fails closed on mismatch.
- [ ] The Eco runtime starts with one Tokio core worker, a bounded blocking policy, and direct cgroup-v2/resource discovery that reports unavailable controllers honestly.
- [ ] A hardened systemd unit runs the daemon as an unprivileged dedicated identity with managed state/runtime directories and the accepted 500 MiB, 0.5 CPU, no-swap, and task limits.
- [ ] The release output contains no frontend source tree, Node.js/Python/Rust toolchain, package manager, or development cache.
- [ ] The editor uses original placeholder identity/assets and no n8n source, copy, icons, or distinctive trade dress.
- [ ] A clean-install smoke test drives service start, readiness, editor load, release identity, restart, and clean uninstall-with-state-preservation behavior.
- [ ] Dependency licenses, initial SBOM data, and locked checksums are generated for the tracer bundle.
