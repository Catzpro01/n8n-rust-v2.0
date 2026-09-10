---
status: accepted
---
# Compose query-first policy-driven memory

AI Agents use an ordered Memory Stack whose layers may be scoped to Run, Thread, Workflow, Project, or Global and backed by scratch, summaries, episodes, vectors, Obsidian, Graphify, or future adapters. Retrieval is query-first and budgeted rather than injecting all stored content; writes retain provenance and pass redaction so multiple layers do not silently consume context or leak secrets.
