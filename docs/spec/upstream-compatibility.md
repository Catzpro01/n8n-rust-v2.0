# Upstream compatibility process

The initial Compatibility Profile targets n8n 2.39.0. A watcher produces weekly source, workflow-schema, node-catalog, editor, license, and security deltas. Reports may open review work but cannot merge code, change the active profile, republish a workflow, or alter migrations automatically.

A new profile requires passing import/export fixtures, core-node behavior tests, expression tests, webhook tests, editor contract tests, and migration rollback tests.
