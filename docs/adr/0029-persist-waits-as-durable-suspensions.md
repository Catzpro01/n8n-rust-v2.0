---
status: accepted
---
# Persist waits as durable suspensions

Waits, schedules, webhook resumes, external events, and human approvals become Durable Suspensions that consume no running thread or worker slot and survive process restarts. Resume tokens are signed, expiring, one-use, and bound to the exact Run and Activation, while timeout and duplicate delivery are explicit outcomes.
