// SPDX-License-Identifier: AGPL-3.0-or-later
//! Deterministic revision compiler and immutable publication store.
//!
//! `compile_workflow` is a pure transformation (ADR 0007): one Workflow
//! Revision document, the exact Node Contract Locks, one Compatibility
//! Profile, and one versioned policy in; a canonical Execution Plan or
//! structured diagnostics out. It performs no database, network,
//! filesystem, clock, random, or executor I/O, so the same inputs always
//! produce the same plan bytes and the same algorithm-tagged digests.
//!
//! Published Revisions are insert-only: publishing stores the frozen
//! document, its digest, the pinned Execution Plan, the publication
//! evidence, and an HMAC signature derived from the Owner master key.
//! Rollback only moves the `current_revision` pointer; it never edits or
//! deletes an immutable revision (ADR 0026).

use crate::config::ServeConfig;
use crate::draft::{
    lease_holder_generation_matches, load_draft_document, NodeInstance, WorkflowDraft,
};
use crate::security::SecurityService;
use canopy_node_contract::{digest as canonical_digest, lock, NodeContractLock};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const COMPILER_ALGORITHM: &str = "canopy.compiler/1";
pub const PLAN_FORMAT: &str = "canopy.execution-plan/1";
pub const CONTRACT_API: &str = "v1alpha1";
pub const PROFILE_ID: &str = "native";
pub const LANE_INLINE_NATIVE: &str = "inline-native";
pub const SIGNATURE_ALGORITHM: &str = "canopy.revision-signature/1";
const MAX_DIAGNOSTICS: usize = 64;

#[derive(Debug, Clone, Copy)]
pub struct CompilePolicy {
    pub allowed_effect_classes: &'static [&'static str],
    pub allowed_capabilities: &'static [&'static str],
    pub max_wall_millis: u64,
    pub max_cpu_millis: u64,
    pub max_memory_bytes: u64,
    pub max_input_bytes: u64,
    pub max_output_bytes: u64,
    pub max_artifact_bytes: u64,
    pub max_concurrency: u64,
    pub max_input_count: u64,
    pub max_output_count: u64,
}

impl CompilePolicy {
    pub const CURRENT: Self = Self {
        allowed_effect_classes: &["pure"],
        allowed_capabilities: &[],
        max_wall_millis: 60_000,
        max_cpu_millis: 10_000,
        max_memory_bytes: 256 * 1024 * 1024,
        max_input_bytes: 64 * 1024 * 1024,
        max_output_bytes: 64 * 1024 * 1024,
        max_artifact_bytes: 256 * 1024 * 1024,
        max_concurrency: 16,
        max_input_count: 100_000,
        max_output_count: 100_000,
    };
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CompileDiagnostic {
    pub code: String,
    pub severity: String,
    pub require_acknowledgement: bool,
    pub path: String,
    pub message: String,
}

impl CompileDiagnostic {
    fn error(code: &str, path: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: "error".into(),
            require_acknowledgement: false,
            path: path.into(),
            message: message.into(),
        }
    }
    fn warning(code: &str, path: &str, message: impl Into<String>, require_acknowledgement: bool) -> Self {
        Self {
            code: code.into(),
            severity: "warning".into(),
            require_acknowledgement,
            path: path.into(),
            message: message.into(),
        }
    }
    fn has_errors(diagnostics: &[CompileDiagnostic]) -> bool {
        diagnostics.iter().any(|diagnostic| diagnostic.severity == "error")
    }
}

