# Canopy Node Contract SDK

Apache-2.0 interoperability material for the unstable `v1alpha1` Node Contract. `meta-schema.json` defines the normative top-level boundary; namespaced `extensions` round-trip without authority. Canonical JSON recursively sorts object keys, retains array order and JSON scalar distinctions, emits compact UTF-8 JSON, and uses an algorithm-tagged `sha256:<lowercase-hex>` digest. A Node Contract Lock pins namespace, name, semantic version, and that exact digest.

The conformance fixture schema and Manual Trigger fixture are implementation-independent inputs. `v1alpha1` does not promise a stable Rust binary ABI.
