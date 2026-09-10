---
status: accepted
---
# Ship signed release slots with verified recovery

## Context

The default production installation must keep one Rust daemon, embedded SQLite, static editor assets, and ordinary execution within 500 MiB RAM, sustained 0.5 CPU, and 10 GiB managed disk. It must survive daemon crashes, failed upgrades, database corruption, and later host loss without leaving build toolchains or an always-resident updater/backup stack.

The owner wants native systemd as the lightest default while retaining Docker and Podman choices. Optional GPU and external Agent Engines must not make the default daemon heavy. Recovery must cover SQLite, Artifacts, encrypted vault metadata, pinned plans/contracts, and configuration identity together rather than treating one database copy as a complete backup.

## Decision

### One signed bundle and three deployment presentations

Produce one signed Production Bundle containing:

- one stripped Rust daemon with embedded SQLite support, migrations, and content-hashed editor assets;
- a signed Release Manifest, checksums, SBOM, notices, provenance, compiler/plan/schema compatibility ranges, and migration inventory;
- hardened systemd service/socket/timer definitions;
- OCI image metadata plus Podman Quadlet and Docker Compose examples;
- configuration and recovery documentation.

No Rust/Node/Python toolchain, package manager, source checkout, frontend build tree, or compatibility runtime remains in the default production installation. Minimal OCI images run the same release binary and contain only required runtime trust/timezone data and metadata; they do not become a different product.

Native systemd is the default. The installer creates a dedicated unprivileged service identity, root-owned immutable release directories, managed state/runtime directories, and systemd credentials for the vault master key and configured TLS material. The unit applies the Eco CPU, memory, swap, task, filesystem, device, syscall, and network hardening profile with documented exceptions.

The same binary exposes narrowly scoped one-shot commands such as `serve`, `doctor`, `verify`, `backup`, `restore`, and `migrate`. systemd may invoke those commands through timers or privileged maintenance units, but no second updater or backup daemon remains resident.

### Container parity

Publish an OCI image of the same signed binary/digest for optional Podman or Docker operation. Prefer Podman plus Quadlet in low-resource documentation while testing the same image with Docker. Run non-root with a read-only root filesystem, dropped capabilities, no container-engine socket, explicit CPU/memory/task limits, health checks, and one writable state volume. Secrets enter through the platform's secret/credential mechanism rather than image layers.

Container deployments use the same state schema, Release Manifest, Recovery Set, and restore validation as systemd. The container does not overwrite its own image; the host pulls and activates a verified immutable image digest.

### Release trust and slots

Trust only Release Manifests authorized by an owner-approved Release Trust Root. A manifest authenticates bundle digests, version, provenance, SBOM/notices, migrations, required disk reserve, and supported database/plan/contract ranges. Expired metadata, invalid signatures, undeclared files, unsupported formats, or unauthorized rollback are rejected. Owner builds may use a separately configured owner trust root.

Keep immutable **Current** and **Previous** Release Slots. Create a temporary **Staging** slot only during update and delete failed/unneeded staging content. Older signed releases may be re-fetched by digest rather than occupying managed disk.

The unprivileged daemon may check, download, and verify an update into staging. Activation requires explicit owner approval or an owner-enabled maintenance window. A root-owned one-shot helper verifies all material again before changing release pointers; untrusted staged paths never directly control privileged writes.

### Upgrade transaction

Before activation:

1. verify trust, anti-rollback state, provenance/advisory policy, and bundle completeness;
2. prove the candidate can read every active/suspended Execution Plan or defer the update;
3. check worst-case Recovery Reserve for staging, migration, checkpoint, backup, and rollback;
4. stop new Run Admission, settle/checkpoint or durably suspend work, and close public side-effect ingress;
5. create and verify a pre-upgrade Recovery Set;
6. run checksummed migration preflight and then the controlled migration;
7. start the candidate without public traffic and execute readiness, database, Artifact, vault-metadata, editor/API, plan-load, and synthetic no-side-effect tests.

Only after those checks pass does the candidate become Current and the old Current become Previous. If candidate startup, migration, or pre-traffic health fails, stop it, restore the verified pre-upgrade Recovery Set, and reactivate Previous automatically. Once new production traffic has been admitted, a destructive rollback is no longer automatic; it follows the explicit recovery flow so newly written data is not silently discarded.

Updates are never activated when a required backup, Recovery Kit validation, plan compatibility check, or Recovery Reserve check fails.

### Protected disk budget

Partition the managed 10 GiB policy budget among live state, Causal Trace/Artifact retention, a Recovery Reserve, Current/Previous release slots, and temporary update/backup work. Exact thresholds are measured in the implementation specification, but ordinary workflow growth can never consume the Recovery Reserve.

When the reserve is approached, first expire/compact eligible unpinned terminal evidence, remove aged safe orphans/staging, reduce admission, and durably suspend resumable work. Never consume the final bytes, delete active/pinned evidence, or begin an update that cannot preserve a rollback path.

