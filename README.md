# Canopy Workbench

Canopy Workbench is an independently implemented, self-hosted workflow automation platform with a Rust execution core and documented compatibility seams. The name and geometric editor mark are placeholders for the first runnable.

The current production slice provides one Rust daemon with embedded TypeScript/Preact editor assets, durable bundled SQLite startup, versioned health/release/resource APIs, direct HTTP or HTTPS, and a hardened native systemd package. It can author and sign an immutable Manual Trigger revision, durably admit its exact pinned plan, execute one deterministic Activation, expose reconnectable progress, cancel cooperatively, and verify a checkpointed Causal Trace. Further workflow behavior arrives through the dependency-ordered tickets under `.scratch/eco-100k-first-runnable/`.

## Builder quick start

The build is pinned to Rust 1.85.1 and Node.js 22.19.0.

```bash
make editor
make test
./scripts/build-release.sh
python3 -m unittest tests/acceptance/test_release_bundle.py
```

For Owner bootstrap and recovery-key handling, see [`docs/operations/owner-bootstrap.md`](docs/operations/owner-bootstrap.md). The first public Run, SSE, cancellation, checkpoint, queue, and Causal Trace contract is documented in [`docs/run-durability.md`](docs/run-durability.md). For native installation, resource limits, HTTPS configuration, and state-preserving uninstall, see [`docs/operations/install-systemd.md`](docs/operations/install-systemd.md).

The mandatory clean-room policy is in [`docs/legal/clean-room-policy.md`](docs/legal/clean-room-policy.md). Do not copy n8n source, tests, Enterprise files, UI assets, icons, product copy, or distinctive trade dress.
