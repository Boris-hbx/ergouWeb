//! `/api/praxis/contacts` - Praxis relation contacts.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde_json::{json, Map, Value as JsonValue};
use tracing::error;

use crate::auth::AdminUserId;
use crate::models::praxis_contact::{
    CreatePraxisContactRequest, PraxisContact, UpdatePraxisContactRequest,
};
use crate::state::AppState;

const SELECT_COLS: &str = "id, name, layer, last_contact_at, last_quality, risk, note, cycle_off, sort_order, created_at, updated_at";

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn db_error(ctx: &str, e: rusqlite::Error) -> (StatusCode, Json<JsonValue>) {
    error!(target: "praxis_contacts", "{} db error: {}", ctx, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "error": "内部错误" })),
    )
}

fn validate_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("name must not be empty".into());
    }
    if trimmed.chars().count() > 60 {
        return Err("name must be 60 characters or fewer".into());
    }
    Ok(trimmed.to_string())
}

fn validate_layer(layer: &str) -> Result<String, String> {
    match layer {
        "core" | "important" | "normal" => Ok(layer.to_string()),
        _ => Err("layer must be core, important, or normal".into()),
    }
}

fn validate_quality(quality: Option<String>) -> Result<Option<String>, String> {
    match quality.as_deref() {
        None | Some("") => Ok(None),
        Some("shallow" | "effective" | "deep") => Ok(quality),
        _ => Err("lastQuality must be shallow, effective, deep, or null".into()),
    }
}

fn field_string(fields: &Map<String, JsonValue>, key: &str) -> Option<String> {
    fields
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn field_nullable_string(
    fields: &Map<String, JsonValue>,
    key: &str,
) -> Result<Option<Option<String>>, String> {
    match fields.get(key) {
        None => Ok(None),
        Some(JsonValue::Null) => Ok(Some(None)),
        Some(JsonValue::String(s)) if s.is_empty() => Ok(Some(None)),
        Some(JsonValue::String(s)) => Ok(Some(Some(s.clone()))),
        Some(_) => Err(format!("{key} must be a string or null")),
    }
}

fn field_bool(fields: &Map<String, JsonValue>, key: &str) -> Option<bool> {
    fields.get(key).and_then(|v| v.as_bool())
}

fn field_f64(fields: &Map<String, JsonValue>, key: &str) -> Option<f64> {
    fields.get(key).and_then(|v| v.as_f64())
}

fn row_to_contact(row: &rusqlite::Row) -> rusqlite::Result<PraxisContact> {
    let risk: i64 = row.get(5)?;
    let cycle_off: i64 = row.get(7)?;
    Ok(PraxisContact {
        id: row.get(0)?,
        name: row.get(1)?,
        layer: row.get(2)?,
        last_contact_at: row.get(3)?,
        last_quality: row.get(4)?,
        risk: risk != 0,
        note: row.get(6)?,
        cycle_off: cycle_off != 0,
        sort_order: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn load_contact(db: &Connection, user_id: &str, id: i64) -> Option<PraxisContact> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM praxis_contacts WHERE id = ?1 AND user_id = ?2 AND deleted = 0"
    );
    db.query_row(&sql, params![id, user_id], row_to_contact)
        .ok()
}

pub fn list_contacts_impl(db: &Connection, user_id: &str) -> Result<Vec<PraxisContact>, String> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM praxis_contacts
         WHERE user_id = ?1 AND deleted = 0
         ORDER BY sort_order ASC, updated_at DESC, id ASC"
    );
    let mut stmt = db.prepare(&sql).map_err(|e| format!("prepare: {e}"))?;
    let rows = stmt
        .query_map(params![user_id], row_to_contact)
        .map_err(|e| format!("query: {e}"))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn create_contact_impl(
    db: &Connection,
    user_id: &str,
    req: &CreatePraxisContactRequest,
) -> Result<PraxisContact, String> {
    let name = validate_name(&req.name)?;
    let layer = validate_layer(&req.layer)?;
    let last_quality = validate_quality(req.last_quality.clone())?;
    let now = now_rfc3339();
    let sort_order = req.sort_order.unwrap_or_else(|| {
        db.query_row(
            "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM praxis_contacts WHERE user_id = ?1 AND deleted = 0",
            params![user_id],
            |r| r.get(0),
        )
        .unwrap_or(1.0)
    });

    db.execute(
        "INSERT INTO praxis_contacts
           (user_id, name, layer, last_contact_at, last_quality, risk, note, cycle_off, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        params![
            user_id,
            &name,
            &layer,
            &req.last_contact_at,
            &last_quality,
            if req.risk { 1 } else { 0 },
            &req.note,
            if req.cycle_off { 1 } else { 0 },
            sort_order,
            &now,
        ],
    )
    .map_err(|e| format!("insert: {e}"))?;

    let id = db.last_insert_rowid();
    load_contact(db, user_id, id).ok_or_else(|| "reload failed".into())
}

pub fn update_contact_impl(
    db: &Connection,
    user_id: &str,
    id: i64,
    patch: UpdatePraxisContactRequest,
) -> Result<Option<PraxisContact>, String> {
    let Some(mut current) = load_contact(db, user_id, id) else {
        return Ok(None);
    };

    if let Some(name) = field_string(&patch.fields, "name") {
        current.name = validate_name(&name)?;
    }
    if let Some(layer) = field_string(&patch.fields, "layer") {
        current.layer = validate_layer(&layer)?;
    }
    if let Some(last_contact_at) = field_nullable_string(&patch.fields, "lastContactAt")? {
        current.last_contact_at = last_contact_at;
    }
    if let Some(last_quality) = field_nullable_string(&patch.fields, "lastQuality")? {
        current.last_quality = validate_quality(last_quality)?;
    }
    if let Some(risk) = field_bool(&patch.fields, "risk") {
        current.risk = risk;
    }
    if let Some(note) = field_string(&patch.fields, "note") {
        current.note = note;
    }
    if let Some(cycle_off) = field_bool(&patch.fields, "cycleOff") {
        current.cycle_off = cycle_off;
    }
    if let Some(sort_order) = field_f64(&patch.fields, "sortOrder") {
        current.sort_order = sort_order;
    }

    let now = now_rfc3339();
    db.execute(
        "UPDATE praxis_contacts
         SET name = ?1, layer = ?2, last_contact_at = ?3, last_quality = ?4,
             risk = ?5, note = ?6, cycle_off = ?7, sort_order = ?8, updated_at = ?9
         WHERE id = ?10 AND user_id = ?11 AND deleted = 0",
        params![
            &current.name,
            &current.layer,
            &current.last_contact_at,
            &current.last_quality,
            if current.risk { 1 } else { 0 },
            &current.note,
            if current.cycle_off { 1 } else { 0 },
            current.sort_order,
            &now,
            id,
            user_id,
        ],
    )
    .map_err(|e| format!("update: {e}"))?;

    Ok(load_contact(db, user_id, id))
}

pub async fn list_contacts(
    State(state): State<AppState>,
    admin: AdminUserId,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match list_contacts_impl(&db, &admin.0) {
        Ok(items) => (
            StatusCode::OK,
            Json(json!({ "success": true, "items": items, "count": items.len() })),
        ),
        Err(e) => {
            error!(target: "praxis_contacts", "list: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "内部错误" })),
            )
        }
    }
}

pub async fn create_contact(
    State(state): State<AppState>,
    admin: AdminUserId,
    Json(req): Json<CreatePraxisContactRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match create_contact_impl(&db, &admin.0, &req) {
        Ok(item) => (
            StatusCode::OK,
            Json(json!({ "success": true, "item": item })),
        ),
        Err(e) if e.contains("must") => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": e })),
        ),
        Err(e) => {
            error!(target: "praxis_contacts", "create: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "内部错误" })),
            )
        }
    }
}

pub async fn update_contact(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path(id): Path<i64>,
    Json(patch): Json<UpdatePraxisContactRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match update_contact_impl(&db, &admin.0, id, patch) {
        Ok(Some(item)) => (
            StatusCode::OK,
            Json(json!({ "success": true, "item": item })),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到关系人" })),
        ),
        Err(e) if e.contains("must") => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": e })),
        ),
        Err(e) => {
            error!(target: "praxis_contacts", "update: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "内部错误" })),
            )
        }
    }
}

