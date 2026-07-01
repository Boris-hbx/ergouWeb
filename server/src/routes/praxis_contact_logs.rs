//! `/api/praxis/contacts/:id/logs` - Praxis contact interaction logs.
//!
//! See SPEC: `C:\Project\ergouPM\specs\praxis\spec.md` §7

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde_json::{json, Value as JsonValue};
use tracing::error;

use crate::auth::AdminUserId;
use crate::models::praxis_contact_log::{CreateContactLogRequest, PraxisContactLog};
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

/// Verify the contact exists and belongs to the user; returns its current
/// `last_contact_at` (Option) when found, or None when the contact is absent.
fn contact_owned(db: &Connection, user_id: &str, contact_id: i64) -> Option<Option<String>> {
    db.query_row(
        "SELECT last_contact_at FROM praxis_contacts WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
        params![contact_id, user_id],
        |r| r.get::<_, Option<String>>(0),
    )
    .ok()
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
    if contact_owned(&db, &admin.0, contact_id).is_none() {
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
    let Some(prev_last) = contact_owned(&db, &admin.0, contact_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到关系人" })),
        );
    };

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

    // 回写关系人：若本次交流比现有 last_contact_at 晚，更新最近联系时间与质量，
    // 驱动节点状态（dim→solid 等）。空 last_contact_at 视为更早，直接回写。
    let is_later = match &prev_last {
        None => true,
        Some(prev) => at.as_str() > prev.as_str(),
    };
    let mut contact_updated = false;
    if is_later {
        match db.execute(
            "UPDATE praxis_contacts SET last_contact_at = ?1, last_quality = ?2, updated_at = ?3
             WHERE id = ?4 AND user_id = ?5 AND deleted = 0",
            params![&at, &quality, &now, contact_id, &admin.0],
        ) {
            Ok(_) => contact_updated = true,
            Err(e) => return db_error("writeback", e),
        }
    }

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
