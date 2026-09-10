---
status: accepted
---
# Review package updates before adoption

Installed packages lock commits and versions; Hub may notify about updates and prepare visual, permission, dependency, compatibility, and migration diffs, but never changes a Published Revision automatically. This allows security updates without letting mutable branches, releases, or repository takeovers silently alter production behavior.
