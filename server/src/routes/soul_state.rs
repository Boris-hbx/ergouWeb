use axum::{extract::State, http::StatusCode, Json};
use serde_json::json;

use crate::auth::UserId;
use crate::models::soul_state::{
    field_range, row_to_soul_state, SoulState, SoulStateResponse, UpdateSoulStateRequest,
};
use crate::state::AppState;

/// Query or lazily create a default soul state for the given user.
pub fn ensure_soul_state(db: &rusqlite::Connection, user_id: &str) -> SoulState {
    // Try to load existing
    if let Ok(state) = db.query_row(
        "SELECT user_id, classical_ratio, warmth_level, verbosity_level, proactivity_level, trust_level, relationship_stage, total_interactions, updated_at FROM soul_states WHERE user_id = ?1",
        [user_id],
        row_to_soul_state,
    ) {
        return state;
    }

    // Lazy-create with defaults
    let now = chrono::Utc::now().to_rfc3339();
    db.execute(
        "INSERT OR IGNORE INTO soul_states (user_id, updated_at) VALUES (?1, ?2)",
        rusqlite::params![user_id, now],
    )
    .ok();

    // Re-read (INSERT OR IGNORE may no-op on race, but row should exist)
    db.query_row(
        "SELECT user_id, classical_ratio, warmth_level, verbosity_level, proactivity_level, trust_level, relationship_stage, total_interactions, updated_at FROM soul_states WHERE user_id = ?1",
        [user_id],
        row_to_soul_state,
    )
    .unwrap_or(SoulState {
        user_id: user_id.to_string(),
        classical_ratio: 0.9,
        warmth_level: 0.3,
        verbosity_level: 0.3,
        proactivity_level: 0.2,
        trust_level: 0.1,
        relationship_stage: "stranger".into(),
        total_interactions: 0,
        updated_at: now,
    })
}

/// GET /api/soul-state
pub async fn get_soul_state(
    State(state): State<AppState>,
    UserId(user_id): UserId,
) -> Result<Json<SoulStateResponse>, StatusCode> {
    let db = state.db.lock();
    let soul = ensure_soul_state(&db, &user_id);
    Ok(Json(SoulStateResponse {
        success: true,
        soul_state: soul,
    }))
}

/// PUT /api/soul-state — partial update with range validation
pub async fn update_soul_state(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    Json(req): Json<UpdateSoulStateRequest>,
) -> Result<Json<SoulStateResponse>, (StatusCode, Json<serde_json::Value>)> {
    let db = state.db.lock();
    let current = ensure_soul_state(&db, &user_id);

    // Validate ranges
    let mut invalid = Vec::new();
    let fields: Vec<(&str, Option<f64>)> = vec![
        ("classical_ratio", req.classical_ratio),
        ("warmth_level", req.warmth_level),
        ("verbosity_level", req.verbosity_level),
        ("proactivity_level", req.proactivity_level),
        ("trust_level", req.trust_level),
    ];

    for (name, value) in &fields {
        if let Some(v) = value {
            if let Some(range) = field_range(name) {
                if *v < range.min || *v > range.max {
                    invalid.push(json!({
                        "field": name,
                        "value": v,
                        "min": range.min,
                        "max": range.max,
                    }));
                }
            }
        }
    }

    if let Some(ref stage) = req.relationship_stage {
        let valid = ["stranger", "acquaintance", "familiar", "close", "intimate"];
        if !valid.contains(&stage.as_str()) {
            invalid.push(json!({
                "field": "relationship_stage",
                "value": stage,
                "allowed": valid,
            }));
        }
    }

    if !invalid.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "message": "字段值超出合法范围",
                "invalid_fields": invalid,
            })),
        ));
    }

    // Apply updates + log changes
    let now = chrono::Utc::now().to_rfc3339();
    let mut changes: Vec<(&str, f64, f64)> = Vec::new();

    let new_classical = req.classical_ratio.unwrap_or(current.classical_ratio);
    let new_warmth = req.warmth_level.unwrap_or(current.warmth_level);
    let new_verbosity = req.verbosity_level.unwrap_or(current.verbosity_level);
    let new_proactivity = req.proactivity_level.unwrap_or(current.proactivity_level);
    let new_trust = req.trust_level.unwrap_or(current.trust_level);
    let new_stage = req
        .relationship_stage
        .as_deref()
        .unwrap_or(&current.relationship_stage);

    if (new_classical - current.classical_ratio).abs() > f64::EPSILON {
        changes.push(("classical_ratio", current.classical_ratio, new_classical));
    }
    if (new_warmth - current.warmth_level).abs() > f64::EPSILON {
        changes.push(("warmth_level", current.warmth_level, new_warmth));
    }
    if (new_verbosity - current.verbosity_level).abs() > f64::EPSILON {
        changes.push(("verbosity_level", current.verbosity_level, new_verbosity));
    }
    if (new_proactivity - current.proactivity_level).abs() > f64::EPSILON {
        changes.push((
            "proactivity_level",
            current.proactivity_level,
            new_proactivity,
        ));
    }
    if (new_trust - current.trust_level).abs() > f64::EPSILON {
        changes.push(("trust_level", current.trust_level, new_trust));
    }

    db.execute(
        "UPDATE soul_states SET classical_ratio=?1, warmth_level=?2, verbosity_level=?3, proactivity_level=?4, trust_level=?5, relationship_stage=?6, updated_at=?7 WHERE user_id=?8",
        rusqlite::params![new_classical, new_warmth, new_verbosity, new_proactivity, new_trust, new_stage, now, user_id],
    )
    .ok();

    // Log each changed field
    for (param, old, new_val) in &changes {
        db.execute(
            "INSERT INTO soul_evolution_log (user_id, parameter, old_value, new_value, trigger_type, created_at) VALUES (?1, ?2, ?3, ?4, 'manual', ?5)",
            rusqlite::params![user_id, param, old, new_val, now],
        )
        .ok();
    }

    // Log relationship stage change if applicable
    if new_stage != current.relationship_stage {
        // Use stage index as numeric value for the log
        let stage_val = |s: &str| -> f64 {
            match s {
                "stranger" => 0.0,
                "acquaintance" => 1.0,
                "familiar" => 2.0,
                "close" => 3.0,
                "intimate" => 4.0,
                _ => 0.0,
            }
        };
        db.execute(
            "INSERT INTO soul_evolution_log (user_id, parameter, old_value, new_value, trigger_type, created_at) VALUES (?1, 'relationship_stage', ?2, ?3, 'manual', ?4)",
            rusqlite::params![user_id, stage_val(&current.relationship_stage), stage_val(new_stage), now],
        )
        .ok();
    }

    let updated = ensure_soul_state(&db, &user_id);
    Ok(Json(SoulStateResponse {
        success: true,
        soul_state: updated,
    }))
}
