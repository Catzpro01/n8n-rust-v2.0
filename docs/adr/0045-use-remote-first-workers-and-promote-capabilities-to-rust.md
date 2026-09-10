---
status: accepted
---
# Use remote-first workers and promote capabilities to Rust

Capabilities requiring Node.js, full Python, browsers, or external agent harnesses are remote-first and may run in a local isolated worker only through explicit opt-in and a separate visible budget. Each commonly used capability enters a Rust Promotion Track with conformance fixtures, benchmarks, and staged preference for a Native Node implementation, preserving behavior and fallback while steadily reducing non-Rust runtime dependence.

The track ports functionality that can reasonably live in Rust—crawling, parsing, extraction, scheduling, protocol clients, connectors, transforms, and orchestration—but does not claim to rewrite third-party cloud services or complete browser engines. Browser automation can become Rust-controlled through CDP, WebDriver, or provider protocols while the browser itself remains an external or remote engine.
