// SPDX-License-Identifier: Apache-2.0
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NodeContractLock {
    pub api_version: String,
    pub namespace: String,
    pub name: String,
    pub version: String,
    pub digest: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContractError {
    #[error("contract must be an object")]
    NotObject,
    #[error("missing normative field: {0}")]
    MissingField(String),
    #[error("unknown normative field: {0}")]
    UnknownField(String),
    #[error("invalid normative field: {0}")]
    InvalidField(String),
    #[error("extension key must be namespaced: {0}")]
    ExtensionNamespace(String),
}

pub const NORMATIVE_FIELDS: [&str; 9] = [
    "identity",
    "configuration",
    "ports",
    "activation",
    "effects",
    "capabilities",
    "resources",
    "outcomes",
    "compatibility",
];

pub fn validate(value: &Value) -> Result<(), ContractError> {
    let object = value.as_object().ok_or(ContractError::NotObject)?;
    for key in object.keys() {
        if !NORMATIVE_FIELDS.contains(&key.as_str()) && key != "extensions" {
            return Err(ContractError::UnknownField(key.clone()));
        }
    }
    for field in NORMATIVE_FIELDS {
        if !object.contains_key(field) {
            return Err(ContractError::MissingField(field.into()));
        }
    }
    strict_object(
        value,
        "identity",
        &[
            "api_version",
            "license",
            "name",
            "namespace",
            "publisher",
            "version",
        ],
    )?;
    strict_object(
        value,
        "configuration",
        &["defaults", "editor_hints", "schema"],
    )?;
    strict_object(value, "ports", &["inputs", "outputs"])?;
    strict_array_objects(
        value,
        "ports",
        "inputs",
        &["cardinality", "id", "multiplicity", "required", "schema"],
    )?;
    strict_array_objects(
        value,
        "ports",
        "outputs",
        &["cardinality", "id", "multiplicity", "required", "schema"],
    )?;
    strict_object(value, "activation", &["bounded_batch", "ordering", "shape"])?;
    strict_object(
        value,
        "effects",
        &[
            "approval_required",
            "class",
            "deterministic",
            "idempotent",
            "reconciliation",
            "retry",
        ],
    )?;
    if !value["capabilities"].is_array() {
        return Err(ContractError::InvalidField("capabilities".into()));
    }
    strict_object(value, "resources", &["default", "hard"])?;
    strict_child_object(value, "resources", "default", &BUDGET_FIELDS)?;
    strict_child_object(value, "resources", "hard", &BUDGET_FIELDS)?;
    strict_object(value, "outcomes", &["allowed", "error_namespace"])?;
    strict_object(
        value,
        "compatibility",
        &["aliases", "profile", "safe_opaque_fields"],
    )?;
    if let Some(extensions) = object.get("extensions") {
        let extensions = extensions
            .as_object()
            .ok_or_else(|| ContractError::InvalidField("extensions".into()))?;
        for key in extensions.keys() {
            if !namespaced_extension(key) {
                return Err(ContractError::ExtensionNamespace(key.clone()));
            }
        }
    }
    Ok(())
}

const BUDGET_FIELDS: [&str; 9] = [
    "artifact_bytes",
    "concurrency",
    "cpu_millis",
    "input_bytes",
    "input_count",
    "memory_bytes",
    "output_bytes",
    "output_count",
    "wall_millis",
];

fn strict_object(value: &Value, field: &str, allowed: &[&str]) -> Result<(), ContractError> {
    let object = value[field]
        .as_object()
        .ok_or_else(|| ContractError::InvalidField(field.into()))?;
    reject_unknown(object, field, allowed)
}

fn strict_child_object(
    value: &Value,
    parent: &str,
    field: &str,
    allowed: &[&str],
) -> Result<(), ContractError> {
    let path = format!("{parent}.{field}");
    let object = value[parent][field]
        .as_object()
        .ok_or_else(|| ContractError::InvalidField(path.clone()))?;
    reject_unknown(object, &path, allowed)
}

fn strict_array_objects(
    value: &Value,
    parent: &str,
    field: &str,
    allowed: &[&str],
) -> Result<(), ContractError> {
    let path = format!("{parent}.{field}");
    let array = value[parent][field]
        .as_array()
        .ok_or_else(|| ContractError::InvalidField(path.clone()))?;
    for (index, item) in array.iter().enumerate() {
        let object = item
            .as_object()
            .ok_or_else(|| ContractError::InvalidField(format!("{path}[{index}]")))?;
        reject_unknown(object, &format!("{path}[{index}]"), allowed)?;
    }
    Ok(())
}

fn reject_unknown(
    object: &Map<String, Value>,
    path: &str,
    allowed: &[&str],
) -> Result<(), ContractError> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(ContractError::UnknownField(format!("{path}.{key}")));
        }
    }
    for key in allowed {
        if !object.contains_key(*key) {
            return Err(ContractError::MissingField(format!("{path}.{key}")));
        }
    }
    Ok(())
}

