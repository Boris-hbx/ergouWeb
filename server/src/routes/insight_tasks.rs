//! `/api/insight-tasks` — 洞察 v0.3 单层任务 CRUD + claim/release + reports(T-122)。
//!
//! See SPEC: `C:\Project\ergouPM\specs\insight\spec.md` v0.3 § 十
//!
//! 架构(spec 原则 1 Hybrid):
//!   - Web 后端 = 收件箱(录入 + 看 + 反馈)+ 存储(本路由组)
//!   - Claude Code(Boris 本机)= 工人,claim → 生成 → POST /reports
//!
//! 鉴权:全 endpoint 走 `UserId` 提取器,自动支持 session cookie + Bearer PAT(T-116)。
//! 全查询 `WHERE user_id = ?` 强隔离(铁律)。

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
use crate::models::insight_task::{
    derive_title, detect_input_type, is_valid_input_type, is_valid_template, CreateReportRequest,
    CreateTaskRequest, InsightReport, InsightTask, UpdateTaskRequest,
};
use crate::state::AppState;

// ============ 共用 ============

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn db_error(ctx: &str, e: rusqlite::Error) -> (StatusCode, Json<JsonValue>) {
    error!(target: "insight_tasks", "{} db error: {}", ctx, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "error": "内部错误" })),
    )
}

const TASK_COLS: &str = "id, title, input_type, input_content, template, status, \
    current_report_id, feedback_note, source_snapshot, created_at, updated_at";

fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<InsightTask> {
    Ok(InsightTask {
        id: row.get(0)?,
        title: row.get(1)?,
        input_type: row.get(2)?,
        input_content: row.get(3)?,
        template: row.get(4)?,
        status: row.get(5)?,
        current_report_id: row.get(6)?,
        feedback_note: row.get(7)?,
        source_snapshot: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

const REPORT_COLS: &str = "id, task_id, version, template, content_md, parent_report_id, \
    revision_note, generated_by, model_used, created_at";

fn row_to_report(row: &rusqlite::Row) -> rusqlite::Result<InsightReport> {
    Ok(InsightReport {
        id: row.get(0)?,
        task_id: row.get(1)?,
        version: row.get(2)?,
        template: row.get(3)?,
        content_md: row.get(4)?,
        parent_report_id: row.get(5)?,
        revision_note: row.get(6)?,
        generated_by: row.get(7)?,
        model_used: row.get(8)?,
        created_at: row.get(9)?,
    })
}

/// 取单个 task(含 source_snapshot)并包成 `{success, item}`。
fn fetch_task(db: &Connection, user_id: &str, id: i64) -> (StatusCode, Json<JsonValue>) {
    let sql = format!(
        "SELECT {TASK_COLS} FROM insight_tasks WHERE id = ?1 AND user_id = ?2 AND deleted = 0"
    );
    match db.query_row(&sql, params![id, user_id], row_to_task) {
        Ok(t) => (StatusCode::OK, Json(json!({ "success": true, "item": t }))),
        Err(rusqlite::Error::QueryReturnedNoRows) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察任务" })),
        ),
        Err(e) => db_error("fetch_task", e),
    }
}

fn fetch_latest_report(db: &Connection, task_id: i64) -> Option<InsightReport> {
    let sql = format!(
        "SELECT {REPORT_COLS} FROM insight_reports WHERE task_id = ?1 ORDER BY version DESC LIMIT 1"
    );
    db.query_row(&sql, params![task_id], row_to_report).ok()
}

// ============ HTTP handlers ============

/// GET /api/insight-tasks?status=&limit=
#[derive(Debug, Default, Deserialize)]
pub struct ListFilters {
    pub status: Option<String>,
    pub limit: Option<i64>,
}

/// 列表页(Web)+ CC `?status=ready`。**不返回 source_snapshot**(可能很大),
/// 附带最新报告 version + 时间方便列表展示(spec § 8.2)。
pub async fn list_tasks(
    State(state): State<AppState>,
    user_id: UserId,
    Query(f): Query<ListFilters>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let limit = f.limit.unwrap_or(0).clamp(0, 200);
    let limit_sql = if limit > 0 {
        format!(" LIMIT {limit}")
    } else {
        String::new()
    };
    let status_filter = match f.status.as_deref() {
        Some(s) if !s.is_empty() => format!(" AND t.status = '{}'", s.replace('\'', "")),
        _ => String::new(),
    };
    let sql = format!(
        "SELECT t.id, t.title, t.input_type, t.input_content, t.template, t.status, \
                t.current_report_id, t.feedback_note, t.created_at, t.updated_at, \
                (SELECT MAX(version) FROM insight_reports r WHERE r.task_id = t.id) AS latest_version, \
                (SELECT MAX(created_at) FROM insight_reports r WHERE r.task_id = t.id) AS latest_report_at \
         FROM insight_tasks t \
         WHERE t.user_id = ?1 AND t.deleted = 0{status_filter} \
         ORDER BY t.updated_at DESC{limit_sql}"
    );
    let mut stmt = match db.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return db_error("list prepare", e),
    };
    let rows = stmt.query_map(params![user_id.0], |row| {
        Ok(json!({
            "id": row.get::<_, i64>(0)?,
            "title": row.get::<_, String>(1)?,
            "inputType": row.get::<_, String>(2)?,
            "inputContent": row.get::<_, String>(3)?,
            "template": row.get::<_, Option<String>>(4)?,
            "status": row.get::<_, String>(5)?,
            "currentReportId": row.get::<_, Option<i64>>(6)?,
            "feedbackNote": row.get::<_, String>(7)?,
            "createdAt": row.get::<_, String>(8)?,
            "updatedAt": row.get::<_, String>(9)?,
            "latestVersion": row.get::<_, Option<i64>>(10)?,
            "latestReportAt": row.get::<_, Option<String>>(11)?,
        }))
    });
    let rows = match rows {
        Ok(r) => r,
        Err(e) => return db_error("list query", e),
    };
    let items: Vec<JsonValue> = rows.filter_map(|r| r.ok()).collect();
    (
        StatusCode::OK,
        Json(json!({ "success": true, "items": items, "count": items.len() })),
    )
}

/// POST /api/insight-tasks — 创建任务(status=ready)。
/// input_type:前端传则后端校验四选一;留空则后端自动识别(双重保险)。
pub async fn create_task(
    State(state): State<AppState>,
    user_id: UserId,
    Json(req): Json<CreateTaskRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let input_content = req.input_content.trim().to_string();
    if input_content.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "inputContent 不能为空" })),
        );
    }
    // input_type:用户传则校验;否则后端识别
    let input_type = match req.input_type.as_deref() {
        Some(s) if !s.is_empty() => {
            if !is_valid_input_type(s) {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "success": false, "error": format!("invalid inputType: {s}") })),
                );
            }
            s.to_string()
        }
        _ => detect_input_type(&input_content).to_string(),
    };
    // template:可选;传则校验
    if let Some(ref t) = req.template {
        if !is_valid_template(t) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": format!("invalid template: {t}") })),
            );
        }
    }
    let title = if req.title.trim().is_empty() {
        derive_title(&input_content)
    } else {
        req.title.trim().to_string()
    };
    let db = state.db.lock();
    let now = now_rfc3339();
    let res = db.execute(
        "INSERT INTO insight_tasks \
            (user_id, title, input_type, input_content, template, status, feedback_note, source_snapshot, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, 'ready', '', '', ?6, ?6)",
        params![&user_id.0, &title, &input_type, &input_content, &req.template, &now],
    );
    match res {
        Ok(_) => {
            let id = db.last_insert_rowid();
            fetch_task(&db, &user_id.0, id)
        }
        Err(e) => db_error("create insert", e),
    }
}

/// GET /api/insight-tasks/:id — 详情(含最新 report 内嵌)。
pub async fn get_task(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let resp = fetch_task(&db, &user_id.0, id);
    if resp.0 != StatusCode::OK {
        return resp;
    }
    let mut body = resp.1 .0;
    if let Some(item) = body.get_mut("item") {
        let report = fetch_latest_report(&db, id);
        item["report"] = serde_json::to_value(&report).unwrap_or(JsonValue::Null);
    }
    (StatusCode::OK, Json(body))
}

