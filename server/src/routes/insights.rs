//! `/api/insights` — Insight CRUD + claim/release + reports/share 入口。
//!
//! See SPEC: `C:\Project\ergouPM\specs\insight\spec.md`
//! Task ticket: T-105
//!
//! 架构(spec 原则 1 Hybrid):
//!   - Web 后端 = 收件箱 + 抓取 + 存储 + 公开分享(本路由组)
//!   - Claude Code(Boris 本机) = 调 LLM 写报告,POST /api/insights/:id/reports
//!   - 同事/领导 = 公开 /r/{token},不要 session

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use tracing::error;

use crate::auth::UserId;
use crate::models::insight::{
    is_valid_status, is_valid_template, CreateInsightRequest, Insight, UpdateInsightRequest,
};
use crate::state::AppState;

// ============ 共用 ============

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn db_error(ctx: &str, e: rusqlite::Error) -> (StatusCode, Json<JsonValue>) {
    error!(target: "insights", "{} db error: {}", ctx, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "error": "内部错误" })),
    )
}

/// v0.2:加 pending_revision_note 字段
const SELECT_COLS: &str = "id, title, topic, template, status, current_report_id, \
    pending_revision_note, created_at, updated_at";

fn row_to_insight(row: &rusqlite::Row) -> rusqlite::Result<Insight> {
    Ok(Insight {
        id: row.get(0)?,
        title: row.get(1)?,
        topic: row.get(2)?,
        template: row.get(3)?,
        status: row.get(4)?,
        current_report_id: row.get(5)?,
        pending_revision_note: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

// ============ HTTP handlers ============

/// GET /api/insights?status=&limit=
#[derive(Debug, Default, Deserialize)]
pub struct ListFilters {
    pub status: Option<String>,
    pub limit: Option<i64>,
}

pub async fn list_insights(
    State(state): State<AppState>,
    user_id: UserId,
    Query(f): Query<ListFilters>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let limit = f.limit.unwrap_or(0).clamp(0, 200);
    let limit_sql = if limit > 0 { format!(" LIMIT {limit}") } else { String::new() };
    let (sql, params_v): (String, Vec<Box<dyn rusqlite::ToSql>>) = match f.status.as_deref() {
        Some(s) if !s.is_empty() => (
            format!(
                "SELECT {SELECT_COLS} FROM insights \
                 WHERE user_id = ?1 AND deleted = 0 AND status = ?2 \
                 ORDER BY updated_at DESC{limit_sql}"
            ),
            vec![Box::new(user_id.0.clone()), Box::new(s.to_string())],
        ),
        _ => (
            format!(
                "SELECT {SELECT_COLS} FROM insights \
                 WHERE user_id = ?1 AND deleted = 0 \
                 ORDER BY updated_at DESC{limit_sql}"
            ),
            vec![Box::new(user_id.0.clone())],
        ),
    };
    let mut stmt = match db.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return db_error("list prepare", e),
    };
    let params_ref: Vec<&dyn rusqlite::ToSql> = params_v.iter().map(|b| &**b).collect();
    let rows = match stmt.query_map(params_ref.as_slice(), row_to_insight) {
        Ok(r) => r,
        Err(e) => return db_error("list query", e),
    };
    let items: Vec<Insight> = rows.filter_map(|r| r.ok()).collect();
    (
        StatusCode::OK,
        Json(json!({ "success": true, "items": items, "count": items.len() })),
    )
}

/// POST /api/insights
pub async fn create_insight(
    State(state): State<AppState>,
    user_id: UserId,
    Json(req): Json<CreateInsightRequest>,
) -> (StatusCode, Json<JsonValue>) {
    if !is_valid_template(&req.template) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": format!("invalid template: {}", req.template) })),
        );
    }
    let db = state.db.lock();
    let now = now_rfc3339();
    // v0.2:初始 status='collecting'(无报告);CC 写入 v1 后副作用切到 'editing'
    let res = db.execute(
        "INSERT INTO insights (user_id, title, topic, template, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'collecting', ?5, ?5)",
        params![&user_id.0, &req.title, &req.topic, &req.template, &now],
    );
    match res {
        Ok(_) => {
            let id = db.last_insert_rowid();
            fetch_insight(&db, &user_id.0, id)
        }
        Err(e) => db_error("create insert", e),
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct GetParams {
    pub include: Option<String>,
}

/// GET /api/insights/:id?include=sources
pub async fn get_insight(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
    Query(p): Query<GetParams>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let resp = fetch_insight(&db, &user_id.0, id);
    if resp.0 != StatusCode::OK {
        return resp;
    }
    let mut body = resp.1 .0;
    // ?include=sources 时附带 source 全量
    if p.include.as_deref().map(|s| s.contains("sources")).unwrap_or(false) {
        match crate::routes::sources::list_for_insight(&db, &user_id.0, Some(id), false) {
            Ok(items) => {
                if let Some(item) = body.get_mut("item") {
                    item["sources"] = serde_json::to_value(&items).unwrap_or(json!([]));
                }
            }
            Err(e) => {
                error!(target: "insights", "include sources: {}", e);
            }
        }
    }
    (StatusCode::OK, Json(body))
}

/// PATCH /api/insights/:id
pub async fn update_insight(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
    Json(patch): Json<UpdateInsightRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let mut sets: Vec<&str> = Vec::new();
    let mut vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(v) = patch.title {
        sets.push("title = ?");
        vals.push(Box::new(v));
    }
    if let Some(v) = patch.topic {
        sets.push("topic = ?");
        vals.push(Box::new(v));
    }
    if let Some(v) = patch.template {
        if !is_valid_template(&v) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": "invalid template" })),
            );
        }
        sets.push("template = ?");
        vals.push(Box::new(v));
    }
    if let Some(v) = patch.status {
        if !is_valid_status(&v) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": "invalid status" })),
            );
        }
        sets.push("status = ?");
        vals.push(Box::new(v));
    }
    if let Some(v) = patch.current_report_id {
        sets.push("current_report_id = ?");
        vals.push(Box::new(v));
    }
    if sets.is_empty() {
        return fetch_insight(&db, &user_id.0, id);
    }
    sets.push("updated_at = ?");
    vals.push(Box::new(now_rfc3339()));
    vals.push(Box::new(id));
    vals.push(Box::new(user_id.0.clone()));
    let sql = format!(
        "UPDATE insights SET {} WHERE id = ? AND user_id = ? AND deleted = 0",
        sets.join(", ")
    );
    let params_ref: Vec<&dyn rusqlite::ToSql> = vals.iter().map(|b| &**b).collect();
    match db.execute(&sql, params_ref.as_slice()) {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察" })),
        ),
        Ok(_) => fetch_insight(&db, &user_id.0, id),
        Err(e) => db_error("update", e),
    }
}

