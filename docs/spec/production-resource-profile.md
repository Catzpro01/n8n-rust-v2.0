# Constrained production resource profile

## Hard deployment target

The default installed system must operate under:

- memory: 500 MiB hard service limit;
- CPU: 50% of one logical core as a sustained systemd quota;
- persistent disk: 10 GiB maximum managed footprint;
- server application processes at idle and for Inline Native workloads: one Rust daemon;
- database: embedded SQLite;
- editor: prebuilt static assets executed in the user's browser.

The Rust daemon incorporates HTTP/TLS ingress, API, scheduler, execution engine, vault access, observability, and static-file serving to avoid mandatory reverse-proxy, database, queue, or JavaScript server processes.

## Build and installation

Frontend compilation and compatibility artifact generation may use Node.js; Rust compilation uses Cargo; tests and packaging may use Python or other tools. These are build-time dependencies and are not present in the minimal production bundle. Prefer reproducible CI or builder-host builds, then deploy a stripped Rust binary, compressed static assets, migrations, trust roots, and signed packages. Builder caches and source trees do not count toward or remain inside the 10 GiB production footprint.

## Optional workload runtimes

Full Node.js/npm, CPython packages, Playwright, Crawlee, Scrapy, Camoufox, external agents, and arbitrary processes are not part of the Rust-only default profile. A workflow that explicitly selects them must either use a remote adapter or launch an isolated local worker under a separate visible resource budget. Such a Run cannot claim the Rust-only 500 MiB profile unless measured total usage remains within it.

## Disk budget

The system enforces quotas across the binary and static assets, SQLite, Artifacts, logs/traces, package cache, local snapshots, and temporary spill. Admission control rejects work whose estimated durable footprint cannot fit; retention and content-addressing reclaim space before the filesystem is endangered.