fn namespaced_extension(key: &str) -> bool {
    key.char_indices()
        .find(|(_, character)| *character == '/' || *character == ':')
        .is_some_and(|(index, _)| index > 0 && index + 1 < key.len())
}

pub fn canonical_json(value: &Value) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(&sorted(value))
}

pub fn digest(value: &Value) -> Result<String, serde_json::Error> {
    Ok(format!(
        "sha256:{:x}",
        Sha256::digest(canonical_json(value)?)
    ))
}

pub fn lock(value: &Value) -> Result<NodeContractLock, String> {
    validate(value).map_err(|error| error.to_string())?;
    let identity = value["identity"].as_object().ok_or("missing identity")?;
    Ok(NodeContractLock {
        api_version: text(identity, "api_version")?,
        namespace: text(identity, "namespace")?,
        name: text(identity, "name")?,
        version: text(identity, "version")?,
        digest: digest(value).map_err(|error| error.to_string())?,
    })
}

fn text(object: &Map<String, Value>, key: &str) -> Result<String, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("missing identity.{key}"))
}

fn sorted(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| (key.clone(), sorted(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(array) => Value::Array(array.iter().map(sorted).collect()),
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn published() -> Value {
        serde_json::from_str(include_str!(
            "../../../contracts/manual-trigger.v1alpha1.json"
        ))
        .unwrap()
    }

    #[test]
    fn canonical_digest_ignores_object_key_order() {
        let a = json!({"z": {"b": 2, "a": 1}, "a": [2, 1]});
        let b = json!({"a": [2, 1], "z": {"a": 1, "b": 2}});
        assert_eq!(canonical_json(&a).unwrap(), canonical_json(&b).unwrap());
        assert_eq!(digest(&a).unwrap(), digest(&b).unwrap());
    }

    #[test]
    fn normative_fields_are_required_and_strict() {
        let mut unknown = published();
        unknown["identity"]["mystery"] = json!(true);
        assert!(
            matches!(validate(&unknown), Err(ContractError::UnknownField(path)) if path == "identity.mystery")
        );
        let mut missing = published();
        missing.as_object_mut().unwrap().remove("effects");
        assert_eq!(
            validate(&missing),
            Err(ContractError::MissingField("effects".into()))
        );
    }

    #[test]
    fn extension_requires_namespace_and_round_trips() {
        let mut contract = published();
        contract["extensions"]["plain"] = json!(1);
        assert!(matches!(
            validate(&contract),
            Err(ContractError::ExtensionNamespace(_))
        ));
        let original = published();
        let round_trip: Value =
            serde_json::from_slice(&canonical_json(&original).unwrap()).unwrap();
        assert_eq!(round_trip["extensions"], original["extensions"]);
    }

    #[test]
    fn published_manual_trigger_matches_its_independent_fixture() {
        let contract = published();
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../sdk/node-contract/v1alpha1/fixtures/manual-trigger.json"
        ))
        .unwrap();
        validate(&contract).unwrap();
        let exact_lock = lock(&contract).unwrap();
        assert_eq!(
            serde_json::to_value(exact_lock).unwrap(),
            fixture["expected_lock"]
        );
    }
}
