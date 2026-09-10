# Network policy

- Editor and administration: private network only.
- Public Gateway: explicit published webhook, form, OAuth callback, MCP, and health routes only.
- Outbound traffic: denied unless matched by a Node Contract and Capability Grant.
- Default SSRF blocks: cloud metadata, loopback, link-local, private ranges, DNS rebinding, disallowed ports, and redirects outside policy.
- Secret Leases are bound to approved destinations where possible.
- Every ingress publication and egress decision is auditable without logging secret material.
