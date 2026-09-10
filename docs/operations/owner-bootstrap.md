# Owner bootstrap and recovery root

The private control surface permits exactly one first-run Owner. The systemd installer creates a root-owned 32-byte master key at `/etc/workflowd/master.key`; systemd exposes it to the unprivileged daemon through `LoadCredential`. The key is never written to the environment file, SQLite, logs, or the editor response.

With the daemon on its default private loopback address, submit setup with the exact browser origin:

```bash
curl --fail --request POST http://127.0.0.1:8787/api/v1/setup \
  --header 'Origin: http://127.0.0.1:8787' \
  --header 'Content-Type: application/json' \
  --data '{"email":"owner@example.test","password":"replace-with-a-long-password","recovery_passphrase":"use-a-separate-long-recovery-passphrase"}'
```

The response contains an encrypted Recovery Kit document and its SHA-256 checksum. Save the document outside this server before acknowledging the checksum. The plaintext recovery passphrase is not retained. Setup closes permanently after the transaction creates the Owner, encrypted vault sentinel, recovery checksum, and audit event.

Login returns a CSRF token and rotates a random session cookie. Mutating private requests require the exact configured `Origin`, the `HttpOnly; Secure; SameSite=Strict` session cookie, and `X-Canopy-CSRF`. Repeated failures are limited. Renewal rotates both session and CSRF values; logout invalidates the session.

`GET /health/ready` reports one of:

- `setup-required`;
- `recovery-kit-unacknowledged`;
- `local-recovery-only` after acknowledgement;
- a future recovery ticket may grant `disaster-recovery-ready` only after verified off-site evidence.

Only `/public/v1/health/live` exists in the initial Public Gateway. Setup, sessions, recovery, audit, backup, restore, update, Draft, and credential functions are not routed beneath `/public/`.

Default uninstall preserves both `/var/lib/workflow-rust` and `/etc/workflowd/master.key`. Losing either side prevents ordinary recovery. Destruction must therefore be an explicit Owner action, not an uninstall side effect.
