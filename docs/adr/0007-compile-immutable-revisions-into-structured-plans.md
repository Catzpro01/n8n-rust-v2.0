---
status: accepted
---
# Compile immutable revisions into structured execution plans

Each Run targets one immutable Workflow Revision compiled into an Execution Plan; cycles are valid only through explicit loop, map, wait, or retry constructs. Sub-workflows may be collapsed and expanded lazily, and the editor must virtualize its viewport rather than render the full graph, enabling very large workflows without accepting arbitrary non-terminating cycles.
