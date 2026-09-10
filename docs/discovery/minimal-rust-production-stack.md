# Research: minimal Rust production stack

Date: 2026-09-11

## Question

Which maintained Rust libraries and deployment techniques best satisfy one-binary HTTPS/API/static serving, async execution, SQLite durability, content-addressed storage, structured tracing, cryptography, cgroup observation, and minimal binary/RSS constraints?

## Constraints already decided

This research does not reopen the accepted architecture: one always-resident Rust daemon, embedded SQLite in WAL mode, prebuilt static editor assets, local content-addressed Artifacts, optional non-Rust workers absent from the default installation, cgroup-aware adaptation, 500 MiB `MemoryMax`, sustained 0.5 CPU, and a 10 GiB managed footprint.

## Recommendation

Use a deliberately narrow Tokio/Axum/Rusqlite stack with explicit feature flags and no ORM, general plugin host, OpenTelemetry collector, dynamic asset directory, or unbounded executor in the first production slice.

| Concern | First-release choice | Why it fits |
|---|---|---|
| Async runtime | Tokio 1.x, manually constructed runtime | Maintained with Axum; runtime builder exposes current-thread/multi-thread and thread-count controls. Eco can use one core thread while larger profiles scale permits without semantic changes. [S1] |
| HTTP/API | Axum 0.8.x + Tower/Tower HTTP 0.6.x | Macro-free routing, typed extraction, predictable errors, and composable timeout, request-ID, tracing, limit, and compression middleware. Axum is intentionally built on Tokio, Hyper, and Tower. [S2][S3] |
| Direct HTTPS | rustls 0.23.x through one Axum-compatible server adapter; exactly one crypto provider | TLS 1.2/1.3 without an OpenSSL runtime. Rustls permits choosing `aws-lc-rs` or `ring`; include only the measured winner, not both. [S4] |
| Static editor | `rust-embed` 8.x or a generated `include_bytes!` table; precompressed, content-hashed files | Keeps editor files inside the daemon and toolchains outside production. `rust-embed` supports per-file compression and deterministic timestamps; a generated table remains the lower-dependency fallback. [S5] |
| JSON/protocol | Serde + `serde_json`, `bytes`, `http` | Mature, narrow protocol surface already aligned with Axum. Keep durable records versioned; do not serialize internal Rust layouts as contracts. |
| SQLite | Rusqlite 0.40.x with `bundled`, `backup`, `hooks`, and `limits`; no ORM | Direct transaction ownership, bundled/pinned SQLite, online backup, hooks, and runtime limits without an async ORM or pool. Current Rusqlite documentation identifies these capabilities and the bundled build. [S6] |
| Artifact identity | BLAKE3, incremental single-threaded default | Streaming hash API and fixed 32-byte digest. Rayon/mmap are opt-in and should remain disabled in Eco unless measurements justify them. [S7] |
| Structured diagnostics | `tracing` + `tracing-subscriber` JSON formatting | Context-aware spans/events and runtime filtering without making an OpenTelemetry SDK resident. [S8] |
| Password hashing | RustCrypto `argon2`, Argon2id PHC strings | Memory-hard password hashing with parameters carried in the stored PHC string; tune against the 500 MiB envelope. [S9] |
| Vault AEAD | RustCrypto `chacha20poly1305` stable release, XChaCha20-Poly1305 mode, plus `secrecy`/`zeroize` | Authenticated encryption with an extended nonce option; secret wrappers make exposure explicit and zeroize on drop. [S10][S11] |
| Revision signatures | `ed25519-dalek`, strict verification | Compact signing/verifying keys and strict verification support for immutable Published Revisions. [S12] |
| Resource observation | Small in-repo cgroup v2 and PSI parser | Kernel files are the authoritative API. Direct keyed parsing avoids a broad platform crate and permits explicit “controller unavailable” states. [S13] |
| Service manager | systemd system service and optional socket unit | cgroup controls, restart policy, state directories, hardening, accounting, socket activation, and atomic version switching without a container runtime. [S14][S15] |

