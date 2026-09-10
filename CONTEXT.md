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
