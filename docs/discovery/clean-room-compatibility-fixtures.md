# Research: clean-room compatibility fixtures for n8n 2.39.0

Date: 2026-09-11

## Question

What clean-room process and fixture sources can establish n8n 2.39.0 workflow JSON, expression, webhook, and core-node compatibility without copying implementation, Enterprise code, protected assets, or distinctive product presentation?

## Baseline fact

The Compatibility Profile remains exactly **n8n 2.39.0**, release tag `n8n@2.39.0`, signed release commit `1620ec42cab26e2936e8fd4c1c011c5d210e3a95`, dated 2026-09-08. GitHub labels this release as a pre-release. The project chose it explicitly, so later stable/beta labels do not silently move the profile. [N1]

Compatibility means independently reproducing documented and observed user-visible behavior, not copying internal architecture, source code, tests, UI, wording, assets, or known bugs.

## Approved evidence hierarchy

Use only these sources, in order:

1. **Public documentation and public API specifications**, pinned by URL, retrieval date, and content hash. Current docs can drift after 2.39.0, so a claim from current docs is provisional until confirmed against the frozen reference runtime.
2. **Owner-authored workflows exported from a private n8n 2.39.0 reference instance.** The workflow arrangement, inputs, names, and expected intent are authored for this project; n8n supplies only the compatibility-shaped serialization.
3. **Black-box observations** of the frozen reference instance through the Editor UI, public API, documented CLI, and webhook HTTP surface.
4. **Independently written expected behavior and conformance assertions** distilled to functional facts.

Never use upstream n8n implementation source, upstream test suites, Enterprise `.ee.` files, internal `/rest` traffic reverse-engineering, template-library content, screenshots, icons, product copy, or CSS as fixture inputs. Public GitHub release metadata may identify the frozen binary; the implementation tree is not an allowed requirements source.

This boundary follows the repository clean-room policy and also avoids importing code governed by n8n's Sustainable Use or Enterprise terms. n8n's own license documentation says the main repository is under its Sustainable Use License except specified branches and `.ee.` files; that source is not used here. [N2]

## Reference-lab procedure

### 1. Freeze the oracle

Provision an isolated, private reference instance using the official n8n 2.39.0 Docker image or exact npm package. Record:

- release tag and signed commit;
- image registry, immutable image digest, package integrity hash, Node version, architecture, locale, and timezone;
- Community/Business/Enterprise feature state;
- every environment variable that can affect a case;
- SQLite/Postgres choice and reference data reset procedure.

Official installation docs allow pulling a specific Docker version, and npm installation supports `n8n@<version>`. Docker is preferred because the whole runtime can be pinned by digest. [N3][N4]

The reference runtime is test infrastructure only. It is not linked, bundled, redistributed, or required by the clean-room product.

### 2. Separate observation from implementation

Use two explicit roles/directories even when one owner operates both:

- **Reference operator:** may read the allowed public docs, create original workflows in the reference instance, invoke public interfaces, sanitize exports, and write an observation record.
- **Clean-room implementer:** receives only the sanitized fixture, provenance manifest, and independently written behavioral assertion. The implementer does not inspect n8n source or upstream tests.

An agent/session exposed to forbidden source cannot implement the corresponding compatibility behavior. Record the contamination boundary and regenerate the implementation task from clean fixtures.

### 3. Author one behavior per case

Each case starts from an original, minimal workflow and deterministic input. Avoid imported templates and realistic personal data. Freeze time, timezone, randomness, and external HTTP responses where relevant. Create paired positive, negative, boundary, and malformed cases rather than one broad golden workflow.

### 4. Sanitize before commit

n8n documents that exported workflow JSON contains credential names and IDs and may contain authentication headers imported from cURL. Therefore every export passes a fail-closed sanitizer before entering Git. [N5]

The sanitizer rejects or replaces:

