# Prototype the virtualized independent editor

Type: prototype
Status: resolved
Blocked by: 01

## Question

Which independent editor state and rendering model keeps navigation, selection, search, grouping, and editing responsive for a 100,000-node logical graph without rendering every node or copying n8n trade dress?

## Answer

Use a hybrid, group-first virtualized editor. The Owner compared three independently designed variants and selected “Gabungan”, then explicitly accepted the recommended composition: Structure Deck for macro navigation, Atlas Canvas for focused graph detail, and Command Map for global search and direct jump.

The throwaway primary source is branch `prototype/virtualized-editor` at commit `811d4332121778adb5578fc645a4e93dac3087df`. It contains the three variants, the accepted verdict, and one dependency-free HTML fixture with an actual 100,000-node/99,999-edge synthetic DAG.

### Interaction model

- At macro scale, present stable groups and aggregate connectivity rather than a 100,000-node hairball. Opening a group enters a focused detail canvas.
- In detail, use Atlas-style viewport navigation with a collapsible inspector. Keep global `/` command search and direct node/group jump available everywhere.
- Use semantic zoom: group summaries at overview scale, compact node marks at intermediate scale, and detailed nodes plus local edges only when zoom and focus justify them.
- Preserve search, keyboard/tree navigation, and inspector controls as bounded, virtualized DOM surfaces. The canvas is not the only accessibility or navigation surface.

### State and rendering model

1. Keep canonical Node Instance identity and graph data independent of the DOM in a packed structure-of-arrays snapshot with stable IDs.
2. Keep selection, focus, group membership changes, and Mutable Draft edits as sparse overlays rather than cloned 100,000-object view models.
3. Build compact spatial, group, and search indices; return bounded result windows even when the logical match count is large.
4. Use Canvas 2D for the first renderer and draw only candidates returned by the spatial index. DOM size must remain independent of logical node count.
5. Apply level-of-detail rules to edges as well as nodes; never draw all global edges merely because the document contains them.
6. Build/decode large snapshots and derived indices outside the interaction path with a browser worker or chunked scheduling. The Rust daemon remains the authoritative compiler and publish authority.
7. Put the renderer behind a narrow interface. A WebGL implementation is a later benchmark-driven substitution, not a first-release requirement.

### Prototype evidence and limits

The packed typed graph columns and CSR-style spatial index use approximately 1.54 MiB before browser/runtime overhead. A Node.js smoke execution of the embedded model built and compiled all 100,000 nodes, found all 16,667 Transform nodes, selected and grouped 512 nodes, and produced compile hash `2fee35e5`. Its observed build and compile times were about 19 ms and 2.9 ms; these are sanity observations, not portable browser guarantees. The browser's **Run proof** action measures compile, search, jump, selection, grouping, viewport rendering, and DOM size on the actual client.

No prototype variant is promoted directly. The dependent editor-interface decision must still choose the production browser framework, worker/message boundary, accessible virtual tree behavior, autosave protocol, and measurable cross-browser budgets.
