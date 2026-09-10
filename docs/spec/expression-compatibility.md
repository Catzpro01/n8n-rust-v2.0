# Expression compatibility

The parser accepts the supported n8n expression surface and compiles safe common operations to a deterministic Rust VM. The compiler reports unsupported JavaScript behavior before publication and can route an explicitly approved expression to a sandboxed lane. New workflows may use a typed expression form with schema-aware completion and validation.

Expression evaluation has declared limits for instructions, recursion, output size, wall time, and Artifact reads. It has no ambient filesystem, network, process, or credential access.
