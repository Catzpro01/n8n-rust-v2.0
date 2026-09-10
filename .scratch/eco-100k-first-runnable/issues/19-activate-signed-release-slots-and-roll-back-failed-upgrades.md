# 19: Activate signed Release Slots and roll back failed upgrades

**What to build:** The Owner can stage and approve an authenticated update, migrate only after safe preflight, activate it without public traffic, and automatically return to Previous plus the known Recovery Set when pre-traffic checks fail.

**Blocked by:** 18: Create verified Recovery Sets and Quarantine Mode

**Status:** ready-for-agent

- [ ] Release Manifest authenticates bundle digest, version, provenance, SBOM/notices, migrations, compatibility ranges, advisory policy, and required Recovery Reserve through an Owner-approved trust root.
- [ ] Invalid/expired signature metadata, undeclared files, corrupt digest, unauthorized downgrade, unsupported database/plan/contract format, or failed policy blocks staging/activation.
- [ ] Current and Previous are immutable Release Slots; Staging exists only during update and older releases are re-fetchable by digest.
- [ ] Background download does not activate the update; Owner approval or an explicitly enabled maintenance window is required.
- [ ] A privileged one-shot maintenance path re-verifies staged material and cannot follow untrusted paths into privileged writes.
- [ ] Preflight blocks new admission/public side effects, settles/checkpoints or suspends Runs, verifies active-plan readability and Recovery Reserve, and creates a verified pre-upgrade Recovery Set.
- [ ] Checksummed migration and candidate readiness run before public traffic; tests cover database, Artifact/vault metadata, editor/API, plan load, and synthetic no-side-effect execution.
- [ ] Candidate failure restores the pre-upgrade set and Previous automatically and is measured against the under-five-minute target.
- [ ] After candidate production traffic/state begins, destructive automatic rollback is refused and an explicit recovery flow is required.
- [ ] Update, migration, rollback, trust, and Owner decisions produce redacted audit evidence.
