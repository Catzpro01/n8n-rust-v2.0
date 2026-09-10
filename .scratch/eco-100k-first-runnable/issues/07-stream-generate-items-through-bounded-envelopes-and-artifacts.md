# 07: Stream Generate Items through bounded Envelopes and Artifacts

**What to build:** Generate Items can emit the Eco item stream through bounded Envelopes, backpressure, and encrypted content-addressed Artifact spill without materializing the whole stream in RAM.

**Blocked by:** 06: Run and trace Manual Trigger durably

**Status:** ready-for-agent

- [ ] Generate Items has a locked Pure deterministic Per-Item/expansion contract with declarative count/start/step configuration and hard output budgets.
- [ ] For the Eco configuration it emits exactly 49,998 ordered items from one trigger input while preserving provenance.
- [ ] Engine queues and output emitters apply backpressure and remain bounded while a slow consumer is exercised.
- [ ] Large configured values and forced-spill cases stream to encrypted algorithm-tagged content-addressed Artifacts in the Owner namespace.
- [ ] Artifact staging, sync, atomic placement, SQLite reference commit, deduplication, authorization, and safe orphan cleanup follow the accepted crash-safe order.
- [ ] No credential or Secret Lease value can enter Artifact content or metadata through this node.
- [ ] The editor shows generated progress, output count, spill/backpressure state, and authorized lazy Artifact detail without loading all items.
- [ ] Fault tests cover cancellation, output/item/byte budget exceeded, digest mismatch, staging interruption, and reference denial.
- [ ] RSS and queue evidence demonstrate that memory does not grow linearly with 49,998 output items.
