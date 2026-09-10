# Implementation Map: Eco 100K First Runnable

Type: implementation-map
Status: ready-for-agent
Blocked by: none

## Parent specification

- [First Runnable Independent Workflow Platform — Eco 100K](../first-runnable-platform/spec.md)

## Frontier

- Ticket 01 can start immediately. A later ticket becomes frontier when every ticket listed in its `Blocked by` field is complete.

## Tickets

1. [Boot the production-shaped daemon and editor shell](issues/01-boot-the-production-shaped-daemon-and-editor-shell.md) — blocked by None (can start immediately).
2. [Establish the Owner and recovery root](issues/02-establish-the-owner-and-recovery-root.md) — blocked by 01: Boot the production-shaped daemon and editor shell.
3. [Create the first Node Contract and durable Draft](issues/03-create-the-first-node-contract-and-durable-draft.md) — blocked by 02: Establish the Owner and recovery root.
4. [Recover and arbitrate Draft editing](issues/04-recover-and-arbitrate-draft-editing.md) — blocked by 03: Create the first Node Contract and durable Draft.
5. [Publish and roll back a Manual Trigger revision](issues/05-publish-and-roll-back-a-manual-trigger-revision.md) — blocked by 04: Recover and arbitrate Draft editing.
6. [Run and trace Manual Trigger durably](issues/06-run-and-trace-manual-trigger-durably.md) — blocked by 05: Publish and roll back a Manual Trigger revision.
7. [Stream Generate Items through bounded Envelopes and Artifacts](issues/07-stream-generate-items-through-bounded-envelopes-and-artifacts.md) — blocked by 06: Run and trace Manual Trigger durably.
8. [Transform items with Edit Fields and the safe expression VM](issues/08-transform-items-with-edit-fields-and-the-safe-expression-vm.md) — blocked by 07: Stream Generate Items through bounded Envelopes and Artifacts.
9. [Route items deterministically with If](issues/09-route-items-deterministically-with-if.md) — blocked by 08: Transform items with Edit Fields and the safe expression VM.
10. [Merge closed branch streams without unbounded memory](issues/10-merge-closed-branch-streams-without-unbounded-memory.md) — blocked by 09: Route items deterministically with If.
11. [Summarize the exact Eco 100K Run](issues/11-summarize-the-exact-eco-100k-run.md) — blocked by 10: Merge closed branch streams without unbounded memory.
12. [Recover Eco 100K after an ungraceful daemon kill](issues/12-recover-eco-100k-after-an-ungraceful-daemon-kill.md) — blocked by 11: Summarize the exact Eco 100K Run.
13. [Govern bounded work and scale across cgroup CPU profiles](issues/13-govern-bounded-work-and-scale-across-cgroup-cpu-profiles.md) — blocked by 12: Recover Eco 100K after an ungraceful daemon kill.
14. [Prove the 100,000-Node-Instance editor seam](issues/14-prove-the-100-000-node-instance-editor-seam.md) — blocked by 05: Publish and roll back a Manual Trigger revision.
15. [Import the first n8n 2.39.0 compatibility subset](issues/15-import-the-first-n8n-2-39-0-compatibility-subset.md) — blocked by 05: Publish and roll back a Manual Trigger revision.
16. [Complete the critical journey without relying on Canvas or one browser](issues/16-complete-the-critical-journey-without-relying-on-canvas-or-one-browser.md) — blocked by 11: Summarize the exact Eco 100K Run; 14: Prove the 100,000-Node-Instance editor seam; 15: Import the first n8n 2.39.0 compatibility subset.
17. [Retain, pin, compact, and expire evidence safely](issues/17-retain-pin-compact-and-expire-evidence-safely.md) — blocked by 13: Govern bounded work and scale across cgroup CPU profiles.
18. [Create verified Recovery Sets and Quarantine Mode](issues/18-create-verified-recovery-sets-and-quarantine-mode.md) — blocked by 17: Retain, pin, compact, and expire evidence safely.
19. [Activate signed Release Slots and roll back failed upgrades](issues/19-activate-signed-release-slots-and-roll-back-failed-upgrades.md) — blocked by 18: Create verified Recovery Sets and Quarantine Mode.
20. [Run the identical signed bundle with Podman and Docker](issues/20-run-the-identical-signed-bundle-with-podman-and-docker.md) — blocked by 19: Activate signed Release Slots and roll back failed upgrades.
21. [Certify the Eco 100K first runnable release candidate](issues/21-certify-the-eco-100k-first-runnable-release-candidate.md) — blocked by 12: Recover Eco 100K after an ungraceful daemon kill; 13: Govern bounded work and scale across cgroup CPU profiles; 15: Import the first n8n 2.39.0 compatibility subset; 16: Complete the critical journey without relying on Canvas or one browser; 18: Create verified Recovery Sets and Quarantine Mode; 19: Activate signed Release Slots and roll back failed upgrades; 20: Run the identical signed bundle with Podman and Docker.

## Completion gate

- Ticket 21 certifies the release only after every direct and transitive blocker is complete and the external evidence manifest passes.
