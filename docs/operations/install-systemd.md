# Native systemd installation

The first runnable uses one native `workflowd` service. Node.js, Python, Cargo, and Rust are build-host tools only and are not copied into the production bundle.

## Build on a trusted builder

The repository pins Rust in `rust-toolchain.toml`, Node in `editor/.nvmrc`, Cargo dependencies in `Cargo.lock`, and editor dependencies in `editor/package-lock.json`.

```bash
./scripts/build-release.sh
python3 -m unittest tests/acceptance/test_release_bundle.py
```

The result is `out/tracer-bundle/` plus `out/tracer-bundle.tar.gz`. The bundle includes one stripped daemon, a hardened systemd unit, release identity, dependency checksums, a CycloneDX SBOM, license inventory, and file checksums. Verify `checksums.sha256` before moving it to a target host.

## Install on the target

Run from a checkout containing the installer and unpacked bundle:

```bash
sudo ./scripts/install.sh --bundle out/tracer-bundle --bind 127.0.0.1:8787
curl --fail http://127.0.0.1:8787/health/ready
```

The installer verifies the bundle, creates the locked `workflowd` system user, installs the daemon and unit, and starts `workflowd.service`. The private editor binds to loopback by default. To terminate TLS directly in the daemon, add readable certificate/key paths as `WORKFLOWD_TLS_CERT` and `WORKFLOWD_TLS_KEY` in `/etc/workflowd/workflowd.env`; both values are required together.

Useful diagnostics:

```bash
systemctl status workflowd.service
journalctl -u workflowd.service
curl http://127.0.0.1:8787/api/v1/release
curl http://127.0.0.1:8787/api/v1/resources
```

The unit enforces `MemoryMax=500M`, `CPUQuota=50%`, `MemorySwapMax=0`, and `TasksMax=64`. The resource API reports the effective cgroup-v2 files rather than assuming those controllers are present.

## Restart and uninstall

```bash
sudo systemctl restart workflowd.service
sudo ./scripts/uninstall.sh
```

Uninstall removes executable, ordinary configuration, and service files but deliberately preserves `/var/lib/workflow-rust` and the root-owned `/etc/workflowd/master.key`. Back up or explicitly remove that directory only when the Owner intends to destroy state.

The destructive clean-install smoke test is intended only for an expendable test host because it removes any existing `/var/lib/workflow-rust` before installation:

```bash
./scripts/test-systemd-smoke.sh out/tracer-bundle
```
