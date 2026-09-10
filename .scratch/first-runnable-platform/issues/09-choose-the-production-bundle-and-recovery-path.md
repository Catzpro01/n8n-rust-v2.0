# Choose the production bundle and recovery path

Type: grilling
Status: resolved
Blocked by: 06, 07

## Question

How should the stripped daemon, static editor, migrations, trust roots, service unit, updater, backup, and rollback be packaged so a clean install and failed upgrade both remain inside the Eco disk and runtime envelope?


## Answer

Ship the signed systemd-first bundle, optional container parity, protected update slots, complete backup manifests, and recovery behavior in [ADR-0055](../../../docs/adr/0055-ship-signed-release-slots-with-verified-recovery.md).

Native systemd is the default lightest production form. A minimal OCI image runs the exact same signed Rust binary and state format; Podman+Quadlet is the preferred low-overhead container documentation while Docker/Compose remains supported and tested. Current and Previous immutable Release Slots plus temporary Staging permit bounded rollback under the 10 GiB envelope.

Updates may download in the background but require an owner-approved maintenance activation. A Release Trust Root, signed Release Manifest, anti-rollback metadata, digest/SBOM/provenance and compatibility checks, Recovery Reserve, active-plan check, and verified pre-upgrade Recovery Set are mandatory. Public traffic remains closed during migration/readiness; a candidate failure restores the known pre-upgrade set and Previous automatically. Suspected ordinary corruption enters non-executing Quarantine Mode and requires owner-approved restore rather than silently replacing newer evidence.

Recovery Sets cover SQLite, Artifact roots, encrypted vault metadata, configuration/instance identity, migration history, and exact release/plan/contract/compatibility/optional-component locks. Local sets and Restore Drills support the first runnable; encrypted off-site transfer plus an externally held Recovery Kit is required for Disaster-Recovery Ready status. Practical initial targets are restart under one minute, bad pre-traffic update rollback under five minutes, local restore under thirty minutes with about one hour RPO, and off-site host-loss recovery under four hours with about 24 hours RPO.

The managed disk always protects a Recovery Reserve. Work admission/retention yields before checkpoint, backup, staging, or rollback can be starved.

At the owner's request, Advanced Settings exposes GPU Off by default and Safe Auto as explicit opt-in. The default bundle contains no heavy GPU stack; only tested eligible worker implementations receive restricted devices, with visible trace and declared CPU/remote/fail fallback. External Agent Engines and 9Router-style routing remain optional signed remote-first/local-opt-in components whose Blueprint/route locks and references are recoverable without making their runtimes resident.

The owner explicitly accepted systemd default with Docker/Podman choices, staged owner-approved updates, practical recovery objectives, offline Recovery Kit, protected disk reserve, verified slot rollback, automatic Restore Drills, local-then-off-site backup, signed anti-rollback releases, diagnosis-before-corruption-restore, identical container state semantics, and the Off/Safe-Auto GPU policy on 2026-09-11.
