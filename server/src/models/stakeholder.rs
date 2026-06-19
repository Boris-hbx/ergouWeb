//! Stakeholder records for the Work Hub stakeholder module.
//!
//! See SPEC: `C:\Project\ergouPM\specs\stakeholder\spec.md`

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stakeholder {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub team: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub duty: Vec<String>,
    #[serde(default)]
    pub liaison: Vec<String>,
    #[serde(default)]
    pub relation: String,
    #[serde(default)]
    pub method: Vec<String>,
    #[serde(default)]
    pub cadence: String,
    #[serde(default)]
    pub strategy: String,
    #[serde(default)]
    pub notes: String,
    #[serde(rename = "customFields", default)]
    pub custom_fields: Map<String, JsonValue>,
    #[serde(rename = "sortOrder", default)]
    pub sort_order: f64,
    #[serde(rename = "createdAt", default)]
    pub created_at: String,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct CreateStakeholderRequest {
    pub name: String,
    #[serde(default)]
    pub team: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub duty: Vec<String>,
    #[serde(default)]
    pub liaison: Vec<String>,
    #[serde(default)]
    pub relation: String,
    #[serde(default)]
    pub method: Vec<String>,
    #[serde(default)]
    pub cadence: String,
    #[serde(default)]
    pub strategy: String,
    #[serde(default)]
    pub notes: String,
    #[serde(rename = "customFields", default)]
    pub custom_fields: Option<Map<String, JsonValue>>,
    #[serde(rename = "sortOrder")]
    pub sort_order: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateStakeholderRequest {
    pub name: Option<String>,
    pub team: Option<String>,
    pub region: Option<String>,
    pub title: Option<String>,
    pub duty: Option<Vec<String>>,
    pub liaison: Option<Vec<String>>,
    pub relation: Option<String>,
    pub method: Option<Vec<String>>,
    pub cadence: Option<String>,
    pub strategy: Option<String>,
    pub notes: Option<String>,
    #[serde(rename = "customFields")]
    pub custom_fields: Option<Map<String, JsonValue>>,
    #[serde(rename = "sortOrder")]
    pub sort_order: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_fields_uses_camel_case() {
        let s = Stakeholder {
            id: 1,
            name: "Alice".into(),
            team: String::new(),
            region: String::new(),
            title: String::new(),
            duty: vec![],
            liaison: vec![],
            relation: String::new(),
            method: vec![],
            cadence: String::new(),
            strategy: String::new(),
            notes: String::new(),
            custom_fields: Map::new(),
            sort_order: 0.0,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let j = serde_json::to_string(&s).unwrap();
        assert!(j.contains("customFields"));
        assert!(j.contains("sortOrder"));
        assert!(!j.contains("custom_fields"));
        assert!(!j.contains("sort_order"));
    }
}
