---
status: accepted
---
# Isolate native plugins before promotion

Custom Rust Native nodes must build reproducibly, pass source and dependency checks, receive a local signature, and run first in a resource-limited native worker. A trusted node may be promoted into the daemon only by an explicit reviewed rebuild after soak testing, preserving a maximum-performance path without letting experimental native code share the initial daemon blast radius.