### Complete Recovery Sets

A Recovery Set contains a signed/hashed manifest over:

- a consistent SQLite online/offline snapshot and migration ledger;
- every immutable Artifact root referenced by that snapshot;
- encrypted vault metadata, never plaintext credentials;
- configuration and instance identity digests with secret references redacted;
- exact release, compiler/Execution Plan, Node Contract, Compatibility Profile, Agent Blueprint/Model Route lock, and optional-worker identities needed to interpret state;
- retention, integrity, and restore-tool version metadata.

Pin snapshot references while backup collection is in progress. Copy immutable content incrementally by digest so repeated local/off-site generations do not duplicate unchanged Artifacts unnecessarily. A manifest is complete only after all referenced objects are verified.

Keep local snapshots under the protected budget, including a recent operational generation and every pre-upgrade set until the candidate is accepted. Target an initially practical local corruption recovery point of at most about one hour. Add encrypted deduplicating off-site transfer through restic or another reviewed destination when credentials are configured; target no more than about 24 hours of host-loss exposure initially.

A local-only installation remains usable for the first runnable but displays that it is not Disaster-Recovery Ready. Grant that status only while a recent off-site Recovery Set and usable owner-held Recovery Kit pass the declared Recovery Objective.

### Recovery Kit and drills

The installer creates a separately encrypted owner-held Recovery Kit containing the material and instructions needed to authenticate releases, identify the instance, and unlock restored encrypted state after host loss. It is never stored plaintext, committed to Git, or kept only beside the state it protects. First-run and health status remain incomplete until the owner acknowledges an external copy and validates its checksum.

Verify every backup manifest and perform periodic Restore Drills into a temporary isolated state root. A drill opens and checks SQLite, migration history, Artifact roots, encrypted vault metadata, release/plan/contract readability, and output digests without enabling triggers, public ingress, credentials, network side effects, or production schedules. Record drill evidence and alert when objectives are stale.

Recovery objectives for the private initial installation are targets, not claims beyond available infrastructure:

- daemon crash: automatic restart and bounded replay in under one minute;
- failed pre-traffic upgrade: automatic Previous-slot rollback in under five minutes;
- local corruption: approved verified local restore in under thirty minutes with a target one-hour recovery point;
- host loss after off-site activation: rebuild in under four hours with a target 24-hour recovery point.

### Corruption and quarantine

Ordinary daemon crashes restart automatically from Durable Checkpoints. Suspected database, Artifact, vault, or migration corruption instead enters Quarantine Mode: stop writes, Run Admission, triggers, schedules, and side effects; preserve suspect files as evidence; run non-destructive diagnosis; list verified Recovery Sets and estimated loss; require owner approval before replacing state.

Automatic data restoration is reserved for the known pre-upgrade snapshot before candidate traffic opens. It is not used as a generic response to corruption because a stale backup could destroy recoverable newer evidence.

### Private control and public ingress

Keep editor/administration on the private control bind and expose only configured Public Gateway routes. Serve direct rustls HTTPS from provisioned/reloadable certificate material, with optional owner-managed proxying. Backup, restore, migration, update, and Recovery Kit operations are never public ingress functions.

### Optional GPU and external engines

Advanced Settings offers an Accelerator Policy with **Off** as the default. With **Safe Auto** enabled, discover and health-test GPU capability, then use it only through an eligible signed local/remote worker for Node Contracts whose certified implementation declares compatible semantics, budgets, and fallback. Record device, VRAM, implementation, fallback, cost, and selection in Causal Trace. The default bundle carries no CUDA, ROCm, model weights, or heavy GPU runtime.

OCI GPU access is explicit per worker device and never uses a privileged container. Loss or saturation of a GPU follows the pinned CPU/remote/fail policy without silently changing deterministic outputs.

Hermes, OpenClaw, OpenCode, Claude Code, MiroFish, Antigravity, 9Router, browser workers, and future Agent Engines remain optional signed remote-first/local-opt-in components. Their executables/runtimes are absent and stopped by default. Production backup retains their approved manifests, Agent Blueprint/Model Route locks, scopes, and credential references but not unreviewed binaries or plaintext secrets.

## Consequences

The smallest installation remains one hardened Rust daemon managed directly by systemd, while the same binary can run under Podman or Docker without format drift. Signed slots and a complete pre-upgrade Recovery Set make failed upgrades reversible before traffic is reopened; protected reserve prevents the recovery mechanism from being starved by ordinary workload growth.

Local snapshots enable the first runnable but do not create a false disaster-recovery claim. Off-site verification, an owner-held Recovery Kit, and restore drills turn backup existence into measurable recoverability. Optional GPU and external agent ecosystems can expand later without becoming resident dependencies or changing the Eco baseline.
