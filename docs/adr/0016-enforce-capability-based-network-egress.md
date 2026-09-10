---
status: accepted
---
# Enforce capability-based network egress

Outbound network access is denied unless a Node Contract declares approved protocols, destinations, ports, redirects, and credential scopes and receives matching Capability Grants. Cloud metadata, loopback, private networks, DNS rebinding, and redirect-based policy escapes remain blocked unless explicitly authorized, trading unrestricted convenience for containment of SSRF and data exfiltration.
