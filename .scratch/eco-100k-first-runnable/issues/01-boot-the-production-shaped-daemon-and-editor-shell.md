# 01: Boot the production-shaped daemon and editor shell

**What to build:** A clean build installs and starts one production-shaped Rust daemon that opens durable SQLite safely, serves an embedded original editor shell, reports release/health/resource identity, and runs through the default systemd path inside the idle Eco envelope.

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] A locked Rust workspace and build-only TypeScript/Preact editor pipeline produce one stripped daemon with embedded content-hashed editor assets.
- [x] An installed daemon serves the editor shell, versioned capability/release identity, liveness, and readiness through externally tested HTTPS/HTTP surfaces.
- [x] Startup creates or opens SQLite, asserts an approved runtime, sets and reads back WAL, FULL synchronous, foreign keys, busy timeout, and configured limits, and fails closed on mismatch.
- [x] The Eco runtime starts with one Tokio core worker, a bounded blocking policy, and direct cgroup-v2/resource discovery that reports unavailable controllers honestly.
- [x] A hardened systemd unit runs the daemon as an unprivileged dedicated identity with managed state/runtime directories and the accepted 500 MiB, 0.5 CPU, no-swap, and task limits.
- [x] The release output contains no frontend source tree, Node.js/Python/Rust toolchain, package manager, or development cache.
- [x] The editor uses original placeholder identity/assets and no n8n source, copy, icons, or distinctive trade dress.
- [x] A clean-install smoke test drives service start, readiness, editor load, release identity, restart, and clean uninstall-with-state-preservation behavior.
- [x] Dependency licenses, initial SBOM data, and locked checksums are generated for the tracer bundle.


## Answer

Implemented one locked, production-shaped Rust daemon and an independently authored TypeScript/Preact editor shell. The daemon embeds content-hashed browser assets, owns bundled SQLite on a dedicated bounded worker, fails closed unless the approved SQLite version, compile options, WAL/FULL/foreign-key/busy-timeout/limit read-backs all match, and reports versioned health, release, capability, and cgroup-v2 resource identity over HTTP or direct rustls HTTPS.

The native bundle installs through a hardened systemd unit as the dedicated `workflowd` identity with managed state/runtime directories, `MemoryMax=500M`, `CPUQuota=50%`, `MemorySwapMax=0`, and `TasksMax=64`. The default uninstaller removes program files while preserving state.

Verification on the target VPS (2026-09-11):

- Rust formatting, locked check, Clippy with warnings denied, and all Cargo tests passed.
- Four external daemon tests passed against both debug and stripped release binaries, covering HTTP, HTTPS, durable SQLite policy/fail-closed startup, embedded assets, release/capability identity, and available/unavailable cgroup controllers.
- The runtime-only bundle inspection passed with one executable, no source/toolchain/package-manager/cache payload, valid locked checksums, CycloneDX SBOM, dependency checksum inventory, and complete declared dependency licenses.
- npm audit reported zero vulnerabilities.
- The destructive systemd smoke test passed install, service start, readiness, editor load, release identity, cgroup controls, restart, and uninstall with state preservation.
- Measured stripped daemon size: 5,390,112 bytes; idle RSS: 6,979,584 bytes; two-second idle CPU: 0.0000 cores; systemd hardening exposure: 1.6 (`OK`).

Review used fixed point `7b0e26e4f4e95aee14503a92023f61f006776a32`. Standards review found one ShellCheck SC2094 checksum-generation hazard; it was corrected by writing through a temporary file, after which ShellCheck and bundle verification passed. Spec review found no missing, incorrect, or out-of-scope behavior.