/// DELETE /api/insights/:id — soft delete
pub async fn delete_insight(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let now = now_rfc3339();
    let res = db.execute(
        "UPDATE insights SET deleted = 1, updated_at = ?1 WHERE id = ?2 AND user_id = ?3 AND deleted = 0",
        params![&now, id, &user_id.0],
    );
    match res {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察" })),
        ),
        Ok(_) => (StatusCode::OK, Json(json!({ "success": true }))),
        Err(e) => db_error("delete", e),
    }
}

/// POST /api/insights/:id/claim — Claude Code 抢占处理(spec § 6.3)
/// status: ready → processing (atomic);非 ready 状态拒绝。
pub async fn claim_insight(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let now = now_rfc3339();
    // 原子化:只有当前 status='ready' 时才改成 'processing'
    let n = db.execute(
        "UPDATE insights SET status = 'processing', updated_at = ?1 \
         WHERE id = ?2 AND user_id = ?3 AND status = 'ready' AND deleted = 0",
        params![&now, id, &user_id.0],
    );
    match n {
        Ok(1) => fetch_insight(&db, &user_id.0, id),
        Ok(0) => {
            // 是不存在,还是状态不对?分开报错让 Claude Code 能判断
            let cur_status: Option<String> = db
                .query_row(
                    "SELECT status FROM insights WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
                    params![id, &user_id.0],
                    |r| r.get(0),
                )
                .ok();
            match cur_status {
                None => (
                    StatusCode::NOT_FOUND,
                    Json(json!({ "success": false, "error": "未找到洞察" })),
                ),
                Some(s) => (
                    StatusCode::CONFLICT,
                    Json(json!({
                        "success": false,
                        "error": format!("无法 claim:当前状态为 '{}',只允许在 'ready' 状态抢占", s),
                        "currentStatus": s,
                    })),
                ),
            }
        }
        Ok(_) => fetch_insight(&db, &user_id.0, id), // 不应该 >1,容错走 OK
        Err(e) => db_error("claim", e),
    }
}

/// POST /api/insights/:id/regenerate — Boris "让 CC 改一版"(T-107 v0.2 § 6.3.3)
/// 行为:写入 pending_revision_note + status 切到 ready;Claude Code 会看到这个 ready 状态
/// + 非空的 pending_revision_note + current_report_id 非空 → 进入修订模式。
///
/// 前置:status 必须是 editing(已有一版报告);否则 409(spec § 5.3 状态转移)。
#[derive(Debug, Deserialize)]
pub struct RegenerateRequest {
    #[serde(rename = "revisionNote", default)]
    pub revision_note: String,
}

