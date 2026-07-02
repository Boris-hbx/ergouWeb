//! Praxis perspectives (视角) — 每视角一套完全隔离的关系人数据集.
//!
//! See SPEC: `C:\Project\ergouPM\specs\praxis\spec.md` §11.1

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PraxisPerspective {
    pub id: i64,
    pub name: String,
    #[serde(rename = "sortOrder", default)]
    pub sort_order: f64,
    #[serde(rename = "createdAt", default)]
    pub created_at: String,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct CreatePerspectiveRequest {
    pub name: String,
    #[serde(rename = "sortOrder")]
    pub sort_order: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdatePerspectiveRequest {
    #[serde(flatten)]
    pub fields: Map<String, JsonValue>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perspective_uses_camel_case_json_fields() {
        let p = PraxisPerspective {
            id: 1,
            name: "工作".into(),
            sort_order: 1.0,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let j = serde_json::to_string(&p).unwrap();
        assert!(j.contains("sortOrder"));
        assert!(j.contains("createdAt"));
        assert!(!j.contains("sort_order"));
    }
}
