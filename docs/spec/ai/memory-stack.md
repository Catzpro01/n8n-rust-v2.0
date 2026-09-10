# AI Memory Stack

Memory Layers declare scope, read/write policy, provenance, retention, sensitivity, retrieval method, token budget, and conflict behavior. Supported adapter targets may include scratch state, conversation summaries, episodic records, semantic/vector stores, Obsidian vaults, Graphify/knowledge graphs, and future external memory systems.

Retrieval returns ranked references and bounded excerpts. Full content stays in Artifacts until selected. Writes are asynchronous where safe, deduplicated, redacted, and traceable to the producing Run, Activation, tool, and source.
