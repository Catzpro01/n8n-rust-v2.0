---
status: accepted
---
# Build local recovery before adding off-site storage

Begin with consistent local snapshots of SQLite, the Artifact store, and encrypted vault metadata plus automated restore drills, then add encrypted off-site restic storage when destination credentials are available. Git records source and Workflow Revisions but never secret values; local-only copies are an interim milestone, not the final disaster-recovery posture.
