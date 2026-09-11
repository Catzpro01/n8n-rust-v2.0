// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pure deterministic Generate Items expansion. No storage, network, clock, or scheduler I/O.

use crate::{
    canonical::digest,
    compiler::{ExecutionPlan, COMPILER_ABI, PLAN_FORMAT},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const GENERATE_ENGINE_ABI: &str = "canopy.generate-engine/v1alpha1";
pub const MAX_GENERATED_ITEMS: u64 = 50_000;
pub const MAX_LOGICAL_OUTPUT_BYTES: u64 = 64 * 1024 * 1024;
pub const MICRO_BATCH_ITEMS: usize = 64;
pub const MICRO_BATCH_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneratedEnvelope {
    pub ordinal: u64,
    pub item: Value,
    pub logical_bytes: u64,
    pub provenance: Value,
}

#[derive(Clone, Debug, Default)]
pub struct GenerateResume {
    pub next_ordinal: u64,
    pub logical_bytes: u64,
    pub stream_digest: String,
}

pub struct GenerateStart<'a> {
    pub run_id: String,
    pub revision_digest: String,
    pub plan_digest: String,
    pub plan: &'a ExecutionPlan,
    pub input: Value,
    pub logical_data_override: Option<Value>,
    pub physical_data: Option<Value>,
    pub resume: Option<GenerateResume>,
}

