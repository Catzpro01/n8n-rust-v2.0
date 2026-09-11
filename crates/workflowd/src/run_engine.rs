// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pure deterministic Run state transitions for the first native Manual Trigger.
//! This module performs no storage, network, clock, or scheduler I/O.

use crate::{
    canonical::digest,
    compiler::{ExecutionPlan, COMPILER_ABI, PLAN_FORMAT},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const ENGINE_ABI: &str = "canopy.run-engine/v1alpha1";
pub const CORRECTNESS_DIGEST_SCHEMA: &str = "canopy.correctness-digest/v1alpha1";

#[derive(Clone, Debug)]
pub struct ManualActivationInput {
    pub run_id: String,
    pub revision_id: String,
    pub revision_digest: String,
    pub plan_digest: String,
    pub plan: ExecutionPlan,
    pub captured_invocation: Value,
    pub cancellation_observed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivationOutcome {
    Success,
    Cancelled,
    PermanentFailure,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManualActivationResult {
    pub activation_id: String,
    pub node_instance_id: String,
    pub logical_order: u64,
    pub attempt: u32,
    pub outcome: ActivationOutcome,
    pub input: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
    pub output_port: String,
    pub correctness_digest: String,
    pub counters: ActivationCounters,
    pub provenance: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<EngineFailure>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivationCounters {
    pub attempted: u64,
    pub succeeded: u64,
    pub cancelled: u64,
    pub failed: u64,
    pub output_count: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EngineFailure {
    pub code: String,
    pub message: String,
}

pub fn execute_manual(input: ManualActivationInput) -> ManualActivationResult {
    let validation = validate_plan(&input);
    let node_instance_id = input
        .plan
        .nodes
        .first()
        .map(|node| node.node_instance_id.clone())
        .unwrap_or_else(|| "invalid-plan".into());
    let activation_id = activation_id(&input.run_id, &node_instance_id);
    let provenance = json!({
        "engine_abi": ENGINE_ABI,
        "revision_id": input.revision_id,
        "revision_digest": input.revision_digest,
        "plan_digest": input.plan_digest,
        "node_contract": input.plan.nodes.first().map(|node| &node.contract_lock),
        "lane": "native-cpu",
        "effect_class": "pure"
    });

    if let Err(failure) = validation {
        return ManualActivationResult {
            activation_id,
            node_instance_id,
            logical_order: 1,
            attempt: 1,
            outcome: ActivationOutcome::PermanentFailure,
            input: input.captured_invocation,
            output: None,
            output_port: "invocation".into(),
            correctness_digest: failed_digest(&input.revision_digest, &input.plan_digest, &failure),
            counters: ActivationCounters {
                attempted: 1,
                succeeded: 0,
                cancelled: 0,
                failed: 1,
                output_count: 0,
            },
            provenance,
            failure: Some(failure),
        };
    }

    if input.cancellation_observed {
        return ManualActivationResult {
            activation_id,
            node_instance_id,
            logical_order: 1,
            attempt: 1,
            outcome: ActivationOutcome::Cancelled,
            input: input.captured_invocation,
            output: None,
            output_port: "invocation".into(),
            correctness_digest: cancelled_correctness_digest(
                &input.revision_digest,
                &input.plan_digest,
            ),
            counters: ActivationCounters {
                attempted: 1,
                succeeded: 0,
                cancelled: 1,
                failed: 0,
                output_count: 0,
            },
            provenance,
            failure: None,
        };
    }

    let output = input.captured_invocation.clone();
    let correctness_digest = success_digest(
        &input.revision_digest,
        &input.plan_digest,
        &node_instance_id,
        &output,
    );
    ManualActivationResult {
        activation_id,
        node_instance_id,
        logical_order: 1,
        attempt: 1,
        outcome: ActivationOutcome::Success,
        input: input.captured_invocation,
        output: Some(output),
        output_port: "invocation".into(),
        correctness_digest,
        counters: ActivationCounters {
            attempted: 1,
            succeeded: 1,
            cancelled: 0,
            failed: 0,
            output_count: 1,
        },
        provenance,
        failure: None,
    }
}

fn validate_plan(input: &ManualActivationInput) -> Result<(), EngineFailure> {
    let plan = &input.plan;
    if plan.format != PLAN_FORMAT
        || plan.compiler_abi != COMPILER_ABI
        || plan.revision_digest != input.revision_digest
        || plan.nodes.len() != 1
        || !plan.scheduling_dependencies.is_empty()
        || plan.lane_eligibility != ["native-cpu"]
    {
        return Err(invalid_plan(
            "The pinned plan is not an eligible one-node native plan.",
        ));
    }
    let node = &plan.nodes[0];
    let is_manual = node.contract_lock.namespace == "canopy.native"
        && node.contract_lock.name == "manual-trigger"
        && node.configuration == json!({"capture_mode": "manual"})
        && node.activation["shape"] == "source"
        && node.activation["ordering"] == "captured_invocation"
        && node.effects["class"] == "pure"
        && node.effects["deterministic"] == true
        && node.capabilities.is_empty()
        && node.input_ports.as_array().is_some_and(Vec::is_empty)
        && node.output_ports.as_array().is_some_and(|ports| {
            ports.len() == 1 && ports[0]["id"] == "invocation" && ports[0]["cardinality"] == "one"
        });
    if !is_manual {
        return Err(invalid_plan(
            "The pinned plan does not contain the supported Manual Trigger contract.",
        ));
    }
    Ok(())
}

fn activation_id(run_id: &str, node_instance_id: &str) -> String {
    let identity = json!({
        "schema": "canopy.activation-identity/v1alpha1",
        "run_id": run_id,
        "node_instance_id": node_instance_id,
        "logical_order": 1,
        "attempt": 1
    });
    let tagged = digest(&identity).expect("Activation identity is JCS serializable");
    format!("activation-{}", &tagged[7..39])
}

fn success_digest(
    revision_digest: &str,
    plan_digest: &str,
    node_instance_id: &str,
    output: &Value,
) -> String {
    digest(&json!({
        "schema": CORRECTNESS_DIGEST_SCHEMA,
        "revision_digest": revision_digest,
        "plan_digest": plan_digest,
        "logical_outcomes": [{
            "logical_order": 1,
            "node_instance_id": node_instance_id,
            "outcome": "success",
            "port": "invocation",
            "output": output
        }]
    }))
    .expect("correctness evidence is JCS serializable")
}

pub(crate) fn cancelled_correctness_digest(revision_digest: &str, plan_digest: &str) -> String {
    digest(&json!({
        "schema": CORRECTNESS_DIGEST_SCHEMA,
        "revision_digest": revision_digest,
        "plan_digest": plan_digest,
        "complete": false,
        "logical_outcomes": []
    }))
    .expect("cancelled correctness evidence is JCS serializable")
}

fn failed_digest(revision_digest: &str, plan_digest: &str, failure: &EngineFailure) -> String {
    digest(&json!({
        "schema": CORRECTNESS_DIGEST_SCHEMA,
        "revision_digest": revision_digest,
        "plan_digest": plan_digest,
        "complete": false,
        "failure": failure
    }))
    .expect("failed correctness evidence is JCS serializable")
}

fn invalid_plan(message: &str) -> EngineFailure {
    EngineFailure {
        code: "canopy.run.invalid_pinned_plan".into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler;
    use crate::draft::{Layout, NodeInstance, WorkflowDraft};
    use canopy_node_contract::lock;

    fn input(run_id: &str) -> ManualActivationInput {
        let contract: Value = serde_json::from_str(include_str!(
            "../../../contracts/manual-trigger.v1alpha1.json"
        ))
        .unwrap();
        let contract_lock = lock(&contract).unwrap();
        let draft = WorkflowDraft {
            workflow_id: "wf-engine".into(),
            name: "Engine fixture".into(),
            draft_version: 1,
            settings: json!({}),
            annotation: "".into(),
            compatibility_metadata: json!({}),
            nodes: vec![NodeInstance {
                id: "manual-1".into(),
                name: "Manual Trigger".into(),
                contract_lock,
                configuration: json!({"capture_mode": "manual"}),
                layout: Layout { x: 0.0, y: 0.0 },
                annotation: "".into(),
                compatibility_metadata: json!({}),
            }],
            connections: vec![],
        };
        let compiled = compiler::compile(draft).unwrap();
        ManualActivationInput {
            run_id: run_id.into(),
            revision_id: "revision-fixture".into(),
            revision_digest: compiled.revision_digest,
            plan_digest: compiled.plan_digest.unwrap(),
            plan: compiled.plan.unwrap(),
            captured_invocation: json!({"manual": true}),
            cancellation_observed: false,
        }
    }

    #[test]
    fn deterministic_manual_activation_has_one_logical_outcome() {
        let first = execute_manual(input("run-same"));
        let second = execute_manual(input("run-same"));
        assert_eq!(first.activation_id, second.activation_id);
        assert_eq!(first.correctness_digest, second.correctness_digest);
        assert_eq!(first.logical_order, 1);
        assert_eq!(first.outcome, ActivationOutcome::Success);
        assert_eq!(first.output, Some(json!({"manual": true})));
        assert_eq!(first.counters.output_count, 1);
    }

    #[test]
    fn cancellation_is_a_pure_input_to_the_transition() {
        let mut fixture = input("run-cancelled");
        fixture.cancellation_observed = true;
        let result = execute_manual(fixture);
        assert_eq!(result.outcome, ActivationOutcome::Cancelled);
        assert_eq!(result.output, None);
        assert_eq!(result.counters.cancelled, 1);
    }

    #[test]
    fn refuses_to_recompile_or_run_a_mutated_plan() {
        let mut fixture = input("run-invalid");
        fixture.plan.nodes[0].configuration = json!({"capture_mode": "network"});
        let result = execute_manual(fixture);
        assert_eq!(result.outcome, ActivationOutcome::PermanentFailure);
        assert_eq!(
            result.failure.unwrap().code,
            "canopy.run.invalid_pinned_plan"
        );
    }
}
