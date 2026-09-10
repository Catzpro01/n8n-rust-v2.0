// SPDX-License-Identifier: AGPL-3.0-or-later
use canopy_node_contract::{lock, NodeContractLock};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
#[derive(Clone)]
pub struct DraftService {
    database: PathBuf,
    manual_contract: Value,
    manual_lock: NodeContractLock,
}
#[derive(Debug)]
pub enum DraftError {
    NotFound,
    AlreadyExists,
    Stale { current: u64 },
    DuplicateIdentity,
    InvalidContractLock,
    Invalid(String),
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
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftCommand {
    pub command_id: String,
    pub base_draft_version: u64,
    pub operation: DraftOperation,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DraftOperation {
    AddNode {
        node_instance: NodeInstance,
    },
    ConfigureNode {
        node_instance_id: String,
        configuration: Value,
    },
    SetWorkflowAnnotation {
        annotation: String,
    },
}
#[derive(Serialize, Deserialize, PartialEq)]
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
    pub display_name: &'static str,
    pub description: &'static str,
    pub contract_lock: NodeContractLock,
    pub configuration_schema: Value,
    pub editor_hints: Value,
}
impl DraftService {
    pub fn initialize(state: &Path) -> Result<Self, String> {
        let manual_contract: Value = serde_json::from_str(include_str!(
            "../../../contracts/manual-trigger.v1alpha1.json"
        ))
        .map_err(err)?;
        let manual_lock = lock(&manual_contract)?;
        let s = Self {
            database: state.join("workflow.sqlite3"),
            manual_contract,
            manual_lock,
        };
        let c = s.connect()?;
        c.execute_batch("CREATE TABLE IF NOT EXISTS workflow_drafts(workflow_id TEXT PRIMARY KEY,name TEXT NOT NULL,draft_version INTEGER NOT NULL,document_json TEXT NOT NULL,created_at INTEGER NOT NULL DEFAULT(unixepoch())) STRICT;CREATE TABLE IF NOT EXISTS draft_commands(workflow_id TEXT NOT NULL REFERENCES workflow_drafts(workflow_id),command_id TEXT NOT NULL,accepted_version INTEGER NOT NULL,response_json TEXT NOT NULL,PRIMARY KEY(workflow_id,command_id)) STRICT;").map_err(err)?;
        Ok(s)
    }
    pub fn catalog(&self) -> Catalog {
        let c = &self.manual_contract;
        Catalog {
            contract_api_version: "v1alpha1",
            nodes: vec![CatalogNode {
                display_name: "Manual Trigger",
                description: "Begin a Workflow with one owner-captured invocation.",
                contract_lock: self.manual_lock.clone(),
                configuration_schema: c["configuration"]["schema"].clone(),
                editor_hints: c["configuration"]["editor_hints"].clone(),
            }],
        }
    }
    pub fn contract(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> Result<Value, DraftError> {
        if namespace == self.manual_lock.namespace
            && name == self.manual_lock.name
            && version == self.manual_lock.version
        {
            Ok(self.manual_contract.clone())
        } else {
            Err(DraftError::NotFound)
        }
    }
    pub fn create(&self, r: CreateWorkflow) -> Result<WorkflowDraft, DraftError> {
        identifier(&r.workflow_id)?;
        if r.name.trim().is_empty() {
            return Err(DraftError::Invalid("name".into()));
        }
        objects(&r.settings, &r.compatibility_metadata)?;
        let d = WorkflowDraft {
            workflow_id: r.workflow_id,
            name: r.name.trim().into(),
            draft_version: 0,
            nodes: vec![],
            connections: vec![],
            annotation: r.annotation,
            settings: r.settings,
            compatibility_metadata: r.compatibility_metadata,
        };
        let json = serde_json::to_string(&d).map_err(storage)?;
        let c = self.connect().map_err(DraftError::Storage)?;
        c.execute("INSERT INTO workflow_drafts(workflow_id,name,draft_version,document_json) VALUES(?1,?2,0,?3)",params![d.workflow_id,d.name,json]).map_err(|e|if matches!(e,rusqlite::Error::SqliteFailure(_,Some(ref m)) if m.contains("UNIQUE")){DraftError::AlreadyExists}else{storage(e)})?;
        Ok(d)
    }
    pub fn load(&self, id: &str) -> Result<WorkflowDraft, DraftError> {
        let c = self.connect().map_err(DraftError::Storage)?;
        load(&c, id)
    }
    pub fn command(&self, id: &str, command: DraftCommand) -> Result<CommandAccepted, DraftError> {
        identifier(&command.command_id)?;
        let mut c = self.connect().map_err(DraftError::Storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        if let Some(response) = tx
            .query_row(
                "SELECT response_json FROM draft_commands WHERE workflow_id=?1 AND command_id=?2",
                params![id, command.command_id],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(storage)?
        {
            return serde_json::from_str(&response).map_err(storage);
        }
        let mut draft = load_tx(&tx, id)?;
        if draft.draft_version != command.base_draft_version {
            return Err(DraftError::Stale {
                current: draft.draft_version,
            });
        }
        let (affected, delta) = match command.operation {
            DraftOperation::AddNode { node_instance } => {
                if node_instance.contract_lock != self.manual_lock {
                    return Err(DraftError::InvalidContractLock);
                }
                if draft.nodes.iter().any(|n| n.id == node_instance.id) {
                    return Err(DraftError::DuplicateIdentity);
                }
                identifier(&node_instance.id)?;
                objects(
                    &node_instance.configuration,
                    &node_instance.compatibility_metadata,
                )?;
                let id = node_instance.id.clone();
                draft.nodes.push(node_instance);
                (
                    vec![id.clone()],
                    json!({"kind":"node_added","node_instance_id":id}),
                )
            }
            DraftOperation::ConfigureNode {
                node_instance_id,
                configuration,
            } => {
                objects(&configuration, &json!({}))?;
                manual_configuration(&configuration)?;
                let n = draft
                    .nodes
                    .iter_mut()
                    .find(|n| n.id == node_instance_id)
                    .ok_or(DraftError::NotFound)?;
                n.configuration = configuration;
                (
                    vec![node_instance_id.clone()],
                    json!({"kind":"node_configured","node_instance_id":node_instance_id}),
                )
            }
            DraftOperation::SetWorkflowAnnotation { annotation } => {
                draft.annotation = annotation;
                (vec![id.into()], json!({"kind":"workflow_annotation_set"}))
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
        let document = serde_json::to_string(&draft).map_err(storage)?;
        let response = serde_json::to_string(&accepted).map_err(storage)?;
        tx.execute(
            "UPDATE workflow_drafts SET draft_version=?1,document_json=?2 WHERE workflow_id=?3",
            params![draft.draft_version as i64, document, id],
        )
        .map_err(storage)?;
        tx.execute("INSERT INTO draft_commands(workflow_id,command_id,accepted_version,response_json) VALUES(?1,?2,?3,?4)",params![id,accepted.command_id,accepted.draft_version as i64,response]).map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(accepted)
    }
    fn connect(&self) -> Result<Connection, String> {
        let c = Connection::open(&self.database).map_err(err)?;
        c.busy_timeout(Duration::from_secs(5)).map_err(err)?;
        c.pragma_update(None, "journal_mode", "WAL").map_err(err)?;
        c.pragma_update(None, "synchronous", "FULL").map_err(err)?;
        c.pragma_update(None, "foreign_keys", true).map_err(err)?;
        Ok(c)
    }
}
fn load(c: &Connection, id: &str) -> Result<WorkflowDraft, DraftError> {
    let text = c
        .query_row(
            "SELECT document_json FROM workflow_drafts WHERE workflow_id=?1",
            params![id],
            |r| r.get::<_, String>(0),
        )
        .map_err(|e| {
            if matches!(e, rusqlite::Error::QueryReturnedNoRows) {
                DraftError::NotFound
            } else {
                storage(e)
            }
        })?;
    serde_json::from_str(&text).map_err(storage)
}
fn load_tx(c: &Transaction, id: &str) -> Result<WorkflowDraft, DraftError> {
    load(c, id)
}
fn identifier(v: &str) -> Result<(), DraftError> {
    if !v.is_empty()
        && v.len() <= 128
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        Ok(())
    } else {
        Err(DraftError::Invalid("identity".into()))
    }
}
fn objects(a: &Value, b: &Value) -> Result<(), DraftError> {
    if a.is_object() && b.is_object() {
        Ok(())
    } else {
        Err(DraftError::Invalid("metadata".into()))
    }
}
fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}
fn storage<E: std::fmt::Display>(e: E) -> DraftError {
    DraftError::Storage(e.to_string())
}

fn manual_configuration(value: &Value) -> Result<(), DraftError> {
    let object = value
        .as_object()
        .ok_or_else(|| DraftError::Invalid("configuration".into()))?;
    if object.len() == 1 && object.get("capture_mode").and_then(Value::as_str) == Some("manual") {
        Ok(())
    } else {
        Err(DraftError::Invalid("manual_trigger_configuration".into()))
    }
}