#[derive(Clone, Debug)]
pub struct GenerateSession {
    run_id: String,
    revision_digest: String,
    plan_digest: String,
    manual_node_id: String,
    generate_node_id: String,
    input: Value,
    data: Value,
    transport_data: Value,
    count: u64,
    start: i64,
    step: i64,
    next: u64,
    logical_bytes: u64,
    stream_digest: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerateSummary {
    pub manual_node_id: String,
    pub generate_node_id: String,
    pub generated_count: u64,
    pub logical_bytes: u64,
    pub stream_digest: String,
    pub correctness_digest: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerateFailure {
    pub code: String,
    pub message: String,
}

impl GenerateSession {
    pub fn start(request: GenerateStart<'_>) -> Result<Self, GenerateFailure> {
        let GenerateStart {
            run_id,
            revision_digest,
            plan_digest,
            plan,
            input,
            logical_data_override,
            physical_data,
            resume,
        } = request;
        if plan.format != PLAN_FORMAT
            || plan.compiler_abi != COMPILER_ABI
            || plan.revision_digest != revision_digest
            || plan.nodes.len() != 2
            || plan.scheduling_dependencies.len() != 1
            || plan.lane_eligibility != ["native-cpu"]
        {
            return Err(invalid(
                "The pinned plan is not an eligible two-node native plan.",
            ));
        }
        let manual = plan
            .nodes
            .iter()
            .find(|node| node.contract_lock.name == "manual-trigger")
            .ok_or_else(|| invalid("Manual Trigger is unavailable in the pinned plan."))?;
        let generate = plan
            .nodes
            .iter()
            .find(|node| node.contract_lock.name == "generate-items")
            .ok_or_else(|| invalid("Generate Items is unavailable in the pinned plan."))?;
        let dependency = &plan.scheduling_dependencies[0];
        if manual.contract_lock.namespace != "canopy.native"
            || generate.contract_lock.namespace != "canopy.native"
            || manual.configuration != json!({"capture_mode":"manual"})
            || dependency.source_node_id != manual.node_instance_id
            || dependency.source_port_id != "invocation"
            || dependency.target_node_id != generate.node_instance_id
            || dependency.target_port_id != "input"
            || !manual.capabilities.is_empty()
            || !generate.capabilities.is_empty()
            || generate.effects["class"] != "pure"
            || generate.effects["deterministic"] != true
        {
            return Err(invalid(
                "The pinned Generate Items topology or contract is invalid.",
            ));
        }
        let configuration = generate
            .configuration
            .as_object()
            .ok_or_else(|| invalid("Generate Items configuration is not an object."))?;
        let count = configuration
            .get("count")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("Generate Items count is invalid."))?;
        let start = configuration
            .get("start")
            .and_then(Value::as_i64)
            .ok_or_else(|| invalid("Generate Items start is invalid."))?;
        let step = configuration
            .get("step")
            .and_then(Value::as_i64)
            .ok_or_else(|| invalid("Generate Items step is invalid."))?;
        let configured_data = configuration
            .get("data")
            .cloned()
            .ok_or_else(|| invalid("Generate Items data is missing."))?;
        let data = logical_data_override.unwrap_or(configured_data);
        if count > MAX_GENERATED_ITEMS {
            return Err(GenerateFailure {
                code: "canopy.generate-items.output_count_exceeded".into(),
                message: "The configured item count exceeds the hard output limit.".into(),
            });
        }
        if count > 0 {
            checked_value(start, step, count - 1)?;
        }
        let resume = resume.unwrap_or_else(|| GenerateResume {
            next_ordinal: 0,
            logical_bytes: 0,
            stream_digest: "genesis".into(),
        });
        if resume.next_ordinal > count
            || resume.logical_bytes > MAX_LOGICAL_OUTPUT_BYTES
            || (resume.next_ordinal == 0
                && (resume.logical_bytes != 0 || resume.stream_digest != "genesis"))
            || (resume.next_ordinal > 0
                && (!resume.stream_digest.starts_with("sha256:")
                    || resume.stream_digest.len() != 71))
        {
            return Err(invalid(
                "The durable Generate Items resume cursor is invalid.",
            ));
        }
        Ok(Self {
            run_id,
            revision_digest,
            plan_digest,
            manual_node_id: manual.node_instance_id.clone(),
            generate_node_id: generate.node_instance_id.clone(),
            input,
            transport_data: physical_data.unwrap_or_else(|| data.clone()),
            data,
            count,
            start,
            step,
            next: resume.next_ordinal,
            logical_bytes: resume.logical_bytes,
            stream_digest: resume.stream_digest,
        })
    }

    pub fn next_batch(&mut self) -> Result<Option<Vec<GeneratedEnvelope>>, GenerateFailure> {
        if self.next >= self.count {
            return Ok(None);
        }
        let mut batch = Vec::with_capacity(MICRO_BATCH_ITEMS);
        let mut batch_bytes = 0_usize;
        while self.next < self.count && batch.len() < MICRO_BATCH_ITEMS {
            let value = checked_value(self.start, self.step, self.next)?;
            let logical_item = json!({"index": self.next, "value": value, "data": self.data});
            let logical_item_bytes =
                serde_jcs::to_vec(&logical_item).map_err(|error| GenerateFailure {
                    code: "canopy.generate-items.canonicalization_failed".into(),
                    message: error.to_string(),
                })?;
            let item = json!({"index": self.next, "value": value, "data": self.transport_data});
            let envelope = GeneratedEnvelope {
                ordinal: self.next,
                item,
                logical_bytes: logical_item_bytes.len() as u64,
                provenance: json!({
                    "run_id":self.run_id,
                    "source_node_instance_id":self.manual_node_id,
                    "source_output_port":"invocation",
                    "generate_node_instance_id":self.generate_node_id,
                    "generate_output_port":"items",
                    "input":self.input,
                    "ordinal":self.next
                }),
            };
            let physical_envelope_bytes =
                serde_jcs::to_vec(&envelope).map_err(|error| GenerateFailure {
                    code: "canopy.generate-items.canonicalization_failed".into(),
                    message: error.to_string(),
                })?;
            if physical_envelope_bytes.len() > MICRO_BATCH_BYTES {
                return Err(GenerateFailure {
                    code: "canopy.generate-items.item_bytes_exceeded".into(),
                    message: "One physical generated Envelope exceeds the micro-batch byte limit."
                        .into(),
                });
            }
            if !batch.is_empty()
                && batch_bytes.saturating_add(physical_envelope_bytes.len()) > MICRO_BATCH_BYTES
            {
                break;
            }
            let next_total = self
                .logical_bytes
                .saturating_add(logical_item_bytes.len() as u64);
            if next_total > MAX_LOGICAL_OUTPUT_BYTES {
                return Err(GenerateFailure {
                    code: "canopy.generate-items.output_bytes_exceeded".into(),
                    message: "Generated logical output exceeds the hard byte limit.".into(),
                });
            }
            self.stream_digest = digest(&json!({
                "schema":"canopy.generated-item-chain/v1alpha1",
                "previous":self.stream_digest,
                "ordinal":self.next,
                "item":logical_item
            }))
            .map_err(|message| GenerateFailure {
                code: "canopy.generate-items.digest_failed".into(),
                message,
            })?;
            batch_bytes += physical_envelope_bytes.len();
            self.logical_bytes = next_total;
            batch.push(envelope);
            self.next += 1;
        }
        Ok(Some(batch))
    }

    pub fn progress(&self) -> (u64, u64, &str) {
        (self.next, self.logical_bytes, &self.stream_digest)
    }

    pub fn finish(self) -> Result<GenerateSummary, GenerateFailure> {
        if self.next != self.count {
            return Err(invalid(
                "Generate Items finished before its configured count.",
            ));
        }
        let correctness_digest = digest(&json!({
            "schema":"canopy.correctness-digest/v1alpha1",
            "revision_digest":self.revision_digest,
            "plan_digest":self.plan_digest,
            "logical_outcomes":[
                {"logical_order":1,"node_instance_id":self.manual_node_id,"outcome":"success","port":"invocation","output":self.input},
                {"logical_order":2,"node_instance_id":self.generate_node_id,"outcome":"success","port":"items","generated_count":self.count,"logical_bytes":self.logical_bytes,"stream_digest":self.stream_digest}
            ]
        })).map_err(|message| GenerateFailure { code: "canopy.generate-items.digest_failed".into(), message })?;
        Ok(GenerateSummary {
            manual_node_id: self.manual_node_id,
            generate_node_id: self.generate_node_id,
            generated_count: self.count,
            logical_bytes: self.logical_bytes,
            stream_digest: self.stream_digest,
            correctness_digest,
        })
    }
}

fn checked_value(start: i64, step: i64, ordinal: u64) -> Result<i64, GenerateFailure> {
    let ordinal = i64::try_from(ordinal).map_err(|_| overflow())?;
    step.checked_mul(ordinal)
        .and_then(|delta| start.checked_add(delta))
        .ok_or_else(overflow)
}

fn invalid(message: &str) -> GenerateFailure {
    GenerateFailure {
        code: "canopy.generate-items.invalid_pinned_plan".into(),
        message: message.into(),
    }
}

fn overflow() -> GenerateFailure {
    GenerateFailure {
        code: "canopy.generate-items.range_overflow".into(),
        message: "The configured signed range overflows i64.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::{CompatibilityProfile, PlanDependency, PlanNode};
    use canopy_node_contract::lock;

    fn plan(count: u64, start: i64, step: i64, data: Value) -> ExecutionPlan {
        let manual_contract: Value = serde_json::from_str(include_str!(
            "../../../contracts/manual-trigger.v1alpha1.json"
        ))
        .unwrap();
        let generate_contract: Value = serde_json::from_str(include_str!(
            "../../../contracts/generate-items.v1alpha1.json"
        ))
        .unwrap();
        let node = |id: &str, contract: &Value, configuration: Value| PlanNode {
            node_instance_id: id.into(),
            contract_lock: lock(contract).unwrap(),
            configuration,
            activation: contract["activation"].clone(),
            effects: contract["effects"].clone(),
            capabilities: vec![],
            resources: contract["resources"]["hard"].clone(),
            input_ports: contract["ports"]["inputs"].clone(),
            output_ports: contract["ports"]["outputs"].clone(),
        };
        ExecutionPlan {
            format: PLAN_FORMAT.into(),
            compiler_abi: COMPILER_ABI.into(),
            revision_digest: "sha256:revision".into(),
            compatibility_profile: CompatibilityProfile {
                profile_id: "canopy.native/v1alpha1".into(),
                status: "native".into(),
            },
            policy_digest: "sha256:policy".into(),
            contract_locks: vec![
                lock(&manual_contract).unwrap(),
                lock(&generate_contract).unwrap(),
            ],
            nodes: vec![
                node("manual", &manual_contract, json!({"capture_mode":"manual"})),
                node(
                    "generate",
                    &generate_contract,
                    json!({"count":count,"start":start,"step":step,"data":data,"storage_mode":"auto"}),
                ),
            ],
            scheduling_dependencies: vec![PlanDependency {
                connection_id: "manual-to-generate".into(),
                source_node_id: "manual".into(),
                source_port_id: "invocation".into(),
                target_node_id: "generate".into(),
                target_port_id: "input".into(),
            }],
            segment_candidates: vec![vec!["manual".into(), "generate".into()]],
            lane_eligibility: vec!["native-cpu".into()],
        }
    }

    fn session(
        plan: &ExecutionPlan,
        physical_data: Option<Value>,
        resume: Option<GenerateResume>,
    ) -> Result<GenerateSession, GenerateFailure> {
        GenerateSession::start(GenerateStart {
            run_id: "run-generate-test".into(),
            revision_digest: "sha256:revision".into(),
            plan_digest: "sha256:plan".into(),
            plan,
            input: json!({"manual":true}),
            logical_data_override: None,
            physical_data,
            resume,
        })
    }

    #[test]
    fn eco_stream_is_exact_ordered_and_micro_bounded() {
        let plan = plan(49_998, 0, 1, Value::Null);
        let mut session = session(&plan, None, None).unwrap();
        let mut expected = 0_u64;
        while let Some(batch) = session.next_batch().unwrap() {
            assert!(!batch.is_empty() && batch.len() <= MICRO_BATCH_ITEMS);
            let physical_bytes: usize = batch
                .iter()
                .map(|envelope| serde_jcs::to_vec(envelope).unwrap().len())
                .sum();
            assert!(physical_bytes <= MICRO_BATCH_BYTES);
            for envelope in batch {
                assert_eq!(envelope.ordinal, expected);
                assert_eq!(envelope.item["index"], expected);
                assert_eq!(envelope.item["value"], expected);
                assert_eq!(envelope.item["data"], Value::Null);
                assert_eq!(envelope.provenance["ordinal"], expected);
                expected += 1;
            }
        }
        let summary = session.finish().unwrap();
        assert_eq!(expected, 49_998);
        assert_eq!(summary.generated_count, 49_998);
        assert!(summary.logical_bytes <= MAX_LOGICAL_OUTPUT_BYTES);
    }

    #[test]
    fn resume_starts_at_the_durable_suffix_and_preserves_digest() {
        let plan = plan(2_100, -5, 3, json!({"fixed":true}));
        let mut uninterrupted = session(&plan, None, None).unwrap();
        while uninterrupted.progress().0 < 1_024 {
            uninterrupted.next_batch().unwrap();
        }
        let progress = uninterrupted.progress();
        let resume = GenerateResume {
            next_ordinal: progress.0,
            logical_bytes: progress.1,
            stream_digest: progress.2.into(),
        };
        let mut resumed = session(&plan, None, Some(resume)).unwrap();
        assert_eq!(resumed.next_batch().unwrap().unwrap()[0].ordinal, 1_024);
        while uninterrupted.next_batch().unwrap().is_some() {}
        while resumed.next_batch().unwrap().is_some() {}
        let uninterrupted = uninterrupted.finish().unwrap();
        let resumed = resumed.finish().unwrap();
        assert_eq!(resumed.stream_digest, uninterrupted.stream_digest);
        assert_eq!(resumed.correctness_digest, uninterrupted.correctness_digest);
    }

    #[test]
    fn spill_changes_only_physical_data_not_logical_digest() {
        let logical_data = json!({"large":"x".repeat(5_000)});
        let plan = plan(130, 8, -2, logical_data);
        let mut inline = session(&plan, None, None).unwrap();
        let mut spilled = session(
            &plan,
            Some(json!({"$artifact":{"artifact_id":"artifact-opaque"}})),
            None,
        )
        .unwrap();
        while inline.next_batch().unwrap().is_some() {}
        while let Some(batch) = spilled.next_batch().unwrap() {
            assert!(batch
                .iter()
                .all(|envelope| envelope.item["data"].get("$artifact").is_some()));
        }
        let inline = inline.finish().unwrap();
        let spilled = spilled.finish().unwrap();
        assert_eq!(inline.logical_bytes, spilled.logical_bytes);
        assert_eq!(inline.stream_digest, spilled.stream_digest);
        assert_eq!(inline.correctness_digest, spilled.correctness_digest);
    }

    #[test]
    fn authored_hard_limits_and_signed_overflow_are_permanent_failures() {
        let too_many = plan(MAX_GENERATED_ITEMS + 1, 0, 1, Value::Null);
        assert_eq!(
            session(&too_many, None, None).unwrap_err().code,
            "canopy.generate-items.output_count_exceeded"
        );
        let overflow = plan(2, i64::MAX, 1, Value::Null);
        assert_eq!(
            session(&overflow, None, None).unwrap_err().code,
            "canopy.generate-items.range_overflow"
        );
        let oversized_item = plan(1, 0, 1, Value::String("x".repeat(MICRO_BATCH_BYTES)));
        let mut oversized_item = session(&oversized_item, None, None).unwrap();
        assert_eq!(
            oversized_item.next_batch().unwrap_err().code,
            "canopy.generate-items.item_bytes_exceeded"
        );
        let bytes = plan(MAX_GENERATED_ITEMS, 0, 1, Value::String("x".repeat(2_000)));
        let mut session =
            session(&bytes, Some(json!({"$artifact":"artifact-opaque"})), None).unwrap();
        let error = loop {
            match session.next_batch() {
                Ok(Some(_)) => continue,
                Ok(None) => panic!("logical byte budget unexpectedly passed"),
                Err(error) => break error,
            }
        };
        assert_eq!(error.code, "canopy.generate-items.output_bytes_exceeded");
    }
}
