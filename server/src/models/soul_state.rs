use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulState {
    pub user_id: String,
    pub classical_ratio: f64,
    pub warmth_level: f64,
    pub verbosity_level: f64,
    pub proactivity_level: f64,
    pub trust_level: f64,
    pub relationship_stage: String,
    pub total_interactions: i64,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSoulStateRequest {
    #[serde(default)]
    pub classical_ratio: Option<f64>,
    #[serde(default)]
    pub warmth_level: Option<f64>,
    #[serde(default)]
    pub verbosity_level: Option<f64>,
    #[serde(default)]
    pub proactivity_level: Option<f64>,
    #[serde(default)]
    pub trust_level: Option<f64>,
    #[serde(default)]
    pub relationship_stage: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SoulStateResponse {
    pub success: bool,
    pub soul_state: SoulState,
}

pub fn row_to_soul_state(row: &rusqlite::Row) -> Result<SoulState, rusqlite::Error> {
    Ok(SoulState {
        user_id: row.get("user_id")?,
        classical_ratio: row.get("classical_ratio")?,
        warmth_level: row.get("warmth_level")?,
        verbosity_level: row.get("verbosity_level")?,
        proactivity_level: row.get("proactivity_level")?,
        trust_level: row.get("trust_level")?,
        relationship_stage: row.get("relationship_stage")?,
        total_interactions: row.get("total_interactions")?,
        updated_at: row.get("updated_at")?,
    })
}

/// Field validation ranges
pub struct FieldRange {
    pub min: f64,
    pub max: f64,
}

pub fn field_range(name: &str) -> Option<FieldRange> {
    match name {
        "classical_ratio" => Some(FieldRange { min: 0.6, max: 1.0 }),
        "warmth_level" => Some(FieldRange { min: 0.0, max: 1.0 }),
        "verbosity_level" => Some(FieldRange { min: 0.0, max: 1.0 }),
        "proactivity_level" => Some(FieldRange { min: 0.0, max: 0.8 }),
        "trust_level" => Some(FieldRange { min: 0.0, max: 1.0 }),
        _ => None,
    }
}

pub fn clamp_field(name: &str, value: f64) -> f64 {
    if let Some(range) = field_range(name) {
        value.clamp(range.min, range.max)
    } else {
        value
    }
}
