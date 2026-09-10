---
status: accepted
---
# Use an envelope-encrypted scoped credential vault

Each credential is encrypted with its own data key and data keys are wrapped by a rotatable master key supplied as a systemd credential from a root-owned source. Activations receive short-lived Secret Leases containing only authorized fields and scopes, while logs, Artifacts, errors, UI responses, workflow exports, and remediation evidence redact secret material by construction.
