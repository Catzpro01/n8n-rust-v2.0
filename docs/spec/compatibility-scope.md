# n8n compatibility scope

The frozen baseline is n8n 2.39.0. Compatibility is assessed at user-visible seams:

- workflow JSON import and export;
- common expression behavior;
- webhook request and response behavior;
- credential references without exporting secret material;
- core workflow-control node behavior;
- compatibility-worker execution of supported n8n packages;
- editor interactions required to create, configure, run, inspect, and debug workflows.

Compatibility does not require n8n's internal database schema, private interfaces, known bugs, or identical resource consumption. Every imported workflow receives a machine-readable Compatibility Report and is inactive until validation succeeds.