pub async fn delete_contact(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let now = now_rfc3339();
    match db.execute(
        "UPDATE praxis_contacts SET deleted = 1, updated_at = ?1
         WHERE id = ?2 AND user_id = ?3 AND deleted = 0",
        params![&now, id, &admin.0],
    ) {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到关系人" })),
        ),
        Ok(_) => (StatusCode::OK, Json(json!({ "success": true }))),
        Err(e) => db_error("delete", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{auth_cookie, create_admin_user, create_test_user, test_state};
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tower::ServiceExt;

    async fn body_json(resp: axum::response::Response) -> JsonValue {
        let status = resp.status();
        let body = to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        serde_json::from_slice(&body).unwrap_or_else(|e| {
            panic!(
                "parse fail status={} body={} err={}",
                status,
                String::from_utf8_lossy(&body),
                e
            )
        })
    }

    #[tokio::test]
    async fn praxis_contacts_require_admin_and_crud_per_user() {
        let state = test_state();
        let (_user_id, user_token) = create_test_user(&state, "px-user", "Pa55word1");
        let (_admin_id, admin_token) = create_admin_user(&state, "px-admin", "Pa55word1");
        let (_other_id, other_token) = create_admin_user(&state, "px-other", "Pa55word1");
        let app = crate::build_app(state);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/praxis/contacts")
                    .header("Cookie", auth_cookie(&user_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/praxis/contacts")
                    .header("Cookie", auth_cookie(&admin_token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"name":"  Boris  ","layer":"core","lastContactAt":"2026-06-20","lastQuality":"deep","risk":true,"note":"weekly"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["success"], true);
        assert_eq!(j["item"]["name"], "Boris");
        assert_eq!(j["item"]["layer"], "core");
        assert_eq!(j["item"]["lastQuality"], "deep");
        assert_eq!(j["item"]["risk"], true);
        let id = j["item"]["id"].as_i64().unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/praxis/contacts")
                    .header("Cookie", auth_cookie(&other_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["count"], 0);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/praxis/contacts/{id}"))
                    .header("Cookie", auth_cookie(&admin_token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"layer":"important","lastQuality":null,"cycleOff":true}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["layer"], "important");
        assert_eq!(j["item"]["lastQuality"], JsonValue::Null);
        assert_eq!(j["item"]["cycleOff"], true);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/praxis/contacts/{id}"))
                    .header("Cookie", auth_cookie(&admin_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["success"], true);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/praxis/contacts")
                    .header("Cookie", auth_cookie(&admin_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["count"], 0);
    }

    #[tokio::test]
    async fn praxis_contacts_validate_fields() {
        let state = test_state();
        let (_admin_id, admin_token) = create_admin_user(&state, "px-admin-2", "Pa55word1");
        let app = crate::build_app(state);

        for body in [
            r#"{"name":"","layer":"normal"}"#,
            r#"{"name":"Boris","layer":"outer"}"#,
            r#"{"name":"Boris","layer":"normal","lastQuality":"bad"}"#,
        ] {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/praxis/contacts")
                        .header("Cookie", auth_cookie(&admin_token))
                        .header("Content-Type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        }
    }
}