pub async fn regenerate_insight(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
    Json(req): Json<RegenerateRequest>,
) -> (StatusCode, Json<JsonValue>) {
    if req.revision_note.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": "revisionNote 不能为空(写一句要 CC 改什么)"
            })),
        );
    }
    let db = state.db.lock();
    // 必须 editing 状态才能 regenerate(已有一版才能修订)
    let cur: Option<(String, Option<i64>)> = db
        .query_row(
            "SELECT status, current_report_id FROM insights WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
            params![id, &user_id.0],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?)),
        )
        .ok();
    let Some((status, current_report_id)) = cur else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察" })),
        );
    };
    if status != "editing" {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": format!("regenerate 仅在 editing 状态可用,当前 '{}'", status),
                "currentStatus": status,
            })),
        );
    }
    if current_report_id.is_none() {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": "current_report_id 为空(没有可基于的上一版),先生成 v1"
            })),
        );
    }
    let now = now_rfc3339();
    let _ = db.execute(
        "UPDATE insights SET status = 'ready', pending_revision_note = ?1, updated_at = ?2 \
         WHERE id = ?3 AND user_id = ?4",
        params![&req.revision_note, &now, id, &user_id.0],
    );
    fetch_insight(&db, &user_id.0, id)
}

/// POST /api/insights/:id/release — claim 失败放回(processing → ready)
pub async fn release_insight(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let now = now_rfc3339();
    let n = db.execute(
        "UPDATE insights SET status = 'ready', updated_at = ?1 \
         WHERE id = ?2 AND user_id = ?3 AND status = 'processing' AND deleted = 0",
        params![&now, id, &user_id.0],
    );
    match n {
        Ok(1) => fetch_insight(&db, &user_id.0, id),
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到 processing 状态的洞察" })),
        ),
        Ok(_) => fetch_insight(&db, &user_id.0, id),
        Err(e) => db_error("release", e),
    }
}

// ============ 给本 crate 复用的 helper ============

pub fn fetch_insight(db: &Connection, user_id: &str, id: i64) -> (StatusCode, Json<JsonValue>) {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM insights WHERE id = ?1 AND user_id = ?2 AND deleted = 0"
    );
    match db.query_row(&sql, params![id, user_id], row_to_insight) {
        Ok(t) => (
            StatusCode::OK,
            Json(json!({ "success": true, "item": t })),
        ),
        Err(rusqlite::Error::QueryReturnedNoRows) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察" })),
        ),
        Err(e) => db_error("fetch_insight", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{auth_cookie, create_test_user, test_state};
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tower::ServiceExt;

    async fn body_json(resp: axum::response::Response) -> JsonValue {
        let status = resp.status();
        let body = to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        let body_str = String::from_utf8_lossy(&body);
        serde_json::from_slice(&body).unwrap_or_else(|e| {
            panic!("parse fail status={} body={:?} err={}", status, body_str, e)
        })
    }

    #[tokio::test]
    async fn create_then_list() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "alice", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/insights")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"Rust async","topic":"评估 runtime 选型"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["success"], true);
        assert_eq!(j["item"]["title"], "Rust async");
        assert_eq!(j["item"]["status"], "collecting"); // v0.2
        assert_eq!(j["item"]["template"], "survey"); // 默认模板

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/insights")
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["count"], 1);
    }

    #[tokio::test]
    async fn invalid_template_rejected() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "u2", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/insights")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"x","template":"nonsense"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn claim_only_works_on_ready() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "u3", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        // Create (status=drafting)
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/insights")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"x"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = body_json(resp).await["item"]["id"].as_i64().unwrap();

        // Claim while drafting → 409
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{id}/claim"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CONFLICT);

        // 标记为 ready
        app.clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/insights/{id}"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"status":"ready"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Claim 1st → 200
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{id}/claim"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["status"], "processing");

        // Claim 2nd → 409 (already processing)
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{id}/claim"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn regenerate_writes_pending_note_and_sets_ready() {
        let state = test_state();
        let (_u, token) = create_test_user(&state, "u-regen", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        // 创建 insight + report → editing
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/insights")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"x"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let iid = body_json(resp).await["item"]["id"].as_i64().unwrap();
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid}/reports"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"template":"survey","contentMd":"v1"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        // regenerate
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid}/regenerate"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"revisionNote":"再加个反例"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["status"], "ready");
        assert_eq!(j["item"]["pendingRevisionNote"], "再加个反例");

        // 从 collecting 直接 regenerate 应 409
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/insights")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"empty"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let iid2 = body_json(resp).await["item"]["id"].as_i64().unwrap();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid2}/regenerate"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"revisionNote":"x"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn user_isolation() {
        let state = test_state();
        let (_uid_a, token_a) = create_test_user(&state, "alice2", "Pa55word1");
        let (_uid_b, token_b) = create_test_user(&state, "bob2", "Pa55word1");
        let cookie_a = auth_cookie(&token_a);
        let cookie_b = auth_cookie(&token_b);
        let app = crate::build_app(state);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/insights")
                    .header("Cookie", &cookie_a)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"alice's secret"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = body_json(resp).await["item"]["id"].as_i64().unwrap();

        // Bob 看不到 alice 的
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/insights/{id}"))
                    .header("Cookie", &cookie_b)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/insights")
                    .header("Cookie", &cookie_b)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["count"], 0);
    }
}