/// PATCH /api/insight-tasks/:id — 改 title / template / feedbackNote。
/// feedback_note 从空变非空 + 当前 status=done → 自动回 ready(spec § 七)。
pub async fn update_task(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
    Json(patch): Json<UpdateTaskRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    // 取当前 status + feedback_note 判断是否触发状态切换
    let cur: Option<(String, String)> = db
        .query_row(
            "SELECT status, feedback_note FROM insight_tasks WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
            params![id, &user_id.0],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .ok();
    let Some((cur_status, cur_feedback)) = cur else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察任务" })),
        );
    };

    let mut sets: Vec<&str> = Vec::new();
    let mut vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(v) = patch.title {
        sets.push("title = ?");
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
    if let Some(v) = patch.feedback_note {
        let new_feedback = v.trim().to_string();
        // 空→非空 且当前 done → 回 ready(用户写反馈)
        if cur_feedback.trim().is_empty() && !new_feedback.is_empty() && cur_status == "done" {
            sets.push("status = 'ready'");
        }
        sets.push("feedback_note = ?");
        vals.push(Box::new(new_feedback));
    }

    if sets.is_empty() {
        return fetch_task(&db, &user_id.0, id);
    }
    sets.push("updated_at = ?");
    vals.push(Box::new(now_rfc3339()));
    vals.push(Box::new(id));
    vals.push(Box::new(user_id.0.clone()));
    let sql = format!(
        "UPDATE insight_tasks SET {} WHERE id = ? AND user_id = ? AND deleted = 0",
        sets.join(", ")
    );
    let params_ref: Vec<&dyn rusqlite::ToSql> = vals.iter().map(|b| &**b).collect();
    match db.execute(&sql, params_ref.as_slice()) {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察任务" })),
        ),
        Ok(_) => fetch_task(&db, &user_id.0, id),
        Err(e) => db_error("update", e),
    }
}

/// DELETE /api/insight-tasks/:id — 软删。
pub async fn delete_task(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let now = now_rfc3339();
    let res = db.execute(
        "UPDATE insight_tasks SET deleted = 1, deleted_at = ?1, updated_at = ?1 \
         WHERE id = ?2 AND user_id = ?3 AND deleted = 0",
        params![&now, id, &user_id.0],
    );
    match res {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察任务" })),
        ),
        Ok(_) => (StatusCode::OK, Json(json!({ "success": true }))),
        Err(e) => db_error("delete", e),
    }
}

/// POST /api/insight-tasks/:id/claim — CC 抢占(ready → processing,原子)。冲突 409。
pub async fn claim_task(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let now = now_rfc3339();
    let n = db.execute(
        "UPDATE insight_tasks SET status = 'processing', updated_at = ?1 \
         WHERE id = ?2 AND user_id = ?3 AND status = 'ready' AND deleted = 0",
        params![&now, id, &user_id.0],
    );
    match n {
        Ok(1) => fetch_task(&db, &user_id.0, id),
        Ok(0) => {
            let cur_status: Option<String> = db
                .query_row(
                    "SELECT status FROM insight_tasks WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
                    params![id, &user_id.0],
                    |r| r.get(0),
                )
                .ok();
            match cur_status {
                None => (
                    StatusCode::NOT_FOUND,
                    Json(json!({ "success": false, "error": "未找到洞察任务" })),
                ),
                Some(s) => (
                    StatusCode::CONFLICT,
                    Json(json!({
                        "success": false,
                        "error": format!("无法 claim:当前状态为 '{s}',只允许在 'ready' 状态抢占"),
                        "currentStatus": s,
                    })),
                ),
            }
        }
        Ok(_) => fetch_task(&db, &user_id.0, id),
        Err(e) => db_error("claim", e),
    }
}

/// POST /api/insight-tasks/:id/release — CC 释放(processing → ready,**feedback_note 保留**)。
pub async fn release_task(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let now = now_rfc3339();
    let n = db.execute(
        "UPDATE insight_tasks SET status = 'ready', updated_at = ?1 \
         WHERE id = ?2 AND user_id = ?3 AND status = 'processing' AND deleted = 0",
        params![&now, id, &user_id.0],
    );
    match n {
        Ok(1) => fetch_task(&db, &user_id.0, id),
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到 processing 状态的洞察任务" })),
        ),
        Ok(_) => fetch_task(&db, &user_id.0, id),
        Err(e) => db_error("release", e),
    }
}

// ============ Reports ============

/// 内部:确认 user 拥有该 task。
fn check_owns_task(db: &Connection, user_id: &str, task_id: i64) -> bool {
    db.query_row(
        "SELECT COUNT(*) > 0 FROM insight_tasks WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
        params![task_id, user_id],
        |r| r.get(0),
    )
    .unwrap_or(false)
}

