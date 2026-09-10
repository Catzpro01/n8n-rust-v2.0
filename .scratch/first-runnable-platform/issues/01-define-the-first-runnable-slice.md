# Define the first runnable vertical slice

Type: grilling
Status: resolved
Blocked by: none

## Question

What is the smallest end-to-end user journey that proves the independent editor, Published Revision model, Rust daemon, durable Run, Causal Trace, and Eco resource profile without becoming a horizontal architecture demo?

## Answer

The first runnable vertical slice is the deterministic **Eco 100K** user journey:

1. The Owner opens the independent private editor and creates a Workflow from the Eco 100K template.
2. The canvas contains six built-in Native Nodes: Manual Trigger, Generate Items, Edit Fields, If, Merge, and Summarize; Summarize uses its Output Digest operation for the fixture.
3. The Owner can add, connect, configure, validate, and search the nodes; the Mutable Draft autosaves.
4. Publishing creates an immutable signed Published Revision under the current Compatibility Profile.
5. Starting a Run generates and processes 100,000 lightweight Activations through the graph with bounded Envelopes and no internet or credential dependency.
6. The editor shows progress, effective CPU quota, peak memory, queue/backpressure state, disk use, and throttling without retaining every payload in browser memory.
7. Summarize / Output Digest returns count, branch counts, deterministic content hash, elapsed wall and CPU time, and resource evidence.
8. A required fault test sends an ungraceful kill during the Run, restarts the daemon, resumes from durable state, avoids duplicating committed output, and produces the same digest as an uninterrupted Run.
9. The Owner opens the Causal Trace, inspects any logical Activation and retry/resume evidence, and can roll back the Workflow to its preceding Published Revision.

Scale is proved by two separate fixtures through the same compiler and Run interface:

- **Execution fixture:** the six-node graph produces 100,000 Activations under the Eco Resource Profile.
- **Document fixture:** a synthetic Workflow Revision contains 100,000 Node Instances and proves compile, search, navigation, selection, grouping, and virtualized rendering without creating 100,000 DOM elements.

The external acceptance seams are the browser-visible Owner journey, versioned daemon interface used by the editor, process crash/restart behavior, deterministic digest, and cgroup-enforced benchmark evidence. Credentials, external HTTP, multi-user auth, AI, scraping, Hub, and broad node parity are deliberately excluded from this first slice.
