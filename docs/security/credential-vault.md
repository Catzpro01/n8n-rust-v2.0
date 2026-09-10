# Credential vault requirements

- Envelope encryption with one data key per credential.
- Rotatable master key supplied through systemd credentials from root-owned storage.
- Authenticated encryption and integrity verification for every record.
- Secret Leases scoped to one Activation, selected fields, purpose, and expiry.
- No secret values in workflow JSON, exports, logs, traces, Artifacts, cache keys, errors, or remediation prompts.
- Redaction tests cover exact values, common encodings, headers, URLs, and structured fields.
- Rotation and revocation are audited and do not require editing Workflow definitions.
