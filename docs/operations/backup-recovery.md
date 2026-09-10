# Backup and recovery requirements

The first deployment creates consistent local snapshots of metadata, Workflow Revisions, encrypted vault records, and Artifacts, and regularly restores them into an isolated verification directory. Off-site encrypted restic replication is required before the system is considered disaster-recovery complete; the destination remains pending owner-provided storage credentials.
