# Initial environment survey

Surveyed 2026-09-10 on the target VPS.

- Host: Ubuntu 24.04.4 LTS, Linux 6.8, x86-64 KVM
- CPU: 2 vCPU, Intel Broadwell-class virtual CPU
- Memory: 1.9 GiB RAM and 4.0 GiB swap
- Storage: ext4, 48 GiB total, approximately 41 GiB free
- Account: `matt3`, passwordless non-interactive sudo available
- Present: Git 2.43, Python 3.12, SQLite 3.45, GCC 13.3, Make
- Absent at survey: Rust/Cargo, Node.js/npm, Docker/Podman, PostgreSQL, Redis, Nginx/Caddy
- Network: only SSH was listening publicly

The project must not assume that swap satisfies the 500 MiB RSS SLO.
