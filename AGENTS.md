# Agent instructions

Read `CONTEXT.md` before naming domain concepts and read relevant ADRs under `docs/adr/` before changing architecture. The clean-room policy in `docs/legal/clean-room-policy.md` is mandatory. Do not copy n8n source, enterprise files, UI assets, icons, product copy, or distinctive trade dress.

## Agent skills

### Issue tracker

Work is tracked as local Markdown under `.scratch/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Canonical triage roles map directly to local status strings. See `docs/agents/triage-labels.md`.

### Domain docs

The repository uses one root context and system-wide ADR directory. See `docs/agents/domain.md`.

## Verification

Build behavior through externally observable tests at the highest stable interface. Resource claims require cgroup-enforced benchmarks and recorded evidence. Never claim completion without running the relevant verification commands.
