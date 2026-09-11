// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pure, deterministic Draft-to-plan compilation. This module performs no I/O.
use crate::canonical::{digest, CANONICALIZATION, DIGEST_ALGORITHM};
use crate::draft::WorkflowDraft;
use canopy_node_contract::{lock, validate, NodeContractLock};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const COMPILER_ABI: &str = "canopy.compiler/v1alpha1";
pub const PLAN_FORMAT: &str = "canopy.plan+jcs/v1alpha1";
pub const REVISION_FORMAT: &str = "canopy.workflow-revision+jcs/v1alpha1";
pub const POLICY_ID: &str = "canopy.publication-policy/v1alpha1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompatibilityProfile {
    pub profile_id: String,
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CompilerPolicy {
    pub policy_id: String,
    pub allowed_capabilities: Vec<String>,
    pub designated_warning_codes: Vec<String>,
    pub maximum_resources: Value,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RevisionPayload {
    pub format: String,
    pub draft: WorkflowDraft,
    pub compatibility_profile: CompatibilityProfile,
    pub contract_locks: Vec<NodeContractLock>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlanNode {
    pub node_instance_id: String,
    pub contract_lock: NodeContractLock,
    pub configuration: Value,
    pub activation: Value,
    pub effects: Value,
    pub capabilities: Vec<String>,
    pub resources: Value,
    pub input_ports: Value,
    pub output_ports: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlanDependency {
    pub connection_id: String,
    pub source_node_id: String,
    pub source_port_id: String,
    pub target_node_id: String,
    pub target_port_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ExecutionPlan {
    pub format: String,
    pub compiler_abi: String,
    pub revision_digest: String,
    pub compatibility_profile: CompatibilityProfile,
    pub policy_digest: String,
    pub contract_locks: Vec<NodeContractLock>,
    pub nodes: Vec<PlanNode>,
    pub scheduling_dependencies: Vec<PlanDependency>,
    pub segment_candidates: Vec<Vec<String>>,
    pub lane_eligibility: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Diagnostic {
    pub code: String,
    pub severity: String,
    pub subject: String,
    pub message: String,
    pub arguments: Value,
    pub requires_ack: bool,
    pub fingerprint: String,
}

#[derive(Clone, Serialize)]
struct CompileInput<'a> {
    compiler_abi: &'static str,
    revision: &'a RevisionPayload,
    contracts: &'a [Value],
    compatibility_profile: &'a CompatibilityProfile,
    policy: &'a CompilerPolicy,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CompileResult {
    pub workflow_id: String,
    pub draft_version: u64,
    pub canonicalization: String,
    pub digest_algorithm: String,
    pub compiler_abi: String,
    pub plan_format: String,
    pub compile_input_digest: String,
    pub revision_digest: String,
    pub compatibility_profile: CompatibilityProfile,
    pub contract_locks: Vec<NodeContractLock>,
    pub diagnostics: Vec<Diagnostic>,
    pub can_publish: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<ExecutionPlan>,
    #[serde(skip_serializing)]
    pub revision: RevisionPayload,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConnectionSpec {
    id: String,
    source: ConnectionEndpoint,
    target: ConnectionEndpoint,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConnectionEndpoint {
    node_id: String,
    port_id: String,
}

pub fn native_profile() -> CompatibilityProfile {
    CompatibilityProfile {
        profile_id: "canopy.native/v1alpha1".into(),
        status: "native".into(),
    }
}

pub fn publication_policy() -> CompilerPolicy {
    CompilerPolicy {
        policy_id: POLICY_ID.into(),
        allowed_capabilities: vec![],
        designated_warning_codes: vec!["W_OUTPUT_UNUSED".into()],
        maximum_resources: json!({
            "artifact_bytes": 67_108_864_u64,
            "concurrency": 64_u64,
            "cpu_millis": 300_000_u64,
            "input_bytes": 67_108_864_u64,
            "input_count": 1_000_000_u64,
            "memory_bytes": 268_435_456_u64,
            "output_bytes": 67_108_864_u64,
            "output_count": 1_000_000_u64,
            "wall_millis": 900_000_u64
        }),
    }
}

fn native_contracts() -> Result<Vec<Value>, String> {
    [
        (
            "Manual Trigger",
            include_str!("../../../contracts/manual-trigger.v1alpha1.json"),
        ),
        (
            "Generate Items",
            include_str!("../../../contracts/generate-items.v1alpha1.json"),
        ),
    ]
    .into_iter()
    .map(|(name, source)| {
        serde_json::from_str(source)
            .map_err(|error| format!("embedded {name} contract is invalid: {error}"))
    })
    .collect()
}

pub fn compile(draft: WorkflowDraft) -> Result<CompileResult, String> {
    compile_with_inputs(
        draft,
        native_contracts()?,
        native_profile(),
        publication_policy(),
    )
}

pub fn compile_input_digest(draft: &WorkflowDraft) -> Result<String, String> {
    let contracts = native_contracts()?;
    let compatibility_profile = native_profile();
    let policy = publication_policy();
    let revision = RevisionPayload {
        format: REVISION_FORMAT.into(),
        draft: draft.clone(),
        compatibility_profile: compatibility_profile.clone(),
        contract_locks: unique_locks(draft),
    };
    digest(&CompileInput {
        compiler_abi: COMPILER_ABI,
        revision: &revision,
        contracts: &contracts,
        compatibility_profile: &compatibility_profile,
        policy: &policy,
    })
}

pub fn compile_with_inputs(
    draft: WorkflowDraft,
    contracts: Vec<Value>,
    compatibility_profile: CompatibilityProfile,
    policy: CompilerPolicy,
) -> Result<CompileResult, String> {
    let contract_locks = unique_locks(&draft);
    let revision = RevisionPayload {
        format: REVISION_FORMAT.into(),
        draft,
        compatibility_profile: compatibility_profile.clone(),
        contract_locks: contract_locks.clone(),
    };
    let revision_digest = digest(&revision)?;
    let input = CompileInput {
        compiler_abi: COMPILER_ABI,
        revision: &revision,
        contracts: &contracts,
        compatibility_profile: &compatibility_profile,
        policy: &policy,
    };
    let compile_input_digest = digest(&input)?;
    let policy_digest = digest(&policy)?;
    let mut diagnostics = vec![];
    validate_all(
        &revision,
        &contracts,
        &compatibility_profile,
        &policy,
        &mut diagnostics,
    )?;
    diagnostics.sort_by(|left, right| {
        severity_order(&left.severity)
            .cmp(&severity_order(&right.severity))
            .then(left.code.cmp(&right.code))
            .then(left.subject.cmp(&right.subject))
            .then(left.fingerprint.cmp(&right.fingerprint))
    });
    let has_error = diagnostics.iter().any(|item| item.severity == "error");
    let plan = if has_error {
        None
    } else {
        Some(build_plan(
            &revision,
            &contracts,
            revision_digest.clone(),
            policy_digest,
        )?)
    };
    let plan_digest = plan.as_ref().map(digest).transpose()?;
    Ok(CompileResult {
        workflow_id: revision.draft.workflow_id.clone(),
        draft_version: revision.draft.draft_version,
        canonicalization: CANONICALIZATION.into(),
        digest_algorithm: DIGEST_ALGORITHM.into(),
        compiler_abi: COMPILER_ABI.into(),
        plan_format: PLAN_FORMAT.into(),
        compile_input_digest,
        revision_digest,
        compatibility_profile,
        contract_locks,
        diagnostics,
        can_publish: !has_error,
        plan_digest,
        plan,
        revision,
    })
}

fn unique_locks(draft: &WorkflowDraft) -> Vec<NodeContractLock> {
    let mut keyed = BTreeMap::new();
    for node in &draft.nodes {
        let key = format!(
            "{}\0{}\0{}\0{}",
            node.contract_lock.namespace,
            node.contract_lock.name,
            node.contract_lock.version,
            node.contract_lock.digest
        );
        keyed
            .entry(key)
            .or_insert_with(|| node.contract_lock.clone());
    }
    keyed.into_values().collect()
}

fn validate_all(
    revision: &RevisionPayload,
    contracts: &[Value],
    profile: &CompatibilityProfile,
    policy: &CompilerPolicy,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    let draft = &revision.draft;
    if draft.nodes.is_empty() {
        push(
            diagnostics,
            "E_GRAPH_EMPTY",
            "error",
            format!("workflow:{}", draft.workflow_id),
            "The Workflow has no activation node.",
            json!({"minimum_nodes": 1}),
            false,
        )?;
    }
    let mut node_ids = BTreeSet::new();
    let mut contracts_by_digest = BTreeMap::new();
    for contract in contracts {
        match validate(contract) {
            Ok(()) => match lock(contract) {
                Ok(contract_lock) => {
                    contracts_by_digest.insert(contract_lock.digest.clone(), contract);
                }
                Err(error) => push(
                    diagnostics,
                    "E_CONTRACT_IDENTITY",
                    "error",
                    "contract-set".into(),
                    "A Node Contract identity could not be computed.",
                    json!({"detail": error}),
                    false,
                )?,
            },
            Err(error) => push(
                diagnostics,
                "E_CONTRACT_INVALID",
                "error",
                "contract-set".into(),
                "A Node Contract is structurally invalid.",
                json!({"detail": error.to_string()}),
                false,
            )?,
        }
    }
    for node in &draft.nodes {
        if !valid_identifier(&node.id) {
            push(
                diagnostics,
                "E_NODE_IDENTITY",
                "error",
                format!("node:{}", node.id),
                "The node identity is not a stable identifier.",
                json!({"node_instance_id": node.id}),
                false,
            )?;
        }
        if !node_ids.insert(node.id.clone()) {
            push(
                diagnostics,
                "E_NODE_IDENTITY_DUPLICATE",
                "error",
                format!("node:{}", node.id),
                "The node identity is duplicated.",
                json!({"node_instance_id": node.id}),
                false,
            )?;
        }
        let Some(contract) = contracts_by_digest.get(&node.contract_lock.digest).copied() else {
            push(
                diagnostics,
                "E_CONTRACT_LOCK_UNAVAILABLE",
                "error",
                format!("node:{}", node.id),
                "The exact locked Node Contract is unavailable.",
                json!({"contract_lock": node.contract_lock}),
                false,
            )?;
            continue;
        };
        let actual = lock(contract).map_err(|error| error.to_string())?;
        if actual != node.contract_lock {
            push(
                diagnostics,
                "E_CONTRACT_LOCK_MISMATCH",
                "error",
                format!("node:{}", node.id),
                "The locked Node Contract identity does not match its bytes.",
                json!({"expected": node.contract_lock, "actual": actual}),
                false,
            )?;
            continue;
        }
        validate_configuration(node, diagnostics)?;
        validate_expressions(&node.configuration, &node.id, "$", diagnostics)?;
        validate_capabilities(contract, node, policy, diagnostics)?;
        validate_effects(contract, node, diagnostics)?;
        validate_budgets(contract, node, policy, diagnostics)?;
        validate_compatibility(contract, node, profile, diagnostics)?;
    }
    validate_connections(draft, &contracts_by_digest, diagnostics)?;
    validate_unused_outputs(draft, &contracts_by_digest, policy, diagnostics)?;
    Ok(())
}

fn validate_configuration(
    node: &crate::draft::NodeInstance,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    let valid = match node.contract_lock.name.as_str() {
        "manual-trigger" => node.configuration.as_object().is_some_and(|object| {
            object.len() == 1 && object.get("capture_mode") == Some(&json!("manual"))
        }),
        "generate-items" => node.configuration.as_object().is_some_and(|object| {
            if object.len() != 5
                || !matches!(
                    object.get("storage_mode").and_then(Value::as_str),
                    Some("auto" | "artifact")
                )
            {
                return false;
            }
            let Some(count) = object.get("count").and_then(Value::as_u64) else {
                return false;
            };
            let Some(start) = object.get("start").and_then(Value::as_i64) else {
                return false;
            };
            let Some(step) = object.get("step").and_then(Value::as_i64) else {
                return false;
            };
            object.contains_key("data")
                && count <= 50_000
                && (count == 0
                    || i64::try_from(count - 1)
                        .ok()
                        .and_then(|ordinal| step.checked_mul(ordinal))
                        .and_then(|delta| start.checked_add(delta))
                        .is_some())
        }),
        _ => false,
    };
    if !valid {
        push(
            diagnostics,
            "E_CONFIGURATION_INVALID",
            "error",
            format!("node:{}", node.id),
            "The node configuration does not satisfy the locked schema.",
            json!({"path": "configuration", "contract": node.contract_lock.name}),
            false,
        )?;
    }
    Ok(())
}

fn validate_expressions(
    value: &Value,
    node_id: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    match value {
        Value::String(text) if text.trim_start().starts_with("={{") => push(
            diagnostics,
            "E_EXPRESSION_UNSUPPORTED",
            "error",
            format!("node:{node_id}"),
            "This locked node field does not accept an expression.",
            json!({"path": path}),
            false,
        )?,
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                validate_expressions(item, node_id, &format!("{path}[{index}]"), diagnostics)?;
            }
        }
        Value::Object(object) => {
            for (key, item) in object {
                validate_expressions(item, node_id, &format!("{path}.{key}"), diagnostics)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_capabilities(
    contract: &Value,
    node: &crate::draft::NodeInstance,
    policy: &CompilerPolicy,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    let allowed: BTreeSet<_> = policy
        .allowed_capabilities
        .iter()
        .map(String::as_str)
        .collect();
    let capabilities = contract["capabilities"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for capability in capabilities {
        let Some(capability) = capability.as_str() else {
            push(
                diagnostics,
                "E_CAPABILITY_INVALID",
                "error",
                format!("node:{}", node.id),
                "A requested capability is not a string identity.",
                json!({}),
                false,
            )?;
            continue;
        };
        if !allowed.contains(capability) {
            push(
                diagnostics,
                "E_CAPABILITY_DENIED",
                "error",
                format!("node:{}", node.id),
                "The policy does not allow a requested capability.",
                json!({"capability": capability}),
                false,
            )?;
        }
    }
    Ok(())
}

fn validate_effects(
    contract: &Value,
    node: &crate::draft::NodeInstance,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    let effects = &contract["effects"];
    let valid = effects["class"]
        .as_str()
        .is_some_and(|class| ["pure", "read", "write", "destructive"].contains(&class))
        && effects["deterministic"].is_boolean()
        && effects["idempotent"].is_boolean()
        && effects["approval_required"].is_boolean()
        && effects["retry"].is_string()
        && effects["reconciliation"].is_string();
    if !valid {
        push(
            diagnostics,
            "E_EFFECTS_INVALID",
            "error",
            format!("node:{}", node.id),
            "The locked effect declaration is incomplete or invalid.",
            json!({"effects": effects}),
            false,
        )?;
    }
    Ok(())
}

fn validate_budgets(
    contract: &Value,
    node: &crate::draft::NodeInstance,
    policy: &CompilerPolicy,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    let Some(defaults) = contract["resources"]["default"].as_object() else {
        return push(
            diagnostics,
            "E_BUDGET_INVALID",
            "error",
            format!("node:{}", node.id),
            "Default resource budgets are missing.",
            json!({}),
            false,
        );
    };
    let Some(hard) = contract["resources"]["hard"].as_object() else {
        return push(
            diagnostics,
            "E_BUDGET_INVALID",
            "error",
            format!("node:{}", node.id),
            "Hard resource budgets are missing.",
            json!({}),
            false,
        );
    };
    for (name, default) in defaults {
        let Some(default) = default.as_u64() else {
            push(
                diagnostics,
                "E_BUDGET_INVALID",
                "error",
                format!("node:{}", node.id),
                "A default resource budget is not a non-negative integer.",
                json!({"budget": name}),
                false,
            )?;
            continue;
        };
        let Some(hard) = hard.get(name).and_then(Value::as_u64) else {
            push(
                diagnostics,
                "E_BUDGET_INVALID",
                "error",
                format!("node:{}", node.id),
                "A hard resource budget is not a non-negative integer.",
                json!({"budget": name}),
                false,
            )?;
            continue;
        };
        let policy_max = policy.maximum_resources.get(name).and_then(Value::as_u64);
        if default > hard || policy_max.is_some_and(|maximum| hard > maximum) {
            push(
                diagnostics,
                "E_BUDGET_EXCEEDED",
                "error",
                format!("node:{}", node.id),
                "The resource budget exceeds its hard or policy limit.",
                json!({"budget": name, "default": default, "hard": hard, "policy_maximum": policy_max}),
                false,
            )?;
        }
    }
    Ok(())
}

fn validate_compatibility(
    contract: &Value,
    node: &crate::draft::NodeInstance,
    profile: &CompatibilityProfile,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    let contract_profile = contract["compatibility"]["profile"].as_str();
    if profile.status != "native" || contract_profile != Some("native") {
        push(
            diagnostics,
            "E_COMPATIBILITY_UNSUPPORTED",
            "error",
            format!("node:{}", node.id),
            "The node is not certified for the selected Compatibility Profile.",
            json!({"selected_profile": profile.profile_id, "contract_profile": contract_profile}),
            false,
        )?;
    }
    Ok(())
}

fn validate_connections(
    draft: &WorkflowDraft,
    contracts: &BTreeMap<String, &Value>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    let nodes: BTreeMap<_, _> = draft
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let mut identities = BTreeSet::new();
    let mut incoming: BTreeMap<(String, String), usize> = BTreeMap::new();
    for (index, raw) in draft.connections.iter().enumerate() {
        let connection: ConnectionSpec = match serde_json::from_value(raw.clone()) {
            Ok(value) => value,
            Err(error) => {
                push(
                    diagnostics,
                    "E_CONNECTION_INVALID",
                    "error",
                    format!("connection-index:{index}"),
                    "The connection shape is invalid.",
                    json!({"detail": error.to_string()}),
                    false,
                )?;
                continue;
            }
        };
        if !valid_identifier(&connection.id) || !identities.insert(connection.id.clone()) {
            push(
                diagnostics,
                "E_CONNECTION_IDENTITY",
                "error",
                format!("connection:{}", connection.id),
                "The connection identity is invalid or duplicated.",
                json!({"connection_id": connection.id}),
                false,
            )?;
        }
        let Some(source_node) = nodes.get(connection.source.node_id.as_str()) else {
            push(
                diagnostics,
                "E_CONNECTION_SOURCE",
                "error",
                format!("connection:{}", connection.id),
                "The connection source node does not exist.",
                json!({"node_instance_id": connection.source.node_id}),
                false,
            )?;
            continue;
        };
        let Some(target_node) = nodes.get(connection.target.node_id.as_str()) else {
            push(
                diagnostics,
                "E_CONNECTION_TARGET",
                "error",
                format!("connection:{}", connection.id),
                "The connection target node does not exist.",
                json!({"node_instance_id": connection.target.node_id}),
                false,
            )?;
            continue;
        };
        let source_contract = contracts.get(&source_node.contract_lock.digest).copied();
        let target_contract = contracts.get(&target_node.contract_lock.digest).copied();
        let source_port = source_contract.and_then(|contract| {
            find_port(&contract["ports"]["outputs"], &connection.source.port_id)
        });
        if source_port.is_none() {
            push(
                diagnostics,
                "E_PORT_SOURCE",
                "error",
                format!("connection:{}", connection.id),
                "The source output port is not declared by the locked contract.",
                json!({"port_id": connection.source.port_id}),
                false,
            )?;
        }
        let target_port = target_contract.and_then(|contract| {
            find_port(&contract["ports"]["inputs"], &connection.target.port_id)
        });
        if target_port.is_none() {
            push(
                diagnostics,
                "E_PORT_TARGET",
                "error",
                format!("connection:{}", connection.id),
                "The target input port is not declared by the locked contract.",
                json!({"port_id": connection.target.port_id}),
                false,
            )?;
        }
        if let Some(port) = target_port {
            let key = (
                connection.target.node_id.clone(),
                connection.target.port_id.clone(),
            );
            let count = incoming.entry(key).or_default();
            *count += 1;
            if port["cardinality"].as_str() == Some("one") && *count > 1 {
                push(
                    diagnostics,
                    "E_PORT_CARDINALITY",
                    "error",
                    format!("connection:{}", connection.id),
                    "The target input port accepts only one connection.",
                    json!({"port_id": connection.target.port_id}),
                    false,
                )?;
            }
        }
    }
    for node in &draft.nodes {
        let Some(contract) = contracts.get(&node.contract_lock.digest).copied() else {
            continue;
        };
        for input in contract["ports"]["inputs"].as_array().into_iter().flatten() {
            let Some(port_id) = input["id"].as_str() else {
                continue;
            };
            if input["required"].as_bool() == Some(true)
                && incoming
                    .get(&(node.id.clone(), port_id.to_string()))
                    .copied()
                    .unwrap_or_default()
                    == 0
            {
                push(
                    diagnostics,
                    "E_PORT_REQUIRED",
                    "error",
                    format!("node:{}:input:{port_id}", node.id),
                    "A required input port is not connected.",
                    json!({"node_instance_id": node.id, "port_id": port_id}),
                    false,
                )?;
            }
        }
    }
    Ok(())
}

fn validate_unused_outputs(
    draft: &WorkflowDraft,
    contracts: &BTreeMap<String, &Value>,
    policy: &CompilerPolicy,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    let consumed: BTreeSet<(String, String)> = draft
        .connections
        .iter()
        .filter_map(|raw| serde_json::from_value::<ConnectionSpec>(raw.clone()).ok())
        .map(|connection| (connection.source.node_id, connection.source.port_id))
        .collect();
    for node in &draft.nodes {
        let Some(contract) = contracts.get(&node.contract_lock.digest).copied() else {
            continue;
        };
        for output in contract["ports"]["outputs"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let Some(port_id) = output["id"].as_str() else {
                continue;
            };
            if !consumed.contains(&(node.id.clone(), port_id.into())) {
                let code = "W_OUTPUT_UNUSED";
                push(
                    diagnostics,
                    code,
                    "warning",
                    format!("node:{}:output:{port_id}", node.id),
                    "This output is not connected or consumed.",
                    json!({"node_instance_id": node.id, "port_id": port_id}),
                    policy
                        .designated_warning_codes
                        .iter()
                        .any(|item| item == code),
                )?;
            }
        }
    }
    Ok(())
}

fn build_plan(
    revision: &RevisionPayload,
    contracts: &[Value],
    revision_digest: String,
    policy_digest: String,
) -> Result<ExecutionPlan, String> {
    let mut contracts_by_digest = BTreeMap::new();
    for contract in contracts {
        contracts_by_digest.insert(lock(contract)?.digest, contract);
    }
    let mut nodes = Vec::with_capacity(revision.draft.nodes.len());
    for node in &revision.draft.nodes {
        let contract = contracts_by_digest
            .get(&node.contract_lock.digest)
            .ok_or_else(|| "validated contract lock disappeared".to_string())?;
        nodes.push(PlanNode {
            node_instance_id: node.id.clone(),
            contract_lock: node.contract_lock.clone(),
            configuration: node.configuration.clone(),
            activation: contract["activation"].clone(),
            effects: contract["effects"].clone(),
            capabilities: contract["capabilities"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
            resources: contract["resources"].clone(),
            input_ports: contract["ports"]["inputs"].clone(),
            output_ports: contract["ports"]["outputs"].clone(),
        });
    }
    nodes.sort_by(|left, right| left.node_instance_id.cmp(&right.node_instance_id));
    let mut scheduling_dependencies: Vec<_> = revision
        .draft
        .connections
        .iter()
        .filter_map(|raw| serde_json::from_value::<ConnectionSpec>(raw.clone()).ok())
        .map(|connection| PlanDependency {
            connection_id: connection.id,
            source_node_id: connection.source.node_id,
            source_port_id: connection.source.port_id,
            target_node_id: connection.target.node_id,
            target_port_id: connection.target.port_id,
        })
        .collect();
    scheduling_dependencies.sort_by(|left, right| left.connection_id.cmp(&right.connection_id));
    let segment_candidates = nodes
        .iter()
        .map(|node| vec![node.node_instance_id.clone()])
        .collect();
    Ok(ExecutionPlan {
        format: PLAN_FORMAT.into(),
        compiler_abi: COMPILER_ABI.into(),
        revision_digest,
        compatibility_profile: revision.compatibility_profile.clone(),
        policy_digest,
        contract_locks: revision.contract_locks.clone(),
        nodes,
        scheduling_dependencies,
        segment_candidates,
        lane_eligibility: vec!["native-cpu".into()],
    })
}

fn find_port<'a>(ports: &'a Value, id: &str) -> Option<&'a Value> {
    ports
        .as_array()?
        .iter()
        .find(|port| port["id"].as_str() == Some(id))
}

#[allow(clippy::too_many_arguments)]
fn push(
    diagnostics: &mut Vec<Diagnostic>,
    code: &str,
    severity: &str,
    subject: String,
    message: &str,
    arguments: Value,
    requires_ack: bool,
) -> Result<(), String> {
    #[derive(Serialize)]
    struct Fingerprint<'a> {
        code: &'a str,
        severity: &'a str,
        subject: &'a str,
        arguments: &'a Value,
        requires_ack: bool,
    }
    let fingerprint = digest(&Fingerprint {
        code,
        severity,
        subject: &subject,
        arguments: &arguments,
        requires_ack,
    })?;
    diagnostics.push(Diagnostic {
        code: code.into(),
        severity: severity.into(),
        subject,
        message: message.into(),
        arguments,
        requires_ack,
        fingerprint,
    });
    Ok(())
}

fn severity_order(value: &str) -> u8 {
    match value {
        "error" => 0,
        "warning" => 1,
        _ => 2,
    }
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::{Layout, NodeInstance};

    fn draft() -> WorkflowDraft {
        let contract: Value = serde_json::from_str(include_str!(
            "../../../contracts/manual-trigger.v1alpha1.json"
        ))
        .unwrap();
        WorkflowDraft {
            workflow_id: "workflow-one".into(),
            name: "One".into(),
            draft_version: 3,
            nodes: vec![NodeInstance {
                id: "manual".into(),
                name: "Manual Trigger".into(),
                contract_lock: lock(&contract).unwrap(),
                configuration: json!({"capture_mode": "manual"}),
                layout: Layout { x: 20.0, y: 40.0 },
                annotation: String::new(),
                compatibility_metadata: json!({}),
            }],
            connections: vec![],
            annotation: "exact".into(),
            settings: json!({}),
            compatibility_metadata: json!({}),
        }
    }

    #[test]
    fn compile_is_pure_and_deterministic() {
        let first = compile(draft()).unwrap();
        let second = compile(draft()).unwrap();
        assert_eq!(first.compile_input_digest, second.compile_input_digest);
        assert_eq!(first.revision_digest, second.revision_digest);
        assert_eq!(first.plan_digest, second.plan_digest);
        assert_eq!(first.diagnostics, second.diagnostics);
        assert!(first.can_publish);
        assert_eq!(first.diagnostics[0].code, "W_OUTPUT_UNUSED");
        assert!(first.diagnostics[0].requires_ack);
    }

    #[test]
    fn validates_each_compiler_boundary_without_io() {
        let empty = WorkflowDraft {
            nodes: vec![],
            ..draft()
        };
        let result = compile(empty).unwrap();
        assert!(!result.can_publish);
        assert_eq!(result.diagnostics[0].code, "E_GRAPH_EMPTY");
        assert!(result.plan.is_none());

        let mut invalid = draft();
        invalid.nodes[0].configuration = json!({"capture_mode": "={{ unsafe }}"});
        let result = compile(invalid).unwrap();
        let codes: BTreeSet<_> = result
            .diagnostics
            .iter()
            .map(|item| item.code.as_str())
            .collect();
        assert!(codes.contains("E_CONFIGURATION_INVALID"));
        assert!(codes.contains("E_EXPRESSION_UNSUPPORTED"));
    }

    #[test]
    fn profile_capability_effect_and_budget_checks_are_structured() {
        let mut contract: Value = serde_json::from_str(include_str!(
            "../../../contracts/manual-trigger.v1alpha1.json"
        ))
        .unwrap();
        let mut input = draft();
        contract["capabilities"] = json!(["network.egress"]);
        contract["effects"]["class"] = json!("mystery");
        contract["resources"]["hard"]["memory_bytes"] = json!(999_999_999_u64);
        contract["compatibility"]["profile"] = json!("emulated");
        input.nodes[0].contract_lock = lock(&contract).unwrap();
        let result = compile_with_inputs(
            input,
            vec![contract],
            native_profile(),
            publication_policy(),
        )
        .unwrap();
        let codes: BTreeSet<_> = result
            .diagnostics
            .iter()
            .map(|item| item.code.as_str())
            .collect();
        assert!(codes.contains("E_CAPABILITY_DENIED"));
        assert!(codes.contains("E_EFFECTS_INVALID"));
        assert!(codes.contains("E_BUDGET_EXCEEDED"));
        assert!(codes.contains("E_COMPATIBILITY_UNSUPPORTED"));
    }

    #[test]
    fn invalid_ports_and_graph_identities_are_rejected() {
        let mut duplicate = draft();
        duplicate.nodes.push(duplicate.nodes[0].clone());
        let duplicate_result = compile(duplicate).unwrap();
        assert!(duplicate_result
            .diagnostics
            .iter()
            .any(|item| item.code == "E_NODE_IDENTITY_DUPLICATE"));

        let mut input = draft();
        input.connections.push(json!({
            "id": "edge-one",
            "source": {"node_id": "manual", "port_id": "missing"},
            "target": {"node_id": "manual", "port_id": "missing"}
        }));
        let result = compile(input).unwrap();
        let codes: BTreeSet<_> = result
            .diagnostics
            .iter()
            .map(|item| item.code.as_str())
            .collect();
        assert!(codes.contains("E_PORT_SOURCE"));
        assert!(codes.contains("E_PORT_TARGET"));

        let mut contract: Value = serde_json::from_str(include_str!(
            "../../../contracts/manual-trigger.v1alpha1.json"
        ))
        .unwrap();
        contract["ports"]["inputs"] = json!([{
            "id": "required-input",
            "schema": {"type": "dynamic-item"},
            "required": true,
            "cardinality": "one",
            "multiplicity": "one"
        }]);
        let mut missing_required = draft();
        missing_required.nodes[0].contract_lock = lock(&contract).unwrap();
        let result = compile_with_inputs(
            missing_required,
            vec![contract],
            native_profile(),
            publication_policy(),
        )
        .unwrap();
        assert!(result
            .diagnostics
            .iter()
            .any(|item| item.code == "E_PORT_REQUIRED"));
    }
}
