// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::config::ServeConfig;
use canopy_node_contract::{lock, NodeContractLock};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const SNAPSHOT_LIMIT: usize = 4;

#[derive(Clone)]
pub struct DraftService {
    database: PathBuf,
    contracts: Vec<(NodeContractLock, Value)>,
    policy: EditingPolicy,
}
#[derive(Clone)]
struct EditingPolicy {
    lease_ttl_ms: i64,
    takeover_grace_ms: i64,
    snapshot_interval: u64,
    history_limit: usize,
    undo_limit: usize,
}
#[derive(Debug)]
pub enum DraftError {
    NotFound,
    AlreadyExists,
    Stale { current: u64 },
    DuplicateIdentity,
    InvalidContractLock,
    Invalid(String),
    LeaseRequired,
    TakeoverPending,
    TakeoverTooEarly,
    NothingToUndo,
    NothingToRedo,
    ForkResolved,
    Storage(String),
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    pub x: f64,
    pub y: f64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeInstance {
    pub id: String,
    pub name: String,
    pub contract_lock: NodeContractLock,
    pub configuration: Value,
    pub layout: Layout,
    pub annotation: String,
    pub compatibility_metadata: Value,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct WorkflowDraft {
    pub workflow_id: String,
    pub name: String,
    pub draft_version: u64,
    pub nodes: Vec<NodeInstance>,
    pub connections: Vec<Value>,
    pub annotation: String,
    pub settings: Value,
    pub compatibility_metadata: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateWorkflow {
    pub workflow_id: String,
    pub name: String,
    pub annotation: String,
    pub settings: Value,
    pub compatibility_metadata: Value,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftCommand {
    pub editor_session_id: String,
    pub lease_generation: u64,
    pub command_id: String,
    pub base_draft_version: u64,
    pub operation: DraftOperation,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DraftOperation {
    AddNode {
        node_instance: NodeInstance,
    },
    ConfigureNode {
        node_instance_id: String,
        configuration: Value,
    },
    Connect {
        connection: Value,
    },
    SetWorkflowAnnotation {
        annotation: String,
    },
    Undo,
    Redo,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandAccepted {
    pub command_id: String,
    pub draft_version: u64,
    pub delta: Value,
    pub affected_identities: Vec<String>,
    pub diagnostics: Vec<Value>,
}
#[derive(Serialize)]
pub struct Catalog {
    pub contract_api_version: &'static str,
    pub nodes: Vec<CatalogNode>,
}
#[derive(Serialize)]
pub struct CatalogNode {
    pub display_name: String,
    pub description: String,
    pub contract_lock: NodeContractLock,
    pub configuration_schema: Value,
    pub editor_hints: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenEditing {
    pub editor_session_id: String,
    pub label: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionOnly {
    pub editor_session_id: String,
    pub lease_generation: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TakeoverRequest {
    pub editor_session_id: String,
    pub request_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TakeoverResponse {
    pub editor_session_id: String,
    pub lease_generation: u64,
    pub request_id: String,
    pub approve: bool,
}
#[derive(Deserialize)]
pub struct EditingQuery {
    pub editor_session_id: String,
}
#[derive(Clone, Serialize)]
pub struct EditingStatus {
    pub workflow_id: String,
    pub role: &'static str,
    pub lease_generation: u64,
    pub holder: Option<HolderView>,
    pub takeover: Option<TakeoverView>,
    pub server_time: i64,
}
#[derive(Clone, Serialize)]
pub struct HolderView {
    pub label: String,
    pub expires_at: i64,
}
#[derive(Clone, Serialize)]
pub struct TakeoverView {
    pub request_id: String,
    pub state: &'static str,
    pub requester_label: String,
    pub eligible_at: i64,
    pub requested_by_me: bool,
}
#[derive(Serialize)]
pub struct HistoryStatus {
    pub workflow_id: String,
    pub draft_version: u64,
    pub retained_event_count: usize,
    pub snapshot_count: usize,
    pub undo_depth: usize,
    pub redo_depth: usize,
    pub undo_floor_version: u64,
    pub history_limit: usize,
    pub undo_limit: usize,
    pub snapshot_limit: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconcileRequest {
    pub recovery_copy_id: String,
    pub editor_session_id: String,
    pub command: DraftCommand,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplyForkRequest {
    pub editor_session_id: String,
    pub lease_generation: u64,
    pub command_id: String,
    pub base_draft_version: u64,
}
#[derive(Serialize)]
pub struct ReconcileResult {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted: Option<CommandAccepted>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fork: Option<RecoveryForkView>,
}
#[derive(Clone, Serialize)]
pub struct RecoveryForkView {
    pub fork_id: String,
    pub workflow_id: String,
    pub status: String,
    pub original_command_id: String,
    pub pending_operation: Value,
    pub diff: Value,
}
#[derive(Serialize)]
pub struct RecoveryForks {
    pub forks: Vec<RecoveryForkView>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Reversible {
    original_command_id: String,
    applied_version: u64,
    forward: StoredOperation,
    inverse: StoredOperation,
    affected: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum StoredOperation {
    AddNode {
        node_instance: NodeInstance,
    },
    RemoveNode {
        node_instance_id: String,
    },
    ConfigureNode {
        node_instance_id: String,
        configuration: Value,
    },
    Connect {
        connection: Value,
    },
    Disconnect {
        connection: Value,
    },
    SetWorkflowAnnotation {
        annotation: String,
    },
}
#[derive(Clone)]
struct LeaseRow {
    holder_session_id: Option<String>,
    holder_label: Option<String>,
    generation: u64,
    expires_at: i64,
    request_id: Option<String>,
    requester_session_id: Option<String>,
    requester_label: Option<String>,
    eligible_at: Option<i64>,
}

impl DraftService {
    pub fn initialize(config: &ServeConfig) -> Result<Self, String> {
        let contracts = [
            include_str!("../../../contracts/manual-trigger.v1alpha1.json"),
            include_str!("../../../contracts/generate-items.v1alpha1.json"),
        ]
        .into_iter()
        .map(|source| {
            let contract: Value = serde_json::from_str(source).map_err(err)?;
            Ok((lock(&contract)?, contract))
        })
        .collect::<Result<Vec<_>, String>>()?;
        let service = Self {
            database: config.state_dir.join("workflow.sqlite3"),
            contracts,
            policy: EditingPolicy {
                lease_ttl_ms: config.draft_lease_ttl_seconds * 1000,
                takeover_grace_ms: config.draft_takeover_grace_seconds * 1000,
                snapshot_interval: config.draft_snapshot_interval,
                history_limit: config.draft_history_limit,
                undo_limit: config.draft_undo_limit,
            },
        };
        let connection = service.connect()?;
        connection.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS workflow_drafts(
                workflow_id TEXT PRIMARY KEY, name TEXT NOT NULL, draft_version INTEGER NOT NULL,
                document_json TEXT NOT NULL, created_at INTEGER NOT NULL DEFAULT(unixepoch())
            ) STRICT;
            CREATE TABLE IF NOT EXISTS draft_commands(
                workflow_id TEXT NOT NULL REFERENCES workflow_drafts(workflow_id), command_id TEXT NOT NULL,
                accepted_version INTEGER NOT NULL, response_json TEXT NOT NULL,
                PRIMARY KEY(workflow_id,command_id)
            ) STRICT;
            CREATE TABLE IF NOT EXISTS draft_editing_state(
                workflow_id TEXT PRIMARY KEY REFERENCES workflow_drafts(workflow_id) ON DELETE CASCADE,
                undo_json TEXT NOT NULL, redo_json TEXT NOT NULL, undo_floor_version INTEGER NOT NULL
            ) STRICT;
            CREATE TABLE IF NOT EXISTS draft_editor_sessions(
                workflow_id TEXT NOT NULL REFERENCES workflow_drafts(workflow_id) ON DELETE CASCADE,
                editor_session_id TEXT NOT NULL, label TEXT NOT NULL, updated_at INTEGER NOT NULL,
                PRIMARY KEY(workflow_id,editor_session_id)
            ) STRICT;
            CREATE TABLE IF NOT EXISTS draft_leases(
                workflow_id TEXT PRIMARY KEY REFERENCES workflow_drafts(workflow_id) ON DELETE CASCADE,
                holder_session_id TEXT, holder_label TEXT, generation INTEGER NOT NULL,
                expires_at INTEGER NOT NULL, request_id TEXT, requester_session_id TEXT,
                requester_label TEXT, requested_at INTEGER, eligible_at INTEGER
            ) STRICT;
            CREATE TABLE IF NOT EXISTS draft_history_events(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                workflow_id TEXT NOT NULL REFERENCES workflow_drafts(workflow_id) ON DELETE CASCADE,
                command_id TEXT NOT NULL, draft_version INTEGER NOT NULL, action TEXT NOT NULL,
                target_command_id TEXT, delta_json TEXT NOT NULL, created_at INTEGER NOT NULL,
                UNIQUE(workflow_id,command_id)
            ) STRICT;
            CREATE INDEX IF NOT EXISTS draft_history_workflow_id ON draft_history_events(workflow_id,id);
            CREATE TABLE IF NOT EXISTS draft_snapshots(
                workflow_id TEXT NOT NULL REFERENCES workflow_drafts(workflow_id) ON DELETE CASCADE,
                draft_version INTEGER NOT NULL, document_json TEXT NOT NULL, created_at INTEGER NOT NULL,
                PRIMARY KEY(workflow_id,draft_version)
            ) STRICT;
            CREATE TABLE IF NOT EXISTS draft_recovery_forks(
                fork_id TEXT PRIMARY KEY, recovery_copy_id TEXT NOT NULL, workflow_id TEXT NOT NULL
                    REFERENCES workflow_drafts(workflow_id) ON DELETE CASCADE,
                command_json TEXT NOT NULL, diff_json TEXT NOT NULL, status TEXT NOT NULL,
                applied_command_id TEXT, created_at INTEGER NOT NULL,
                UNIQUE(workflow_id,recovery_copy_id)
            ) STRICT;
        "#).map_err(err)?;
        Ok(service)
    }
    pub fn catalog(&self) -> Catalog {
        let nodes = self
            .contracts
            .iter()
            .map(|(contract_lock, contract)| {
                let display = &contract["extensions"]["canopy.workbench/display"];
                CatalogNode {
                    display_name: display["display_name"]
                        .as_str()
                        .unwrap_or("Native Node")
                        .into(),
                    description: display["description"].as_str().unwrap_or("").into(),
                    contract_lock: contract_lock.clone(),
                    configuration_schema: contract["configuration"]["schema"].clone(),
                    editor_hints: contract["configuration"]["editor_hints"].clone(),
                }
            })
            .collect();
        Catalog {
            contract_api_version: "v1alpha1",
            nodes,
        }
    }
    pub fn contract(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> Result<Value, DraftError> {
        self.contracts
            .iter()
            .find(|(contract_lock, _)| {
                namespace == contract_lock.namespace
                    && name == contract_lock.name
                    && version == contract_lock.version
            })
            .map(|(_, contract)| contract.clone())
            .ok_or(DraftError::NotFound)
    }
    pub fn create(&self, request: CreateWorkflow) -> Result<WorkflowDraft, DraftError> {
        identifier(&request.workflow_id)?;
        if request.name.trim().is_empty() {
            return Err(DraftError::Invalid("name".into()));
        }
        objects(&request.settings, &request.compatibility_metadata)?;
        let draft = WorkflowDraft {
            workflow_id: request.workflow_id,
            name: request.name.trim().into(),
            draft_version: 0,
            nodes: vec![],
            connections: vec![],
            annotation: request.annotation,
            settings: request.settings,
            compatibility_metadata: request.compatibility_metadata,
        };
        let document = serde_json::to_string(&draft).map_err(storage)?;
        let mut connection = self.connect().map_err(DraftError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        transaction.execute("INSERT INTO workflow_drafts(workflow_id,name,draft_version,document_json) VALUES(?1,?2,0,?3)", params![draft.workflow_id,draft.name,document]).map_err(|error| if matches!(error,rusqlite::Error::SqliteFailure(_,Some(ref message)) if message.contains("UNIQUE")){DraftError::AlreadyExists}else{storage(error)})?;
        transaction.execute("INSERT INTO draft_editing_state(workflow_id,undo_json,redo_json,undo_floor_version) VALUES(?1,'[]','[]',0)", params![draft.workflow_id]).map_err(storage)?;
        transaction.execute("INSERT INTO draft_snapshots(workflow_id,draft_version,document_json,created_at) VALUES(?1,0,?2,?3)", params![draft.workflow_id,document,now_ms()]).map_err(storage)?;
        transaction.commit().map_err(storage)?;
        Ok(draft)
    }
    pub fn load(&self, workflow_id: &str) -> Result<WorkflowDraft, DraftError> {
        let connection = self.connect().map_err(DraftError::Storage)?;
        load(&connection, workflow_id)
    }

    pub fn open_editing(
        &self,
        workflow_id: &str,
        request: OpenEditing,
    ) -> Result<EditingStatus, DraftError> {
        self.acquire(workflow_id, &request.editor_session_id, &request.label)
    }
    pub fn acquire(
        &self,
        workflow_id: &str,
        session: &str,
        label: &str,
    ) -> Result<EditingStatus, DraftError> {
        editor_identity(session)?;
        editor_label(label)?;
        let mut connection = self.connect().map_err(DraftError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        require_workflow(&transaction, workflow_id)?;
        let now = now_ms();
        register_editor(&transaction, workflow_id, session, label, now)?;
        let current = lease_row(&transaction, workflow_id)?;
        match current {
            None => {
                transaction.execute("INSERT INTO draft_leases(workflow_id,holder_session_id,holder_label,generation,expires_at) VALUES(?1,?2,?3,1,?4)",params![workflow_id,session,label,now+self.policy.lease_ttl_ms]).map_err(storage)?;
            }
            Some(ref lease)
                if lease.holder_session_id.as_deref() == Some(session)
                    && lease.expires_at > now =>
            {
                transaction.execute("UPDATE draft_leases SET holder_label=?2,expires_at=?3 WHERE workflow_id=?1",params![workflow_id,label,now+self.policy.lease_ttl_ms]).map_err(storage)?;
            }
            Some(ref lease) if lease.holder_session_id.is_none() || lease.expires_at <= now => {
                transaction.execute("UPDATE draft_leases SET holder_session_id=?2,holder_label=?3,generation=generation+1,expires_at=?4,request_id=NULL,requester_session_id=NULL,requester_label=NULL,requested_at=NULL,eligible_at=NULL WHERE workflow_id=?1",params![workflow_id,session,label,now+self.policy.lease_ttl_ms]).map_err(storage)?;
            }
            Some(_) => {}
        }
        let status = editing_status_tx(&transaction, workflow_id, session, now)?;
        transaction.commit().map_err(storage)?;
        Ok(status)
    }
    pub fn editing_status(
        &self,
        workflow_id: &str,
        session: &str,
    ) -> Result<EditingStatus, DraftError> {
        editor_identity(session)?;
        let connection = self.connect().map_err(DraftError::Storage)?;
        require_workflow(&connection, workflow_id)?;
        editing_status_conn(&connection, workflow_id, session, now_ms())
    }
    pub fn heartbeat(
        &self,
        workflow_id: &str,
        request: SessionOnly,
    ) -> Result<EditingStatus, DraftError> {
        let session = &request.editor_session_id;
        editor_identity(session)?;
        let mut connection = self.connect().map_err(DraftError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let now = now_ms();
        require_holder_generation(
            &transaction,
            workflow_id,
            session,
            request.lease_generation,
            now,
        )?;
        transaction
            .execute(
                "UPDATE draft_leases SET expires_at=?2 WHERE workflow_id=?1",
                params![workflow_id, now + self.policy.lease_ttl_ms],
            )
            .map_err(storage)?;
        let status = editing_status_tx(&transaction, workflow_id, session, now)?;
        transaction.commit().map_err(storage)?;
        Ok(status)
    }
    pub fn release(
        &self,
        workflow_id: &str,
        request: SessionOnly,
    ) -> Result<EditingStatus, DraftError> {
        let session = &request.editor_session_id;
        editor_identity(session)?;
        let mut connection = self.connect().map_err(DraftError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let now = now_ms();
        require_holder_generation(
            &transaction,
            workflow_id,
            session,
            request.lease_generation,
            now,
        )?;
        transaction.execute("UPDATE draft_leases SET holder_session_id=NULL,holder_label=NULL,expires_at=0,request_id=NULL,requester_session_id=NULL,requester_label=NULL,requested_at=NULL,eligible_at=NULL WHERE workflow_id=?1",params![workflow_id]).map_err(storage)?;
        let status = editing_status_tx(&transaction, workflow_id, session, now)?;
        transaction.commit().map_err(storage)?;
        Ok(status)
    }
    pub fn request_takeover(
        &self,
        workflow_id: &str,
        request: TakeoverRequest,
    ) -> Result<EditingStatus, DraftError> {
        editor_identity(&request.editor_session_id)?;
        identifier(&request.request_id)?;
        let mut connection = self.connect().map_err(DraftError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let now = now_ms();
        let lease = lease_row(&transaction, workflow_id)?.ok_or(DraftError::LeaseRequired)?;
        if lease.expires_at <= now {
            return Err(DraftError::LeaseRequired);
        }
        if lease.holder_session_id.as_deref() == Some(&request.editor_session_id) {
            return editing_status_tx(&transaction, workflow_id, &request.editor_session_id, now);
        }
        if let Some(existing) = lease.request_id {
            if existing != request.request_id
                || lease.requester_session_id.as_deref() != Some(&request.editor_session_id)
            {
                return Err(DraftError::TakeoverPending);
            }
        } else {
            transaction.execute("UPDATE draft_leases SET request_id=?2,requester_session_id=?3,requester_label=?4,requested_at=?5,eligible_at=?6 WHERE workflow_id=?1",params![workflow_id,request.request_id,request.editor_session_id,editor_label_for(&transaction,workflow_id,&request.editor_session_id)?,now,now+self.policy.takeover_grace_ms]).map_err(storage)?;
        }
        let status = editing_status_tx(&transaction, workflow_id, &request.editor_session_id, now)?;
        transaction.commit().map_err(storage)?;
        Ok(status)
    }
    pub fn respond_takeover(
        &self,
        workflow_id: &str,
        request: TakeoverResponse,
    ) -> Result<EditingStatus, DraftError> {
        editor_identity(&request.editor_session_id)?;
        identifier(&request.request_id)?;
        let mut connection = self.connect().map_err(DraftError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let now = now_ms();
        require_holder_generation(
            &transaction,
            workflow_id,
            &request.editor_session_id,
            request.lease_generation,
            now,
        )?;
        let lease = lease_row(&transaction, workflow_id)?.ok_or(DraftError::LeaseRequired)?;
        if lease.request_id.as_deref() != Some(&request.request_id) {
            return Err(DraftError::NotFound);
        }
        if request.approve {
            let requester = lease.requester_session_id.ok_or(DraftError::NotFound)?;
            let label = lease
                .requester_label
                .unwrap_or_else(|| session_label(&requester));
            transfer_lease(
                &transaction,
                workflow_id,
                &requester,
                &label,
                now + self.policy.lease_ttl_ms,
            )?;
        } else {
            clear_takeover(&transaction, workflow_id)?;
        }
        let status = editing_status_tx(&transaction, workflow_id, &request.editor_session_id, now)?;
        transaction.commit().map_err(storage)?;
        Ok(status)
    }
    pub fn claim_takeover(
        &self,
        workflow_id: &str,
        request: TakeoverRequest,
    ) -> Result<EditingStatus, DraftError> {
        editor_identity(&request.editor_session_id)?;
        identifier(&request.request_id)?;
        let mut connection = self.connect().map_err(DraftError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let now = now_ms();
        let lease = lease_row(&transaction, workflow_id)?.ok_or(DraftError::LeaseRequired)?;
        if lease.request_id.as_deref() != Some(&request.request_id)
            || lease.requester_session_id.as_deref() != Some(&request.editor_session_id)
        {
            return Err(DraftError::NotFound);
        }
        if lease.expires_at > now && lease.eligible_at.unwrap_or(i64::MAX) > now {
            return Err(DraftError::TakeoverTooEarly);
        }
        let label = lease
            .requester_label
            .unwrap_or_else(|| session_label(&request.editor_session_id));
        transfer_lease(
            &transaction,
            workflow_id,
            &request.editor_session_id,
            &label,
            now + self.policy.lease_ttl_ms,
        )?;
        let status = editing_status_tx(&transaction, workflow_id, &request.editor_session_id, now)?;
        transaction.commit().map_err(storage)?;
        Ok(status)
    }

    pub fn command(
        &self,
        workflow_id: &str,
        command: DraftCommand,
    ) -> Result<CommandAccepted, DraftError> {
        let mut connection = self.connect().map_err(DraftError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let accepted = self.process_command(&transaction, workflow_id, command)?;
        transaction.commit().map_err(storage)?;
        Ok(accepted)
    }
    fn process_command(
        &self,
        transaction: &Transaction,
        workflow_id: &str,
        command: DraftCommand,
    ) -> Result<CommandAccepted, DraftError> {
        identifier(&command.command_id)?;
        editor_identity(&command.editor_session_id)?;
        let now = now_ms();
        require_holder_generation(
            transaction,
            workflow_id,
            &command.editor_session_id,
            command.lease_generation,
            now,
        )?;
        if let Some(response) = receipt(transaction, workflow_id, &command.command_id)? {
            renew_holder(transaction, workflow_id, now + self.policy.lease_ttl_ms)?;
            return serde_json::from_str(&response).map_err(storage);
        }
        let mut draft = load(transaction, workflow_id)?;
        if draft.draft_version != command.base_draft_version {
            return Err(DraftError::Stale {
                current: draft.draft_version,
            });
        }
        ensure_editing_state(transaction, workflow_id)?;
        let (mut undo, mut redo, mut floor) = load_stacks(transaction, workflow_id)?;
        let (action, target, affected, delta) = match command.operation.clone() {
            DraftOperation::Undo => {
                let reversible = undo.pop().ok_or(DraftError::NothingToUndo)?;
                let affected = apply_stored(&mut draft, &reversible.inverse, self)?;
                let target = reversible.original_command_id.clone();
                redo.push(reversible);
                (
                    "undo",
                    Some(target.clone()),
                    affected,
                    json!({"kind":"undo_applied","target_command_id":target}),
                )
            }
            DraftOperation::Redo => {
                let reversible = redo.pop().ok_or(DraftError::NothingToRedo)?;
                let affected = apply_stored(&mut draft, &reversible.forward, self)?;
                let target = reversible.original_command_id.clone();
                undo.push(reversible);
                (
                    "redo",
                    Some(target.clone()),
                    affected,
                    json!({"kind":"redo_applied","target_command_id":target}),
                )
            }
            operation => {
                let version = draft.draft_version + 1;
                let (mut reversible, affected, delta) =
                    prepare_and_apply(&mut draft, operation, self, &command.command_id, version)?;
                reversible.affected = affected.clone();
                undo.push(reversible);
                redo.clear();
                if undo.len() > self.policy.undo_limit {
                    let removed = undo.remove(0);
                    floor = floor.max(removed.applied_version);
                }
                ("command", None, affected, delta)
            }
        };
        draft.draft_version += 1;
        let accepted = CommandAccepted {
            command_id: command.command_id,
            draft_version: draft.draft_version,
            delta,
            affected_identities: affected,
            diagnostics: vec![],
        };
        persist_command(
            transaction,
            PersistCommand {
                workflow_id,
                draft: &draft,
                accepted: &accepted,
                undo: &undo,
                redo: &redo,
                floor,
                action,
                target: target.as_deref(),
                now,
            },
        )?;
        renew_holder(transaction, workflow_id, now + self.policy.lease_ttl_ms)?;
        if draft.draft_version % self.policy.snapshot_interval == 0 {
            let document = serde_json::to_string(&draft).map_err(storage)?;
            transaction.execute("INSERT OR REPLACE INTO draft_snapshots(workflow_id,draft_version,document_json,created_at) VALUES(?1,?2,?3,?4)",params![workflow_id,draft.draft_version as i64,document,now]).map_err(storage)?;
            compact_snapshots(transaction, workflow_id, SNAPSHOT_LIMIT)?;
        }
        compact_history(transaction, workflow_id, self.policy.history_limit)?;
        Ok(accepted)
    }
    pub fn history(&self, workflow_id: &str) -> Result<HistoryStatus, DraftError> {
        let connection = self.connect().map_err(DraftError::Storage)?;
        let draft = load(&connection, workflow_id)?;
        ensure_editing_state_conn(&connection, workflow_id)?;
        let (undo, redo, floor) = load_stacks_conn(&connection, workflow_id)?;
        let events = count_history_events(&connection, workflow_id)?;
        let snapshots = count_snapshots(&connection, workflow_id)?;
        Ok(HistoryStatus {
            workflow_id: workflow_id.into(),
            draft_version: draft.draft_version,
            retained_event_count: events,
            snapshot_count: snapshots,
            undo_depth: undo.len(),
            redo_depth: redo.len(),
            undo_floor_version: floor,
            history_limit: self.policy.history_limit,
            undo_limit: self.policy.undo_limit,
            snapshot_limit: SNAPSHOT_LIMIT,
        })
    }

    pub fn reconcile(
        &self,
        workflow_id: &str,
        request: ReconcileRequest,
    ) -> Result<ReconcileResult, DraftError> {
        identifier(&request.recovery_copy_id)?;
        editor_identity(&request.editor_session_id)?;
        let same_editor_session = request.command.editor_session_id == request.editor_session_id;
        let mut connection = self.connect().map_err(DraftError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        if let Some(response) = receipt(&transaction, workflow_id, &request.command.command_id)? {
            let accepted = serde_json::from_str(&response).map_err(storage)?;
            transaction.commit().map_err(storage)?;
            return Ok(ReconcileResult {
                status: "already_acknowledged",
                accepted: Some(accepted),
                fork: None,
            });
        }
        let draft = load(&transaction, workflow_id)?;
        let now = now_ms();
        let authority = same_editor_session
            && holder_generation_matches(
                &transaction,
                workflow_id,
                &request.editor_session_id,
                request.command.lease_generation,
                now,
            )?;
        if authority && draft.draft_version == request.command.base_draft_version {
            let accepted = self.process_command(&transaction, workflow_id, request.command)?;
            transaction.commit().map_err(storage)?;
            return Ok(ReconcileResult {
                status: "auto_replayed",
                accepted: Some(accepted),
                fork: None,
            });
        }
        let fork_id = fork_id(workflow_id, &request.recovery_copy_id);
        let diff = json!({"base_draft_version":request.command.base_draft_version,"current_draft_version":draft.draft_version,"authority_changed":!authority,"editor_session_changed":!same_editor_session,"pending_operation":operation_summary(&request.command.operation)});
        let command_json = serde_json::to_string(&request.command).map_err(storage)?;
        let diff_json = serde_json::to_string(&diff).map_err(storage)?;
        transaction.execute("INSERT OR IGNORE INTO draft_recovery_forks(fork_id,recovery_copy_id,workflow_id,command_json,diff_json,status,created_at) VALUES(?1,?2,?3,?4,?5,'open',?6)",params![fork_id,request.recovery_copy_id,workflow_id,command_json,diff_json,now]).map_err(storage)?;
        let fork = load_fork(&transaction, workflow_id, &fork_id)?;
        transaction.commit().map_err(storage)?;
        Ok(ReconcileResult {
            status: "conflict_fork",
            accepted: None,
            fork: Some(fork),
        })
    }
    pub fn list_forks(&self, workflow_id: &str) -> Result<RecoveryForks, DraftError> {
        let connection = self.connect().map_err(DraftError::Storage)?;
        require_workflow(&connection, workflow_id)?;
        let mut statement=connection.prepare("SELECT fork_id,command_json,diff_json,status FROM draft_recovery_forks WHERE workflow_id=?1 ORDER BY created_at,fork_id").map_err(storage)?;
        let rows = statement
            .query_map(params![workflow_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(storage)?;
        let mut forks = vec![];
        for row in rows {
            let (id, command, diff, status) = row.map_err(storage)?;
            forks.push(fork_from_parts(workflow_id, id, command, diff, status)?)
        }
        Ok(RecoveryForks { forks })
    }
    pub fn apply_fork(
        &self,
        workflow_id: &str,
        fork_id: &str,
        request: ApplyForkRequest,
    ) -> Result<CommandAccepted, DraftError> {
        identifier(&request.editor_session_id)?;
        identifier(&request.command_id)?;
        identifier(fork_id)?;
        let mut connection = self.connect().map_err(DraftError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let(command_json,status):(String,String)=transaction.query_row("SELECT command_json,status FROM draft_recovery_forks WHERE workflow_id=?1 AND fork_id=?2",params![workflow_id,fork_id],|row|Ok((row.get(0)?,row.get(1)?))).map_err(|error|if matches!(error,rusqlite::Error::QueryReturnedNoRows){DraftError::NotFound}else{storage(error)})?;
        if status != "open" {
            return Err(DraftError::ForkResolved);
        }
        let pending: DraftCommand = serde_json::from_str(&command_json).map_err(storage)?;
        if matches!(
            pending.operation,
            DraftOperation::Undo | DraftOperation::Redo
        ) {
            return Err(DraftError::Invalid("recovery_operation".into()));
        }
        let command = DraftCommand {
            editor_session_id: request.editor_session_id,
            lease_generation: request.lease_generation,
            command_id: request.command_id,
            base_draft_version: request.base_draft_version,
            operation: pending.operation,
        };
        let accepted = self.process_command(&transaction, workflow_id, command)?;
        transaction.execute("UPDATE draft_recovery_forks SET status='applied',applied_command_id=?3 WHERE workflow_id=?1 AND fork_id=?2",params![workflow_id,fork_id,accepted.command_id]).map_err(storage)?;
        transaction.commit().map_err(storage)?;
        Ok(accepted)
    }
    fn connect(&self) -> Result<Connection, String> {
        let connection = Connection::open(&self.database).map_err(err)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(err)?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(err)?;
        connection
            .pragma_update(None, "synchronous", "FULL")
            .map_err(err)?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(err)?;
        Ok(connection)
    }
}

fn prepare_and_apply(
    draft: &mut WorkflowDraft,
    operation: DraftOperation,
    service: &DraftService,
    command_id: &str,
    version: u64,
) -> Result<(Reversible, Vec<String>, Value), DraftError> {
    let (forward, inverse, affected, delta) = match operation {
        DraftOperation::AddNode { node_instance } => {
            validate_node(&node_instance, service)?;
            if draft.nodes.iter().any(|node| node.id == node_instance.id) {
                return Err(DraftError::DuplicateIdentity);
            }
            let id = node_instance.id.clone();
            let forward = StoredOperation::AddNode {
                node_instance: node_instance.clone(),
            };
            let inverse = StoredOperation::RemoveNode {
                node_instance_id: id.clone(),
            };
            draft.nodes.push(node_instance);
            (
                forward,
                inverse,
                vec![id.clone()],
                json!({"kind":"node_added","node_instance_id":id}),
            )
        }
        DraftOperation::ConfigureNode {
            node_instance_id,
            configuration,
        } => {
            let node = draft
                .nodes
                .iter_mut()
                .find(|node| node.id == node_instance_id)
                .ok_or(DraftError::NotFound)?;
            validate_configuration(&node.contract_lock.name, &configuration)?;
            let previous = node.configuration.clone();
            node.configuration = configuration.clone();
            (
                StoredOperation::ConfigureNode {
                    node_instance_id: node_instance_id.clone(),
                    configuration,
                },
                StoredOperation::ConfigureNode {
                    node_instance_id: node_instance_id.clone(),
                    configuration: previous,
                },
                vec![node_instance_id.clone()],
                json!({"kind":"node_configured","node_instance_id":node_instance_id}),
            )
        }
        DraftOperation::Connect { connection } => {
            let (connection_id, source_id, target_id) = connection_identity(&connection)?;
            if !draft.nodes.iter().any(|node| node.id == source_id)
                || !draft.nodes.iter().any(|node| node.id == target_id)
            {
                return Err(DraftError::NotFound);
            }
            if draft.connections.iter().any(|existing| {
                existing.get("id").and_then(Value::as_str) == Some(connection_id.as_str())
            }) {
                return Err(DraftError::DuplicateIdentity);
            }
            draft.connections.push(connection.clone());
            (
                StoredOperation::Connect {
                    connection: connection.clone(),
                },
                StoredOperation::Disconnect {
                    connection: connection.clone(),
                },
                vec![connection_id.clone(), source_id, target_id],
                json!({"kind":"connection_added","connection_id":connection_id}),
            )
        }
        DraftOperation::SetWorkflowAnnotation { annotation } => {
            let previous = std::mem::replace(&mut draft.annotation, annotation.clone());
            (
                StoredOperation::SetWorkflowAnnotation { annotation },
                StoredOperation::SetWorkflowAnnotation {
                    annotation: previous,
                },
                vec![draft.workflow_id.clone()],
                json!({"kind":"workflow_annotation_set"}),
            )
        }
        DraftOperation::Undo | DraftOperation::Redo => {
            return Err(DraftError::Invalid("history_operation".into()))
        }
    };
    Ok((
        Reversible {
            original_command_id: command_id.into(),
            applied_version: version,
            forward,
            inverse,
            affected: affected.clone(),
        },
        affected,
        delta,
    ))
}
fn apply_stored(
    draft: &mut WorkflowDraft,
    operation: &StoredOperation,
    service: &DraftService,
) -> Result<Vec<String>, DraftError> {
    match operation {
        StoredOperation::AddNode { node_instance } => {
            validate_node(node_instance, service)?;
            if draft.nodes.iter().any(|node| node.id == node_instance.id) {
                return Err(DraftError::DuplicateIdentity);
            }
            draft.nodes.push(node_instance.clone());
            Ok(vec![node_instance.id.clone()])
        }
        StoredOperation::RemoveNode { node_instance_id } => {
            let index = draft
                .nodes
                .iter()
                .position(|node| node.id == *node_instance_id)
                .ok_or(DraftError::NotFound)?;
            draft.nodes.remove(index);
            Ok(vec![node_instance_id.clone()])
        }
        StoredOperation::ConfigureNode {
            node_instance_id,
            configuration,
        } => {
            let node = draft
                .nodes
                .iter_mut()
                .find(|node| node.id == *node_instance_id)
                .ok_or(DraftError::NotFound)?;
            node.configuration = configuration.clone();
            Ok(vec![node_instance_id.clone()])
        }
        StoredOperation::Connect { connection } => {
            let (connection_id, source_id, target_id) = connection_identity(connection)?;
            if draft.connections.iter().any(|existing| {
                existing.get("id").and_then(Value::as_str) == Some(connection_id.as_str())
            }) {
                return Err(DraftError::DuplicateIdentity);
            }
            draft.connections.push(connection.clone());
            Ok(vec![connection_id, source_id, target_id])
        }
        StoredOperation::Disconnect { connection } => {
            let (connection_id, source_id, target_id) = connection_identity(connection)?;
            let index = draft
                .connections
                .iter()
                .position(|existing| {
                    existing.get("id").and_then(Value::as_str) == Some(connection_id.as_str())
                })
                .ok_or(DraftError::NotFound)?;
            draft.connections.remove(index);
            Ok(vec![connection_id, source_id, target_id])
        }
        StoredOperation::SetWorkflowAnnotation { annotation } => {
            draft.annotation = annotation.clone();
            Ok(vec![draft.workflow_id.clone()])
        }
    }
}
struct PersistCommand<'a> {
    workflow_id: &'a str,
    draft: &'a WorkflowDraft,
    accepted: &'a CommandAccepted,
    undo: &'a [Reversible],
    redo: &'a [Reversible],
    floor: u64,
    action: &'a str,
    target: Option<&'a str>,
    now: i64,
}
fn persist_command(
    transaction: &Transaction,
    command: PersistCommand<'_>,
) -> Result<(), DraftError> {
    let PersistCommand {
        workflow_id,
        draft,
        accepted,
        undo,
        redo,
        floor,
        action,
        target,
        now,
    } = command;
    let document = serde_json::to_string(draft).map_err(storage)?;
    let response = serde_json::to_string(accepted).map_err(storage)?;
    let undo = serde_json::to_string(undo).map_err(storage)?;
    let redo = serde_json::to_string(redo).map_err(storage)?;
    let delta = serde_json::to_string(&accepted.delta).map_err(storage)?;
    transaction
        .execute(
            "UPDATE workflow_drafts SET draft_version=?1,document_json=?2 WHERE workflow_id=?3",
            params![draft.draft_version as i64, document, workflow_id],
        )
        .map_err(storage)?;
    transaction.execute("INSERT INTO draft_commands(workflow_id,command_id,accepted_version,response_json) VALUES(?1,?2,?3,?4)",params![workflow_id,accepted.command_id,accepted.draft_version as i64,response]).map_err(storage)?;
    transaction.execute("UPDATE draft_editing_state SET undo_json=?2,redo_json=?3,undo_floor_version=?4 WHERE workflow_id=?1",params![workflow_id,undo,redo,floor as i64]).map_err(storage)?;
    transaction.execute("INSERT INTO draft_history_events(workflow_id,command_id,draft_version,action,target_command_id,delta_json,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![workflow_id,accepted.command_id,accepted.draft_version as i64,action,target,delta,now]).map_err(storage)?;
    Ok(())
}
fn compact_history(
    transaction: &Transaction,
    workflow_id: &str,
    limit: usize,
) -> Result<(), DraftError> {
    transaction.execute("DELETE FROM draft_history_events WHERE workflow_id=?1 AND id NOT IN (SELECT id FROM draft_history_events WHERE workflow_id=?1 ORDER BY id DESC LIMIT ?2)",params![workflow_id,limit as i64]).map_err(storage)?;
    Ok(())
}
fn compact_snapshots(
    transaction: &Transaction,
    workflow_id: &str,
    limit: usize,
) -> Result<(), DraftError> {
    transaction.execute(
        "DELETE FROM draft_snapshots WHERE workflow_id=?1 AND draft_version NOT IN (SELECT draft_version FROM draft_snapshots WHERE workflow_id=?1 ORDER BY draft_version DESC LIMIT ?2)",
        params![workflow_id, limit as i64],
    ).map_err(storage)?;
    Ok(())
}
fn receipt(
    connection: &Connection,
    workflow_id: &str,
    command_id: &str,
) -> Result<Option<String>, DraftError> {
    connection
        .query_row(
            "SELECT response_json FROM draft_commands WHERE workflow_id=?1 AND command_id=?2",
            params![workflow_id, command_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage)
}
fn ensure_editing_state(transaction: &Transaction, workflow_id: &str) -> Result<(), DraftError> {
    transaction.execute("INSERT OR IGNORE INTO draft_editing_state(workflow_id,undo_json,redo_json,undo_floor_version) VALUES(?1,'[]','[]',0)",params![workflow_id]).map_err(storage)?;
    Ok(())
}
fn ensure_editing_state_conn(connection: &Connection, workflow_id: &str) -> Result<(), DraftError> {
    connection.execute("INSERT OR IGNORE INTO draft_editing_state(workflow_id,undo_json,redo_json,undo_floor_version) VALUES(?1,'[]','[]',0)",params![workflow_id]).map_err(storage)?;
    Ok(())
}
fn load_stacks(
    connection: &Connection,
    workflow_id: &str,
) -> Result<(Vec<Reversible>, Vec<Reversible>, u64), DraftError> {
    load_stacks_conn(connection, workflow_id)
}
fn load_stacks_conn(
    connection: &Connection,
    workflow_id: &str,
) -> Result<(Vec<Reversible>, Vec<Reversible>, u64), DraftError> {
    let(undo,redo,floor):(String,String,i64)=connection.query_row("SELECT undo_json,redo_json,undo_floor_version FROM draft_editing_state WHERE workflow_id=?1",params![workflow_id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).map_err(storage)?;
    Ok((
        serde_json::from_str(&undo).map_err(storage)?,
        serde_json::from_str(&redo).map_err(storage)?,
        floor as u64,
    ))
}
fn load(connection: &Connection, workflow_id: &str) -> Result<WorkflowDraft, DraftError> {
    let text = connection
        .query_row(
            "SELECT document_json FROM workflow_drafts WHERE workflow_id=?1",
            params![workflow_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| {
            if matches!(error, rusqlite::Error::QueryReturnedNoRows) {
                DraftError::NotFound
            } else {
                storage(error)
            }
        })?;
    serde_json::from_str(&text).map_err(storage)
}
fn require_workflow(connection: &Connection, workflow_id: &str) -> Result<(), DraftError> {
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM workflow_drafts WHERE workflow_id=?1)",
            params![workflow_id],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if exists == 1 {
        Ok(())
    } else {
        Err(DraftError::NotFound)
    }
}
fn lease_row(connection: &Connection, workflow_id: &str) -> Result<Option<LeaseRow>, DraftError> {
    connection.query_row("SELECT holder_session_id,holder_label,generation,expires_at,request_id,requester_session_id,requester_label,eligible_at FROM draft_leases WHERE workflow_id=?1",params![workflow_id],|row|Ok(LeaseRow{holder_session_id:row.get(0)?,holder_label:row.get(1)?,generation:row.get::<_,i64>(2)? as u64,expires_at:row.get(3)?,request_id:row.get(4)?,requester_session_id:row.get(5)?,requester_label:row.get(6)?,eligible_at:row.get(7)?})).optional().map_err(storage)
}
fn editing_status_conn(
    connection: &Connection,
    workflow_id: &str,
    session: &str,
    now: i64,
) -> Result<EditingStatus, DraftError> {
    let lease = lease_row(connection, workflow_id)?;
    Ok(status_from_lease(workflow_id, session, lease, now))
}
fn editing_status_tx(
    transaction: &Transaction,
    workflow_id: &str,
    session: &str,
    now: i64,
) -> Result<EditingStatus, DraftError> {
    let lease = lease_row(transaction, workflow_id)?;
    Ok(status_from_lease(workflow_id, session, lease, now))
}
fn status_from_lease(
    workflow_id: &str,
    session: &str,
    lease: Option<LeaseRow>,
    now: i64,
) -> EditingStatus {
    let Some(lease) = lease else {
        return EditingStatus {
            workflow_id: workflow_id.into(),
            role: "available",
            lease_generation: 0,
            holder: None,
            takeover: None,
            server_time: now,
        };
    };
    let active = lease.holder_session_id.is_some() && lease.expires_at > now;
    let role = if !active {
        "available"
    } else if lease.holder_session_id.as_deref() == Some(session) {
        "holder"
    } else {
        "read_only"
    };
    let holder = active.then(|| HolderView {
        label: lease.holder_label.unwrap_or_else(|| "Editor tab".into()),
        expires_at: lease.expires_at,
    });
    let takeover = if active {
        lease.request_id.map(|request_id| TakeoverView {
            request_id,
            state: "pending",
            requester_label: lease
                .requester_label
                .unwrap_or_else(|| "Another tab".into()),
            eligible_at: lease.eligible_at.unwrap_or(now),
            requested_by_me: lease.requester_session_id.as_deref() == Some(session),
        })
    } else {
        None
    };
    EditingStatus {
        workflow_id: workflow_id.into(),
        role,
        lease_generation: lease.generation,
        holder,
        takeover,
        server_time: now,
    }
}
fn require_holder_generation(
    connection: &Connection,
    workflow_id: &str,
    session: &str,
    generation: u64,
    now: i64,
) -> Result<(), DraftError> {
    require_workflow(connection, workflow_id)?;
    if holder_generation_matches(connection, workflow_id, session, generation, now)? {
        Ok(())
    } else {
        Err(DraftError::LeaseRequired)
    }
}
fn holder_generation_matches(
    connection: &Connection,
    workflow_id: &str,
    session: &str,
    generation: u64,
    now: i64,
) -> Result<bool, DraftError> {
    Ok(lease_row(connection, workflow_id)?.is_some_and(|lease| {
        lease.holder_session_id.as_deref() == Some(session)
            && lease.generation == generation
            && lease.expires_at > now
    }))
}
fn renew_holder(
    transaction: &Transaction,
    workflow_id: &str,
    expires: i64,
) -> Result<(), DraftError> {
    transaction
        .execute(
            "UPDATE draft_leases SET expires_at=?2 WHERE workflow_id=?1",
            params![workflow_id, expires],
        )
        .map_err(storage)?;
    Ok(())
}
fn transfer_lease(
    transaction: &Transaction,
    workflow_id: &str,
    session: &str,
    label: &str,
    expires: i64,
) -> Result<(), DraftError> {
    transaction.execute("UPDATE draft_leases SET holder_session_id=?2,holder_label=?3,generation=generation+1,expires_at=?4,request_id=NULL,requester_session_id=NULL,requester_label=NULL,requested_at=NULL,eligible_at=NULL WHERE workflow_id=?1",params![workflow_id,session,label,expires]).map_err(storage)?;
    Ok(())
}
fn clear_takeover(transaction: &Transaction, workflow_id: &str) -> Result<(), DraftError> {
    transaction.execute("UPDATE draft_leases SET request_id=NULL,requester_session_id=NULL,requester_label=NULL,requested_at=NULL,eligible_at=NULL WHERE workflow_id=?1",params![workflow_id]).map_err(storage)?;
    Ok(())
}
fn register_editor(
    connection: &Connection,
    workflow_id: &str,
    session: &str,
    label: &str,
    now: i64,
) -> Result<(), DraftError> {
    connection.execute("INSERT INTO draft_editor_sessions(workflow_id,editor_session_id,label,updated_at) VALUES(?1,?2,?3,?4) ON CONFLICT(workflow_id,editor_session_id) DO UPDATE SET label=excluded.label,updated_at=excluded.updated_at",params![workflow_id,session,label,now]).map_err(storage)?;
    Ok(())
}
fn editor_label_for(
    connection: &Connection,
    workflow_id: &str,
    session: &str,
) -> Result<String, DraftError> {
    Ok(connection
        .query_row(
            "SELECT label FROM draft_editor_sessions WHERE workflow_id=?1 AND editor_session_id=?2",
            params![workflow_id, session],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage)?
        .unwrap_or_else(|| session_label(session)))
}
fn validate_node(node: &NodeInstance, service: &DraftService) -> Result<(), DraftError> {
    if !service
        .contracts
        .iter()
        .any(|(contract_lock, _)| node.contract_lock == *contract_lock)
    {
        return Err(DraftError::InvalidContractLock);
    }
    identifier(&node.id)?;
    objects(&node.configuration, &node.compatibility_metadata)?;
    validate_configuration(&node.contract_lock.name, &node.configuration)
}
fn count_history_events(connection: &Connection, workflow_id: &str) -> Result<usize, DraftError> {
    let value: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM draft_history_events WHERE workflow_id=?1",
            params![workflow_id],
            |row| row.get(0),
        )
        .map_err(storage)?;
    Ok(value as usize)
}
fn count_snapshots(connection: &Connection, workflow_id: &str) -> Result<usize, DraftError> {
    let value: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM draft_snapshots WHERE workflow_id=?1",
            params![workflow_id],
            |row| row.get(0),
        )
        .map_err(storage)?;
    Ok(value as usize)
}
fn fork_id(workflow_id: &str, recovery_copy_id: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(b"canopy-recovery-fork-v1\0");
    hash.update(workflow_id.as_bytes());
    hash.update([0]);
    hash.update(recovery_copy_id.as_bytes());
    format!("fork-{:x}", hash.finalize())[..37].into()
}
fn load_fork(
    connection: &Connection,
    workflow_id: &str,
    fork_id: &str,
) -> Result<RecoveryForkView, DraftError> {
    let(id,command,diff,status)=connection.query_row("SELECT fork_id,command_json,diff_json,status FROM draft_recovery_forks WHERE workflow_id=?1 AND fork_id=?2",params![workflow_id,fork_id],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?))).map_err(storage)?;
    fork_from_parts(workflow_id, id, command, diff, status)
}
fn fork_from_parts(
    workflow_id: &str,
    id: String,
    command: String,
    diff: String,
    status: String,
) -> Result<RecoveryForkView, DraftError> {
    let command: DraftCommand = serde_json::from_str(&command).map_err(storage)?;
    Ok(RecoveryForkView {
        fork_id: id,
        workflow_id: workflow_id.into(),
        status,
        original_command_id: command.command_id,
        pending_operation: serde_json::to_value(command.operation).map_err(storage)?,
        diff: serde_json::from_str(&diff).map_err(storage)?,
    })
}
fn operation_summary(operation: &DraftOperation) -> Value {
    match operation {
        DraftOperation::AddNode { node_instance } => {
            json!({"kind":"add_node","affected_identity":node_instance.id})
        }
        DraftOperation::ConfigureNode {
            node_instance_id, ..
        } => json!({"kind":"configure_node","affected_identity":node_instance_id}),
        DraftOperation::Connect { connection } => json!({
            "kind":"connect",
            "affected_identity":connection.get("id").and_then(Value::as_str)
        }),
        DraftOperation::SetWorkflowAnnotation { .. } => json!({"kind":"set_workflow_annotation"}),
        DraftOperation::Undo => json!({"kind":"undo"}),
        DraftOperation::Redo => json!({"kind":"redo"}),
    }
}
fn editor_identity(value: &str) -> Result<(), DraftError> {
    identifier(value)
}
fn editor_label(value: &str) -> Result<(), DraftError> {
    if !value.trim().is_empty() && value.len() <= 80 {
        Ok(())
    } else {
        Err(DraftError::Invalid("editor_label".into()))
    }
}
fn session_label(session: &str) -> String {
    format!("Editor {}", &session[session.len().saturating_sub(6)..])
}
fn identifier(value: &str) -> Result<(), DraftError> {
    if !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
    {
        Ok(())
    } else {
        Err(DraftError::Invalid("identity".into()))
    }
}
fn objects(first: &Value, second: &Value) -> Result<(), DraftError> {
    if first.is_object() && second.is_object() {
        Ok(())
    } else {
        Err(DraftError::Invalid("metadata".into()))
    }
}
fn validate_configuration(name: &str, value: &Value) -> Result<(), DraftError> {
    let object = value
        .as_object()
        .ok_or_else(|| DraftError::Invalid("configuration".into()))?;
    match name {
        "manual-trigger"
            if object.len() == 1
                && object.get("capture_mode").and_then(Value::as_str) == Some("manual") =>
        {
            Ok(())
        }
        "generate-items" => {
            const FIELDS: &[&str] = &["count", "start", "step", "data", "storage_mode"];
            if object.len() != FIELDS.len()
                || FIELDS.iter().any(|field| !object.contains_key(*field))
            {
                return Err(DraftError::Invalid("generate_items_configuration".into()));
            }
            let count = object.get("count").and_then(Value::as_u64);
            let start = object.get("start").and_then(Value::as_i64);
            let step = object.get("step").and_then(Value::as_i64);
            let mode = object.get("storage_mode").and_then(Value::as_str);
            if count.is_some_and(|count| count <= 50_000)
                && start.is_some()
                && step.is_some()
                && matches!(mode, Some("auto" | "artifact"))
            {
                reject_generate_sensitive_keys(
                    object.get("data").expect("required data field was checked"),
                )?;
                Ok(())
            } else {
                Err(DraftError::Invalid("generate_items_configuration".into()))
            }
        }
        _ => Err(DraftError::Invalid("node_configuration".into())),
    }
}

fn reject_generate_sensitive_keys(value: &Value) -> Result<(), DraftError> {
    const SENSITIVE: &[&str] = &[
        "authorization",
        "cookie",
        "password",
        "passwd",
        "secret",
        "token",
        "api_key",
        "apikey",
        "private_key",
        "access_token",
        "refresh_token",
        "id_token",
        "auth_token",
        "bearer_token",
        "session_token",
        "client_secret",
        "credential",
        "credentials",
        "secret_key",
        "signing_key",
        "ssh_key",
        "secret_lease",
        "lease_token",
    ];
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                let normalized = key.to_ascii_lowercase().replace('-', "_");
                let sensitive_suffix = [
                    "_password",
                    "_passwd",
                    "_secret",
                    "_token",
                    "_credential",
                    "_credentials",
                    "_private_key",
                    "_api_key",
                ]
                .iter()
                .any(|suffix| normalized.ends_with(suffix));
                if SENSITIVE.contains(&normalized.as_str()) || sensitive_suffix {
                    return Err(DraftError::Invalid("generate_items_sensitive_field".into()));
                }
                reject_generate_sensitive_keys(nested)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                reject_generate_sensitive_keys(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn connection_identity(value: &Value) -> Result<(String, String, String), DraftError> {
    let object = value
        .as_object()
        .ok_or_else(|| DraftError::Invalid("connection".into()))?;
    if object.len() != 3 {
        return Err(DraftError::Invalid("connection".into()));
    }
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| DraftError::Invalid("connection".into()))?;
    let source = object
        .get("source")
        .and_then(Value::as_object)
        .ok_or_else(|| DraftError::Invalid("connection".into()))?;
    let target = object
        .get("target")
        .and_then(Value::as_object)
        .ok_or_else(|| DraftError::Invalid("connection".into()))?;
    if source.len() != 2 || target.len() != 2 {
        return Err(DraftError::Invalid("connection".into()));
    }
    let source_id = source
        .get("node_id")
        .and_then(Value::as_str)
        .ok_or_else(|| DraftError::Invalid("connection".into()))?;
    let target_id = target
        .get("node_id")
        .and_then(Value::as_str)
        .ok_or_else(|| DraftError::Invalid("connection".into()))?;
    let source_port = source
        .get("port_id")
        .and_then(Value::as_str)
        .ok_or_else(|| DraftError::Invalid("connection".into()))?;
    let target_port = target
        .get("port_id")
        .and_then(Value::as_str)
        .ok_or_else(|| DraftError::Invalid("connection".into()))?;
    for identity in [id, source_id, target_id, source_port, target_port] {
        identifier(identity)?;
    }
    Ok((id.into(), source_id.into(), target_id.into()))
}
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
fn err<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}
fn storage<E: std::fmt::Display>(error: E) -> DraftError {
    DraftError::Storage(error.to_string())
}
