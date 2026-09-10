---
status: accepted
---
# Record causal traces and replay side effects safely

Each Run records a redacted Causal Trace with event ordering, error chains, schema summaries, resource and cost measurements, and OpenTelemetry correlation. Debug replay can begin at a selected Activation using recorded inputs or mocks, but side-effect nodes default to dry-run and require approval before a replay can repeat an external write.
