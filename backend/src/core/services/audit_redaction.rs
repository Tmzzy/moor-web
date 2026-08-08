// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use serde_json::Value;

const MAX_DEPTH: usize = 8;
const MAX_STRING_LENGTH: usize = 200;
const MAX_ARRAY_LENGTH: usize = 50;
const REDACTED: &str = "[REDACTED]";

pub fn redact_for_audit(value: &Value) -> Value {
    redact_value(value, 0)
}

fn redact_value(value: &Value, depth: usize) -> Value {
    if depth >= MAX_DEPTH {
        return Value::String("[Max depth reached]".to_string());
    }
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    if is_sensitive_key(key) {
                        (key.clone(), Value::String(REDACTED.to_string()))
                    } else {
                        (key.clone(), redact_value(value, depth + 1))
                    }
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .take(MAX_ARRAY_LENGTH)
                .map(|item| redact_value(item, depth + 1))
                .collect(),
        ),
        Value::String(s) => Value::String(truncate_string(s)),
        _ => value.clone(),
    }
}

fn truncate_string(value: &str) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(MAX_STRING_LENGTH).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        value.to_string()
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "token",
        "password",
        "secret",
        "key",
        "cookie",
        "authorization",
    ]
    .iter()
    .any(|needle| key.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_sensitive_keys_recursively() {
        let redacted = redact_for_audit(&json!({
            "token": "secret-token",
            "nested": { "Authorization": "Bearer secret", "safe": "visible" },
            "text": "x".repeat(260),
            "items": (0..60).collect::<Vec<_>>()
        }));

        assert_eq!(redacted["token"], json!("[REDACTED]"));
        assert_eq!(redacted["nested"]["Authorization"], json!("[REDACTED]"));
        assert_eq!(redacted["nested"]["safe"], json!("visible"));
        assert_eq!(redacted["text"].as_str().unwrap().len(), 203);
        assert!(redacted["text"].as_str().unwrap().ends_with("..."));
        assert_eq!(redacted["items"].as_array().unwrap().len(), 50);
    }
}
