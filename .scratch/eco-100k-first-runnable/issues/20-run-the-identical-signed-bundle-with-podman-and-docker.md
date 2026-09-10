# 20: Run the identical signed bundle with Podman and Docker

**What to build:** The same signed release binary and state/recovery format run through optional Podman and Docker packaging without feature, security, or compatibility drift from the default systemd installation.

**Blocked by:** 19: Activate signed Release Slots and roll back failed upgrades

**Status:** ready-for-agent

- [ ] A minimal multi-architecture-capable OCI image contains the same verified daemon digest plus only approved runtime trust/timezone metadata.
- [ ] The image runs as non-root with read-only root filesystem, dropped capabilities, no Docker/Podman socket, explicit resource/task limits, healthcheck, and one writable state volume.
- [ ] Podman+Quadlet documentation/smoke is the preferred low-overhead container path and Docker/Compose runs the same image/config contract.
- [ ] Secrets/certificates enter through container secret/credential mounts and never image layers or logged environment dumps.
- [ ] Editor/API, SQLite WAL/FULL, Eco workflow, release identity, health, backup, restore, and Recovery Kit semantics match the systemd build.
- [ ] The container does not self-modify its image; host tooling activates a verified immutable image digest.
- [ ] Container startup reports explicit unsupported host/cgroup/device capabilities rather than silently weakening limits.
- [ ] Optional GPU device access remains absent by default and never requires privileged container mode.
- [ ] SBOM, notices, signature/provenance, vulnerability scan, image size, idle resources, and state-volume ownership are verified for both runtimes.
