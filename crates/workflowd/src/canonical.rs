// SPDX-License-Identifier: AGPL-3.0-or-later
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const CANONICALIZATION: &str = "jcs-rfc8785";
pub const DIGEST_ALGORITHM: &str = "sha256";

pub fn bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_jcs::to_vec(value).map_err(|error| format!("JCS serialization failed: {error}"))
}

pub fn digest<T: Serialize>(value: &T) -> Result<String, String> {
    let canonical = bytes(value)?;
    Ok(format!("sha256:{:x}", Sha256::digest(canonical)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    #[allow(clippy::excessive_precision)]
    fn matches_rfc_8785_serialization_sample() {
        let value = json!({
            "numbers": [333333333.33333329_f64, 1E30_f64, 4.50_f64, 2e-3_f64, 0.000000000000000000000000001_f64],
            "string": "€$\u{000f}\nA'B\"\\\"/",
            "literals": [null, true, false]
        });
        let expected = concat!(
            "{\"literals\":[null,true,false],",
            "\"numbers\":[333333333.3333333,1e+30,4.5,0.002,1e-27],",
            "\"string\":\"€$\\u000f\\nA'B\\\"\\\\\\\"/\"}"
        );
        assert_eq!(String::from_utf8(bytes(&value).unwrap()).unwrap(), expected);
    }

    #[test]
    fn digest_is_tagged() {
        assert_eq!(
            digest(&json!({"a": 1, "b": 2})).unwrap(),
            "sha256:43258cff783fe7036d8a43033f830adfc60ec037382473548ac742b888292777"
        );
    }
}