#[derive(Clone)]
pub struct ContractEntry {
    pub lock: NodeContractLock,
    pub contract: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPlan {
    pub plan_format: String,
    pub compiler: PlanCompilerIdentity,
    pub compatibility_profile: PlanProfile,
    pub revision: PlanRevision,
    pub contract_locks: Vec<NodeContractLock>,
    pub nodes: Vec<PlanNode>,
    pub connections: Vec<PlanConnection>,
    pub topological_order: Vec<String>,
    pub plan_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanCompilerIdentity {
    pub algorithm: String,
    pub contract_api: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanProfile {
    pub id: String,
    pub contract_api: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanRevision {
    pub workflow_id: String,
    pub draft_version: u64,
    pub document_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanNode {
    pub instance_id: String,
    pub name: String,
    pub contract: NodeContractLock,
    pub configuration_digest: String,
    pub activation: PlanActivation,
    pub effects: PlanEffects,
    pub capabilities: Vec<String>,
    pub budgets: PlanBudgets,
    pub lane: String,
    pub depth: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanActivation {
    pub shape: String,
    pub ordering: String,
    pub bounded_batch: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanEffects {
    pub class: String,
    pub deterministic: bool,
    pub idempotent: bool,
    pub retry: String,
    pub reconciliation: String,
    pub approval_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanBudgets {
    pub default: Budget,
    pub hard: Budget,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    pub artifact_bytes: u64,
    pub concurrency: u64,
    pub cpu_millis: u64,
    pub input_bytes: u64,
    pub input_count: u64,
    pub memory_bytes: u64,
    pub output_bytes: u64,
    pub output_count: u64,
    pub wall_millis: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanPort {
    pub node: String,
    pub port: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanConnection {
    pub source: PlanPort,
    pub target: PlanPort,
}

pub enum CompileOutcome {
    Plan {
        plan: ExecutionPlan,
        diagnostics: Vec<CompileDiagnostic>,
    },
    Failed {
        diagnostics: Vec<CompileDiagnostic>,
    },
}

pub fn compile_workflow(
    document: &WorkflowDraft,
    contracts: &BTreeMap<String, ContractEntry>,
    policy: &CompilePolicy,
) -> CompileOutcome {
    let mut diagnostics: Vec<CompileDiagnostic> = Vec::new();
    let mut push = |diagnostic: CompileDiagnostic| {
        if diagnostics.len() < MAX_DIAGNOSTICS {
            diagnostics.push(diagnostic);
        }
    };

    let mut seen_ids: BTreeSet<String> = BTreeSet::new();
    let mut sources: Vec<usize> = Vec::new();
    let mut node_entry: Vec<Option<&ContractEntry>> = Vec::with_capacity(document.nodes.len());

    for (index, node) in document.nodes.iter().enumerate() {
        let path = format!("nodes/{}", node.id);
        if !seen_ids.insert(node.id.clone()) {
            push(CompileDiagnostic::error(
                "duplicate_node_id",
                &path,
                "Node Instance identity appears more than once in the Workflow",
            ));
            node_entry.push(None);
            continue;
        }
        let key = lock_key(&node.contract_lock);
        let Some(entry) = contracts.get(&key) else {
            push(CompileDiagnostic::error(
                "unknown_contract_lock",
                &format!("{path}.contract_lock"),
                "Node Contract Lock is not part of this Compatibility Profile catalog",
            ));
            node_entry.push(None);
            continue;
        };
        if node.contract_lock != entry.lock {
            push(CompileDiagnostic::error(
                "contract_lock_digest_mismatch",
                &format!("{path}.contract_lock"),
                "Node Contract Lock digest does not match the catalog contract",
            ));
            node_entry.push(Some(entry));
            continue;
        }
        if node.contract_lock.api_version != CONTRACT_API {
            push(CompileDiagnostic::error(
                "unsupported_contract_api",
                &format!("{path}.contract_lock"),
                format!("Node Contract api_version {} is not accepted by this compiler", node.contract_lock.api_version),
            ));
            node_entry.push(Some(entry));
            continue;
        }
        let profile = entry.contract["compatibility"]["profile"].as_str().unwrap_or("");
        if profile != PROFILE_ID {
            push(CompileDiagnostic::error(
                "compatibility_profile_mismatch",
                &path,
                format!("Node Contract compatibility profile {profile} does not match the active profile {PROFILE_ID}"),
            ));
        }
        if let Some(list) = entry.contract["capabilities"].as_array() {
            for capability in list.iter().filter_map(Value::as_str) {
                if !policy.allowed_capabilities.contains(&capability) {
                    push(CompileDiagnostic::error(
                        "capability_not_allowed",
                        &format!("{path}.contract"),
                        format!("capability {capability} is not granted by this Compatibility Profile"),
                    ));
                }
            }
        }
        let effects = &entry.contract["effects"];
        let class = effects["class"].as_str().unwrap_or("");
        if !policy.allowed_effect_classes.contains(&class) {
            push(CompileDiagnostic::error(
                "effect_not_allowed",
                &format!("{path}.contract"),
                format!("Effect class {class} is not runnable in this Compatibility Profile"),
            ));
        }
        if effects["approval_required"].as_bool() == Some(true) {
            push(CompileDiagnostic::error(
                "approval_required",
                &format!("{path}.contract"),
                "Nodes whose effects require approval are not runnable in this Compatibility Profile",
            ));
        }
        if effects["deterministic"].as_bool() != Some(true) {
            push(CompileDiagnostic::error(
                "non_deterministic_effect",
                &format!("{path}.contract"),
                "Only deterministic Node Contracts produce reproducible Runs in this Compatibility Profile",
            ));
        }
        check_budgets(&entry.contract, &path, policy, &mut |diagnostic| push(diagnostic));
        validate_schema(
            &node.configuration,
            &entry.contract["configuration"]["schema"],
            &format!("{path}.configuration"),
            &mut |diagnostic| push(diagnostic),
        );
        scan_expressions(
            &node.configuration,
            &format!("{path}.configuration"),
            &mut |diagnostic| push(diagnostic),
        );
        if entry.contract["activation"]["shape"].as_str() == Some("source") {
            sources.push(index);
        }
        node_entry.push(Some(entry));
    }

    let mut name_counts: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, node) in document.nodes.iter().enumerate() {
        name_counts.entry(node.name.clone()).or_default().push(index);
    }
    for (name, indices) in &name_counts {
        if indices.len() > 1 {
            for index in indices {
                push(CompileDiagnostic::warning(
                    "duplicate_node_name",
                    &format!("nodes/{}", document.nodes[*index].id),
                    format!("Node Instances share the display name {name}; display names are not unique identities"),
                    true,
                ));
            }
        }
    }

    if sources.is_empty() {
        push(CompileDiagnostic::error(
            "workflow_requires_source",
            "workflow",
            "Workflow requires exactly one source Node Instance; add a Manual Trigger",
        ));
    } else if sources.len() > 1 {
        push(CompileDiagnostic::error(
            "multiple_triggers",
            "workflow",
            format!("Workflow declares {} source Node Instances; exactly one is allowed", sources.len()),
        ));
    }

    #[derive(Clone)]
    struct RawConnection {
        source_node: String,
        source_port: String,
        target_node: String,
        target_port: String,
    }
    let mut connections: Vec<RawConnection> = Vec::new();
    let mut seen_edges: BTreeSet<String> = BTreeSet::new();
    for (index, raw) in document.connections.iter().enumerate() {
        let path = format!("connections[{index}]");
        let Some(object) = raw.as_object() else {
            push(CompileDiagnostic::error(
                "invalid_connection",
                &path,
                "Connection must declare exactly source and target endpoints",
            ));
            continue;
        };
        if object.len() != 2 || !object.contains_key("source") || !object.contains_key("target") {
            push(CompileDiagnostic::error(
                "invalid_connection",
                &path,
                "Connection must declare exactly source and target endpoints",
            ));
            continue;
        }
        let Some((source_node, source_port)) = parse_endpoint(
            &object["source"],
            "source",
            "output_port",
            &format!("{path}.source"),
            &mut |diagnostic| push(diagnostic),
        ) else {
            continue;
        };
        let Some((target_node, target_port)) = parse_endpoint(
            &object["target"],
            "target",
            "input_port",
            &format!("{path}.target"),
            &mut |diagnostic| push(diagnostic),
        ) else {
            continue;
        };
        if source_node == target_node {
            push(CompileDiagnostic::error(
                "self_connection",
                &path,
                "A Connection cannot route a Node Instance to itself without an explicit loop construct",
            ));
            continue;
        }
        let edge = format!("{source_node}|{source_port}|{target_node}|{target_port}");
        if !seen_edges.insert(edge) {
            push(CompileDiagnostic::error(
                "duplicate_connection",
                &path,
                "The same source and target port pair is declared more than once",
            ));
            continue;
        }
        connections.push(RawConnection {
            source_node,
            source_port,
            target_node,
            target_port,
        });
    }

    let node_index: BTreeMap<&str, usize> = document
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect();
    let mut edges: Vec<(usize, usize)> = Vec::new();
    let mut trigger_output_used: BTreeSet<usize> = BTreeSet::new();
    let mut node_has_input: BTreeSet<usize> = BTreeSet::new();
    for raw in &connections {
        let (Some(source_index), Some(target_index)) = (
            node_index.get(raw.source_node.as_str()).copied(),
            node_index.get(raw.target_node.as_str()).copied(),
        ) else {
            push(CompileDiagnostic::error(
                "connection_unknown_node",
                &format!("connections/{}", raw.source_node),
                "Connection references a Node Instance that does not exist",
            ));
            continue;
        };
        let Some(source_entry) = node_entry.get(source_index).copied().flatten() else {
            continue;
        };
        let outputs = source_entry.contract["ports"]["outputs"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|port| port["id"].as_str().map(str::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !outputs.contains(&raw.source_port) {
            push(CompileDiagnostic::error(
                "connection_unknown_output_port",
                &format!("connections/{source_index}.source"),
                format!("output port {} is not declared by the source contract", raw.source_port),
            ));
            continue;
        }
        if source_entry.contract["activation"]["shape"].as_str() == Some("source") {
            trigger_output_used.insert(source_index);
        }
        let Some(target_entry) = node_entry.get(target_index).copied().flatten() else {
            continue;
        };
        let inputs = target_entry.contract["ports"]["inputs"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|port| port["id"].as_str().map(str::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !inputs.contains(&raw.target_port) {
            push(CompileDiagnostic::error(
                "connection_unknown_input_port",
                &format!("connections/{target_index}.target"),
                format!("input port {} is not declared by the target contract", raw.target_port),
            ));
            continue;
        }
        node_has_input.insert(target_index);
        edges.push((source_index, target_index));
    }

    if CompileDiagnostic::has_errors(&diagnostics) {
        return CompileOutcome::Failed { diagnostics };
    }

    let order = match topological_order(&document.nodes, &edges) {
        Topology::Order(order) => order,
        Topology::Cycle(path) => {
            push(CompileDiagnostic::error(
                "unexpected_cycle",
                &path,
                "Cycles are only valid through explicit loop, map, wait, or retry constructs; none exist in this Compatibility Profile",
            ));
            return CompileOutcome::Failed { diagnostics };
        }
    };

    for source_index in &sources {
        if !trigger_output_used.contains(source_index) {
            let node = &document.nodes[*source_index];
            push(CompileDiagnostic::warning(
                "unconnected_trigger_output",
                &format!("nodes/{}", node.id),
                "The Manual Trigger output is not consumed; a Run will complete without downstream work",
                true,
            ));
        }
    }
    for (index, node) in document.nodes.iter().enumerate() {
        let is_source = node_entry[index]
            .is_some_and(|entry| entry.contract["activation"]["shape"].as_str() == Some("source"));
        if !is_source && !node_has_input.contains(&index) {
            push(CompileDiagnostic::warning(
                "unconnected_node",
                &format!("nodes/{}", node.id),
                "Node Instance receives no input Connection and will never execute",
                false,
            ));
        }
    }

    let document_value = serde_json::to_value(document).expect("Workflow Revision serializes");
    let document_digest = canonical_digest(&document_value).expect("digest is deterministic");

    let mut contract_locks: BTreeSet<(String, String, String)> = BTreeSet::new();
    let mut plan_nodes: Vec<PlanNode> = Vec::with_capacity(document.nodes.len());
    for index in &order.0 {
        let node = &document.nodes[*index];
        let Some(entry) = node_entry[*index] else {
            continue;
        };
        contract_locks.insert((
            entry.lock.namespace.clone(),
            entry.lock.name.clone(),
            entry.lock.version.clone(),
        ));
        let contract = &entry.contract;
        let capabilities = contract["capabilities"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let effects = &contract["effects"];
        plan_nodes.push(PlanNode {
            instance_id: node.id.clone(),
            name: node.name.clone(),
            contract: entry.lock.clone(),
            configuration_digest: canonical_digest(&node.configuration).expect("configuration digests"),
            activation: PlanActivation {
                shape: contract["activation"]["shape"].as_str().unwrap_or_default().to_string(),
                ordering: contract["activation"]["ordering"].as_str().unwrap_or_default().to_string(),
                bounded_batch: contract["activation"]["bounded_batch"].clone(),
            },
            effects: PlanEffects {
                class: effects["class"].as_str().unwrap_or_default().to_string(),
                deterministic: effects["deterministic"].as_bool() == Some(true),
                idempotent: effects["idempotent"].as_bool() == Some(true),
                retry: effects["retry"].as_str().unwrap_or_default().to_string(),
                reconciliation: effects["reconciliation"].as_str().unwrap_or_default().to_string(),
                approval_required: effects["approval_required"].as_bool() == Some(true),
            },
            capabilities,
            budgets: PlanBudgets {
                default: budget_from(contract, "default"),
                hard: budget_from(contract, "hard"),
            },
            lane: LANE_INLINE_NATIVE.to_string(),
            depth: order.1[*index],
        });
    }

    let contract_locks: Vec<NodeContractLock> = contract_locks
        .into_iter()
        .filter_map(|(namespace, name, version)| {
            contracts
                .get(&format!("{namespace}/{name}/{version}"))
                .map(|entry| entry.lock.clone())
        })
        .collect();

    let plan_connections: Vec<PlanConnection> = connections
        .iter()
        .map(|raw| PlanConnection {
            source: PlanPort {
                node: raw.source_node.clone(),
                port: raw.source_port.clone(),
            },
            target: PlanPort {
                node: raw.target_node.clone(),
                port: raw.target_port.clone(),
            },
        })
        .collect();

    let topological_order: Vec<String> = order
        .0
        .iter()
        .map(|index| document.nodes[*index].id.clone())
        .collect();

    // The digest binds the canonical plan body without the digest field
    // itself, so clients can recompute it from any stored plan by dropping
    // its own `plan_digest` key.
    let body = json!({
        "plan_format": PLAN_FORMAT,
        "compiler": { "algorithm": COMPILER_ALGORITHM, "contract_api": CONTRACT_API },
        "compatibility_profile": { "id": PROFILE_ID, "contract_api": CONTRACT_API },
        "revision": {
            "workflow_id": document.workflow_id,
            "draft_version": document.draft_version,
            "document_digest": document_digest,
        },
        "contract_locks": contract_locks,
        "nodes": plan_nodes,
        "connections": plan_connections,
        "topological_order": topological_order,
    });
    let plan_digest = canonical_digest(&body).expect("plan digests are deterministic");
    let plan = ExecutionPlan {
        plan_format: PLAN_FORMAT.to_string(),
        compiler: PlanCompilerIdentity {
            algorithm: COMPILER_ALGORITHM.to_string(),
            contract_api: CONTRACT_API.to_string(),
        },
        compatibility_profile: PlanProfile {
            id: PROFILE_ID.to_string(),
            contract_api: CONTRACT_API.to_string(),
        },
        revision: PlanRevision {
            workflow_id: document.workflow_id.clone(),
            draft_version: document.draft_version,
            document_digest,
        },
        contract_locks,
        nodes: plan_nodes,
        connections: plan_connections,
        topological_order,
        plan_digest,
    };
    CompileOutcome::Plan {
        plan,
        diagnostics,
    }
}

fn lock_key(lock: &NodeContractLock) -> String {
    format!("{}/{}/{}", lock.namespace, lock.name, lock.version)
}

enum Topology {
    Order((Vec<usize>, Vec<u64>)),
    Cycle(String),
}

fn topological_order(nodes: &[NodeInstance], edges: &[(usize, usize)]) -> Topology {
    let count = nodes.len();
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); count];
    let mut indegree = vec![0u32; count];
    for (source, target) in edges {
        adjacency[*source].push(*target);
        indegree[*target] += 1;
    }
    let mut ready: Vec<usize> = (0..count).filter(|index| indegree[*index] == 0).collect();
    let mut order: Vec<usize> = Vec::with_capacity(count);
    let mut depth: Vec<u64> = vec![0; count];
    while let Some(index) = ready.first().copied() {
        ready.remove(0);
        order.push(index);
        for &dependent in &adjacency[index] {
            depth[dependent] = depth[dependent].max(depth[index] + 1);
            indegree[dependent] -= 1;
            if indegree[dependent] == 0 {
                ready.push(dependent);
            }
        }
    }
    if order.len() == count {
        return Topology::Order((order, depth));
    }
    // Cycle reporting: walk from the first node Kahn's algorithm could not
    // order and stop when the walk meets a node already on its own path.
    let mut ordered: BTreeSet<usize> = order.iter().copied().collect();
    let Some(start) = (0..count).find(|index| !ordered.contains(index)) else {
        return Topology::Cycle("cycle".to_string());
    };
    let mut path: Vec<usize> = Vec::new();
    let mut cursor = start;
    loop {
        if let Some(position) = path.iter().position(|index| *index == cursor) {
            let cycle = path.split_off(position);
            return Topology::Cycle(
                cycle
                    .iter()
                    .map(|index| nodes[*index].id.clone())
                    .collect::<Vec<_>>()
                    .join(" -> "),
            );
        }
        path.push(cursor);
        let Some(next) = adjacency[cursor]
            .iter()
            .copied()
            .find(|candidate| !ordered.contains(candidate) || path.contains(candidate))
        else {
            return Topology::Cycle(
                path.iter()
                    .map(|index| nodes[*index].id.clone())
                    .collect::<Vec<_>>()
                    .join(" -> "),
            );
        };
        cursor = next;
    }
}

fn parse_endpoint(
    value: &Value,
    side: &str,
    port_key: &str,
    path: &str,
    push: &mut impl FnMut(CompileDiagnostic),
) -> Option<(String, String)> {
    let object = value.as_object()?;
    if object.len() != 2 || !object.contains_key("node") || !object.contains_key(port_key) {
        push(CompileDiagnostic::error(
            "invalid_connection",
            path,
            format!("Connection endpoint {side} must declare exactly node and {port_key}"),
        ));
        return None;
    }
    let node = object["node"].as_str()?;
    let port = object[port_key].as_str()?;
    if node.is_empty() || port.is_empty() {
        push(CompileDiagnostic::error(
            "invalid_connection",
            path,
            "Connection endpoint identities must be non-empty",
        ));
        return None;
    }
    Some((node.to_string(), port.to_string()))
}

fn check_budgets(
    contract: &Value,
    path: &str,
    policy: &CompilePolicy,
    push: &mut impl FnMut(CompileDiagnostic),
) {
    let fields = [
        ("artifact_bytes", policy.max_artifact_bytes),
        ("concurrency", policy.max_concurrency),
        ("cpu_millis", policy.max_cpu_millis),
        ("input_bytes", policy.max_input_bytes),
        ("input_count", policy.max_input_count),
        ("memory_bytes", policy.max_memory_bytes),
        ("output_bytes", policy.max_output_bytes),
        ("output_count", policy.max_output_count),
        ("wall_millis", policy.max_wall_millis),
    ];
    let default = &contract["resources"]["default"];
    let hard = &contract["resources"]["hard"];
    for (field, cap) in fields {
        match (
            default.get(field).and_then(Value::as_u64),
            hard.get(field).and_then(Value::as_u64),
        ) {
            (Some(default), Some(hard)) => {
                if default > hard {
                    push(CompileDiagnostic::error(
                        "budget_inconsistent",
                        &format!("{path}.resources"),
                        format!("budget {field}: default {default} exceeds hard bound {hard}"),
                    ));
                }
                if hard > cap {
                    push(CompileDiagnostic::error(
                        "budget_exceeds_policy",
                        &format!("{path}.resources"),
                        format!("budget {field}: hard bound {hard} exceeds profile cap {cap}"),
                    ));
                }
            }
            _ => {
                push(CompileDiagnostic::error(
                    "contract_budget_invalid",
                    &format!("{path}.resources.{field}"),
                    "contract budget fields must be non-negative integers",
                ));
            }
        }
    }
}

fn budget_from(contract: &Value, which: &str) -> Budget {
    let object = &contract["resources"][which];
    Budget {
        artifact_bytes: object["artifact_bytes"].as_u64().unwrap_or(0),
        concurrency: object["concurrency"].as_u64().unwrap_or(0),
        cpu_millis: object["cpu_millis"].as_u64().unwrap_or(0),
        input_bytes: object["input_bytes"].as_u64().unwrap_or(0),
        input_count: object["input_count"].as_u64().unwrap_or(0),
        memory_bytes: object["memory_bytes"].as_u64().unwrap_or(0),
        output_bytes: object["output_bytes"].as_u64().unwrap_or(0),
        output_count: object["output_count"].as_u64().unwrap_or(0),
        wall_millis: object["wall_millis"].as_u64().unwrap_or(0),
    }
}

fn type_matches(value: &Value, expected: &str) -> bool {
    match expected {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        "integer" => matches!(
            value,
            Value::Number(number)
                if number.is_i64()
                    || number.is_u64()
                    || number
                        .as_f64()
                        .is_some_and(|float| float.is_finite() && float.fract() == 0.0)
        ),
        "number" => value.is_number(),
        _ => true,
    }
}

fn validate_schema(
    value: &Value,
    schema: &Value,
    path: &str,
    push: &mut impl FnMut(CompileDiagnostic),
) {
    let Some(schema) = schema.as_object() else {
        return;
    };
    if let Some(declared) = schema.get("type") {
        let types: Vec<&str> = match declared {
            Value::String(text) => vec![text.as_str()],
            Value::Array(items) => items.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        };
        if !types.is_empty() && !types.iter().any(|type_name| type_matches(value, type_name)) {
            push(CompileDiagnostic::error(
                "invalid_configuration",
                path,
                format!("value does not match declared type {declared}"),
            ));
            return;
        }
    }
    if let Some(allowed) = schema.get("enum").and_then(Value::as_array) {
        if !allowed.iter().any(|candidate| candidate == value) {
            push(CompileDiagnostic::error(
                "invalid_configuration",
                path,
                format!("value is not one of the allowed values {allowed}"),
            ));
            return;
        }
    }
    if let Some(constant) = schema.get("const") {
        if constant != value {
            push(CompileDiagnostic::error(
                "invalid_configuration",
                path,
                format!("value must equal the declared constant {constant}"),
            ));
            return;
        }
    }
    if let Some(object) = value.as_object() {
        if let Some(required) = schema.get("required").and_then(Value::as_array) {
            for key in required.iter().filter_map(Value::as_str) {
                if !object.contains_key(key) {
                    push(CompileDiagnostic::error(
                        "invalid_configuration",
                        &format!("{path}.{key}"),
                        "required field is missing",
                    ));
                }
            }
        }
        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            for (key, sub_schema) in properties {
                if let Some(child) = object.get(key) {
                    validate_schema(child, sub_schema, &format!("{path}.{key}"), push);
                }
            }
        }
        if let Some(additional) = schema.get("additionalProperties") {
            let declared = schema
                .get("properties")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            for key in object.keys() {
                if declared.contains_key(key) {
                    continue;
                }
                match additional {
                    Value::Bool(false) => {
                        push(CompileDiagnostic::error(
                            "invalid_configuration",
                            &format!("{path}.{key}"),
                            "field is not declared by the contract configuration schema",
                        ));
                    }
                    Value::Bool(true) => {}
                    sub_schema => validate_schema(&object[key], sub_schema, &format!("{path}.{key}"), push),
                }
            }
        }
    }
    if let Some(items) = value.as_array() {
        if let Some(sub_schema) = schema.get("items") {
            for (index, item) in items.iter().enumerate() {
                validate_schema(item, sub_schema, &format!("{path}[{index}]"), push);
            }
        }
        if let Some(minimum) = schema.get("minItems").and_then(Value::as_u64) {
            if (items.len() as u64) < minimum {
                push(CompileDiagnostic::error(
                    "invalid_configuration",
                    path,
                    format!("array has fewer than {minimum} items"),
                ));
            }
        }
        if let Some(maximum) = schema.get("maxItems").and_then(Value::as_u64) {
            if (items.len() as u64) > maximum {
                push(CompileDiagnostic::error(
                    "invalid_configuration",
                    path,
                    format!("array has more than {maximum} items"),
                ));
            }
        }
    }
    if let Some(number) = value.as_f64() {
        if let Some(minimum) = schema.get("minimum").and_then(Value::as_f64) {
            if number < minimum {
                push(CompileDiagnostic::error(
                    "invalid_configuration",
                    path,
                    format!("value {number} is below minimum {minimum}"),
                ));
            }
        }
        if let Some(maximum) = schema.get("maximum").and_then(Value::as_f64) {
            if number > maximum {
                push(CompileDiagnostic::error(
                    "invalid_configuration",
                    path,
                    format!("value {number} is above maximum {maximum}"),
                ));
            }
        }
    }
    if let Some(text) = value.as_str() {
        let length = text.chars().count() as u64;
        if let Some(minimum) = schema.get("minLength").and_then(Value::as_u64) {
            if length < minimum {
                push(CompileDiagnostic::error(
                    "invalid_configuration",
                    path,
                    format!("string is shorter than {minimum} characters"),
                ));
            }
        }
        if let Some(maximum) = schema.get("maxLength").and_then(Value::as_u64) {
            if length > maximum {
                push(CompileDiagnostic::error(
                    "invalid_configuration",
                    path,
                    format!("string is longer than {maximum} characters"),
                ));
            }
        }
    }
}

fn scan_expressions(value: &Value, path: &str, push: &mut impl FnMut(CompileDiagnostic)) {
    match value {
        Value::String(text) if text.contains("{{") || text.contains("}}") => {
            push(CompileDiagnostic::error(
                "unsupported_expression",
                path,
                "expression markers are not supported in this Compatibility Profile yet",
            ));
        }
        Value::Object(map) => {
            for (key, child) in map {
                scan_expressions(child, &format!("{path}.{key}"), push);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                scan_expressions(child, &format!("{path}[{index}]"), push);
            }
        }
        _ => {}
    }
}

#[derive(Debug)]
pub enum RevisionError {
    NotFound,
    Stale { current: u64 },
    LeaseRequired,
    Compilation { diagnostics: Vec<CompileDiagnostic> },
    WarningsUnacknowledged { required: Vec<String> },
    NoPrecedingRevision,
    Storage(String),
}

impl From<String> for RevisionError {
    fn from(reason: String) -> Self {
        RevisionError::Storage(reason)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishRequest {
    pub editor_session_id: String,
    pub lease_generation: u64,
    pub base_draft_version: u64,
    #[serde(default)]
    pub acknowledged_warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct PlanIdentity {
    pub plan_format: &'static str,
    pub compiler_algorithm: &'static str,
    pub plan_digest: String,
    pub node_count: usize,
    pub connection_count: usize,
}

#[derive(Serialize)]
pub struct CompilePreview {
    pub workflow_id: String,
    pub draft_version: u64,
    pub status: String,
    pub diagnostics: Vec<CompileDiagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<PlanIdentity>,
}

#[derive(Serialize)]
pub struct PublicationEvidence {
    pub draft_version: u64,
    pub editor_session_id: String,
    pub lease_generation: u64,
    pub acknowledged_warnings: Vec<String>,
    pub diagnostics: Vec<CompileDiagnostic>,
    pub signature_algorithm: &'static str,
}

#[derive(Serialize)]
pub struct PublishedRevision {
    pub workflow_id: String,
    pub revision_number: u64,
    pub document: WorkflowDraft,
    pub document_digest: String,
    pub plan: ExecutionPlan,
    pub plan_digest: String,
    pub plan_format: &'static str,
    pub compiler_algorithm: &'static str,
    pub contract_locks: Vec<NodeContractLock>,
    pub compatibility_profile: PlanProfile,
    pub signature: String,
    pub published_at: i64,
    pub evidence: PublicationEvidence,
}

#[derive(Serialize)]
pub struct RevisionSummary {
    pub revision_number: u64,
    pub status: &'static str,
    pub draft_version: u64,
    pub document_digest: String,
    pub plan_digest: String,
    pub plan_format: String,
    pub compiler_algorithm: String,
    pub signature: String,
    pub published_at: i64,
}

#[derive(Serialize)]
pub struct PublicationView {
    pub workflow_id: String,
    pub current_revision: Option<u64>,
    pub revisions: Vec<RevisionSummary>,
}

#[derive(Serialize)]
pub struct RevisionRecord {
    pub workflow_id: String,
    pub revision_number: u64,
    pub status: &'static str,
    pub document: WorkflowDraft,
    pub document_digest: String,
    pub plan: Value,
    pub plan_digest: String,
    pub plan_format: String,
    pub compiler_algorithm: String,
    pub contract_locks: Vec<NodeContractLock>,
    pub compatibility_profile: Value,
    pub signature: String,
    pub published_at: i64,
    pub evidence: Value,
}

#[derive(Serialize)]
pub struct DiffNodeRef {
    pub id: String,
    pub name: String,
}

#[derive(Serialize)]
pub struct DiffNodeModified {
    pub id: String,
    pub name: String,
    pub changed: Vec<String>,
}

#[derive(Serialize)]
pub struct DiffNodes {
    pub added: Vec<DiffNodeRef>,
    pub removed: Vec<DiffNodeRef>,
    modified: Vec<DiffNodeModified>,
}

impl DiffNodes {
    fn empty() -> Self {
        Self {
            added: vec![],
            removed: vec![],
            modified: vec![],
        }
    }
}

#[derive(Serialize)]
pub struct DiffConnections {
    added: Vec<String>,
    removed: Vec<String>,
}

impl DiffConnections {
    fn empty() -> Self {
        Self {
            added: vec![],
            removed: vec![],
        }
    }
}

#[derive(Serialize)]
pub struct DiffFields {
    pub name: bool,
    pub annotation: bool,
    pub settings: bool,
    pub compatibility_metadata: bool,
}

#[derive(Serialize)]
pub struct DraftDiffView {
    pub workflow_id: String,
    pub draft_version: u64,
    pub published_revision: Option<u64>,
    pub nodes: DiffNodes,
    pub connections: DiffConnections,
    pub workflow_fields: DiffFields,
}

#[derive(Clone)]
pub struct RevisionService {
    database: PathBuf,
    catalog: BTreeMap<String, ContractEntry>,
    policy: CompilePolicy,
    security: Arc<SecurityService>,
}

impl RevisionService {
    pub fn initialize(config: &ServeConfig, security: Arc<SecurityService>) -> Result<Self, String> {
        let manual_contract: Value = serde_json::from_str(include_str!(
            "../../../contracts/manual-trigger.v1alpha1.json"
        ))
        .map_err(|error| error.to_string())?;
        let manual_lock = lock(&manual_contract).map_err(|error| format!("manual trigger contract: {error}"))?;
        let mut catalog = BTreeMap::new();
        catalog.insert(
            lock_key(&manual_lock),
            ContractEntry {
                lock: manual_lock,
                contract: manual_contract,
            },
        );
        let service = Self {
            database: config.state_dir.join("workflow.sqlite3"),
            catalog,
            policy: CompilePolicy::CURRENT,
            security,
        };
        let connection = service.connect()?;
        connection
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS published_revisions(
                    workflow_id TEXT NOT NULL REFERENCES workflow_drafts(workflow_id) ON DELETE CASCADE,
                    revision_number INTEGER NOT NULL,
                    draft_revision INTEGER NOT NULL,
                    document_json TEXT NOT NULL,
                    document_digest TEXT NOT NULL,
                    plan_json TEXT NOT NULL,
                    plan_digest TEXT NOT NULL,
                    plan_format TEXT NOT NULL,
                    compiler_algorithm TEXT NOT NULL,
                    contract_locks_json TEXT NOT NULL,
                    compatibility_profile TEXT NOT NULL,
                    evidence_json TEXT NOT NULL,
                    signature TEXT NOT NULL,
                    published_at INTEGER NOT NULL,
                    PRIMARY KEY(workflow_id, revision_number)
                ) STRICT;
                CREATE TABLE IF NOT EXISTS workflow_publication_state(
                    workflow_id TEXT PRIMARY KEY REFERENCES workflow_drafts(workflow_id) ON DELETE CASCADE,
                    current_revision INTEGER NOT NULL
                ) STRICT;
                "#,
            )
            .map_err(|error| error.to_string())?;
        Ok(service)
    }

    pub fn compile_preview(&self, workflow_id: &str) -> Result<CompilePreview, RevisionError> {
        let connection = self.connect().map_err(RevisionError::Storage)?;
        let document = load_draft_document(&connection, workflow_id)?;
        Ok(self.preview(&document))
    }

    fn preview(&self, document: &WorkflowDraft) -> CompilePreview {
        match compile_workflow(document, &self.catalog, &self.policy) {
            CompileOutcome::Plan { plan, diagnostics } => CompilePreview {
                workflow_id: document.workflow_id.clone(),
                draft_version: document.draft_version,
                status: if diagnostics.is_empty() { "ok" } else { "warnings" }.into(),
                diagnostics,
                plan: Some(PlanIdentity {
                    plan_format: PLAN_FORMAT,
                    compiler_algorithm: COMPILER_ALGORITHM,
                    plan_digest: plan.plan_digest,
                    node_count: plan.nodes.len(),
                    connection_count: plan.connections.len(),
                }),
            },
            CompileOutcome::Failed { diagnostics } => CompilePreview {
                workflow_id: document.workflow_id.clone(),
                draft_version: document.draft_version,
                status: "failed".into(),
                diagnostics,
                plan: None,
            },
        }
    }

    pub fn publish(
        &self,
        workflow_id: &str,
        request: PublishRequest,
    ) -> Result<PublishedRevision, RevisionError> {
        let mut connection = self.connect().map_err(RevisionError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| RevisionError::Storage(error.to_string()))?;
        require_workflow(&transaction, workflow_id)?;
        let now = now_ms();
        let holder = lease_holder_generation_matches(
            &transaction,
            workflow_id,
            &request.editor_session_id,
            request.lease_generation,
            now,
        )?;
        if !holder {
            return Err(RevisionError::LeaseRequired);
        }
        // Publication flushes pending Draft Commands: every Draft Command is
        // applied synchronously when accepted, and this write transaction
        // serializes with all of them, so the authoritative document read
        // here already includes every acknowledged command and no command
        // can land between the read and the immutable store below.
        let document = load_draft_document(&transaction, workflow_id)?;
        if document.draft_version != request.base_draft_version {
            return Err(RevisionError::Stale {
                current: document.draft_version,
            });
        }
        let (diagnostics, plan) = match compile_workflow(&document, &self.catalog, &self.policy) {
            CompileOutcome::Plan { plan, diagnostics } => (diagnostics, plan),
            CompileOutcome::Failed { diagnostics } => {
                return Err(RevisionError::Compilation { diagnostics })
            }
        };
        let required: Vec<String> = diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.severity == "warning" && diagnostic.require_acknowledgement
            })
            .map(|diagnostic| diagnostic.code.clone())
            .filter(|code| {
                !request.acknowledged_warnings.iter().any(|acknowledged| acknowledged == code)
            })
            .collect();
        if !required.is_empty() {
            return Err(RevisionError::WarningsUnacknowledged { required });
        }

        let document_value = serde_json::to_value(&document).expect("document serializes");
        let document_digest = canonical_digest(&document_value).expect("digest is deterministic");
        let plan_value = serde_json::to_value(&plan).expect("plan serializes");
        // The plan digest is pinned by the compiler over the canonical plan
        // body (the plan minus its own plan_digest field); reuse it so the
        // stored digest and the embedded digest are always the same value.
        let plan_digest = plan.plan_digest.clone();
        let contract_locks: Vec<NodeContractLock> = plan_value["contract_locks"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| serde_json::from_value(item.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();
        let revision_number: i64 = transaction
            .query_row(
                "SELECT COALESCE(MAX(revision_number),0)+1 FROM published_revisions WHERE workflow_id=?1",
                params![workflow_id],
                |row| row.get(0),
            )
            .map_err(|error| RevisionError::Storage(error.to_string()))?;
        let signature_payload = json!({
            "algorithm": SIGNATURE_ALGORITHM,
            "workflow_id": workflow_id,
            "revision_number": revision_number,
            "document_digest": document_digest,
            "plan_digest": plan_digest,
            "plan_format": PLAN_FORMAT,
            "compiler_algorithm": COMPILER_ALGORITHM,
            "compatibility_profile": PROFILE_ID,
        });
        let signature = self
            .security
            .sign_revision_payload(&signature_payload)
            .map_err(|error| RevisionError::Storage(format!("revision signature: {error:?}")))?;
        let evidence = PublicationEvidence {
            draft_version: document.draft_version,
            editor_session_id: request.editor_session_id.clone(),
            lease_generation: request.lease_generation,
            acknowledged_warnings: request
                .acknowledged_warnings
                .iter()
                .filter(|code| {
                    diagnostics.iter().any(|diagnostic| {
                        diagnostic.code.as_str() == code.as_str()
                            && diagnostic.require_acknowledgement
                    })
                })
                .cloned()
                .collect(),
            diagnostics: diagnostics.clone(),
            signature_algorithm: SIGNATURE_ALGORITHM,
        };
        let document_json =
            serde_json::to_string(&document).map_err(|error| RevisionError::Storage(error.to_string()))?;
        let plan_json = plan_value.to_string();
        let contract_locks_json =
            serde_json::to_string(&contract_locks).map_err(|error| RevisionError::Storage(error.to_string()))?;
        let evidence_json =
            serde_json::to_string(&evidence).map_err(|error| RevisionError::Storage(error.to_string()))?;
        transaction
            .execute(
                "INSERT INTO published_revisions(workflow_id,revision_number,draft_revision,document_json,document_digest,plan_json,plan_digest,plan_format,compiler_algorithm,contract_locks_json,compatibility_profile,evidence_json,signature,published_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
                params![workflow_id, revision_number, document.draft_version as i64, document_json, document_digest, plan_json, plan_digest, PLAN_FORMAT, COMPILER_ALGORITHM, contract_locks_json, PROFILE_ID, evidence_json, signature, now],
            )
            .map_err(|error| RevisionError::Storage(error.to_string()))?;
        transaction
            .execute(
                "INSERT INTO workflow_publication_state(workflow_id,current_revision) VALUES(?1,?2) ON CONFLICT(workflow_id) DO UPDATE SET current_revision=excluded.current_revision",
                params![workflow_id, revision_number],
            )
            .map_err(|error| RevisionError::Storage(error.to_string()))?;
        transaction
            .commit()
            .map_err(|error| RevisionError::Storage(error.to_string()))?;
        let _ = self.security.record_audit("revision.publish", "succeeded");
        Ok(PublishedRevision {
            workflow_id: workflow_id.into(),
            revision_number: revision_number as u64,
            document,
            document_digest,
            plan,
            plan_digest,
            plan_format: PLAN_FORMAT,
            compiler_algorithm: COMPILER_ALGORITHM,
            contract_locks,
            compatibility_profile: PlanProfile {
                id: PROFILE_ID.into(),
                contract_api: CONTRACT_API.into(),
            },
            signature,
            published_at: now,
            evidence,
        })
    }

    pub fn rollback(&self, workflow_id: &str) -> Result<PublicationView, RevisionError> {
        let mut connection = self.connect().map_err(RevisionError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| RevisionError::Storage(error.to_string()))?;
        require_workflow(&transaction, workflow_id)?;
        let current: Option<i64> = transaction
            .query_row(
                "SELECT current_revision FROM workflow_publication_state WHERE workflow_id=?1",
                params![workflow_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| RevisionError::Storage(error.to_string()))?;
        let Some(current) = current else {
            return Err(RevisionError::NoPrecedingRevision);
        };
        if current < 2 {
            return Err(RevisionError::NoPrecedingRevision);
        }
        transaction
            .execute(
                "UPDATE workflow_publication_state SET current_revision=?2 WHERE workflow_id=?1",
                params![workflow_id, current - 1],
            )
            .map_err(|error| RevisionError::Storage(error.to_string()))?;
        transaction
            .commit()
            .map_err(|error| RevisionError::Storage(error.to_string()))?;
        let _ = self.security.record_audit("revision.rollback", "succeeded");
        self.publication(workflow_id)
    }

    pub fn publication(&self, workflow_id: &str) -> Result<PublicationView, RevisionError> {
        let connection = self.connect().map_err(RevisionError::Storage)?;
        require_workflow(&connection, workflow_id)?;
        let current: Option<i64> = connection
            .query_row(
                "SELECT current_revision FROM workflow_publication_state WHERE workflow_id=?1",
                params![workflow_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| RevisionError::Storage(error.to_string()))?;
        let current = current.map(|value| value as u64);
        let revisions = load_revisions(&connection, workflow_id)?;
        let summaries = revisions
            .into_iter()
            .map(|row| RevisionSummary {
                status: if current == Some(row.revision_number) {
                    "current"
                } else {
                    "superseded"
                },
                revision_number: row.revision_number,
                draft_version: row.draft_version,
                document_digest: row.document_digest,
                plan_digest: row.plan_digest,
                plan_format: row.plan_format,
                compiler_algorithm: row.compiler_algorithm,
                signature: row.signature,
                published_at: row.published_at,
            })
            .collect();
        Ok(PublicationView {
            workflow_id: workflow_id.into(),
            current_revision: current,
            revisions: summaries,
        })
    }

    pub fn revision(&self, workflow_id: &str, revision_number: u64) -> Result<RevisionRecord, RevisionError> {
        let connection = self.connect().map_err(RevisionError::Storage)?;
        require_workflow(&connection, workflow_id)?;
        let current: Option<i64> = connection
            .query_row(
                "SELECT current_revision FROM workflow_publication_state WHERE workflow_id=?1",
                params![workflow_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| RevisionError::Storage(error.to_string()))?;
        let row = revision_row(&connection, workflow_id, revision_number)?;
        let document: WorkflowDraft =
            serde_json::from_str(&row.document_json).map_err(|error| RevisionError::Storage(error.to_string()))?;
        let plan: Value =
            serde_json::from_str(&row.plan_json).map_err(|error| RevisionError::Storage(error.to_string()))?;
        let contract_locks: Vec<NodeContractLock> =
            serde_json::from_str(&row.contract_locks_json).map_err(|error| RevisionError::Storage(error.to_string()))?;
        let compatibility_profile = json!({
            "id": row.compatibility_profile,
            "contract_api": CONTRACT_API,
        });
        let evidence: Value =
            serde_json::from_str(&row.evidence_json).map_err(|error| RevisionError::Storage(error.to_string()))?;
        Ok(RevisionRecord {
            workflow_id: workflow_id.into(),
            revision_number: row.revision_number,
            status: if current == Some(row.revision_number as i64) {
                "current"
            } else {
                "superseded"
            },
            document,
            document_digest: row.document_digest,
            plan,
            plan_digest: row.plan_digest,
            plan_format: row.plan_format,
            compiler_algorithm: row.compiler_algorithm,
            contract_locks,
            compatibility_profile,
            signature: row.signature,
            published_at: row.published_at,
            evidence,
        })
    }

    pub fn diff(&self, workflow_id: &str) -> Result<DraftDiffView, RevisionError> {
        let connection = self.connect().map_err(RevisionError::Storage)?;
        require_workflow(&connection, workflow_id)?;
        let draft = load_draft_document(&connection, workflow_id)?;
        let current: Option<i64> = connection
            .query_row(
                "SELECT current_revision FROM workflow_publication_state WHERE workflow_id=?1",
                params![workflow_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| RevisionError::Storage(error.to_string()))?;
        let Some(current) = current else {
            return Ok(DraftDiffView {
                workflow_id: workflow_id.into(),
                draft_version: draft.draft_version,
                published_revision: None,
                nodes: DiffNodes::empty(),
                connections: DiffConnections::empty(),
                workflow_fields: DiffFields {
                    name: false,
                    annotation: false,
                    settings: false,
                    compatibility_metadata: false,
                },
            });
        };
        let row = revision_row(&connection, workflow_id, current as u64)?;
        let published: WorkflowDraft =
            serde_json::from_str(&row.document_json).map_err(|error| RevisionError::Storage(error.to_string()))?;
        let published_nodes: BTreeMap<String, &NodeInstance> =
            published.nodes.iter().map(|node| (node.id.clone(), node)).collect();
        let draft_nodes: BTreeMap<String, &NodeInstance> =
            draft.nodes.iter().map(|node| (node.id.clone(), node)).collect();
        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut modified = Vec::new();
        for (id, node) in &draft_nodes {
            match published_nodes.get(id) {
                None => added.push(DiffNodeRef {
                    id: id.clone(),
                    name: node.name.clone(),
                }),
                Some(previous) => {
                    let mut changed = Vec::new();
                    if previous.name != node.name {
                        changed.push("name".into());
                    }
                    if previous.contract_lock != node.contract_lock {
                        changed.push("contract_lock".into());
                    }
                    if previous.configuration != node.configuration {
                        changed.push("configuration".into());
                    }
                    if previous.layout != node.layout {
                        changed.push("layout".into());
                    }
                    if previous.annotation != node.annotation {
                        changed.push("annotation".into());
                    }
                    if previous.compatibility_metadata != node.compatibility_metadata {
                        changed.push("compatibility_metadata".into());
                    }
                    if !changed.is_empty() {
                        modified.push(DiffNodeModified {
                            id: id.clone(),
                            name: node.name.clone(),
                            changed,
                        });
                    }
                }
            }
        }
        for (id, node) in &published_nodes {
            if !draft_nodes.contains_key(id) {
                removed.push(DiffNodeRef {
                    id: id.clone(),
                    name: node.name.clone(),
                });
            }
        }
        let connection_key = |value: &Value| -> Option<String> {
            let source = &value["source"];
            let target = &value["target"];
            Some(format!(
                "{}:{} -> {}:{}",
                source["node"].as_str()?,
                source["output_port"].as_str()?,
                target["node"].as_str()?,
                target["input_port"].as_str()?
            ))
        };
        let published_connections: BTreeSet<String> = published
            .connections
            .iter()
            .filter_map(|connection| connection_key(connection))
            .collect();
        let draft_connections: BTreeSet<String> = draft
            .connections
            .iter()
            .filter_map(|connection| connection_key(connection))
            .collect();
        Ok(DraftDiffView {
            workflow_id: workflow_id.into(),
            draft_version: draft.draft_version,
            published_revision: Some(current as u64),
            nodes: DiffNodes {
                added,
                removed,
                modified,
            },
            connections: DiffConnections {
                added: draft_connections
                    .difference(&published_connections)
                    .cloned()
                    .collect(),
                removed: published_connections
                    .difference(&draft_connections)
                    .cloned()
                    .collect(),
            },
            workflow_fields: DiffFields {
                name: draft.name != published.name,
                annotation: draft.annotation != published.annotation,
                settings: draft.settings != published.settings,
                compatibility_metadata: draft.compatibility_metadata != published.compatibility_metadata,
            },
        })
    }

    fn connect(&self) -> Result<Connection, String> {
        let connection = Connection::open(&self.database).map_err(|error| error.to_string())?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|error| error.to_string())?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|error| error.to_string())?;
        connection
            .pragma_update(None, "synchronous", "FULL")
            .map_err(|error| error.to_string())?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(|error| error.to_string())?;
        Ok(connection)
    }
}

#[derive(Debug)]
struct RevisionRow {
    revision_number: u64,
    draft_version: u64,
    document_json: String,
    document_digest: String,
    plan_json: String,
    plan_digest: String,
    plan_format: String,
    compiler_algorithm: String,
    contract_locks_json: String,
    compatibility_profile: String,
    evidence_json: String,
    signature: String,
    published_at: i64,
}

fn load_revisions(connection: &Connection, workflow_id: &str) -> Result<Vec<RevisionRow>, RevisionError> {
    let mut statement = connection
        .prepare(
            "SELECT revision_number,draft_revision,document_json,document_digest,plan_json,plan_digest,plan_format,compiler_algorithm,contract_locks_json,compatibility_profile,evidence_json,signature,published_at FROM published_revisions WHERE workflow_id=?1 ORDER BY revision_number",
        )
        .map_err(|error| RevisionError::Storage(error.to_string()))?;
    let rows = statement
        .query_map(params![workflow_id], |row| {
            Ok(RevisionRow {
                revision_number: row.get::<_, i64>(0)? as u64,
                draft_version: row.get::<_, i64>(1)? as u64,
                document_json: row.get(2)?,
                document_digest: row.get(3)?,
                plan_json: row.get(4)?,
                plan_digest: row.get(5)?,
                plan_format: row.get(6)?,
                compiler_algorithm: row.get(7)?,
                contract_locks_json: row.get(8)?,
                compatibility_profile: row.get(9)?,
                evidence_json: row.get(10)?,
                signature: row.get(11)?,
                published_at: row.get(12)?,
            })
        })
        .map_err(|error| RevisionError::Storage(error.to_string()))?;
    let mut revisions = Vec::new();
    for row in rows {
        revisions.push(row.map_err(|error| RevisionError::Storage(error.to_string()))?);
    }
    Ok(revisions)
}

fn revision_row(
    connection: &Connection,
    workflow_id: &str,
    revision_number: u64,
) -> Result<RevisionRow, RevisionError> {
    let (draft_revision, document_json, document_digest, plan_json, plan_digest, plan_format, compiler_algorithm, contract_locks_json, compatibility_profile, evidence_json, signature, published_at): (
        i64,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        i64,
    ) = connection
        .query_row(
            "SELECT draft_revision,document_json,document_digest,plan_json,plan_digest,plan_format,compiler_algorithm,contract_locks_json,compatibility_profile,evidence_json,signature,published_at FROM published_revisions WHERE workflow_id=?1 AND revision_number=?2",
            params![workflow_id, revision_number],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                ))
            },
        )
        .map_err(|error| {
            if matches!(error, rusqlite::Error::QueryReturnedNoRows) {
                RevisionError::NotFound
            } else {
                RevisionError::Storage(error.to_string())
            }
        })?;
    Ok(RevisionRow {
        revision_number,
        draft_version: draft_revision as u64,
        document_json,
        document_digest,
        plan_json,
        plan_digest,
        plan_format,
        compiler_algorithm,
        contract_locks_json,
        compatibility_profile,
        evidence_json,
        signature,
        published_at,
    })
}

fn require_workflow(connection: &Connection, workflow_id: &str) -> Result<(), RevisionError> {
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM workflow_drafts WHERE workflow_id=?1)",
            params![workflow_id],
            |row| row.get(0),
        )
        .map_err(|error| RevisionError::Storage(error.to_string()))?;
    if exists == 1 {
        Ok(())
    } else {
        Err(RevisionError::NotFound)
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::Layout;

    fn manual_contract() -> Value {
        serde_json::from_str(include_str!(
            "../../../contracts/manual-trigger.v1alpha1.json"
        ))
        .unwrap()
    }

    fn catalog() -> BTreeMap<String, ContractEntry> {
        let manual = manual_contract();
        let manual_lock = lock(&manual).unwrap();
        let mut catalog = BTreeMap::new();
        catalog.insert(
            lock_key(&manual_lock),
            ContractEntry {
                lock: manual_lock,
                contract: manual,
            },
        );
        catalog
    }

    fn lock_from(catalog: &BTreeMap<String, ContractEntry>) -> NodeContractLock {
        catalog.values().next().unwrap().lock.clone()
    }

    fn trigger(id: &str, name: &str, contract_lock: NodeContractLock) -> NodeInstance {
        NodeInstance {
            id: id.into(),
            name: name.into(),
            contract_lock,
            configuration: json!({ "capture_mode": "manual" }),
            layout: Layout { x: 120.0, y: 80.0 },
            annotation: String::new(),
            compatibility_metadata: json!({}),
        }
    }

    fn passthrough(base: Value, name: &str) -> (NodeContractLock, Value) {
        let mut contract = base;
        contract["identity"]["name"] = json!(name);
        contract["activation"]["shape"] = json!("per_item_stream");
        contract["configuration"]["defaults"] = json!({});
        contract["configuration"]["schema"] = json!({ "type": "object", "additionalProperties": true });
        contract["ports"]["inputs"] = json!([
            {
                "cardinality": "one",
                "id": "items",
                "multiplicity": "many",
                "required": false,
                "schema": { "type": "dynamic-item" }
            }
        ]);
        let entry_lock = lock(&contract).unwrap();
        (entry_lock, contract)
    }

    fn draft(nodes: Vec<NodeInstance>, connections: Vec<Value>) -> WorkflowDraft {
        WorkflowDraft {
            workflow_id: "wf-test".into(),
            name: "Compiler test".into(),
            draft_version: 1,
            nodes,
            connections,
            annotation: String::new(),
            settings: json!({}),
            compatibility_metadata: json!({ "profile": "native" }),
        }
    }

    #[test]
    fn compile_is_deterministic_and_digested() {
        let catalog = catalog();
        let document = draft(vec![trigger("trigger-a", "Start", lock_from(&catalog))], vec![]);
        let plan = match compile_workflow(&document, &catalog, &CompilePolicy::CURRENT) {
            CompileOutcome::Plan { plan, diagnostics } => {
                assert!(diagnostics.iter().any(|d| d.code == "unconnected_trigger_output"));
                plan
            }
            CompileOutcome::Failed { diagnostics } => panic!("expected plan: {diagnostics:?}"),
        };
        let again = match compile_workflow(&document, &catalog, &CompilePolicy::CURRENT) {
            CompileOutcome::Plan { plan, .. } => plan,
            CompileOutcome::Failed { diagnostics } => panic!("expected plan: {diagnostics:?}"),
        };
        assert_eq!(
            serde_json::to_string(&plan).unwrap(),
            serde_json::to_string(&again).unwrap()
        );
        assert!(plan.plan_digest.starts_with("sha256:"));
        assert!(plan.revision.document_digest.starts_with("sha256:"));
        assert_eq!(plan.topological_order, vec!["trigger-a".to_string()]);
        assert_eq!(plan.nodes[0].lane, LANE_INLINE_NATIVE);
        assert_eq!(plan.nodes[0].activation.shape, "source");
    }

    #[test]
    fn missing_and_duplicate_sources_are_errors() {
        let catalog = catalog();
        let empty = draft(vec![], vec![]);
        assert!(matches!(
            compile_workflow(&empty, &catalog, &CompilePolicy::CURRENT),
            CompileOutcome::Failed { diagnostics }
                if diagnostics.iter().any(|d| d.code == "workflow_requires_source")
        ));
        let lock = lock_from(&catalog);
        let both = draft(
            vec![trigger("a", "One", lock.clone()), trigger("b", "Two", lock)],
            vec![],
        );
        assert!(matches!(
            compile_workflow(&both, &catalog, &CompilePolicy::CURRENT),
            CompileOutcome::Failed { diagnostics }
                if diagnostics.iter().any(|d| d.code == "multiple_triggers")
        ));
    }

    #[test]
    fn lock_identity_and_digest_are_verified_separately() {
        let catalog = catalog();
        let mut foreign = lock_from(&catalog);
        foreign.name = "generate-items".into();
        let unknown = draft(vec![trigger("a", "Start", foreign)], vec![]);
        assert!(matches!(
            compile_workflow(&unknown, &catalog, &CompilePolicy::CURRENT),
            CompileOutcome::Failed { diagnostics }
                if diagnostics.iter().any(|d| d.code == "unknown_contract_lock")
        ));
        let mut tampered = lock_from(&catalog);
        tampered.digest = format!("sha256:{}", "0".repeat(64));
        let mismatched = draft(vec![trigger("a", "Start", tampered)], vec![]);
        assert!(matches!(
            compile_workflow(&mismatched, &catalog, &CompilePolicy::CURRENT),
            CompileOutcome::Failed { diagnostics }
                if diagnostics.iter().any(|d| d.code == "contract_lock_digest_mismatch")
        ));
    }

    #[test]
    fn configuration_schema_and_expressions_are_enforced() {
        let catalog = catalog();
        let mut wrong = trigger("a", "Start", lock_from(&catalog));
        wrong.configuration = json!({ "capture_mode": "cron" });
        assert!(matches!(
            compile_workflow(&draft(vec![wrong], vec![]), &catalog, &CompilePolicy::CURRENT),
            CompileOutcome::Failed { diagnostics }
                if diagnostics.iter().any(|d| d.code == "invalid_configuration")
        ));
        let mut extra = trigger("a", "Start", lock_from(&catalog));
        extra.configuration = json!({ "capture_mode": "manual", "unknown": true });
        assert!(matches!(
            compile_workflow(&draft(vec![extra], vec![]), &catalog, &CompilePolicy::CURRENT),
            CompileOutcome::Failed { diagnostics }
                if diagnostics.iter().any(|d| d.code == "invalid_configuration")
        ));
        let mut expression = trigger("a", "Start", lock_from(&catalog));
        expression.configuration = json!({ "capture_mode": "manual" });
        expression.annotation = "{{ $json.x }}".into();
        let document = draft(vec![expression], vec![]);
        // The annotation is not configuration; the plan still compiles.
        match compile_workflow(&document, &catalog, &CompilePolicy::CURRENT) {
            CompileOutcome::Plan { .. } => {}
            CompileOutcome::Failed { diagnostics } => panic!("annotation must not fail compilation: {diagnostics:?}"),
        }
        let mut configured = trigger("a", "Start", lock_from(&catalog));
        configured.configuration = json!({ "capture_mode": "{{ manual }}" });
        assert!(matches!(
            compile_workflow(&draft(vec![configured], vec![]), &catalog, &CompilePolicy::CURRENT),
            CompileOutcome::Failed { diagnostics }
                if diagnostics.iter().any(|d| d.code == "unsupported_expression")
        ));
    }

    #[test]
    fn capabilities_effects_and_budgets_follow_the_profile_policy() {
        let base = manual_contract();
        let (lock, mut contract) = passthrough(base.clone(), "policy-cap");
        contract["capabilities"] = json!(["canopy/native/egress"]);
        let (lock2, mut contract2) = passthrough(base.clone(), "policy-effect");
        contract2["effects"]["class"] = json!("external_write");
        let (lock3, mut contract3) = passthrough(base.clone(), "policy-budget");
        contract3["resources"]["hard"]["wall_millis"] = json!(999_999);
        let mut catalog = BTreeMap::new();
        catalog.insert(
            lock_key(&lock),
            ContractEntry { lock: lock.clone(), contract: contract.clone() },
        );
        catalog.insert(
            lock_key(&lock2),
            ContractEntry { lock: lock2.clone(), contract: contract2.clone() },
        );
        catalog.insert(
            lock_key(&lock3),
            ContractEntry { lock: lock3.clone(), contract: contract3.clone() },
        );
        // Each node is a per_item_stream contract: pair with one real trigger.
        let trigger_lock = {
            let manual = manual_contract();
            lock(&manual).unwrap()
        };
        catalog.insert(
            lock_key(&trigger_lock),
            ContractEntry {
                lock: trigger_lock.clone(),
                contract: manual_contract(),
            },
        );
        let document = draft(
            vec![
                trigger("t", "Start", trigger_lock),
                NodeInstance {
                    id: "cap".into(),
                    name: "Capability".into(),
                    contract_lock: lock,
                    configuration: json!({}),
                    layout: Layout { x: 0.0, y: 0.0 },
                    annotation: String::new(),
                    compatibility_metadata: json!({}),
                },
                NodeInstance {
                    id: "fx".into(),
                    name: "Effect".into(),
                    contract_lock: lock2,
                    configuration: json!({}),
                    layout: Layout { x: 0.0, y: 0.0 },
                    annotation: String::new(),
                    compatibility_metadata: json!({}),
                },
                NodeInstance {
                    id: "budget".into(),
                    name: "Budget".into(),
                    contract_lock: lock3,
                    configuration: json!({}),
                    layout: Layout { x: 0.0, y: 0.0 },
                    annotation: String::new(),
                    compatibility_metadata: json!({}),
                },
            ],
            vec![],
        );
        let CompileOutcome::Failed { diagnostics } =
            compile_workflow(&document, &catalog, &CompilePolicy::CURRENT)
        else {
            panic!("expected policy errors")
        };
        let codes: BTreeSet<&str> = diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert!(codes.contains("capability_not_allowed"));
        assert!(codes.contains("effect_not_allowed"));
        assert!(codes.contains("budget_exceeds_policy"));
    }

    #[test]
    fn connections_validate_ports_and_cycles() {
        let base = manual_contract();
        let (lock_a, contract_a) = passthrough(base.clone(), "port-a");
        let (lock_b, contract_b) = passthrough(base.clone(), "port-b");
        let trigger_lock = lock(&manual_contract()).unwrap();
        let mut catalog = BTreeMap::new();
        let manual = manual_contract();
        catalog.insert(
            lock_key(&trigger_lock),
            ContractEntry { lock: trigger_lock.clone(), contract: manual },
        );
        catalog.insert(lock_key(&lock_a), ContractEntry { lock: lock_a.clone(), contract: contract_a });
        catalog.insert(lock_key(&lock_b), ContractEntry { lock: lock_b.clone(), contract: contract_b });

        let unknown_port = draft(
            vec![
                trigger("a", "Start", trigger_lock.clone()),
                NodeInstance {
                    id: "b".into(),
                    name: "B".into(),
                    contract_lock: lock_a.clone(),
                    configuration: json!({}),
                    layout: Layout { x: 0.0, y: 0.0 },
                    annotation: String::new(),
                    compatibility_metadata: json!({}),
                },
            ],
            vec![json!({
                "source": { "node": "a", "output_port": "invocation" },
                "target": { "node": "b", "input_port": "missing" }
            })],
        );
        assert!(matches!(
            compile_workflow(&unknown_port, &catalog, &CompilePolicy::CURRENT),
            CompileOutcome::Failed { diagnostics }
                if diagnostics.iter().any(|d| d.code == "connection_unknown_input_port")
        ));

        let cycle = draft(
            vec![
                trigger("a", "Start", trigger_lock.clone()),
                NodeInstance {
                    id: "b".into(),
                    name: "B".into(),
                    contract_lock: lock_a.clone(),
                    configuration: json!({}),
                    layout: Layout { x: 0.0, y: 0.0 },
                    annotation: String::new(),
                    compatibility_metadata: json!({}),
                },
                NodeInstance {
                    id: "c".into(),
                    name: "C".into(),
                    contract_lock: lock_b,
                    configuration: json!({}),
                    layout: Layout { x: 0.0, y: 0.0 },
                    annotation: String::new(),
                    compatibility_metadata: json!({}),
                },
            ],
            vec![
                json!({ "source": { "node": "a", "output_port": "invocation" }, "target": { "node": "b", "input_port": "items" } }),
                json!({ "source": { "node": "b", "output_port": "invocation" }, "target": { "node": "c", "input_port": "items" } }),
                json!({ "source": { "node": "c", "output_port": "invocation" }, "target": { "node": "b", "input_port": "items" } }),
            ],
        );
        assert!(matches!(
            compile_workflow(&cycle, &catalog, &CompilePolicy::CURRENT),
            CompileOutcome::Failed { diagnostics }
                if diagnostics.iter().any(|d| d.code == "unexpected_cycle")
        ));

        let self_loop = draft(
            vec![
                trigger("a", "Start", trigger_lock),
                NodeInstance {
                    id: "b".into(),
                    name: "B".into(),
                    contract_lock: lock_a,
                    configuration: json!({}),
                    layout: Layout { x: 0.0, y: 0.0 },
                    annotation: String::new(),
                    compatibility_metadata: json!({}),
                },
            ],
            vec![
                json!({ "source": { "node": "a", "output_port": "invocation" }, "target": { "node": "b", "input_port": "items" } }),
                json!({ "source": { "node": "b", "output_port": "invocation" }, "target": { "node": "b", "input_port": "items" } }),
            ],
        );
        assert!(matches!(
            compile_workflow(&self_loop, &catalog, &CompilePolicy::CURRENT),
            CompileOutcome::Failed { diagnostics }
                if diagnostics.iter().any(|d| d.code == "self_connection")
        ));
    }

    #[test]
    fn connected_plan_records_topological_order_and_depth() {
        let base = manual_contract();
        let (lock_b, contract_b) = passthrough(base, "stream-b");
        let trigger_lock = lock(&manual_contract()).unwrap();
        let mut catalog = BTreeMap::new();
        catalog.insert(
            lock_key(&trigger_lock),
            ContractEntry {
                lock: trigger_lock.clone(),
                contract: manual_contract(),
            },
        );
        catalog.insert(
            lock_key(&lock_b),
            ContractEntry {
                lock: lock_b.clone(),
                contract: contract_b,
            },
        );
        let document = draft(
            vec![
                trigger("a", "Start", trigger_lock),
                NodeInstance {
                    id: "b".into(),
                    name: "B".into(),
                    contract_lock: lock_b,
                    configuration: json!({}),
                    layout: Layout { x: 0.0, y: 0.0 },
                    annotation: String::new(),
                    compatibility_metadata: json!({}),
                },
            ],
            vec![json!({
                "source": { "node": "a", "output_port": "invocation" },
                "target": { "node": "b", "input_port": "items" }
            })],
        );
        let CompileOutcome::Plan { plan, diagnostics } =
            compile_workflow(&document, &catalog, &CompilePolicy::CURRENT)
        else {
            panic!("expected a connected plan")
        };
        assert!(diagnostics.is_empty());
        assert_eq!(plan.topological_order, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(plan.nodes[0].depth, 0);
        assert_eq!(plan.nodes[1].depth, 1);
        assert_eq!(plan.connections.len(), 1);
        assert_eq!(plan.contract_locks.len(), 2);
    }

    #[test]
    fn duplicate_names_are_designated_warnings() {
        let base = manual_contract();
        let (lock_b, contract_b) = passthrough(base.clone(), "twin-b");
        let (lock_c, contract_c) = passthrough(base, "twin-c");
        let trigger_lock = lock(&manual_contract()).unwrap();
        let mut catalog = BTreeMap::new();
        catalog.insert(
            lock_key(&trigger_lock),
            ContractEntry {
                lock: trigger_lock.clone(),
                contract: manual_contract(),
            },
        );
        catalog.insert(lock_key(&lock_b), ContractEntry { lock: lock_b.clone(), contract: contract_b });
        catalog.insert(lock_key(&lock_c), ContractEntry { lock: lock_c.clone(), contract: contract_c });
        let document = draft(
            vec![
                trigger("a", "Start", trigger_lock),
                NodeInstance {
                    id: "b".into(),
                    name: "Twin".into(),
                    contract_lock: lock_b,
                    configuration: json!({}),
                    layout: Layout { x: 0.0, y: 0.0 },
                    annotation: String::new(),
                    compatibility_metadata: json!({}),
                },
                NodeInstance {
                    id: "c".into(),
                    name: "Twin".into(),
                    contract_lock: lock_c,
                    configuration: json!({}),
                    layout: Layout { x: 0.0, y: 0.0 },
                    annotation: String::new(),
                    compatibility_metadata: json!({}),
                },
            ],
            vec![json!({
                "source": { "node": "a", "output_port": "invocation" },
                "target": { "node": "b", "input_port": "items" }
            })],
        );
        let CompileOutcome::Plan { diagnostics, .. } =
            compile_workflow(&document, &catalog, &CompilePolicy::CURRENT)
        else {
            panic!("expected warnings-only compilation")
        };
        let twin = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "duplicate_node_name")
            .count();
        assert_eq!(twin, 2);
        assert!(diagnostics.iter().all(|d| d.severity == "warning"));
        assert!(diagnostics
            .iter()
            .any(|d| d.code == "unconnected_node" && d.path.ends_with("c")));
    }
}