Versions above are the maintained lines observed on the research date, not floating production requirements. Production uses an audited `Cargo.lock`, exact checksums, an SBOM, and a reproducible release manifest.

## Runtime shape

### Tokio

Construct the runtime rather than using a default `#[tokio::main]` profile:

- Eco: one Tokio core worker; bounded scheduler permits; low fixed blocking-thread cap.
- Larger effective CPU envelopes: increase core workers and independent-run/branch permits from the Resource Governor.
- SQLite does not run on Tokio core workers. A dedicated database writer owns the write connection and receives bounded commands.
- CPU-bound Node execution uses the governed execution scheduler, not unconstrained `spawn_blocking`. Tokio documents that long non-yielding work blocks async progress and that blocking-thread limits are configurable. [S1]

This separates I/O concurrency from workflow compute concurrency and preserves behavior when core count changes.

### HTTP and progress transport

Use one Axum router for versioned API routes, owner sessions, static editor assets, health/readiness, and metrics. First-slice Run progress is server-sent events or bounded polling because it is server-to-browser and reconnectable; WebSockets are deferred until a bidirectional requirement exists.

Enable only required Tower HTTP features: request ID propagation, trace, timeout, body limit, sensitive-header marking, and precompressed response serving. Do not enable `full`. CORS defaults to same-origin; public workflow ingress is a separate route tree and policy seam.

For HTTPS, load a provisioned certificate/key and use rustls directly through a maintained Axum server adapter. Automated ACME is optional packaging work, not a prerequisite for the daemon. Prefer systemd socket activation for privileged port 443; otherwise grant only `CAP_NET_BIND_SERVICE`. The daemon must also support an explicitly configured loopback/private bind behind an owner-managed proxy.

### Embedded editor

The frontend build emits content-hashed JavaScript/CSS and precompressed Brotli/gzip variants plus a generated manifest. Embed those outputs in release builds. Serve:

- immutable one-year cache headers for content-hashed assets;
- a short/no-cache shell for `index.html`;
- strong ETags from asset hashes;
- correct MIME and `Content-Encoding` chosen from `Accept-Encoding`;
- SPA fallback only for editor routes, never API or webhook routes.

Do not decompress a solid asset archive into RAM at startup. Per-file embedded bytes remain demand-paged and support range-independent responses.

## SQLite transaction and thread policy

Rusqlite is synchronous by design. Use one writer thread and a very small, explicitly capped read-connection set rather than placing a generic async pool in the daemon.

Required startup checks:

1. Verify the linked SQLite runtime version and compile options. The current bundled Rusqlite line includes a SQLite release newer than the WAL-reset fixes, but the daemon must assert an approved version rather than infer safety from the crate version. [S16]
2. Set and read back `journal_mode=WAL`, `synchronous=FULL`, `foreign_keys=ON`, `busy_timeout`, and configured limits on every relevant connection. SQLite silently ignores unknown pragmas, so read-back is mandatory. [S17]
3. Fail closed if durable settings differ.
4. Keep acknowledged checkpoint state and retained evidence in one transaction.
5. Group commits with bounded item-count and elapsed-time thresholds from the Resource Governor, carrying forward the Eco prototype result.
6. Schedule WAL checkpoints and online backups deliberately; WAL adds a checkpoint operation and companion `-wal`/`-shm` files. [S18]

`NORMAL` is not the default: SQLite documents that in WAL mode it may lose durability after power loss, while `FULL` adds a WAL sync after each transaction commit. [S17]

## Content-addressed Artifact store

Use the standard filesystem plus BLAKE3; a database row is metadata, not the payload body.

1. Stream bytes into a same-filesystem `.partial` file while incrementally hashing.
2. Enforce admission quota before and during the stream.
3. Flush and `fsync` the file; atomically rename to `artifacts/blake3/aa/<remaining-digest>`.
4. `fsync` the containing directory when a durable acknowledgement depends on the new name.
5. Insert/reference the digest and size transactionally in SQLite.
6. Deduplicate by digest, verify length/hash on reads selected for integrity sampling, and garbage-collect only unreferenced/unpinned objects.

