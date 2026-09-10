# Extension security baseline

Every executable Node Definition declares a Node Contract before installation. The contract includes capabilities, side-effect class, retry safety, determinism, network destinations, filesystem roots, credential scopes, CPU time, wall time, memory, output size, and concurrency.

Capability escalation changes the Node Contract hash and invalidates publication approval. Generated tests use synthetic credentials. Unknown MCP annotations are treated pessimistically. Native code begins outside the daemon and can enter it only through Promotion.
