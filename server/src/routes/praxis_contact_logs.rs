//! `/api/praxis/contacts/:id/logs` - Praxis contact interaction logs.
//!
//! See SPEC: `C:\Project\ergouPM\specs\praxis\spec.md` §7

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Map, Value as JsonValue};
use tracing::error;

use crate::auth::AdminUserId;
use crate::models::praxis_contact_log::{
    CreateContactLogRequest, PraxisContactLog, UpdateContactLogRequest,
};
use crate::state::AppState;

const SELECT_COLS: &str = "id, contact_id, at, method, quality, content, note, created_at";

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn db_error(ctx: &str, e: rusqlite::Error) -> (StatusCode, Json<JsonValue>) {
    error!(target: "praxis_contact_logs", "{} db error: {}", ctx, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "error": "内部错误" })),
    )
}

fn validate_quality(quality: Option<String>) -> Result<Option<String>, String> {
    match quality.as_deref() {
        None | Some("") => Ok(None),
        Some("shallow" | "effective" | "deep") => Ok(quality),
        _ => Err("quality must be shallow, effective, deep, or null".into()),
    }
}

/// Verify the contact exists and belongs to the user.
fn contact_owned(db: &Connection, user_id: &str, contact_id: i64) -> bool {
    db.query_row(
        "SELECT 1 FROM praxis_contacts WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
        params![contact_id, user_id],
        |r| r.get::<_, i64>(0),
    )
    .is_ok()
}

/// T-292 §11.3:重算派生缓存——`last_contact_at`/`last_quality` 恒等于该关系人
/// 最新一条交流记录(max at);无记录则清空("从未联系")。交流增/改/删后调用。
/// 返回 contact 行是否真的变了(供前端决定要不要刷新弧)。
fn recalc_last_contact(
    db: &Connection,
    user_id: &str,
    contact_id: i64,
) -> Result<bool, rusqlite::Error> {
    let latest: Option<(String, Option<String>)> = db
        .query_row(
            "SELECT at, quality FROM praxis_contact_logs
              WHERE contact_id = ?1 ORDER BY at DESC, id DESC LIMIT 1",
            params![contact_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (new_at, new_quality) = match latest {
        Some((at, quality)) => (Some(at), quality),
        None => (None, None),
    };
    // IS NOT 是 SQLite 的 null 安全比较;值没变就不动 updated_at。
    let changed = db.execute(
        "UPDATE praxis_contacts
            SET last_contact_at = ?1, last_quality = ?2, updated_at = ?3
          WHERE id = ?4 AND user_id = ?5 AND deleted = 0
            AND (last_contact_at IS NOT ?1 OR last_quality IS NOT ?2)",
        params![&new_at, &new_quality, now_rfc3339(), contact_id, user_id],
    )?;
    Ok(changed > 0)
}

fn row_to_log(row: &rusqlite::Row) -> rusqlite::Result<PraxisContactLog> {
    Ok(PraxisContactLog {
        id: row.get(0)?,
        contact_id: row.get(1)?,
        at: row.get(2)?,
        method: row.get(3)?,
        quality: row.get(4)?,
        content: row.get(5)?,
        note: row.get(6)?,
        created_at: row.get(7)?,
    })
}

pub fn list_logs_impl(db: &Connection, contact_id: i64) -> Result<Vec<PraxisContactLog>, String> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM praxis_contact_logs WHERE contact_id = ?1 ORDER BY at DESC, id DESC"
    );
    let mut stmt = db.prepare(&sql).map_err(|e| format!("prepare: {e}"))?;
    let rows = stmt
        .query_map(params![contact_id], row_to_log)
        .map_err(|e| format!("query: {e}"))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub async fn list_logs(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path(contact_id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    if !contact_owned(&db, &admin.0, contact_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到关系人" })),
        );
    }
    match list_logs_impl(&db, contact_id) {
        Ok(items) => (
            StatusCode::OK,
            Json(json!({ "success": true, "items": items, "count": items.len() })),
        ),
        Err(e) => {
            error!(target: "praxis_contact_logs", "list: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "内部错误" })),
            )
        }
    }
}

pub async fn create_log(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path(contact_id): Path<i64>,
    Json(req): Json<CreateContactLogRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let at = req.at.trim().to_string();
    if at.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "at must not be empty" })),
        );
    }
    let quality = match validate_quality(req.quality.clone()) {
        Ok(q) => q,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": e })),
            )
        }
    };

    let db = state.db.lock();
    if !contact_owned(&db, &admin.0, contact_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到关系人" })),
        );
    }

    let now = now_rfc3339();
    if let Err(e) = db.execute(
        "INSERT INTO praxis_contact_logs
           (contact_id, user_id, at, method, quality, content, note, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            contact_id,
            &admin.0,
            &at,
            &req.method,
            &quality,
            &req.content,
            &req.note,
            &now,
        ],
    ) {
        return db_error("insert", e);
    }
    let log_id = db.last_insert_rowid();

    // T-292 §11.3:增/改/删统一走派生重算,最近联系恒等于最新一条记录,
    // 驱动节点状态（dim→solid 等）。取代 T-287 "若更晚才写"的逻辑。
    let contact_updated = match recalc_last_contact(&db, &admin.0, contact_id) {
        Ok(changed) => changed,
        Err(e) => return db_error("recalc", e),
    };

    let log = db
        .query_row(
            &format!("SELECT {SELECT_COLS} FROM praxis_contact_logs WHERE id = ?1"),
            params![log_id],
            row_to_log,
        )
        .ok();

    (
        StatusCode::OK,
        Json(json!({ "success": true, "item": log, "contactUpdated": contact_updated })),
    )
}

