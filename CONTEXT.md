# Workflow Automation

This context describes the language used to define, execute, and extend personal automation workflows at any practical graph size.

## Language

**Workflow**:
A user-authored graph of Node Instances and Connections that expresses an automation.
_Avoid_: Flow, pipeline, automation graph

**Workflow Revision**:
An immutable snapshot of a Workflow used as the sole definition for a Run.
_Avoid_: Workflow version, saved workflow

**Node Definition**:
A reusable contract describing a node's configuration, ports, capabilities, and execution behavior.
_Avoid_: Plugin, node type, component

**Node Instance**:
A configured occurrence of a Node Definition inside a Workflow Revision.
_Avoid_: Node when the distinction from Node Definition matters

**Connection**:
A directed route from an output port of one Node Instance to an input port of another.
_Avoid_: Edge, wire

**Run**:
One durable execution of exactly one Workflow Revision.
_Avoid_: Execution, job

**Activation**:
One scheduled attempt to process a Node Instance within a Run for a particular input envelope.
_Avoid_: Node execution, task

**Envelope**:
The bounded metadata and references passed between Activations; large values are represented by Artifact references rather than embedded bytes.
_Avoid_: Item when referring to the internal runtime representation

**Artifact**:
Content-addressed run data stored outside an Envelope and read as a stream.
_Avoid_: Binary data, blob, payload file

**Native Node**:
A Node Definition executed by the Rust runtime without a language compatibility layer.
_Avoid_: Built-in node

**Compatibility Node**:
A Node Definition executed by an isolated compatibility worker to preserve behavior of an existing n8n node.
_Avoid_: Legacy node, JavaScript node

**Compensation Workflow**:
A Workflow invoked to counteract completed side effects after a Run can no longer proceed safely.
_Avoid_: Rollback workflow

**Execution Plan**:
The validated, immutable scheduling representation compiled from one Workflow Revision.
_Avoid_: Compiled workflow, runtime graph

**Compatibility Report**:
A pre-activation account of which imported behavior is supported, adapted, blocked, or delegated to Compatibility Nodes.
_Avoid_: Import warnings

**Remediation Patch**:
A reversible proposed change that includes evidence, confidence, expected effects, and a rollback path.
_Avoid_: Auto-fix when approval is still required

**Node Form**:
One of the eight user-facing ways to define or obtain a Node Definition: JavaScript Script, Python Script, WASM Component, Rust Native, External Process, AI-Generated Node, Sub-workflow Node, or MCP Tool Node.
_Avoid_: Security tier, runtime tier

**Execution Lane**:
The resource and isolation class selected for an Activation independently of how its Node Definition was authored.
_Avoid_: Runtime language, node tier

**Inline Native Lane**:
The lowest-overhead Execution Lane for trusted built-in behavior that requires no external runtime.
_Avoid_: Simple mode

**WASM Micro Lane**:
A short-lived capability-limited Execution Lane for portable custom computation.
_Avoid_: Embedded plugin

**Isolated Runtime Lane**:
An on-demand Execution Lane for language runtimes and arbitrary processes that must not share the daemon's trust domain.
_Avoid_: Compatibility mode

**Heavy Orchestrator Lane**:
A separately budgeted Execution Lane for browsers, scrapers, local models, and other resource-intensive systems.
_Avoid_: Heavy node