Do not enable BLAKE3 Rayon by default; the official docs warn that parallel update can be slower for inputs below roughly 128 KiB and must be benchmarked. [S7]

## Cryptography boundaries

- rustls protects transport.
- Argon2id protects the Owner password. Parameters are calibrated at installation and can be upgraded on successful login.
- XChaCha20-Poly1305 protects envelope-encryption keys and credential payloads with random nonces and domain-specific associated data.
- Ed25519 signs canonical Published Revision bytes with domain separation.
- BLAKE3 identifies non-secret content; it is not substituted for password hashing or AEAD.
- `secrecy` and `zeroize` reduce accidental logging/copies but do not claim perfect memory erasure across kernels, allocators, crash dumps, or hardware.

Keep algorithms and key versions in ciphertext/signature envelopes. Never invent a combined “crypto helper” that hides nonce, associated-data, or domain-separation rules.

## Observability and cgroup adaptation

Emit `tracing` spans with stable IDs for request, Workflow, Published Revision, Run, logical Activation/segment, checkpoint, Artifact, and failure class. Default production output is bounded JSON to journald/stdout; payloads, credentials, cookies, authorization headers, and private model reasoning are never fields.

Read cgroup v2 by discovering the process path from `/proc/self/cgroup`, then parsing files by key rather than line position:

- `cpu.max`, `cpu.stat` (`usage_usec`, periods, throttled count/time);
- `memory.current`, `memory.peak`, `memory.events`, `memory.stat`;
- `io.stat` when the I/O controller is enabled;
- `pids.current`/`pids.max`;
- CPU, memory, and I/O pressure files when available.

The kernel specifies that cgroup statistics are keyed and may gain fields; the parser must tolerate unknown fields. Missing delegated controllers are reported as unavailable, not zero. [S13]

## Production bundle and systemd

The installed bundle contains only:

- one stripped Rust daemon with embedded SQLite and editor assets;
- configuration/schema examples and notices/SBOM;
- systemd service/socket units;
- migration and health metadata consumed by the same binary.

No Rust, Node.js, Python, package manager, source tree, Cargo cache, or frontend build output directory remains installed.

Start as an unprivileged dedicated identity using `StateDirectory=` and `RuntimeDirectory=`. Candidate hardening for the first Rust-only slice:

- `NoNewPrivileges=yes`, `PrivateTmp=yes`, `PrivateDevices=yes`;
- `ProtectSystem=strict`, `ProtectHome=yes`, explicit state write path;
- `ProtectKernelTunables=yes`, `ProtectKernelModules=yes`, `ProtectControlGroups=yes`;
- `RestrictSUIDSGID=yes`, `LockPersonality=yes`, `RestrictRealtime=yes`;
- restricted address families;
- `MemoryHigh` below `MemoryMax=500M`, `MemorySwapMax=0`, `CPUQuota=50%`, bounded `TasksMax`, and CPU/memory/I/O accounting.

`MemoryDenyWriteExecute=yes` is suitable for the Rust-only first slice but must not be silently inherited by a future JIT-based WASM worker. systemd documents both filesystem sandboxing and the limitations of those controls; validate the effective unit with `systemd-analyze security` and integration tests. [S14]

## Release profile and dependency policy

Begin with a speed-oriented measured profile because 0.5 CPU is the harder resource than the 10 GiB disk budget:

```toml
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
panic = "abort"
strip = "symbols"
overflow-checks = true
```

Then benchmark `opt-level=2`, `3`, and `s`; Cargo explicitly warns that size levels are not always smaller or faster. Keep a separate symbol artifact keyed by build ID outside the production bundle for crash diagnosis. Cargo documents `opt-level`, LTO, codegen units, panic strategy, and stripping as profile controls. [S19]

Every optional crate feature needs a measured owner. CI records daemon bytes, stripped bytes, cold/warm RSS, startup, idle CPU, dependency count, licenses, advisories, and the cgroup benchmark. Reject a dependency if a small in-repo adapter is safer and meaningfully smaller.

