//! Praxis daily-operation journal records (今日经营记录).
//!
//! See SPEC: `C:\Project\ergouPM\specs\praxis\spec.md` §5

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PraxisJournal {
    pub id: i64,
    #[serde(rename = "entryDate")]
    pub entry_date: String,
    #[serde(rename = "rawText", default)]
    pub raw_text: String,
    /// JSON object of quick signal tags; defaults to `{}`.
    #[serde(default)]
    pub tags: JsonValue,
    /// JSON object of the AI structured result; `null` when not yet analyzed.
    #[serde(default)]
    pub structured: JsonValue,
    #[serde(rename = "analyzedAt", default)]
    pub analyzed_at: Option<String>,
    #[serde(rename = "createdAt", default)]
    pub created_at: String,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct CreateJournalRequest {
    #[serde(rename = "entryDate", default)]
    pub entry_date: Option<String>,
    #[serde(rename = "rawText", default)]
    pub raw_text: String,
    #[serde(default)]
    pub tags: Option<JsonValue>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateJournalRequest {
    #[serde(flatten)]
    pub fields: Map<String, JsonValue>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn journal_uses_camel_case_json_fields() {
        let entry = PraxisJournal {
            id: 1,
            entry_date: "2026-06-30".into(),
            raw_text: "今天复盘了一下".into(),
            tags: json!({"energy": "high"}),
            structured: JsonValue::Null,
            analyzed_at: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let j = serde_json::to_string(&entry).unwrap();
        assert!(j.contains("entryDate"));
        assert!(j.contains("rawText"));
        assert!(j.contains("analyzedAt"));
        assert!(!j.contains("entry_date"));
        assert!(!j.contains("raw_text"));
    }
}