/// POST /api/insight-tasks/:id/reports — CC 写报告。
/// 副作用(spec § 七 / § 九):version 自增 + status → done + 清空 feedback_note +
/// current_report_id 指向新 report;若 task.template 为空,写入本报告的 template。
pub async fn create_report(
    State(state): State<AppState>,
    user_id: UserId,
    Path(task_id): Path<i64>,
    Json(req): Json<CreateReportRequest>,
) -> (StatusCode, Json<JsonValue>) {
    if !is_valid_template(&req.template) {
        return (
            StatusCode::BAD_REQUEST,
            Json(
                json!({ "success": false, "error": format!("invalid template: {}", req.template) }),
            ),
        );
    }
    let mut db = state.db.lock();
    if !check_owns_task(&db, &user_id.0, task_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察任务" })),
        );
    }
    let now = now_rfc3339();
    let generated_by = if req.generated_by.trim().is_empty() {
        "claude-code".to_string()
    } else {
        req.generated_by.clone()
    };

    let tx = match db.transaction() {
        Ok(t) => t,
        Err(e) => return db_error("report tx begin", e),
    };
    let next_version: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM insight_reports WHERE task_id = ?1",
            params![task_id],
            |r| r.get(0),
        )
        .unwrap_or(1);
    if let Err(e) = tx.execute(
        "INSERT INTO insight_reports \
            (task_id, user_id, version, template, content_md, parent_report_id, revision_note, generated_by, model_used, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            task_id,
            &user_id.0,
            next_version,
            &req.template,
            &req.content_md,
            req.parent_report_id,
            &req.revision_note,
            &generated_by,
            &req.model_used,
            &now,
        ],
    ) {
        return db_error("report insert", e);
    }
    let new_report_id = tx.last_insert_rowid();
    // 副作用:status=done + 清空 feedback_note + current_report_id 指向新版;
    // task.template 为空时落定为本报告 template(spec § 六)。
    if let Err(e) = tx.execute(
        "UPDATE insight_tasks SET status = 'done', feedback_note = '', current_report_id = ?1, \
            template = COALESCE(template, ?2), updated_at = ?3 \
         WHERE id = ?4 AND user_id = ?5",
        params![new_report_id, &req.template, &now, task_id, &user_id.0],
    ) {
        return db_error("report update task", e);
    }
    if let Err(e) = tx.commit() {
        return db_error("report tx commit", e);
    }

    let sql = format!("SELECT {REPORT_COLS} FROM insight_reports WHERE id = ?1");
    match db.query_row(&sql, params![new_report_id], row_to_report) {
        Ok(r) => (StatusCode::OK, Json(json!({ "success": true, "item": r }))),
        Err(e) => db_error("report fetch", e),
    }
}

/// GET /api/insight-tasks/:id/reports — 历史版本(version desc)。
pub async fn list_reports(
    State(state): State<AppState>,
    user_id: UserId,
    Path(task_id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    if !check_owns_task(&db, &user_id.0, task_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察任务" })),
        );
    }
    let sql = format!(
        "SELECT {REPORT_COLS} FROM insight_reports WHERE task_id = ?1 ORDER BY version DESC"
    );
    let mut stmt = match db.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return db_error("list reports prepare", e),
    };
    let rows = match stmt.query_map(params![task_id], row_to_report) {
        Ok(r) => r,
        Err(e) => return db_error("list reports query", e),
    };
    let items: Vec<InsightReport> = rows.filter_map(|r| r.ok()).collect();
    (
        StatusCode::OK,
        Json(json!({ "success": true, "items": items, "count": items.len() })),
    )
}

