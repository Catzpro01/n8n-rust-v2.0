---
status: accepted
---
# Split private control access from public workflow ingress

The editor and administration interfaces are reachable only through a private network, while a separately routed Public Gateway exposes only explicitly published webhook, form, OAuth callback, MCP, and health routes over HTTPS. This supports external triggers and OAuth without continuously exposing the highest-authority control surface to the internet.
