# 02: Establish the Owner and recovery root

**What to build:** A first-time Owner can initialize the private installation, authenticate securely, obtain an externally storable Recovery Kit, and verify that private control and public ingress have different authority.

**Blocked by:** 01: Boot the production-shaped daemon and editor shell

**Status:** claimed

- [ ] First-run setup creates exactly one Owner only through the private control surface and becomes unavailable after successful initialization.
- [ ] Owner passwords use calibrated Argon2id PHC storage; login, session renewal, logout, expiration, and password-hash upgrade are externally tested.
- [ ] Same-origin secure session cookies, CSRF/origin validation, request limits, and sensitive-header/log redaction are enforced.
- [ ] Private editor/administration routes and the Public Gateway route tree are independently policy-tested; backup, restore, update, Draft, and credential functions are never public ingress.
- [ ] A root-owned/systemd-supplied master key initializes encrypted vault metadata without writing the plaintext key to SQLite, logs, configuration, Git, or browser responses.
- [ ] Installer/setup creates an encrypted Owner-held Recovery Kit with checksum and one-time acknowledgement instructions.
- [ ] Health clearly distinguishes Recovery Kit unacknowledged, local-recovery-only, and later Disaster-Recovery Ready states.
- [ ] Wrong/missing key, session fixation, repeated login failure, CSRF, and public-route access attempts fail safely with original structured diagnostics.
- [ ] Audit evidence identifies the Owner actor without persisting password, cookie, master key, or private recovery material.
