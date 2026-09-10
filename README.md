# n8n-rust-v2.0

Next-generation workflow automation system with Rust execution engine and Supabase backend integration.

## Architecture & Integration
- **Backend / Engine**: Rust runtime
- **State & Cloud Backend**: Supabase Server
- **Version Control**: GitHub (Catzpro01/n8n-rust-v2.0 - Private)
- **Auto-Sync**: Active via post-commit hooks and auto-git-watcher

## Scripts
- scripts/git-sync.ps1: Manual 1-click commit & push
- scripts/auto-git-watcher.ps1: Real-time change watcher (auto commit & push every 15s)
