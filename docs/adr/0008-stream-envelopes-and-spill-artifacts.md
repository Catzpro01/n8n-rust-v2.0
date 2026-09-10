---
status: accepted
---
# Stream envelopes and spill artifacts while preserving the n8n item facade

The user-facing compatibility surface retains n8n-style JSON Items, while the runtime passes bounded Envelopes under backpressure and represents large values as streamed Artifacts. Compatibility Nodes may materialize item collections only within explicit budgets and must spill or fail predictably rather than exhaust daemon memory.
