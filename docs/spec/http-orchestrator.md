# HTTP Orchestrator

The ordinary HTTP Request node uses the native Rust adapter. HTTP Orchestrator accepts an ordered or automatically scored policy over adapters for native HTTP, compatibility behavior, caches, proxy providers, browser-backed fetch, and future transports.

Selection considers protocol capability, authentication destination binding, response size, streaming, latency, cost ceiling, retry classification, idempotency, robots policy, anti-bot requirements, and health history. The chosen adapter and every fallback are recorded in the Causal Trace. A policy cannot elevate network or credential capabilities.
