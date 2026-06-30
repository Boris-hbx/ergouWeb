//! Praxis contact records.
//!
//! See SPEC: `C:\Project\ergouPM\specs\praxis\spec.md`

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PraxisContact {
    pub id: i64,
    pub name: String,
    pub layer: String,
    #[serde(rename = "lastContactAt", default)]
    pub last_contact_at: Option<String>,
    #[serde(rename = "lastQuality", default)]
    pub last_quality: Option<String>,
    #[serde(default)]
    pub risk: bool,
    #[serde(default)]
    pub note: String,
    #[serde(rename = "cycleOff", default)]
    pub cycle_off: bool,
    #[serde(rename = "sortOrder", default)]
    pub sort_order: f64,
    #[serde(rename = "createdAt", default)]
    pub created_at: String,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct CreatePraxisContactRequest {
    pub name: String,
    #[serde(default = "default_layer")]
    pub layer: String,
    #[serde(rename = "lastContactAt", default)]
    pub last_contact_at: Option<String>,
    #[serde(rename = "lastQuality", default)]
    pub last_quality: Option<String>,
    #[serde(default)]
    pub risk: bool,
    #[serde(default)]
    pub note: String,
    #[serde(rename = "cycleOff", default)]
    pub cycle_off: bool,
    #[serde(rename = "sortOrder")]
    pub sort_order: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdatePraxisContactRequest {
    #[serde(flatten)]
    pub fields: Map<String, JsonValue>,
}

fn default_layer() -> String {
    "normal".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contact_uses_camel_case_json_fields() {
        let contact = PraxisContact {
            id: 1,
            name: "Boris".into(),
            layer: "core".into(),
            last_contact_at: Some("2026-06-29".into()),
            last_quality: Some("deep".into()),
            risk: true,
            note: String::new(),
            cycle_off: false,
            sort_order: 1.0,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let j = serde_json::to_string(&contact).unwrap();
        assert!(j.contains("lastContactAt"));
        assert!(j.contains("lastQuality"));
        assert!(j.contains("cycleOff"));
        assert!(j.contains("sortOrder"));
        assert!(!j.contains("last_contact_at"));
        assert!(!j.contains("cycle_off"));
    }
}
