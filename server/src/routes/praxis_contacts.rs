//! `/api/praxis/contacts` - Praxis relation contacts.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::{json, Map, Value as JsonValue};
use tracing::error;

use crate::auth::AdminUserId;
use crate::models::praxis_contact::{
    CreatePraxisContactRequest, PraxisContact, UpdatePraxisContactRequest,
};
use crate::routes::praxis_perspectives::{ensure_default_perspective, perspective_owned};
use crate::state::AppState;

const SELECT_COLS: &str = "id, perspective_id, name, layer, last_contact_at, last_quality, risk, note, cycle_off, sort_order, created_at, updated_at";

#[derive(Debug, Deserialize, Default)]
pub struct ListContactsQuery {
    #[serde(rename = "perspectiveId")]
    pub perspective_id: Option<i64>,
}

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

fn field_string(fields: &Map<String, JsonValue>, key: &str) -> Option<String> {
    fields
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn field_bool(fields: &Map<String, JsonValue>, key: &str) -> Option<bool> {
    fields.get(key).and_then(|v| v.as_bool())
}

fn field_f64(fields: &Map<String, JsonValue>, key: &str) -> Option<f64> {
    fields.get(key).and_then(|v| v.as_f64())
}

fn row_to_contact(row: &rusqlite::Row) -> rusqlite::Result<PraxisContact> {
    let risk: i64 = row.get(6)?;
    let cycle_off: i64 = row.get(8)?;
    Ok(PraxisContact {
        id: row.get(0)?,
        perspective_id: row.get(1)?,
        name: row.get(2)?,
        layer: row.get(3)?,
        last_contact_at: row.get(4)?,
        last_quality: row.get(5)?,
        risk: risk != 0,
        note: row.get(7)?,
        cycle_off: cycle_off != 0,
        sort_order: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn load_contact(db: &Connection, user_id: &str, id: i64) -> Option<PraxisContact> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM praxis_contacts WHERE id = ?1 AND user_id = ?2 AND deleted = 0"
    );
    db.query_row(&sql, params![id, user_id], row_to_contact)
        .ok()
}

/// 解析请求里的 perspectiveId：显式传入须属于该用户（否则 Err），
/// 缺省回落到默认视角（老前端不带参数时行为不变——存量数据都迁在默认视角）。
fn resolve_perspective(
    db: &Connection,
    user_id: &str,
    requested: Option<i64>,
) -> Result<i64, String> {
    match requested {
        Some(pid) => {
            if perspective_owned(db, user_id, pid) {
                Ok(pid)
            } else {
                Err("perspective not found".into())
            }
        }
        None => ensure_default_perspective(db, user_id),
    }
}

pub fn list_contacts_impl(
    db: &Connection,
    user_id: &str,
    perspective_id: i64,
) -> Result<Vec<PraxisContact>, String> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM praxis_contacts
         WHERE user_id = ?1 AND perspective_id = ?2 AND deleted = 0
         ORDER BY sort_order ASC, updated_at DESC, id ASC"
    );
    let mut stmt = db.prepare(&sql).map_err(|e| format!("prepare: {e}"))?;
    let rows = stmt
        .query_map(params![user_id, perspective_id], row_to_contact)
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
    let perspective_id = resolve_perspective(db, user_id, req.perspective_id)?;
    let now = now_rfc3339();
    let sort_order = req.sort_order.unwrap_or_else(|| {
        db.query_row(
            "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM praxis_contacts WHERE user_id = ?1 AND deleted = 0",
            params![user_id],
            |r| r.get(0),
        )
        .unwrap_or(1.0)
    });

    // T-292 §11.3:last_contact_at/last_quality 是派生缓存,新建一律 NULL("从未联系")。
    db.execute(
        "INSERT INTO praxis_contacts
           (user_id, perspective_id, name, layer, risk, note, cycle_off, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        params![
            user_id,
            perspective_id,
            &name,
            &layer,
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
    // T-292 §11.3:lastContactAt/lastQuality 是派生缓存,PATCH 传入一律忽略
    // (老客户端兼容);§11.5:视角间不迁移,perspectiveId 同样忽略。
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
    // 派生缓存 last_contact_at/last_quality 不在 SET 里——只随交流记录增改删重算。
    db.execute(
        "UPDATE praxis_contacts
         SET name = ?1, layer = ?2,
             risk = ?3, note = ?4, cycle_off = ?5, sort_order = ?6, updated_at = ?7
         WHERE id = ?8 AND user_id = ?9 AND deleted = 0",
        params![
            &current.name,
            &current.layer,
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
    Query(q): Query<ListContactsQuery>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let perspective_id = match resolve_perspective(&db, &admin.0, q.perspective_id) {
        Ok(pid) => pid,
        Err(e) if e.contains("not found") => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "error": "未找到视角" })),
            )
        }
        Err(e) => {
            error!(target: "praxis_contacts", "list resolve perspective: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "内部错误" })),
            );
        }
    };
    match list_contacts_impl(&db, &admin.0, perspective_id) {
        Ok(items) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "items": items,
                "count": items.len(),
                "perspectiveId": perspective_id
            })),
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
        Err(e) if e.contains("not found") => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到视角" })),
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
        // T-292 §11.3:最近联系是派生缓存,创建入参里的 lastContactAt/lastQuality 被忽略
        assert_eq!(j["item"]["lastContactAt"], JsonValue::Null);
        assert_eq!(j["item"]["lastQuality"], JsonValue::Null);
        // 缺省落在默认视角
        assert!(j["item"]["perspectiveId"].as_i64().is_some());
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
    async fn contacts_are_isolated_by_perspective() {
        let state = test_state();
        let (_aid, admin_token) = create_admin_user(&state, "px-persp", "Pa55word1");
        let app = crate::build_app(state);

        // 默认视角里一个关系人(不带 perspectiveId)
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/praxis/contacts")
                    .header("Cookie", auth_cookie(&admin_token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"name":"默认人","layer":"normal"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let default_pid = body_json(resp).await["item"]["perspectiveId"]
            .as_i64()
            .unwrap();

        // 建「工作」视角,往里放一个关系人
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/praxis/perspectives")
                    .header("Cookie", auth_cookie(&admin_token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"name":"工作"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let work_pid = body_json(resp).await["item"]["id"].as_i64().unwrap();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/praxis/contacts")
                    .header("Cookie", auth_cookie(&admin_token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"name":"工作人","layer":"core","perspectiveId":{work_pid}}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["item"]["perspectiveId"], work_pid);

        // 不带参数 = 默认视角,只看到默认人;带 perspectiveId 只看到该视角的人 → 完全隔离
        for (uri, expect_name, expect_pid) in [
            ("/api/praxis/contacts".to_string(), "默认人", default_pid),
            (
                format!("/api/praxis/contacts?perspectiveId={default_pid}"),
                "默认人",
                default_pid,
            ),
            (
                format!("/api/praxis/contacts?perspectiveId={work_pid}"),
                "工作人",
                work_pid,
            ),
        ] {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(uri)
                        .header("Cookie", auth_cookie(&admin_token))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let j = body_json(resp).await;
            assert_eq!(j["count"], 1);
            assert_eq!(j["items"][0]["name"], expect_name);
            assert_eq!(j["perspectiveId"], expect_pid);
        }

        // 不存在/不属于自己的视角:GET 与 POST 都 404
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/praxis/contacts?perspectiveId=99999")
                    .header("Cookie", auth_cookie(&admin_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/praxis/contacts")
                    .header("Cookie", auth_cookie(&admin_token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"name":"越权","layer":"normal","perspectiveId":99999}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn praxis_contacts_validate_fields() {
        let state = test_state();
        let (_admin_id, admin_token) = create_admin_user(&state, "px-admin-2", "Pa55word1");
        let app = crate::build_app(state);

        for body in [
            r#"{"name":"","layer":"normal"}"#,
            r#"{"name":"Boris","layer":"outer"}"#,
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