## Explicitly deferred or rejected for the first slice

- SQLx/SeaORM/Diesel: unnecessary pool/ORM abstraction around a single-writer SQLite design.
- OpenTelemetry SDK/exporter in the default binary: structured local traces come first; OTLP can be a measured feature later.
- Redis, Postgres, NATS, S3, and a container runtime: interfaces admit them later, but they are not installed.
- Runtime Node.js/Python, headless browsers, and local heavy workers: remote-first and absent by default.
- Dynamic native plugins in-process: conflicts with the clean initial blast radius.
- Enabling entire `tokio`, `tower-http`, or Axum feature sets: select only used features.
- Automatic ACME and WebSocket dependencies before a concrete first-slice requirement.
- WebGL/WASM editor runtime in the server: browser rendering is static client code; Canvas 2D won the current prototype.

## Validation gates for the dependent stack decision

Before the production stack is locked, build a tracer daemon with the candidate dependency features and report:

1. exact lockfile, licenses, advisories, SBOM, and source checksums;
2. stripped binary size and clean installed footprint;
3. cold/steady RSS and idle CPU inside the Eco cgroup;
4. HTTPS/API/static/SSE behavior and graceful restart;
5. SQLite FULL crash recovery, online backup/restore, disk-full, nearly-full, and WAL growth behavior;
6. Artifact atomicity and hash verification across injected crashes;
7. cgroup metrics with missing-controller cases;
8. systemd hardening score plus required exceptions;
9. 100,000-Activation benchmark with the same correctness digest used by the prototype.

## Primary sources

- **S1 — Tokio runtime and builder:** https://docs.rs/tokio/1.53.1/tokio/runtime/struct.Builder.html and https://docs.rs/tokio/1.53.1/tokio/
- **S2 — Axum 0.8.9:** https://docs.rs/axum/0.8.9/axum/
- **S3 — Tower HTTP feature inventory:** https://docs.rs/crate/tower-http/0.6.11/features
- **S4 — rustls 0.23.44 and crypto providers:** https://docs.rs/rustls/0.23.44/rustls/
- **S5 — rust-embed 8.12.0:** https://docs.rs/crate/rust-embed/8.12.0
- **S6 — Rusqlite 0.40.2:** https://docs.rs/rusqlite/0.40.2/rusqlite/ and https://docs.rs/crate/libsqlite3-sys/0.38.2
- **S7 — BLAKE3 incremental/parallel behavior:** https://docs.rs/blake3/latest/blake3/struct.Hasher.html
- **S8 — tracing-subscriber structured formatting:** https://docs.rs/tracing-subscriber/0.3.23/tracing_subscriber/fmt/
- **S9 — RustCrypto Argon2:** https://docs.rs/argon2/latest/argon2/
- **S10 — RustCrypto ChaCha20Poly1305:** https://docs.rs/crate/chacha20poly1305/0.10.1
- **S11 — secrecy SecretBox:** https://docs.rs/secrecy/latest/secrecy/struct.SecretBox.html
- **S12 — ed25519-dalek strict verification:** https://docs.rs/ed25519-dalek/latest/ed25519_dalek/struct.VerifyingKey.html
- **S13 — Linux kernel cgroup v2 interface:** https://docs.kernel.org/admin-guide/cgroup-v2.html
- **S14 — systemd execution sandboxing:** https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html
- **S15 — systemd resource controls:** https://www.freedesktop.org/software/systemd/man/latest/systemd.resource-control.html
- **S16 — SQLite WAL-reset bug and fixed versions:** https://www.sqlite.org/wal.html#the_wal_reset_bug
- **S17 — SQLite synchronous pragma:** https://www.sqlite.org/pragma.html#pragma_synchronous
- **S18 — SQLite WAL operation/checkpointing:** https://www.sqlite.org/wal.html
- **S19 — Cargo profile settings:** https://doc.rust-lang.org/cargo/reference/profiles.html