- credentials, credential names/IDs, authorization/cookie headers, OAuth material, webhook secrets, hostnames, email addresses, and personal paths;
- pinned production payloads and binary bodies;
- opaque fields not classified in the fixture manifest;
- unexpectedly large strings or base64 blobs.

Use deterministic placeholders such as `credential-fixture-001`; never retain the original value beside the replacement. Run secret scanning in CI.

### 5. Capture behavior, not presentation

Record structured request/input, sanitized workflow JSON, normalized output, HTTP response facts, and independently classified errors. Do not capture screenshots, DOM, UI labels, colors, layout, or exact error prose unless a documented wire contract requires it. Compare stable error class/path/data; write original diagnostic copy in the product.

## Fixture repository shape

```text
compatibility/
  n8n-2.39.0/
    profile.toml
    provenance/
      sources.toml
      reference-runtime.toml
    workflow-json/<case>/
    expressions/<case>/
    webhooks/<case>/
    core-nodes/<node>/<case>/
    execution-order/<case>/
    import-diagnostics/<case>/
```

Each leaf contains:

```text
case.toml                 # original requirement, source URLs, profile, normalization rules
workflow.sanitized.json   # owner-authored reference export
input.json                # deterministic invocation data
expected.normalized.json  # minimal black-box result facts
expected.http             # only for webhook cases
README.md                 # reproduction steps and why the behavior matters
```

`case.toml` carries hashes for every file, reference image/package digest, node `type` and `typeVersion`, workflow execution-order setting, timezone/clock policy, author, observer, sanitizer version, and an explicit redistribution review state.

## Comparison rules

A single canonicalizer must not erase semantic differences.

- Sort JSON object keys for hashing; preserve array order.
- Preserve missing versus `null`, string versus number/boolean, integer precision behavior, empty item versus no item, output index, item order, and branch order.
- Normalize only fields explicitly declared volatile in `case.toml`, such as generated workflow/execution IDs, timestamps under a clock fixture, and host-specific webhook base URLs.
- Compare binary payloads by byte length, BLAKE3/SHA-256 digest, MIME type, file name/extension where semantic, and item-link provenance—not by embedding large base64 snapshots.
- Compare errors by independent code/category, failing node/parameter path, retryability, and side-effect state. Do not require copied English messages.
- Preserve unknown import fields in an opaque extension bag when safe, and prove export round-trip retention separately from executable support.

Each run produces a correctness digest over canonical expected facts and a Compatibility Report entry.

## Fixture families

### A. Workflow JSON import/export

Official documentation states that workflows are saved/exported as JSON. Public workflow creation requires at least `name`, `nodes`, `connections`, and `settings`; exact exported fields and defaults must come from owner-created 2.39.0 black-box exports. [N5][N6]

Cover:

- empty/minimal workflow and one-node workflow;
- stable node IDs/names, positions, `type`, `typeVersion`, parameters, disabled state, notes, pin data, settings, static data, and tags as observed;
- one and multiple outputs, sparse connection indexes, fan-out/fan-in, disconnected nodes, groups, and unknown safe fields;
- absent versus explicit defaults;
- duplicate names/IDs, dangling connections, unsupported node type/version, malformed parameter types, and excessive sizes;
- import → Compatibility Report → safe normalized Draft → export round trip;
- credential references replaced with unresolved typed placeholders, never credential values.

Do not claim the JSON export is a stable formal standard if the public docs do not say so. Version the importer by Compatibility Profile and preserve original sanitized bytes as evidence.

### B. Item model and linking

n8n documents node data as arrays of items, normally with a required `json` object and optional `binary` data. It also documents links from output items back to prior-node items and references such as `$json`, `$input.item`, and `$("Node").item`. [N7][N8]

Cover:

- zero, one, and many items;
- nested objects/arrays, Unicode, large/safe integers, floating values, booleans, null, missing fields, and empty structures;
- multiple binary properties with metadata and byte digests;
- one-to-one, one-to-many, many-to-one, branch, merge, and ambiguous/missing item-link provenance;
- item ordering and output-port identity.