/// 属主校验:log 须属于该 user + contact(T-292 §11.4)。
fn load_owned_log(
    db: &Connection,
    user_id: &str,
    contact_id: i64,
    log_id: i64,
) -> Option<PraxisContactLog> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM praxis_contact_logs
          WHERE id = ?1 AND contact_id = ?2 AND user_id = ?3"
    );
    db.query_row(&sql, params![log_id, contact_id, user_id], row_to_log)
        .ok()
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

pub async fn update_log(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path((contact_id, log_id)): Path<(i64, i64)>,
    Json(patch): Json<UpdateContactLogRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    if !contact_owned(&db, &admin.0, contact_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到关系人" })),
        );
    }
    let Some(mut current) = load_owned_log(&db, &admin.0, contact_id, log_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到交流记录" })),
        );
    };

    let bad_request = |e: String| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": e })),
        )
    };
    if let Some(at) = field_string(&patch.fields, "at") {
        let at = at.trim().to_string();
        if at.is_empty() {
            return bad_request("at must not be empty".into());
        }
        current.at = at;
    }
    match field_nullable_string(&patch.fields, "method") {
        Ok(Some(method)) => current.method = method,
        Ok(None) => {}
        Err(e) => return bad_request(e),
    }
    match field_nullable_string(&patch.fields, "quality") {
        Ok(Some(quality)) => match validate_quality(quality) {
            Ok(q) => current.quality = q,
            Err(e) => return bad_request(e),
        },
        Ok(None) => {}
        Err(e) => return bad_request(e),
    }
    if let Some(content) = field_string(&patch.fields, "content") {
        current.content = content;
    }
    if let Some(note) = field_string(&patch.fields, "note") {
        current.note = note;
    }

    if let Err(e) = db.execute(
        "UPDATE praxis_contact_logs
            SET at = ?1, method = ?2, quality = ?3, content = ?4, note = ?5
          WHERE id = ?6 AND contact_id = ?7 AND user_id = ?8",
        params![
            &current.at,
            &current.method,
            &current.quality,
            &current.content,
            &current.note,
            log_id,
            contact_id,
            &admin.0,
        ],
    ) {
        return db_error("update", e);
    }

    // 改动可能把"最新一条"换了位置,重算派生缓存(§11.3)。
    let contact_updated = match recalc_last_contact(&db, &admin.0, contact_id) {
        Ok(changed) => changed,
        Err(e) => return db_error("recalc", e),
    };
    let item = load_owned_log(&db, &admin.0, contact_id, log_id);
    (
        StatusCode::OK,
        Json(json!({ "success": true, "item": item, "contactUpdated": contact_updated })),
    )
}

