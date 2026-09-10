---
status: accepted
---
# Keep native node identities with visible compatibility aliases

Every independently implemented Node Definition has a product-native stable identity and an original primary display name. Imported workflows separately preserve the exact external node identity, user-assigned Node Instance name, `typeVersion`, safe parameters/unknown fields, and Compatibility Profile as compatibility metadata.

The editor shows a familiar native name plus a visible compatibility badge/detail such as Native equivalent, Delegated compatible, Preserved opaque, Adapted, Unsupported, or Rejected unsafe. A fixture-backed Compatibility Mapping may expose the external name as a Compatibility Alias, but the alias never changes the native identity or implies that the node is official, endorsed, or implemented from n8n source.

Generic functional names such as Manual Trigger, Edit Fields, If, Merge, and Summarize may be used subject to release legal review. The product does not copy n8n logos, bundled icons, descriptions, visual treatment, or distinctive trade dress. Exact external identities are taken from sanitized frozen-profile fixtures rather than guessed.