These fixtures define the external item facade while allowing the Rust engine to stream Envelopes and spill Artifacts internally.

### C. Expressions

Public expression docs identify `$json`, `$binary`, `$input`, previous-node selectors, `$now`/`$today`, conditionals, and array/string methods. ADR-0025 already limits native execution to a safe compatible subset and delegates unsupported JavaScript semantics explicitly. [N9]

Build small cases for:

- literal versus expression encoding in exported JSON;
- `$json`, nested paths, bracket access, missing/null, truthiness, coercion, equality, arithmetic, concatenation, ternary, and short-circuiting;
- `$input.item/all/first/last`, `$("Node").first/last/all/item`, branch/run indexes, and item linking;
- arrays/objects, mapping/filtering, documented helpers, Unicode, escaping, and expression interpolation inside larger strings;
- frozen clock/timezone for `$now` and `$today`;
- syntax, runtime, missing-node, unpaired-item, unsupported method, and resource-limit failures;
- dangerous capabilities that must be rejected or delegated, never silently evaluated in the daemon.

For every expression, retain input, expected typed output/error, item-link context, and evaluation budget. Do not copy n8n's evaluator or test corpus.

### D. Webhooks

n8n documents separate test and production URLs; production registration on publish; HTTP methods DELETE/GET/HEAD/PATCH/POST/PUT; route parameters; Basic/Header/JWT/None authentication; immediate, last-node, Respond-to-Webhook, and streaming response modes; status/body options; and a documented default payload ceiling. [N10]

Cover:

- test listener lifecycle versus Published Revision production registration;
- every documented method, static/dynamic paths, query repetition, percent encoding, headers, cookies, JSON/form/multipart/raw/binary bodies, and empty bodies;
- authentication success/failure without retaining secret values;
- content type, response mode, status, selected headers, binary response, timeout, disconnect/cancellation, duplicate request/idempotency, and payload over limit;
- path collision, unpublished workflow, disabled trigger, restart/re-registration, and public/private ingress boundary;
- exact request item shape `{ body, headers, params, query }` where documented.

The oracle harness invokes HTTP directly and records a minimal `.http` expectation. It does not inspect internal `/rest` calls.

### E. Core nodes and execution order

Begin with public, deterministic, credential-free nodes that exercise semantics needed by the first compatibility layer:

1. Manual Trigger;
2. Edit Fields (Set), including fixed/expression values, keep-only behavior, dot notation, and binary inclusion;
3. If and Switch across documented data types, AND/OR, output ports, regex, null/missing, and coercion;
4. Merge append, position, matching fields, unmatched items, clash/deep/shallow handling, and multiple matches;
5. Aggregate/Summarize for count and grouped reductions;
6. No Operation, Limit, Sort, Split Out, and Remove Duplicates as expansion cases.

Official node docs describe Edit Fields behavior, If comparisons, and Merge modes/options. [N11][N12][N13]

Pin `typeVersion` in every case. Cover workflow execution order `v1` and imported legacy `v0` separately: official settings docs say v1 completes branches in canvas order while v0 interleaves levels. Canvas coordinates can therefore be semantically relevant to a legacy fixture. [N14]

Code, external HTTP, credentials, AI, browser automation, and community nodes are not native-equivalence requirements for the first fixture wave. Cases for them verify explicit delegation, unresolved capability, or unsupported diagnostics.

### F. Import diagnostics

Every imported workflow produces a machine-readable and browser-visible Compatibility Report. Per node/expression/setting classify:

- **Native equivalent** — fixture-proven in Rust;
- **Delegated compatible** — requires an explicitly configured compatibility worker;
- **Preserved opaque** — round-trips but cannot execute;
- **Adapted with declared difference** — safe intentional semantic difference;
- **Unsupported** — blocked before publish;
- **Rejected unsafe** — forbidden capability or malformed input.

