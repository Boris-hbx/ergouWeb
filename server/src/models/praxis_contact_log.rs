//! Praxis contact interaction logs (关系人交流记录).
//!
//! See SPEC: `C:\Project\ergouPM\specs\praxis\spec.md` §7

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PraxisContactLog {
    pub id: i64,
    #[serde(rename = "contactId")]
    pub contact_id: i64,
    /// Interaction time (ISO).
    pub at: String,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub quality: Option<String>,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub note: String,
    #[serde(rename = "createdAt", default)]
    pub created_at: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct CreateContactLogRequest {
    pub at: String,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub quality: Option<String>,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub note: String,
}

/// PATCH /contacts/{id}/logs/{logId}(T-292 §11.4),部分更新 at/method/quality/content/note。
#[derive(Debug, Deserialize, Default)]
pub struct UpdateContactLogRequest {
    #[serde(flatten)]
    pub fields: Map<String, JsonValue>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_uses_camel_case_json_fields() {
        let log = PraxisContactLog {
            id: 1,
            contact_id: 9,
            at: "2026-06-30".into(),
            method: Some("微信".into()),
            quality: Some("deep".into()),
            content: "聊了合作".into(),
            note: String::new(),
            created_at: String::new(),
        };
        let j = serde_json::to_string(&log).unwrap();
        assert!(j.contains("contactId"));
        assert!(j.contains("createdAt"));
        assert!(!j.contains("contact_id"));
    }
}
