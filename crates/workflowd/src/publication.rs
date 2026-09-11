// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::{
    canonical::{bytes as canonical_bytes, digest, CANONICALIZATION, DIGEST_ALGORITHM},
    compiler::{self, CompatibilityProfile, CompileResult, Diagnostic, ExecutionPlan},
    config::ServeConfig,
    draft::WorkflowDraft,
    security::SecurityService,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use canopy_node_contract::NodeContractLock;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand_core::{OsRng, RngCore};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const SIGNATURE_ALGORITHM: &str = "ed25519-rfc8032";
const SIGNING_DOMAIN: &[u8] = b"canopy-publication-event-v1\0";

#[derive(Clone)]
pub struct PublicationService {
    database: PathBuf,
    security: Arc<SecurityService>,
}

#[derive(Debug)]
pub enum PublicationError {
    NotFound,
    LeaseRequired,
    CompileInputChanged { current_draft_version: u64 },
    CompileFailed { diagnostics: Vec<Diagnostic> },
    WarningAcknowledgementRequired { required: Vec<String> },
    RollbackTargetNotPreceding,
    NoCurrentPublication,
    RequestIdentityConflict,
    Invalid(&'static str),
    Integrity(String),
    Storage(String),
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompilePreviewRequest {
    pub editor_session_id: String,
    pub lease_generation: u64,
    pub draft_version: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishRequest {
    pub publication_id: String,
    pub editor_session_id: String,
    pub lease_generation: u64,
    pub draft_version: u64,
    pub compile_input_digest: String,
    pub acknowledged_diagnostics: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RollbackRequest {
    pub rollback_id: String,
    pub editor_session_id: String,
    pub lease_generation: u64,
    pub draft_version: u64,
    pub target_revision_id: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RevisionRecord {
    pub revision_id: String,
    pub sequence: u64,
    pub source_draft_version: u64,
    pub digest: String,
    pub payload: compiler::RevisionPayload,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PlanRecord {
    pub plan_id: String,
    pub digest: String,
    pub payload: ExecutionPlan,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PublicationEvidence {
    pub digest: String,
    pub compile_input_digest: String,
    pub compiler_result_digest: String,
    pub diagnostic_fingerprints: Vec<String>,
    pub acknowledged_diagnostics: Vec<String>,
    pub compiler_abi: String,
    pub plan_format: String,
    pub canonicalization: String,
    pub digest_algorithm: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SignatureIdentity {
    pub algorithm: String,
    pub key_id: String,
    pub public_key: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PublicationEnvelope {
    pub schema: String,
    pub kind: String,
    pub event_id: String,
    pub event_sequence: u64,
    pub workflow_id: String,
    pub previous_revision_id: Option<String>,
    pub target_revision_id: String,
    pub target_revision_sequence: u64,
    pub revision_digest: String,
    pub plan_digest: String,
    pub evidence_digest: String,
    pub compile_input_digest: String,
    pub compiler_abi: String,
    pub plan_format: String,
    pub canonicalization: String,
    pub digest_algorithm: String,
    pub compatibility_profile: CompatibilityProfile,
    pub contract_locks: Vec<NodeContractLock>,
    pub signature_identity: SignatureIdentity,
    pub occurred_at: i64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SignatureView {
    pub algorithm: String,
    pub canonicalization: String,
    pub key_id: String,
    pub public_key: String,
    pub value: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SignedPublicationEvent {
    pub envelope: PublicationEnvelope,
    pub signature: SignatureView,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PublishedRecord {
    pub publication_id: String,
    pub revision: RevisionRecord,
    pub plan: PlanRecord,
    pub evidence: PublicationEvidence,
    pub event: SignedPublicationEvent,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RollbackRecord {
    pub rollback_id: String,
    pub current_published: RevisionSummary,
    pub event: SignedPublicationEvent,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RevisionSummary {
    pub revision_id: String,
    pub sequence: u64,
    pub source_draft_version: u64,
    pub revision_digest: String,
    pub plan_digest: String,
    pub is_current: bool,
    pub is_newer_than_current: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MutableDraftSummary {
    pub draft_version: u64,
    pub node_count: usize,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DraftDifference {
    pub state: String,
    pub fields: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PublicationStatus {
    pub workflow_id: String,
    pub mutable_draft: MutableDraftSummary,
    pub current_published: Option<RevisionSummary>,
    pub latest_published: Option<RevisionSummary>,
    pub current_event: Option<SignedPublicationEvent>,
    pub revisions: Vec<RevisionSummary>,
    pub difference: DraftDifference,
}

type StoredSigningKey = (String, Vec<u8>, Vec<u8>, Vec<u8>);

struct SigningMaterial {
    key_id: String,
    public_key: [u8; 32],
    signing_key: SigningKey,
}

impl PublicationService {
    pub fn initialize(
        config: &ServeConfig,
        security: Arc<SecurityService>,
    ) -> Result<Self, String> {
        let service = Self {
            database: config.state_dir.join("workflow.sqlite3"),
            security,
        };
        let connection = service.connect()?;
        connection
            .execute_batch(
                r#"
            CREATE TABLE IF NOT EXISTS publication_signing_keys(
                key_id TEXT PRIMARY KEY,
                algorithm TEXT NOT NULL,
                public_key BLOB NOT NULL,
                wrap_algorithm TEXT NOT NULL,
                wrap_context TEXT NOT NULL,
                wrap_nonce BLOB NOT NULL,
                wrapped_seed BLOB NOT NULL,
                created_at INTEGER NOT NULL
            ) STRICT;
            CREATE TABLE IF NOT EXISTS workflow_revisions(
                revision_id TEXT PRIMARY KEY,
                workflow_id TEXT NOT NULL REFERENCES workflow_drafts(workflow_id),
                revision_sequence INTEGER NOT NULL,
                source_draft_version INTEGER NOT NULL,
                revision_digest TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                UNIQUE(workflow_id,revision_sequence)
            ) STRICT;
            CREATE TABLE IF NOT EXISTS execution_plans(
                plan_id TEXT PRIMARY KEY,
                revision_id TEXT NOT NULL UNIQUE REFERENCES workflow_revisions(revision_id),
                plan_digest TEXT NOT NULL,
                compiler_abi TEXT NOT NULL,
                plan_format TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at INTEGER NOT NULL
            ) STRICT;
            CREATE TABLE IF NOT EXISTS publication_evidence(
                revision_id TEXT PRIMARY KEY REFERENCES workflow_revisions(revision_id),
                evidence_digest TEXT NOT NULL,
                evidence_json TEXT NOT NULL,
                created_at INTEGER NOT NULL
            ) STRICT;
            CREATE TABLE IF NOT EXISTS publication_events(
                event_id TEXT PRIMARY KEY,
                workflow_id TEXT NOT NULL REFERENCES workflow_drafts(workflow_id),
                event_sequence INTEGER NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN ('publication','rollback')),
                request_id TEXT NOT NULL,
                request_digest TEXT NOT NULL,
                target_revision_id TEXT NOT NULL REFERENCES workflow_revisions(revision_id),
                envelope_json TEXT NOT NULL,
                signature_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                UNIQUE(workflow_id,event_sequence),
                UNIQUE(workflow_id,kind,request_id)
            ) STRICT;
            CREATE TABLE IF NOT EXISTS workflow_publication_heads(
                workflow_id TEXT PRIMARY KEY REFERENCES workflow_drafts(workflow_id),
                revision_id TEXT NOT NULL REFERENCES workflow_revisions(revision_id),
                event_id TEXT NOT NULL REFERENCES publication_events(event_id),
                updated_at INTEGER NOT NULL
            ) STRICT;
            CREATE TRIGGER IF NOT EXISTS immutable_publication_signing_keys_update
                BEFORE UPDATE ON publication_signing_keys BEGIN
                SELECT RAISE(ABORT,'publication signing keys are immutable'); END;
            CREATE TRIGGER IF NOT EXISTS immutable_publication_signing_keys_delete
                BEFORE DELETE ON publication_signing_keys BEGIN
                SELECT RAISE(ABORT,'publication signing keys are immutable'); END;
            CREATE TRIGGER IF NOT EXISTS immutable_workflow_revisions_update
                BEFORE UPDATE ON workflow_revisions BEGIN
                SELECT RAISE(ABORT,'workflow revisions are immutable'); END;
            CREATE TRIGGER IF NOT EXISTS immutable_workflow_revisions_delete
                BEFORE DELETE ON workflow_revisions BEGIN
                SELECT RAISE(ABORT,'workflow revisions are immutable'); END;
            CREATE TRIGGER IF NOT EXISTS immutable_execution_plans_update
                BEFORE UPDATE ON execution_plans BEGIN
                SELECT RAISE(ABORT,'execution plans are immutable'); END;
            CREATE TRIGGER IF NOT EXISTS immutable_execution_plans_delete
                BEFORE DELETE ON execution_plans BEGIN
                SELECT RAISE(ABORT,'execution plans are immutable'); END;
            CREATE TRIGGER IF NOT EXISTS immutable_publication_evidence_update
                BEFORE UPDATE ON publication_evidence BEGIN
                SELECT RAISE(ABORT,'publication evidence is immutable'); END;
            CREATE TRIGGER IF NOT EXISTS immutable_publication_evidence_delete
                BEFORE DELETE ON publication_evidence BEGIN
                SELECT RAISE(ABORT,'publication evidence is immutable'); END;
            CREATE TRIGGER IF NOT EXISTS immutable_publication_events_update
                BEFORE UPDATE ON publication_events BEGIN
                SELECT RAISE(ABORT,'publication events are immutable'); END;
            CREATE TRIGGER IF NOT EXISTS immutable_publication_events_delete
                BEFORE DELETE ON publication_events BEGIN
                SELECT RAISE(ABORT,'publication events are immutable'); END;
            "#,
            )
            .map_err(storage)?;
        Ok(service)
    }

    pub fn preview(
        &self,
        workflow_id: &str,
        request: CompilePreviewRequest,
    ) -> Result<CompileResult, PublicationError> {
        validate_identifier(workflow_id, "workflow_id")?;
        validate_identifier(&request.editor_session_id, "editor_session_id")?;
        let connection = self.connect().map_err(PublicationError::Storage)?;
        require_authority(
            &connection,
            workflow_id,
            &request.editor_session_id,
            request.lease_generation,
        )?;
        let draft = load_draft(&connection, workflow_id)?;
        if draft.draft_version != request.draft_version {
            return Err(PublicationError::CompileInputChanged {
                current_draft_version: draft.draft_version,
            });
        }
        compiler::compile(draft).map_err(PublicationError::Integrity)
    }

    pub fn publish(
        &self,
        workflow_id: &str,
        request: PublishRequest,
    ) -> Result<(PublishedRecord, bool), PublicationError> {
        validate_identifier(workflow_id, "workflow_id")?;
        validate_identifier(&request.publication_id, "publication_id")?;
        validate_identifier(&request.editor_session_id, "editor_session_id")?;
        validate_tagged_digest(&request.compile_input_digest, "compile_input_digest")?;
        let request_digest = digest(&request).map_err(PublicationError::Integrity)?;
        let connection = self.connect().map_err(PublicationError::Storage)?;
        require_authority(
            &connection,
            workflow_id,
            &request.editor_session_id,
            request.lease_generation,
        )?;
        if let Some((existing_digest, revision_id)) = publication_receipt(
            &connection,
            workflow_id,
            "publication",
            &request.publication_id,
        )? {
            if existing_digest != request_digest {
                return Err(PublicationError::RequestIdentityConflict);
            }
            return Ok((self.load_revision(workflow_id, &revision_id)?, false));
        }
        let draft = load_draft(&connection, workflow_id)?;
        if draft.draft_version != request.draft_version {
            return Err(PublicationError::CompileInputChanged {
                current_draft_version: draft.draft_version,
            });
        }
        let result = compiler::compile(draft).map_err(PublicationError::Integrity)?;
        if result.compile_input_digest != request.compile_input_digest {
            return Err(PublicationError::CompileInputChanged {
                current_draft_version: result.draft_version,
            });
        }
        if !result.can_publish {
            return Err(PublicationError::CompileFailed {
                diagnostics: result.diagnostics,
            });
        }
        let required = required_acknowledgements(&result);
        let supplied: BTreeSet<_> = request.acknowledged_diagnostics.iter().cloned().collect();
        let required_set: BTreeSet<_> = required.iter().cloned().collect();
        if supplied != required_set || supplied.len() != request.acknowledged_diagnostics.len() {
            return Err(PublicationError::WarningAcknowledgementRequired { required });
        }
        let signing = self.load_or_create_signing_key()?;
        let mut connection = self.connect().map_err(PublicationError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage_error)?;
        require_authority(
            &transaction,
            workflow_id,
            &request.editor_session_id,
            request.lease_generation,
        )?;
        if let Some((existing_digest, revision_id)) = publication_receipt(
            &transaction,
            workflow_id,
            "publication",
            &request.publication_id,
        )? {
            if existing_digest != request_digest {
                return Err(PublicationError::RequestIdentityConflict);
            }
            transaction.commit().map_err(storage_error)?;
            return Ok((self.load_revision(workflow_id, &revision_id)?, false));
        }
        let current_draft = load_draft(&transaction, workflow_id)?;
        if current_draft.draft_version != request.draft_version
            || compiler::compile_input_digest(&current_draft)
                .map_err(PublicationError::Integrity)?
                != result.compile_input_digest
        {
            return Err(PublicationError::CompileInputChanged {
                current_draft_version: current_draft.draft_version,
            });
        }
        let plan = result.plan.clone().ok_or_else(|| {
            PublicationError::Integrity("successful compiler result has no plan".into())
        })?;
        let plan_digest = result.plan_digest.clone().ok_or_else(|| {
            PublicationError::Integrity("successful compiler result has no plan digest".into())
        })?;
        let revision_sequence = next_revision_sequence(&transaction, workflow_id)?;
        let event_sequence = next_event_sequence(&transaction, workflow_id)?;
        let revision_id = stable_id("revision", workflow_id, &request.publication_id);
        let plan_id = stable_id("plan", workflow_id, &request.publication_id);
        let event_id = stable_id("event-publication", workflow_id, &request.publication_id);
        let compiler_result_digest = digest(&result).map_err(PublicationError::Integrity)?;
        let diagnostic_fingerprints = result
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.fingerprint.clone())
            .collect();
        let mut evidence = PublicationEvidence {
            digest: String::new(),
            compile_input_digest: result.compile_input_digest.clone(),
            compiler_result_digest,
            diagnostic_fingerprints,
            acknowledged_diagnostics: required,
            compiler_abi: result.compiler_abi.clone(),
            plan_format: result.plan_format.clone(),
            canonicalization: CANONICALIZATION.into(),
            digest_algorithm: DIGEST_ALGORITHM.into(),
        };
        evidence.digest = evidence_digest(&evidence)?;
        let previous_revision_id = current_head(&transaction, workflow_id)?;
        let envelope = PublicationEnvelope {
            schema: "canopy.publication-event/v1alpha1".into(),
            kind: "publication".into(),
            event_id: event_id.clone(),
            event_sequence,
            workflow_id: workflow_id.into(),
            previous_revision_id,
            target_revision_id: revision_id.clone(),
            target_revision_sequence: revision_sequence,
            revision_digest: result.revision_digest.clone(),
            plan_digest: plan_digest.clone(),
            evidence_digest: evidence.digest.clone(),
            compile_input_digest: result.compile_input_digest.clone(),
            compiler_abi: result.compiler_abi.clone(),
            plan_format: result.plan_format.clone(),
            canonicalization: CANONICALIZATION.into(),
            digest_algorithm: DIGEST_ALGORITHM.into(),
            compatibility_profile: result.compatibility_profile.clone(),
            contract_locks: result.contract_locks.clone(),
            signature_identity: signing.identity(),
            occurred_at: now_ms(),
        };
        let signed_event = sign_envelope(envelope, &signing)?;
        let revision = RevisionRecord {
            revision_id: revision_id.clone(),
            sequence: revision_sequence,
            source_draft_version: result.draft_version,
            digest: result.revision_digest.clone(),
            payload: result.revision,
        };
        let plan = PlanRecord {
            plan_id: plan_id.clone(),
            digest: plan_digest,
            payload: plan,
        };
        let payload_json = canonical_text(&revision.payload)?;
        let plan_json = canonical_text(&plan.payload)?;
        let evidence_json = canonical_text(&evidence)?;
        let envelope_json = canonical_text(&signed_event.envelope)?;
        let signature_json = canonical_text(&signed_event.signature)?;
        let created_at = signed_event.envelope.occurred_at;
        transaction.execute(
            "INSERT INTO workflow_revisions(revision_id,workflow_id,revision_sequence,source_draft_version,revision_digest,payload_json,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![revision_id,workflow_id,revision_sequence as i64,result.draft_version as i64,result.revision_digest,payload_json,created_at],
        ).map_err(storage_error)?;
        transaction.execute(
            "INSERT INTO execution_plans(plan_id,revision_id,plan_digest,compiler_abi,plan_format,payload_json,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![plan_id,revision_id,plan.digest,result.compiler_abi,result.plan_format,plan_json,created_at],
        ).map_err(storage_error)?;
        transaction.execute(
            "INSERT INTO publication_evidence(revision_id,evidence_digest,evidence_json,created_at) VALUES(?1,?2,?3,?4)",
            params![revision_id,evidence.digest,evidence_json,created_at],
        ).map_err(storage_error)?;
        transaction.execute(
            "INSERT INTO publication_events(event_id,workflow_id,event_sequence,kind,request_id,request_digest,target_revision_id,envelope_json,signature_json,created_at) VALUES(?1,?2,?3,'publication',?4,?5,?6,?7,?8,?9)",
            params![event_id,workflow_id,event_sequence as i64,request.publication_id,request_digest,revision_id,envelope_json,signature_json,created_at],
        ).map_err(storage_error)?;
        transaction.execute(
            "INSERT INTO workflow_publication_heads(workflow_id,revision_id,event_id,updated_at) VALUES(?1,?2,?3,?4) ON CONFLICT(workflow_id) DO UPDATE SET revision_id=excluded.revision_id,event_id=excluded.event_id,updated_at=excluded.updated_at",
            params![workflow_id,revision_id,event_id,created_at],
        ).map_err(storage_error)?;
        transaction.commit().map_err(storage_error)?;
        let record = PublishedRecord {
            publication_id: request.publication_id,
            revision,
            plan,
            evidence,
            event: signed_event,
        };
        self.verify_record(&record)?;
        Ok((record, true))
    }

    pub fn rollback(
        &self,
        workflow_id: &str,
        request: RollbackRequest,
    ) -> Result<(RollbackRecord, bool), PublicationError> {
        validate_identifier(workflow_id, "workflow_id")?;
        validate_identifier(&request.rollback_id, "rollback_id")?;
        validate_identifier(&request.editor_session_id, "editor_session_id")?;
        validate_identifier(&request.target_revision_id, "target_revision_id")?;
        let request_digest = digest(&request).map_err(PublicationError::Integrity)?;
        let connection = self.connect().map_err(PublicationError::Storage)?;
        require_authority(
            &connection,
            workflow_id,
            &request.editor_session_id,
            request.lease_generation,
        )?;
        let draft = load_draft(&connection, workflow_id)?;
        if draft.draft_version != request.draft_version {
            return Err(PublicationError::CompileInputChanged {
                current_draft_version: draft.draft_version,
            });
        }
        if let Some((existing_digest, target_revision_id)) =
            publication_receipt(&connection, workflow_id, "rollback", &request.rollback_id)?
        {
            if existing_digest != request_digest {
                return Err(PublicationError::RequestIdentityConflict);
            }
            let event =
                load_event_for_request(&connection, workflow_id, "rollback", &request.rollback_id)?;
            let target = self.summary_for(workflow_id, &target_revision_id, true, false)?;
            return Ok((
                RollbackRecord {
                    rollback_id: request.rollback_id,
                    current_published: target,
                    event,
                },
                false,
            ));
        }
        let target = self.load_revision(workflow_id, &request.target_revision_id)?;
        let signing = self.load_or_create_signing_key()?;
        let mut connection = self.connect().map_err(PublicationError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage_error)?;
        require_authority(
            &transaction,
            workflow_id,
            &request.editor_session_id,
            request.lease_generation,
        )?;
        let current_draft = load_draft(&transaction, workflow_id)?;
        if current_draft.draft_version != request.draft_version {
            return Err(PublicationError::CompileInputChanged {
                current_draft_version: current_draft.draft_version,
            });
        }
        if let Some((existing_digest, target_revision_id)) =
            publication_receipt(&transaction, workflow_id, "rollback", &request.rollback_id)?
        {
            if existing_digest != request_digest {
                return Err(PublicationError::RequestIdentityConflict);
            }
            transaction.commit().map_err(storage_error)?;
            let event = load_event_for_request(
                &self.connect().map_err(PublicationError::Storage)?,
                workflow_id,
                "rollback",
                &request.rollback_id,
            )?;
            let target = self.summary_for(workflow_id, &target_revision_id, true, false)?;
            return Ok((
                RollbackRecord {
                    rollback_id: request.rollback_id,
                    current_published: target,
                    event,
                },
                false,
            ));
        }
        let verified_target = load_record(&transaction, workflow_id, &request.target_revision_id)?;
        self.verify_record_with_connection(&transaction, &verified_target)?;
        let current_revision_id = current_head(&transaction, workflow_id)?
            .ok_or(PublicationError::NoCurrentPublication)?;
        let current = load_record(&transaction, workflow_id, &current_revision_id)?;
        self.verify_record_with_connection(&transaction, &current)?;
        if target.revision.sequence >= current.revision.sequence {
            return Err(PublicationError::RollbackTargetNotPreceding);
        }
        let event_sequence = next_event_sequence(&transaction, workflow_id)?;
        let event_id = stable_id("event-rollback", workflow_id, &request.rollback_id);
        let envelope = PublicationEnvelope {
            schema: "canopy.publication-event/v1alpha1".into(),
            kind: "rollback".into(),
            event_id: event_id.clone(),
            event_sequence,
            workflow_id: workflow_id.into(),
            previous_revision_id: Some(current_revision_id),
            target_revision_id: target.revision.revision_id.clone(),
            target_revision_sequence: target.revision.sequence,
            revision_digest: target.revision.digest.clone(),
            plan_digest: target.plan.digest.clone(),
            evidence_digest: target.evidence.digest.clone(),
            compile_input_digest: target.evidence.compile_input_digest.clone(),
            compiler_abi: target.evidence.compiler_abi.clone(),
            plan_format: target.evidence.plan_format.clone(),
            canonicalization: CANONICALIZATION.into(),
            digest_algorithm: DIGEST_ALGORITHM.into(),
            compatibility_profile: target.revision.payload.compatibility_profile.clone(),
            contract_locks: target.revision.payload.contract_locks.clone(),
            signature_identity: signing.identity(),
            occurred_at: now_ms(),
        };
        let signed_event = sign_envelope(envelope, &signing)?;
        let envelope_json = canonical_text(&signed_event.envelope)?;
        let signature_json = canonical_text(&signed_event.signature)?;
        let occurred_at = signed_event.envelope.occurred_at;
        transaction.execute(
            "INSERT INTO publication_events(event_id,workflow_id,event_sequence,kind,request_id,request_digest,target_revision_id,envelope_json,signature_json,created_at) VALUES(?1,?2,?3,'rollback',?4,?5,?6,?7,?8,?9)",
            params![event_id,workflow_id,event_sequence as i64,request.rollback_id,request_digest,target.revision.revision_id,envelope_json,signature_json,occurred_at],
        ).map_err(storage_error)?;
        transaction.execute(
            "UPDATE workflow_publication_heads SET revision_id=?2,event_id=?3,updated_at=?4 WHERE workflow_id=?1",
            params![workflow_id,target.revision.revision_id,event_id,occurred_at],
        ).map_err(storage_error)?;
        transaction.commit().map_err(storage_error)?;
        verify_event(&signed_event)?;
        let summary = self.summary_for(workflow_id, &target.revision.revision_id, true, false)?;
        Ok((
            RollbackRecord {
                rollback_id: request.rollback_id,
                current_published: summary,
                event: signed_event,
            },
            true,
        ))
    }

    pub fn load_revision(
        &self,
        workflow_id: &str,
        revision_id: &str,
    ) -> Result<PublishedRecord, PublicationError> {
        validate_identifier(workflow_id, "workflow_id")?;
        validate_identifier(revision_id, "revision_id")?;
        let connection = self.connect().map_err(PublicationError::Storage)?;
        let record = load_record(&connection, workflow_id, revision_id)?;
        self.verify_record_with_connection(&connection, &record)?;
        Ok(record)
    }

    pub fn status(&self, workflow_id: &str) -> Result<PublicationStatus, PublicationError> {
        validate_identifier(workflow_id, "workflow_id")?;
        let connection = self.connect().map_err(PublicationError::Storage)?;
        let draft = load_draft(&connection, workflow_id)?;
        let projection: Option<(String, String)> = connection
            .query_row(
                "SELECT revision_id,event_id FROM workflow_publication_heads WHERE workflow_id=?1",
                params![workflow_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(storage_error)?;
        let current_id = projection.as_ref().map(|item| item.0.clone());
        let current_event = projection
            .as_ref()
            .map(|item| load_event_by_id(&connection, workflow_id, &item.1))
            .transpose()?;
        let mut statement = connection.prepare(
            "SELECT r.revision_id,r.revision_sequence,r.source_draft_version,r.revision_digest,p.plan_digest FROM workflow_revisions r JOIN execution_plans p ON p.revision_id=r.revision_id WHERE r.workflow_id=?1 ORDER BY r.revision_sequence"
        ).map_err(storage_error)?;
        let raw = statement
            .query_map(params![workflow_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)? as u64,
                    row.get::<_, i64>(2)? as u64,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(storage_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage_error)?;
        let current_sequence = raw
            .iter()
            .find(|item| Some(item.0.as_str()) == current_id.as_deref())
            .map(|item| item.1);
        let revisions: Vec<_> = raw
            .into_iter()
            .map(|item| RevisionSummary {
                revision_id: item.0.clone(),
                sequence: item.1,
                source_draft_version: item.2,
                revision_digest: item.3,
                plan_digest: item.4,
                is_current: Some(item.0.as_str()) == current_id.as_deref(),
                is_newer_than_current: current_sequence.is_some_and(|sequence| item.1 > sequence),
            })
            .collect();
        let current_published = revisions.iter().find(|item| item.is_current).cloned();
        let latest_published = revisions.last().cloned();
        let difference = if let Some(current) = &current_published {
            let published = load_record(&connection, workflow_id, &current.revision_id)?;
            self.verify_record_with_connection(&connection, &published)?;
            let event = current_event.as_ref().ok_or_else(|| {
                PublicationError::Integrity("current publication event is missing".into())
            })?;
            if event.envelope.target_revision_id != published.revision.revision_id
                || event.envelope.target_revision_sequence != published.revision.sequence
                || event.envelope.revision_digest != published.revision.digest
                || event.envelope.plan_digest != published.plan.digest
                || event.envelope.evidence_digest != published.evidence.digest
                || event.envelope.compile_input_digest != published.evidence.compile_input_digest
            {
                return Err(PublicationError::Integrity(
                    "current publication event linkage verification failed".into(),
                ));
            }
            compare_draft(&draft, &published.revision.payload.draft)
        } else {
            DraftDifference {
                state: "unpublished".into(),
                fields: vec![
                    "name".into(),
                    "nodes".into(),
                    "connections".into(),
                    "annotation".into(),
                    "settings".into(),
                    "compatibility_metadata".into(),
                ],
            }
        };
        Ok(PublicationStatus {
            workflow_id: workflow_id.into(),
            mutable_draft: MutableDraftSummary {
                draft_version: draft.draft_version,
                node_count: draft.nodes.len(),
            },
            current_published,
            latest_published,
            current_event,
            revisions,
            difference,
        })
    }

    fn summary_for(
        &self,
        workflow_id: &str,
        revision_id: &str,
        is_current: bool,
        is_newer: bool,
    ) -> Result<RevisionSummary, PublicationError> {
        let record = self.load_revision(workflow_id, revision_id)?;
        Ok(RevisionSummary {
            revision_id: record.revision.revision_id,
            sequence: record.revision.sequence,
            source_draft_version: record.revision.source_draft_version,
            revision_digest: record.revision.digest,
            plan_digest: record.plan.digest,
            is_current,
            is_newer_than_current: is_newer,
        })
    }

    fn load_or_create_signing_key(&self) -> Result<SigningMaterial, PublicationError> {
        let mut connection = self.connect().map_err(PublicationError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage_error)?;
        let row: Option<StoredSigningKey> = transaction.query_row(
            "SELECT key_id,public_key,wrap_nonce,wrapped_seed FROM publication_signing_keys ORDER BY created_at DESC,key_id DESC LIMIT 1",
            [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))
        ).optional().map_err(storage_error)?;
        let material = if let Some((key_id, public_key, nonce, wrapped_seed)) = row {
            if public_key.len() != 32 {
                return Err(PublicationError::Integrity(
                    "stored publication public key has invalid length".into(),
                ));
            }
            let seed = self
                .security
                .unwrap_publication_seed(&key_id, &nonce, &wrapped_seed)
                .map_err(|error| {
                    PublicationError::Integrity(format!("publication signing key: {error:?}"))
                })?;
            let signing_key = SigningKey::from_bytes(&seed);
            let mut public = [0_u8; 32];
            public.copy_from_slice(&public_key);
            if signing_key.verifying_key().to_bytes() != public || key_id != key_id_for(&public) {
                return Err(PublicationError::Integrity(
                    "stored publication signing key identity mismatch".into(),
                ));
            }
            SigningMaterial {
                key_id,
                public_key: public,
                signing_key,
            }
        } else {
            let mut seed = zeroize::Zeroizing::new([0_u8; 32]);
            OsRng.fill_bytes(seed.as_mut());
            let signing_key = SigningKey::from_bytes(&seed);
            let public = signing_key.verifying_key().to_bytes();
            let key_id = key_id_for(&public);
            let wrapped = self
                .security
                .wrap_publication_seed(&key_id, &seed)
                .map_err(|error| {
                    PublicationError::Integrity(format!("publication signing key: {error:?}"))
                })?;
            transaction.execute(
                "INSERT INTO publication_signing_keys(key_id,algorithm,public_key,wrap_algorithm,wrap_context,wrap_nonce,wrapped_seed,created_at) VALUES(?1,?2,?3,'xchacha20poly1305','canopy:publication-signing-key-wrap:v1',?4,?5,?6)",
                params![key_id,SIGNATURE_ALGORITHM,public.as_slice(),wrapped.nonce.as_slice(),wrapped.ciphertext,now_ms()]
            ).map_err(storage_error)?;
            SigningMaterial {
                key_id,
                public_key: public,
                signing_key,
            }
        };
        transaction.commit().map_err(storage_error)?;
        Ok(material)
    }

    fn verify_record(&self, record: &PublishedRecord) -> Result<(), PublicationError> {
        let connection = self.connect().map_err(PublicationError::Storage)?;
        self.verify_record_with_connection(&connection, record)
    }

    fn verify_record_with_connection(
        &self,
        connection: &Connection,
        record: &PublishedRecord,
    ) -> Result<(), PublicationError> {
        if digest(&record.revision.payload).map_err(PublicationError::Integrity)?
            != record.revision.digest
        {
            return Err(PublicationError::Integrity(
                "revision digest verification failed".into(),
            ));
        }
        if digest(&record.plan.payload).map_err(PublicationError::Integrity)? != record.plan.digest
            || record.plan.payload.revision_digest != record.revision.digest
        {
            return Err(PublicationError::Integrity(
                "pinned plan digest verification failed".into(),
            ));
        }
        if evidence_digest(&record.evidence)? != record.evidence.digest {
            return Err(PublicationError::Integrity(
                "publication evidence digest verification failed".into(),
            ));
        }
        if record.revision.payload.format != compiler::REVISION_FORMAT
            || record.plan.payload.format != compiler::PLAN_FORMAT
            || record.plan.payload.compiler_abi != compiler::COMPILER_ABI
            || record.plan.payload.compatibility_profile
                != record.revision.payload.compatibility_profile
            || record.plan.payload.contract_locks != record.revision.payload.contract_locks
            || record.evidence.compiler_abi != compiler::COMPILER_ABI
            || record.evidence.plan_format != compiler::PLAN_FORMAT
            || record.evidence.canonicalization != CANONICALIZATION
            || record.evidence.digest_algorithm != DIGEST_ALGORITHM
            || record.event.envelope.kind != "publication"
            || record.event.envelope.workflow_id != record.revision.payload.draft.workflow_id
            || record.event.envelope.target_revision_id != record.revision.revision_id
            || record.event.envelope.target_revision_sequence != record.revision.sequence
            || record.event.envelope.revision_digest != record.revision.digest
            || record.event.envelope.plan_digest != record.plan.digest
            || record.event.envelope.evidence_digest != record.evidence.digest
            || record.event.envelope.compile_input_digest != record.evidence.compile_input_digest
            || record.event.envelope.compiler_abi != record.evidence.compiler_abi
            || record.event.envelope.plan_format != record.evidence.plan_format
            || record.event.envelope.canonicalization != CANONICALIZATION
            || record.event.envelope.digest_algorithm != DIGEST_ALGORITHM
            || record.event.envelope.contract_locks != record.revision.payload.contract_locks
            || record.event.envelope.compatibility_profile
                != record.revision.payload.compatibility_profile
        {
            return Err(PublicationError::Integrity(
                "publication envelope linkage verification failed".into(),
            ));
        }
        verify_event(&record.event)?;
        let stored_public: Vec<u8> = connection
            .query_row(
                "SELECT public_key FROM publication_signing_keys WHERE key_id=?1",
                params![record.event.signature.key_id],
                |row| row.get(0),
            )
            .map_err(storage_error)?;
        if URL_SAFE_NO_PAD.encode(stored_public) != record.event.signature.public_key {
            return Err(PublicationError::Integrity(
                "publication key registry verification failed".into(),
            ));
        }
        Ok(())
    }

    fn connect(&self) -> Result<Connection, String> {
        let connection = Connection::open(&self.database).map_err(storage)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(storage)?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(storage)?;
        connection
            .pragma_update(None, "synchronous", "FULL")
            .map_err(storage)?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(storage)?;
        Ok(connection)
    }
}

impl SigningMaterial {
    fn identity(&self) -> SignatureIdentity {
        SignatureIdentity {
            algorithm: SIGNATURE_ALGORITHM.into(),
            key_id: self.key_id.clone(),
            public_key: URL_SAFE_NO_PAD.encode(self.public_key),
        }
    }
}

fn sign_envelope(
    envelope: PublicationEnvelope,
    signing: &SigningMaterial,
) -> Result<SignedPublicationEvent, PublicationError> {
    let canonical = canonical_bytes(&envelope).map_err(PublicationError::Integrity)?;
    let mut message = Vec::with_capacity(SIGNING_DOMAIN.len() + canonical.len());
    message.extend_from_slice(SIGNING_DOMAIN);
    message.extend_from_slice(&canonical);
    let signature = signing.signing_key.sign(&message);
    Ok(SignedPublicationEvent {
        envelope,
        signature: SignatureView {
            algorithm: SIGNATURE_ALGORITHM.into(),
            canonicalization: CANONICALIZATION.into(),
            key_id: signing.key_id.clone(),
            public_key: URL_SAFE_NO_PAD.encode(signing.public_key),
            value: URL_SAFE_NO_PAD.encode(signature.to_bytes()),
        },
    })
}

fn verify_event(event: &SignedPublicationEvent) -> Result<(), PublicationError> {
    let identity = &event.envelope.signature_identity;
    if event.envelope.schema != "canopy.publication-event/v1alpha1"
        || !matches!(event.envelope.kind.as_str(), "publication" | "rollback")
        || event.envelope.canonicalization != CANONICALIZATION
        || event.envelope.digest_algorithm != DIGEST_ALGORITHM
        || event.envelope.compiler_abi != compiler::COMPILER_ABI
        || event.envelope.plan_format != compiler::PLAN_FORMAT
        || event.signature.algorithm != SIGNATURE_ALGORITHM
        || event.signature.canonicalization != CANONICALIZATION
        || identity.algorithm != SIGNATURE_ALGORITHM
        || identity.key_id != event.signature.key_id
        || identity.public_key != event.signature.public_key
    {
        return Err(PublicationError::Integrity(
            "signature identity verification failed".into(),
        ));
    }
    let public: [u8; 32] = URL_SAFE_NO_PAD
        .decode(&event.signature.public_key)
        .map_err(integrity)?
        .try_into()
        .map_err(|_| {
            PublicationError::Integrity("signature public key has invalid length".into())
        })?;
    if key_id_for(&public) != event.signature.key_id {
        return Err(PublicationError::Integrity(
            "signature key ID verification failed".into(),
        ));
    }
    let signature_bytes: [u8; 64] = URL_SAFE_NO_PAD
        .decode(&event.signature.value)
        .map_err(integrity)?
        .try_into()
        .map_err(|_| PublicationError::Integrity("signature has invalid length".into()))?;
    let signature = Signature::from_bytes(&signature_bytes);
    let verifying = VerifyingKey::from_bytes(&public).map_err(integrity)?;
    let canonical = canonical_bytes(&event.envelope).map_err(PublicationError::Integrity)?;
    let mut message = Vec::with_capacity(SIGNING_DOMAIN.len() + canonical.len());
    message.extend_from_slice(SIGNING_DOMAIN);
    message.extend_from_slice(&canonical);
    verifying
        .verify_strict(&message, &signature)
        .map_err(|error| {
            PublicationError::Integrity(format!("Ed25519 verification failed: {error}"))
        })
}

fn load_record(
    connection: &Connection,
    workflow_id: &str,
    revision_id: &str,
) -> Result<PublishedRecord, PublicationError> {
    let row: (i64,i64,String,String,String,String,String,String,String,String,String) = connection.query_row(
        "SELECT r.revision_sequence,r.source_draft_version,r.revision_digest,r.payload_json,p.plan_id,p.plan_digest,p.payload_json,e.evidence_json,v.request_id,v.envelope_json,v.signature_json FROM workflow_revisions r JOIN execution_plans p ON p.revision_id=r.revision_id JOIN publication_evidence e ON e.revision_id=r.revision_id JOIN publication_events v ON v.target_revision_id=r.revision_id AND v.kind='publication' WHERE r.workflow_id=?1 AND r.revision_id=?2",
        params![workflow_id,revision_id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?,row.get(10)?))
    ).map_err(not_found_or_storage)?;
    let revision_payload = serde_json::from_str(&row.3).map_err(integrity)?;
    let plan_payload = serde_json::from_str(&row.6).map_err(integrity)?;
    let evidence = serde_json::from_str(&row.7).map_err(integrity)?;
    let envelope = serde_json::from_str(&row.9).map_err(integrity)?;
    let signature = serde_json::from_str(&row.10).map_err(integrity)?;
    if canonical_text(&revision_payload)? != row.3
        || canonical_text(&plan_payload)? != row.6
        || canonical_text(&evidence)? != row.7
        || canonical_text(&envelope)? != row.9
        || canonical_text(&signature)? != row.10
    {
        return Err(PublicationError::Integrity(
            "stored publication JSON is not RFC 8785 canonical".into(),
        ));
    }
    Ok(PublishedRecord {
        publication_id: row.8,
        revision: RevisionRecord {
            revision_id: revision_id.into(),
            sequence: row.0 as u64,
            source_draft_version: row.1 as u64,
            digest: row.2,
            payload: revision_payload,
        },
        plan: PlanRecord {
            plan_id: row.4,
            digest: row.5,
            payload: plan_payload,
        },
        evidence,
        event: SignedPublicationEvent {
            envelope,
            signature,
        },
    })
}

fn load_event_by_id(
    connection: &Connection,
    workflow_id: &str,
    event_id: &str,
) -> Result<SignedPublicationEvent, PublicationError> {
    let (envelope, signature): (String, String) = connection
        .query_row(
            "SELECT envelope_json,signature_json FROM publication_events WHERE workflow_id=?1 AND event_id=?2",
            params![workflow_id, event_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(not_found_or_storage)?;
    let event = SignedPublicationEvent {
        envelope: serde_json::from_str(&envelope).map_err(integrity)?,
        signature: serde_json::from_str(&signature).map_err(integrity)?,
    };
    if event.envelope.event_id != event_id {
        return Err(PublicationError::Integrity(
            "current publication event identity mismatch".into(),
        ));
    }
    if canonical_text(&event.envelope)? != envelope
        || canonical_text(&event.signature)? != signature
    {
        return Err(PublicationError::Integrity(
            "stored current-publication event is not RFC 8785 canonical".into(),
        ));
    }
    verify_event(&event)?;
    Ok(event)
}

fn load_event_for_request(
    connection: &Connection,
    workflow_id: &str,
    kind: &str,
    request_id: &str,
) -> Result<SignedPublicationEvent, PublicationError> {
    let (envelope,signature):(String,String) = connection.query_row(
        "SELECT envelope_json,signature_json FROM publication_events WHERE workflow_id=?1 AND kind=?2 AND request_id=?3",
        params![workflow_id,kind,request_id], |row| Ok((row.get(0)?,row.get(1)?))
    ).map_err(not_found_or_storage)?;
    let event = SignedPublicationEvent {
        envelope: serde_json::from_str(&envelope).map_err(integrity)?,
        signature: serde_json::from_str(&signature).map_err(integrity)?,
    };
    if canonical_text(&event.envelope)? != envelope
        || canonical_text(&event.signature)? != signature
    {
        return Err(PublicationError::Integrity(
            "stored publication receipt is not RFC 8785 canonical".into(),
        ));
    }
    verify_event(&event)?;
    Ok(event)
}

fn publication_receipt(
    connection: &Connection,
    workflow_id: &str,
    kind: &str,
    request_id: &str,
) -> Result<Option<(String, String)>, PublicationError> {
    connection.query_row(
        "SELECT request_digest,target_revision_id FROM publication_events WHERE workflow_id=?1 AND kind=?2 AND request_id=?3",
        params![workflow_id,kind,request_id], |row| Ok((row.get(0)?,row.get(1)?))
    ).optional().map_err(storage_error)
}

fn load_draft(
    connection: &Connection,
    workflow_id: &str,
) -> Result<WorkflowDraft, PublicationError> {
    let text: String = connection
        .query_row(
            "SELECT document_json FROM workflow_drafts WHERE workflow_id=?1",
            params![workflow_id],
            |row| row.get(0),
        )
        .map_err(not_found_or_storage)?;
    serde_json::from_str(&text)
        .map_err(|error| PublicationError::Integrity(format!("stored Draft is invalid: {error}")))
}

fn require_authority(
    connection: &Connection,
    workflow_id: &str,
    session: &str,
    generation: u64,
) -> Result<(), PublicationError> {
    let row: Option<(Option<String>, i64, i64)> = connection
        .query_row(
            "SELECT holder_session_id,generation,expires_at FROM draft_leases WHERE workflow_id=?1",
            params![workflow_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(storage_error)?;
    match row {
        Some((Some(holder), stored_generation, expires))
            if holder == session
                && stored_generation as u64 == generation
                && expires > now_ms() =>
        {
            Ok(())
        }
        _ => {
            let exists: i64 = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM workflow_drafts WHERE workflow_id=?1)",
                    params![workflow_id],
                    |row| row.get(0),
                )
                .map_err(storage_error)?;
            if exists == 0 {
                Err(PublicationError::NotFound)
            } else {
                Err(PublicationError::LeaseRequired)
            }
        }
    }
}

fn current_head(
    connection: &Connection,
    workflow_id: &str,
) -> Result<Option<String>, PublicationError> {
    connection
        .query_row(
            "SELECT revision_id FROM workflow_publication_heads WHERE workflow_id=?1",
            params![workflow_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage_error)
}

fn next_revision_sequence(
    connection: &Connection,
    workflow_id: &str,
) -> Result<u64, PublicationError> {
    connection.query_row("SELECT COALESCE(MAX(revision_sequence),0)+1 FROM workflow_revisions WHERE workflow_id=?1",params![workflow_id],|row|row.get::<_,i64>(0)).map(|value| value as u64).map_err(storage_error)
}

fn next_event_sequence(
    connection: &Connection,
    workflow_id: &str,
) -> Result<u64, PublicationError> {
    connection
        .query_row(
            "SELECT COALESCE(MAX(event_sequence),0)+1 FROM publication_events WHERE workflow_id=?1",
            params![workflow_id],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value as u64)
        .map_err(storage_error)
}

fn required_acknowledgements(result: &CompileResult) -> Vec<String> {
    let mut required: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|item| item.requires_ack)
        .map(|item| item.fingerprint.clone())
        .collect();
    required.sort();
    required
}

fn evidence_digest(evidence: &PublicationEvidence) -> Result<String, PublicationError> {
    #[derive(Serialize)]
    struct Identity<'a> {
        compile_input_digest: &'a str,
        compiler_result_digest: &'a str,
        diagnostic_fingerprints: &'a [String],
        acknowledged_diagnostics: &'a [String],
        compiler_abi: &'a str,
        plan_format: &'a str,
        canonicalization: &'a str,
        digest_algorithm: &'a str,
    }
    digest(&Identity {
        compile_input_digest: &evidence.compile_input_digest,
        compiler_result_digest: &evidence.compiler_result_digest,
        diagnostic_fingerprints: &evidence.diagnostic_fingerprints,
        acknowledged_diagnostics: &evidence.acknowledged_diagnostics,
        compiler_abi: &evidence.compiler_abi,
        plan_format: &evidence.plan_format,
        canonicalization: &evidence.canonicalization,
        digest_algorithm: &evidence.digest_algorithm,
    })
    .map_err(PublicationError::Integrity)
}

fn canonical_text<T: Serialize>(value: &T) -> Result<String, PublicationError> {
    String::from_utf8(canonical_bytes(value).map_err(PublicationError::Integrity)?)
        .map_err(integrity)
}

fn compare_draft(mutable: &WorkflowDraft, published: &WorkflowDraft) -> DraftDifference {
    let mut fields = vec![];
    if mutable.name != published.name {
        fields.push("name".into());
    }
    if serde_json::to_value(&mutable.nodes).ok() != serde_json::to_value(&published.nodes).ok() {
        fields.push("nodes".into());
    }
    if mutable.connections != published.connections {
        fields.push("connections".into());
    }
    if mutable.annotation != published.annotation {
        fields.push("annotation".into());
    }
    if mutable.settings != published.settings {
        fields.push("settings".into());
    }
    if mutable.compatibility_metadata != published.compatibility_metadata {
        fields.push("compatibility_metadata".into());
    }
    DraftDifference {
        state: if fields.is_empty() {
            "matches".into()
        } else {
            "changed".into()
        },
        fields,
    }
}

fn key_id_for(public: &[u8; 32]) -> String {
    format!("ed25519:{:x}", Sha256::digest(public))
}

fn stable_id(kind: &str, workflow_id: &str, request_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"canopy-publication-identity-v1\0");
    hasher.update(kind.as_bytes());
    hasher.update([0]);
    hasher.update(workflow_id.as_bytes());
    hasher.update([0]);
    hasher.update(request_id.as_bytes());
    format!("{kind}-{:x}", hasher.finalize())
}

fn validate_identifier(value: &str, field: &'static str) -> Result<(), PublicationError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        Err(PublicationError::Invalid(field))
    } else {
        Ok(())
    }
}

fn validate_tagged_digest(value: &str, field: &'static str) -> Result<(), PublicationError> {
    let valid = value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase());
    if valid {
        Ok(())
    } else {
        Err(PublicationError::Invalid(field))
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
fn storage<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}
fn storage_error<E: std::fmt::Display>(error: E) -> PublicationError {
    PublicationError::Storage(error.to_string())
}
fn integrity<E: std::fmt::Display>(error: E) -> PublicationError {
    PublicationError::Integrity(error.to_string())
}
fn not_found_or_storage(error: rusqlite::Error) -> PublicationError {
    if matches!(error, rusqlite::Error::QueryReturnedNoRows) {
        PublicationError::NotFound
    } else {
        storage_error(error)
    }
}