pub async fn delete_log(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path((contact_id, log_id)): Path<(i64, i64)>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    if !contact_owned(&db, &admin.0, contact_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到关系人" })),
        );
    }
    match db.execute(
        "DELETE FROM praxis_contact_logs WHERE id = ?1 AND contact_id = ?2 AND user_id = ?3",
        params![log_id, contact_id, &admin.0],
    ) {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到交流记录" })),
        ),
        Ok(_) => {
            // 删掉最新一条会自动回退到上一条;删光则清空(§11.3)。
            let contact_updated = match recalc_last_contact(&db, &admin.0, contact_id) {
                Ok(changed) => changed,
                Err(e) => return db_error("recalc", e),
            };
            (
                StatusCode::OK,
                Json(json!({ "success": true, "contactUpdated": contact_updated })),
            )
        }
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

    async fn create_contact(app: &axum::Router, token: &str, body: &'static str) -> i64 {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/praxis/contacts")
                    .header("Cookie", auth_cookie(token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        body_json(resp).await["item"]["id"].as_i64().unwrap()
    }

    #[tokio::test]
    async fn logs_require_admin() {
        let state = test_state();
        let (_uid, user_token) = create_test_user(&state, "pl-user", "Pa55word1");
        let app = crate::build_app(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/praxis/contacts/1/logs")
                    .header("Cookie", auth_cookie(&user_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn create_log_writes_back_contact_and_isolates() {
        let state = test_state();
        let (_aid, admin_token) = create_admin_user(&state, "pl-admin", "Pa55word1");
        let (_oid, other_token) = create_admin_user(&state, "pl-other", "Pa55word1");
        let app = crate::build_app(state);

        // contact starts dim (no last_contact_at)
        let cid = create_contact(&app, &admin_token, r#"{"name":"Mentor","layer":"core"}"#).await;

        // add a log → should write back last_contact_at + last_quality
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/praxis/contacts/{cid}/logs"))
                    .header("Cookie", auth_cookie(&admin_token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"at":"2026-06-29","method":"微信","quality":"deep","content":"聊了方向"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["success"], true);
        assert_eq!(j["contactUpdated"], true);
        assert_eq!(j["item"]["quality"], "deep");

        // contact now reflects the writeback
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/praxis/contacts")
                    .header("Cookie", auth_cookie(&admin_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["items"][0]["lastContactAt"], "2026-06-29");
        assert_eq!(j["items"][0]["lastQuality"], "deep");

        // an EARLIER log must NOT overwrite the newer last_contact_at
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/praxis/contacts/{cid}/logs"))
                    .header("Cookie", auth_cookie(&admin_token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"at":"2026-06-01","quality":"shallow"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["contactUpdated"], false);

        // logs list returns both, newest first
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/praxis/contacts/{cid}/logs"))
                    .header("Cookie", auth_cookie(&admin_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["count"], 2);
        assert_eq!(j["items"][0]["at"], "2026-06-29");

        // other admin cannot touch this contact's logs
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/praxis/contacts/{cid}/logs"))
                    .header("Cookie", auth_cookie(&other_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    async fn contact_last(app: &axum::Router, token: &str) -> (JsonValue, JsonValue) {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/praxis/contacts")
                    .header("Cookie", auth_cookie(token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        (
            j["items"][0]["lastContactAt"].clone(),
            j["items"][0]["lastQuality"].clone(),
        )
    }

    #[tokio::test]
    async fn update_and_delete_log_recalc_last_contact() {
        let state = test_state();
        let (_aid, admin_token) = create_admin_user(&state, "pl-recalc", "Pa55word1");
        let (_oid, other_token) = create_admin_user(&state, "pl-recalc-other", "Pa55word1");
        let app = crate::build_app(state);
        let cid = create_contact(&app, &admin_token, r#"{"name":"Mentor","layer":"core"}"#).await;

        // 两条记录:A 早 / B 晚 → 派生 = B
        let mut ids = Vec::new();
        for body in [
            r#"{"at":"2026-06-01","quality":"shallow","content":"A"}"#,
            r#"{"at":"2026-06-29","quality":"deep","content":"B"}"#,
        ] {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/api/praxis/contacts/{cid}/logs"))
                        .header("Cookie", auth_cookie(&admin_token))
                        .header("Content-Type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            ids.push(body_json(resp).await["item"]["id"].as_i64().unwrap());
        }
        let (log_a, log_b) = (ids[0], ids[1]);
        assert_eq!(
            contact_last(&app, &admin_token).await,
            (json!("2026-06-29"), json!("deep"))
        );

        // 属主校验:别的 admin 改/删 → 404
        for method in ["PATCH", "DELETE"] {
            let mut builder = Request::builder()
                .method(method)
                .uri(format!("/api/praxis/contacts/{cid}/logs/{log_b}"))
                .header("Cookie", auth_cookie(&other_token));
            let body = if method == "PATCH" {
                builder = builder.header("Content-Type", "application/json");
                Body::from(r#"{"at":"2026-01-01"}"#)
            } else {
                Body::empty()
            };
            let resp = app
                .clone()
                .oneshot(builder.body(body).unwrap())
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        }

        // 编辑 B 把时间改到最早 → 派生回退到 A(2026-06-01 shallow)
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/praxis/contacts/{cid}/logs/{log_b}"))
                    .header("Cookie", auth_cookie(&admin_token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"at":"2026-05-01","content":"B改"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["at"], "2026-05-01");
        assert_eq!(j["item"]["content"], "B改");
        assert_eq!(j["contactUpdated"], true);
        assert_eq!(
            contact_last(&app, &admin_token).await,
            (json!("2026-06-01"), json!("shallow"))
        );

        // PATCH 校验:空 at / 非法 quality → 400
        for body in [r#"{"at":"  "}"#, r#"{"quality":"meh"}"#] {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("PATCH")
                        .uri(format!("/api/praxis/contacts/{cid}/logs/{log_a}"))
                        .header("Cookie", auth_cookie(&admin_token))
                        .header("Content-Type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        }

        // 删掉最新一条(A) → 回退到 B(2026-05-01 deep)
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/praxis/contacts/{cid}/logs/{log_a}"))
                    .header("Cookie", auth_cookie(&admin_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["contactUpdated"], true);
        assert_eq!(
            contact_last(&app, &admin_token).await,
            (json!("2026-05-01"), json!("deep"))
        );

        // 删光 → 派生清空("从未联系");再删同一条 → 404
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/praxis/contacts/{cid}/logs/{log_b}"))
                    .header("Cookie", auth_cookie(&admin_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["success"], true);
        assert_eq!(
            contact_last(&app, &admin_token).await,
            (JsonValue::Null, JsonValue::Null)
        );
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/praxis/contacts/{cid}/logs/{log_b}"))
                    .header("Cookie", auth_cookie(&admin_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn create_log_validates() {
        let state = test_state();
        let (_aid, admin_token) = create_admin_user(&state, "pl-admin-2", "Pa55word1");
        let app = crate::build_app(state);
        let cid = create_contact(&app, &admin_token, r#"{"name":"X","layer":"normal"}"#).await;

        // empty at → 400
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/praxis/contacts/{cid}/logs"))
                    .header("Cookie", auth_cookie(&admin_token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"at":"  "}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // bad quality → 400
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/praxis/contacts/{cid}/logs"))
                    .header("Cookie", auth_cookie(&admin_token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"at":"2026-06-30","quality":"meh"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