/// GET /api/insight-tasks/:id/reports/latest — 最新版报告。
pub async fn get_latest_report(
    State(state): State<AppState>,
    user_id: UserId,
    Path(task_id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    if !check_owns_task(&db, &user_id.0, task_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察任务" })),
        );
    }
    match fetch_latest_report(&db, task_id) {
        Some(r) => (StatusCode::OK, Json(json!({ "success": true, "item": r }))),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "尚无报告版本" })),
        ),
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
        let body = to_bytes(resp.into_body(), 2_000_000).await.unwrap();
        let body_str = String::from_utf8_lossy(&body);
        serde_json::from_slice(&body)
            .unwrap_or_else(|e| panic!("parse fail status={status} body={body_str:?} err={e}"))
    }

    async fn create_task_req(app: &axum::Router, cookie: &str, body: &str) -> JsonValue {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/insight-tasks")
                    .header("Cookie", cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        body_json(resp).await
    }

    #[tokio::test]
    async fn create_detects_input_type() {
        let state = test_state();
        let (_u, token) = create_test_user(&state, "u-detect", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        // URL
        let j = create_task_req(
            &app,
            &cookie,
            r#"{"inputContent":"https://example.com/post"}"#,
        )
        .await;
        assert_eq!(j["item"]["inputType"], "url");
        assert_eq!(j["item"]["status"], "ready");

        // 短文本 → topic
        let j = create_task_req(&app, &cookie, r#"{"inputContent":"Gemini 是啥"}"#).await;
        assert_eq!(j["item"]["inputType"], "topic");

        // 长指令 → prompt
        let long_prompt = "帮我整理一下 Anthropic constitutional AI 的核心思想给我老板看顺便对比 OpenAI 的方法以及它们在对齐上的差异和取舍要点尽量详细一些谢谢";
        let j = create_task_req(
            &app,
            &cookie,
            &format!(r#"{{"inputContent":"{long_prompt}"}}"#),
        )
        .await;
        assert_eq!(j["item"]["inputType"], "prompt");
    }

    #[tokio::test]
    async fn invalid_input_type_rejected() {
        let state = test_state();
        let (_u, token) = create_test_user(&state, "u-inv", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/insight-tasks")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"inputContent":"x","inputType":"nonsense"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn claim_only_works_on_ready() {
        let state = test_state();
        let (_u, token) = create_test_user(&state, "u-claim", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let j = create_task_req(&app, &cookie, r#"{"inputContent":"主题甲"}"#).await;
        let id = j["item"]["id"].as_i64().unwrap();

        // claim 1 → 200 processing
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insight-tasks/{id}/claim"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["status"], "processing");

        // claim 2 → 409
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insight-tasks/{id}/claim"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn report_version_increments_and_clears_feedback() {
        let state = test_state();
        let (_u, token) = create_test_user(&state, "u-rep", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let j = create_task_req(&app, &cookie, r#"{"inputContent":"研究 X"}"#).await;
        let id = j["item"]["id"].as_i64().unwrap();

        // 写 v1 → status done, template 落定
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insight-tasks/{id}/reports"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"template":"survey","contentMd":"报告 v1"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["version"], 1);

        // task 现在 done + template=survey
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/insight-tasks/{id}"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["status"], "done");
        assert_eq!(j["item"]["template"], "survey");
        assert_eq!(j["item"]["report"]["contentMd"], "报告 v1");

        // 写反馈 → 回 ready
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/insight-tasks/{id}"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"feedbackNote":"再加个反例"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["status"], "ready");
        assert_eq!(j["item"]["feedbackNote"], "再加个反例");

        // 写 v2 → version 2 + feedback 清空 + done
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insight-tasks/{id}/reports"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"template":"survey","contentMd":"报告 v2","revisionNote":"再加个反例"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["version"], 2);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/insight-tasks/{id}"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["status"], "done");
        assert_eq!(j["item"]["feedbackNote"], "");
    }

    #[tokio::test]
    async fn feedback_does_not_flip_when_not_done() {
        let state = test_state();
        let (_u, token) = create_test_user(&state, "u-fb", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        // 新建 ready 任务,直接写反馈 → 仍 ready(只有 done→ready 才翻转)
        let j = create_task_req(&app, &cookie, r#"{"inputContent":"主题乙"}"#).await;
        let id = j["item"]["id"].as_i64().unwrap();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/insight-tasks/{id}"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"feedbackNote":"提前写点想法"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["status"], "ready");
        assert_eq!(j["item"]["feedbackNote"], "提前写点想法");
    }

    #[tokio::test]
    async fn user_isolation() {
        let state = test_state();
        let (_ua, token_a) = create_test_user(&state, "iso-a", "Pa55word1");
        let (_ub, token_b) = create_test_user(&state, "iso-b", "Pa55word1");
        let cookie_a = auth_cookie(&token_a);
        let cookie_b = auth_cookie(&token_b);
        let app = crate::build_app(state);

        let j = create_task_req(&app, &cookie_a, r#"{"inputContent":"alice 的秘密"}"#).await;
        let id = j["item"]["id"].as_i64().unwrap();

        // bob 看不到详情
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/insight-tasks/{id}"))
                    .header("Cookie", &cookie_b)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        // bob 列表为空
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/insight-tasks")
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
