---
status: accepted
---
# Compile compatible expressions to a safe Rust VM

Retain n8n expression syntax and variables where behavior can be implemented safely, compiling the common subset into a Rust expression VM and offering a typed expression form for new workflows. Expressions requiring unsupported JavaScript semantics are identified in the Compatibility Report and delegated explicitly to a sandboxed lane rather than silently loading JavaScript into the daemon.