The report names the profile and exact fixture evidence, required runtime/capabilities, credential placeholders, expected side effects, and migration action. It never silently upgrades node versions, expression behavior, execution order, or permissions.

## Oracle harness protocol

For each case:

1. Reset the private reference instance to a known snapshot.
2. Import/create the owner-authored sanitized workflow through a documented surface.
3. Inject deterministic input through manual execution or documented webhook/API.
4. Collect only the external output/execution facts required by the case.
5. Sanitize and canonicalize using the case-specific rules.
6. Hash the normalized record and commit it with provenance.
7. Run the same case against the clean-room platform.
8. Produce a field-level diff and Compatibility Report status.

The harness is development/test tooling and may use Node.js or Python outside the production bundle. It must never import n8n packages, implementation modules, node-description code, or upstream tests.

## Legal and provenance controls

- Reference n8n only nominatively to identify the external format/profile; no endorsement claim.
- Owner-authored fixture workflow names, descriptions, test payloads, and expected diagnostics use original copy and neutral styling.
- Keep a review queue for any generated export whose redistribution status is uncertain; internal tests can remain private.
- Do not publish the frozen n8n image/package or credentials.
- Record all source URLs and hashes; public/commercial release still requires specialist legal review, as already required by the project policy.

## Acceptance criteria for this fixture program

The fixture foundation is ready when:

- the 2.39.0 oracle is pinned by immutable digest and reproducibly resettable;
- sanitizer tests prove fail-closed removal of all credential/personal fields;
- each fixture has provenance, deterministic inputs, explicit normalization, and correctness hash;
- workflow JSON, item/linking, expressions, webhook, core-node, execution-order, and diagnostics families each contain positive, negative, and boundary cases;
- no fixture or implementation task depends on n8n source, upstream tests, Enterprise files, assets, UI snapshots, or exact product prose;
- the clean-room runner can emit a field-level diff and Compatibility Report;
- adding a later Compatibility Profile never mutates expected evidence for 2.39.0.

## Primary sources

- **N1 — Frozen release metadata:** https://github.com/n8n-io/n8n/releases/tag/n8n@2.39.0
- **N2 — n8n Sustainable Use License explanation/source coverage:** https://github.com/n8n-io/n8n-docs/blob/main/docs/sustainable-use-license.md
- **N3 — Official Docker installation/version selection:** https://docs.n8n.io/deploy/host-n8n/install-options/install-with-docker
- **N4 — Official npm installation/version selection:** https://docs.n8n.io/deploy/host-n8n/install-options/install-with-npm
- **N5 — Workflow JSON export/import and credential warning:** https://docs.n8n.io/build/manage-workflows/export-and-import
- **N6 — Public API workflow object requirements/reference:** https://docs.n8n.io/connect/n8n-api/workflow
- **N7 — n8n item data structure:** https://docs.n8n.io/build/work-with-data/understand-n8ns-data-structure
- **N8 — Item linking:** https://docs.n8n.io/build/work-with-data/reference-data/link-data-items and https://docs.n8n.io/build/work-with-data/reference-data/reference-previous-nodes
- **N9 — Expression reference:** https://docs.n8n.io/build/work-with-data/transform-data/expression-reference
- **N10 — Webhook behavior:** https://docs.n8n.io/integrations/builtin/core-nodes/n8n-nodes-base.webhook
- **N11 — Edit Fields (Set):** https://docs.n8n.io/integrations/builtin/core-nodes/n8n-nodes-base.set
- **N12 — If:** https://docs.n8n.io/integrations/builtin/core-nodes/n8n-nodes-base.if
- **N13 — Merge:** https://docs.n8n.io/integrations/builtin/core-nodes/n8n-nodes-base.merge
- **N14 — Workflow execution order:** https://docs.n8n.io/build/manage-workflows/configure-workflow-settings
