//! `/api/insight-factory/*` — isolated Insight Factory P0 API (T-204).

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use tracing::error;

use crate::auth::UserId;
use crate::models::factory::{
    derive_title, detect_input_type, is_valid_input_type, is_valid_memory_type, is_valid_template,
    CreateFactoryReportRequest, CreateFactoryTaskRequest, CreateJobRequest, CreateMemoryRequest,
    FactoryJob, FactoryMemory, FactoryReport, FactoryTask, FeedbackRequest,
    UpdateFactoryTaskRequest, UpdateMemoryRequest,
};
use crate::state::AppState;

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn db_error(ctx: &str, e: rusqlite::Error) -> (StatusCode, Json<JsonValue>) {
    error!(target: "insight_factory", "{} db error: {}", ctx, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "error": "内部错误" })),
    )
}

fn conflict_active_job() -> (StatusCode, Json<JsonValue>) {
    (
        StatusCode::CONFLICT,
        Json(json!({ "success": false, "error": "该任务已有 active job" })),
    )
}

fn normalize_provider(provider: Option<String>) -> String {
    provider
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "codex".to_string())
}

const TASK_COLS: &str = "id, title, input_type, input_content, template, status, \
    current_report_id, source_snapshot, created_at, updated_at";

fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<FactoryTask> {
    Ok(FactoryTask {
        id: row.get(0)?,
        title: row.get(1)?,
        input_type: row.get(2)?,
        input_content: row.get(3)?,
        template: row.get(4)?,
        status: row.get(5)?,
        current_report_id: row.get(6)?,
        source_snapshot: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

const JOB_COLS: &str = "id, task_id, mode, status, provider, template, feedback_note, \
    parent_report_id, retry_of_job_id, error_message, started_at, finished_at, created_at";

fn row_to_job(row: &rusqlite::Row) -> rusqlite::Result<FactoryJob> {
    Ok(FactoryJob {
        id: row.get(0)?,
        task_id: row.get(1)?,
        mode: row.get(2)?,
        status: row.get(3)?,
        provider: row.get(4)?,
        template: row.get(5)?,
        feedback_note: row.get(6)?,
        parent_report_id: row.get(7)?,
        retry_of_job_id: row.get(8)?,
        error_message: row.get(9)?,
        started_at: row.get(10)?,
        finished_at: row.get(11)?,
        created_at: row.get(12)?,
    })
}

const REPORT_COLS: &str = "id, task_id, job_id, version, template, content_md, parent_report_id, \
    revision_note, generated_by, provider, model_used, created_at";

fn row_to_report(row: &rusqlite::Row) -> rusqlite::Result<FactoryReport> {
    Ok(FactoryReport {
        id: row.get(0)?,
        task_id: row.get(1)?,
        job_id: row.get(2)?,
        version: row.get(3)?,
        template: row.get(4)?,
        content_md: row.get(5)?,
        parent_report_id: row.get(6)?,
        revision_note: row.get(7)?,
        generated_by: row.get(8)?,
        provider: row.get(9)?,
        model_used: row.get(10)?,
        created_at: row.get(11)?,
    })
}

fn row_to_memory(row: &rusqlite::Row) -> rusqlite::Result<FactoryMemory> {
    let enabled: i64 = row.get(6)?;
    Ok(FactoryMemory {
        id: row.get(0)?,
        memory_type: row.get(1)?,
        title: row.get(2)?,
        body: row.get(3)?,
        source: row.get(4)?,
        source_ref: row.get(5)?,
        enabled: enabled != 0,
        importance: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn fetch_task(db: &Connection, user_id: &str, id: i64) -> (StatusCode, Json<JsonValue>) {
    let sql = format!(
        "SELECT {TASK_COLS} FROM factory_tasks WHERE id = ?1 AND user_id = ?2 AND deleted = 0"
    );
    match db.query_row(&sql, params![id, user_id], row_to_task) {
        Ok(t) => (StatusCode::OK, Json(json!({ "success": true, "item": t }))),
        Err(rusqlite::Error::QueryReturnedNoRows) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察工厂任务" })),
        ),
        Err(e) => db_error("fetch_task", e),
    }
}

fn owns_task(db: &Connection, user_id: &str, task_id: i64) -> bool {
    db.query_row(
        "SELECT COUNT(*) > 0 FROM factory_tasks WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
        params![task_id, user_id],
        |r| r.get(0),
    )
    .unwrap_or(false)
}

fn fetch_latest_report(db: &Connection, task_id: i64) -> Option<FactoryReport> {
    let sql =
        format!("SELECT {REPORT_COLS} FROM factory_reports WHERE task_id = ?1 ORDER BY version DESC LIMIT 1");
    db.query_row(&sql, params![task_id], row_to_report).ok()
}

fn fetch_active_job(db: &Connection, task_id: i64) -> Option<FactoryJob> {
    let sql = format!(
        "SELECT {JOB_COLS} FROM factory_jobs WHERE task_id = ?1 AND status IN ('pending', 'running') ORDER BY id DESC LIMIT 1"
    );
    db.query_row(&sql, params![task_id], row_to_job).ok()
}

fn insert_job(
    db: &Connection,
    user_id: &str,
    task_id: i64,
    mode: &str,
    provider: &str,
    template: Option<String>,
    feedback_note: &str,
    parent_report_id: Option<i64>,
    retry_of_job_id: Option<i64>,
) -> Result<i64, rusqlite::Error> {
    let now = now_rfc3339();
    db.execute(
        "INSERT INTO factory_jobs
            (task_id, user_id, mode, status, provider, template, feedback_note, parent_report_id, retry_of_job_id, created_at)
         VALUES (?1, ?2, ?3, 'pending', ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            task_id,
            user_id,
            mode,
            provider,
            template,
            feedback_note,
            parent_report_id,
            retry_of_job_id,
            now,
        ],
    )?;
    Ok(db.last_insert_rowid())
}

fn fetch_job_by_id(db: &Connection, user_id: &str, id: i64) -> (StatusCode, Json<JsonValue>) {
    let sql = format!("SELECT {JOB_COLS} FROM factory_jobs WHERE id = ?1 AND user_id = ?2");
    match db.query_row(&sql, params![id, user_id], row_to_job) {
        Ok(j) => (StatusCode::OK, Json(json!({ "success": true, "item": j }))),
        Err(rusqlite::Error::QueryReturnedNoRows) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察工厂 job" })),
        ),
        Err(e) => db_error("fetch_job", e),
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct ListTasksQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
}

pub async fn list_tasks(
    State(state): State<AppState>,
    user_id: UserId,
    Query(q): Query<ListTasksQuery>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let limit = q.limit.unwrap_or(0).clamp(0, 200);
    let limit_sql = if limit > 0 {
        format!(" LIMIT {limit}")
    } else {
        String::new()
    };
    let status_filter = match q.status.as_deref() {
        Some(s) if !s.is_empty() => format!(" AND t.status = '{}'", s.replace('\'', "")),
        _ => String::new(),
    };
    let sql = format!(
        "SELECT t.id, t.title, t.input_type, t.input_content, t.template, t.status,
                t.current_report_id, t.source_snapshot, t.created_at, t.updated_at,
                (SELECT MAX(version) FROM factory_reports r WHERE r.task_id = t.id) AS latest_version,
                (SELECT MAX(created_at) FROM factory_reports r WHERE r.task_id = t.id) AS latest_report_at
         FROM factory_tasks t
         WHERE t.user_id = ?1 AND t.deleted = 0{status_filter}
         ORDER BY t.updated_at DESC{limit_sql}"
    );
    let mut stmt = match db.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return db_error("list_tasks prepare", e),
    };
    let rows = match stmt.query_map(params![user_id.0], |row| {
        Ok(json!({
            "id": row.get::<_, i64>(0)?,
            "title": row.get::<_, String>(1)?,
            "inputType": row.get::<_, String>(2)?,
            "inputContent": row.get::<_, String>(3)?,
            "template": row.get::<_, Option<String>>(4)?,
            "status": row.get::<_, String>(5)?,
            "currentReportId": row.get::<_, Option<i64>>(6)?,
            "sourceSnapshot": row.get::<_, String>(7)?,
            "createdAt": row.get::<_, String>(8)?,
            "updatedAt": row.get::<_, String>(9)?,
            "latestVersion": row.get::<_, Option<i64>>(10)?,
            "latestReportAt": row.get::<_, Option<String>>(11)?,
        }))
    }) {
        Ok(r) => r,
        Err(e) => return db_error("list_tasks query", e),
    };
    let items: Vec<JsonValue> = rows.filter_map(|r| r.ok()).collect();
    (
        StatusCode::OK,
        Json(json!({ "success": true, "items": items, "count": items.len() })),
    )
}

pub async fn create_task(
    State(state): State<AppState>,
    user_id: UserId,
    Json(req): Json<CreateFactoryTaskRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let input_content = req.input_content.trim().to_string();
    if input_content.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "inputContent 不能为空" })),
        );
    }
    let input_type = match req.input_type.as_deref() {
        Some(s) if !s.is_empty() => {
            if !is_valid_input_type(s) {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "success": false, "error": "invalid inputType" })),
                );
            }
            s.to_string()
        }
        _ => detect_input_type(&input_content).to_string(),
    };
    if let Some(ref t) = req.template {
        if !is_valid_template(t) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": "invalid template" })),
            );
        }
    }
    let title = if req.title.trim().is_empty() {
        derive_title(&input_content)
    } else {
        req.title.trim().to_string()
    };
    let mut db = state.db.lock();
    let tx = match db.transaction() {
        Ok(t) => t,
        Err(e) => return db_error("create_task tx", e),
    };
    let now = now_rfc3339();
    let status = if req.create_generate_job {
        "pending"
    } else {
        "idle"
    };
    if let Err(e) = tx.execute(
        "INSERT INTO factory_tasks
            (user_id, title, input_type, input_content, template, status, source_snapshot, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![&user_id.0, title, input_type, input_content, req.template, status, req.source_snapshot, now],
    ) {
        return db_error("create_task insert", e);
    }
    let task_id = tx.last_insert_rowid();
    let mut job_id = None;
    if req.create_generate_job {
        if let Err(e) = tx.execute(
            "INSERT INTO factory_jobs
                (task_id, user_id, mode, status, provider, template, created_at)
             VALUES (?1, ?2, 'generate', 'pending', ?3, ?4, ?5)",
            params![
                task_id,
                &user_id.0,
                normalize_provider(req.provider),
                req.template,
                now
            ],
        ) {
            return db_error("create_task job", e);
        }
        job_id = Some(tx.last_insert_rowid());
    }
    if let Err(e) = tx.commit() {
        return db_error("create_task commit", e);
    }
    let resp = fetch_task(&db, &user_id.0, task_id);
    let mut body = resp.1 .0;
    if let Some(id) = job_id {
        body["jobId"] = json!(id);
    }
    (resp.0, Json(body))
}

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
        item["latestReport"] =
            serde_json::to_value(fetch_latest_report(&db, id)).unwrap_or(JsonValue::Null);
        item["activeJob"] =
            serde_json::to_value(fetch_active_job(&db, id)).unwrap_or(JsonValue::Null);
    }
    (StatusCode::OK, Json(body))
}

pub async fn update_task(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
    Json(patch): Json<UpdateFactoryTaskRequest>,
) -> (StatusCode, Json<JsonValue>) {
    if let Some(ref t) = patch.template {
        if !is_valid_template(t) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": "invalid template" })),
            );
        }
    }
    let db = state.db.lock();
    let mut sets: Vec<&str> = Vec::new();
    let mut vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(v) = patch.title {
        sets.push("title = ?");
        vals.push(Box::new(v.trim().to_string()));
    }
    if let Some(v) = patch.template {
        sets.push("template = ?");
        vals.push(Box::new(v));
    }
    if sets.is_empty() {
        return fetch_task(&db, &user_id.0, id);
    }
    sets.push("updated_at = ?");
    vals.push(Box::new(now_rfc3339()));
    vals.push(Box::new(id));
    vals.push(Box::new(user_id.0.clone()));
    let sql = format!(
        "UPDATE factory_tasks SET {} WHERE id = ? AND user_id = ? AND deleted = 0",
        sets.join(", ")
    );
    let params_ref: Vec<&dyn rusqlite::ToSql> = vals.iter().map(|b| &**b).collect();
    match db.execute(&sql, params_ref.as_slice()) {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察工厂任务" })),
        ),
        Ok(_) => fetch_task(&db, &user_id.0, id),
        Err(e) => db_error("update_task", e),
    }
}

pub async fn delete_task(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let now = now_rfc3339();
    match db.execute(
        "UPDATE factory_tasks SET deleted = 1, deleted_at = ?1, updated_at = ?1 WHERE id = ?2 AND user_id = ?3 AND deleted = 0",
        params![now, id, user_id.0],
    ) {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({ "success": false, "error": "未找到洞察工厂任务" }))),
        Ok(_) => (StatusCode::OK, Json(json!({ "success": true }))),
        Err(e) => db_error("delete_task", e),
    }
}

pub async fn generate_task(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
    Json(req): Json<CreateJobRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    if !owns_task(&db, &user_id.0, id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察工厂任务" })),
        );
    }
    let template: Option<String> = db
        .query_row(
            "SELECT template FROM factory_tasks WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
            params![id, &user_id.0],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    match insert_job(
        &db,
        &user_id.0,
        id,
        "generate",
        &normalize_provider(req.provider),
        template,
        "",
        None,
        None,
    ) {
        Ok(job_id) => {
            let now = now_rfc3339();
            if let Err(e) = db.execute(
                "UPDATE factory_tasks SET status = 'pending', updated_at = ?1 WHERE id = ?2 AND user_id = ?3",
                params![now, id, &user_id.0],
            ) {
                return db_error("generate update task", e);
            }
            fetch_job_by_id(&db, &user_id.0, job_id)
        }
        Err(rusqlite::Error::SqliteFailure(_, _)) => conflict_active_job(),
        Err(e) => db_error("generate insert", e),
    }
}

pub async fn feedback_task(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
    Json(req): Json<FeedbackRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let feedback = req.feedback_note.trim().to_string();
    if feedback.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "feedbackNote 不能为空" })),
        );
    }
    let db = state.db.lock();
    let cur: Option<(Option<String>, Option<i64>)> = db
        .query_row(
            "SELECT template, current_report_id FROM factory_tasks WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
            params![id, &user_id.0],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let Some((template, parent_report_id)) = cur else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察工厂任务" })),
        );
    };
    match insert_job(
        &db,
        &user_id.0,
        id,
        "revise",
        &normalize_provider(req.provider),
        template,
        &feedback,
        parent_report_id,
        None,
    ) {
        Ok(job_id) => {
            let now = now_rfc3339();
            if let Err(e) = db.execute(
                "UPDATE factory_tasks SET status = 'pending', updated_at = ?1 WHERE id = ?2 AND user_id = ?3",
                params![now, id, &user_id.0],
            ) {
                return db_error("feedback update task", e);
            }
            fetch_job_by_id(&db, &user_id.0, job_id)
        }
        Err(rusqlite::Error::SqliteFailure(_, _)) => conflict_active_job(),
        Err(e) => db_error("feedback insert", e),
    }
}

pub async fn list_jobs(
    State(state): State<AppState>,
    user_id: UserId,
    Path(task_id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    if !owns_task(&db, &user_id.0, task_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察工厂任务" })),
        );
    }
    let sql = format!("SELECT {JOB_COLS} FROM factory_jobs WHERE task_id = ?1 AND user_id = ?2 ORDER BY created_at DESC");
    let mut stmt = match db.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return db_error("list_jobs prepare", e),
    };
    let rows = match stmt.query_map(params![task_id, &user_id.0], row_to_job) {
        Ok(r) => r,
        Err(e) => return db_error("list_jobs query", e),
    };
    let items: Vec<FactoryJob> = rows.filter_map(|r| r.ok()).collect();
    (
        StatusCode::OK,
        Json(json!({ "success": true, "items": items, "count": items.len() })),
    )
}

pub async fn list_reports(
    State(state): State<AppState>,
    user_id: UserId,
    Path(task_id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    if !owns_task(&db, &user_id.0, task_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察工厂任务" })),
        );
    }
    let sql = format!("SELECT {REPORT_COLS} FROM factory_reports WHERE task_id = ?1 AND user_id = ?2 ORDER BY version DESC");
    let mut stmt = match db.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return db_error("list_reports prepare", e),
    };
    let rows = match stmt.query_map(params![task_id, &user_id.0], row_to_report) {
        Ok(r) => r,
        Err(e) => return db_error("list_reports query", e),
    };
    let items: Vec<FactoryReport> = rows.filter_map(|r| r.ok()).collect();
    (
        StatusCode::OK,
        Json(json!({ "success": true, "items": items, "count": items.len() })),
    )
}

pub async fn latest_report(
    State(state): State<AppState>,
    user_id: UserId,
    Path(task_id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    if !owns_task(&db, &user_id.0, task_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察工厂任务" })),
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

pub async fn create_report(
    State(state): State<AppState>,
    user_id: UserId,
    Path(task_id): Path<i64>,
    Json(req): Json<CreateFactoryReportRequest>,
) -> (StatusCode, Json<JsonValue>) {
    if !is_valid_template(&req.template) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "invalid template" })),
        );
    }
    let mut db = state.db.lock();
    if !owns_task(&db, &user_id.0, task_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察工厂任务" })),
        );
    }
    let job: Option<(String, String)> = match db
        .query_row(
            "SELECT status, mode FROM factory_jobs WHERE id = ?1 AND task_id = ?2 AND user_id = ?3",
            params![req.job_id, task_id, &user_id.0],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
    {
        Ok(v) => v,
        Err(e) => return db_error("create_report job lookup", e),
    };
    let Some((_job_status, _mode)) = job else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察工厂 job" })),
        );
    };
    let tx = match db.transaction() {
        Ok(t) => t,
        Err(e) => return db_error("create_report tx", e),
    };
    let now = now_rfc3339();
    let next_version: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM factory_reports WHERE task_id = ?1",
            params![task_id],
            |r| r.get(0),
        )
        .unwrap_or(1);
    let generated_by = if req.generated_by.trim().is_empty() {
        "factory-worker".to_string()
    } else {
        req.generated_by
    };
    let provider = if req.provider.trim().is_empty() {
        "codex".to_string()
    } else {
        req.provider
    };
    if let Err(e) = tx.execute(
        "INSERT INTO factory_reports
            (task_id, job_id, user_id, version, template, content_md, parent_report_id, revision_note, generated_by, provider, model_used, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            task_id,
            req.job_id,
            &user_id.0,
            next_version,
            req.template,
            req.content_md,
            req.parent_report_id,
            req.revision_note,
            generated_by,
            provider,
            req.model_used,
            now,
        ],
    ) {
        return db_error("create_report insert", e);
    }
    let report_id = tx.last_insert_rowid();
    if let Err(e) = tx.execute(
        "UPDATE factory_jobs SET status = 'done', finished_at = ?1 WHERE id = ?2 AND user_id = ?3",
        params![now, req.job_id, &user_id.0],
    ) {
        return db_error("create_report job done", e);
    }
    if let Err(e) = tx.execute(
        "UPDATE factory_tasks SET status = 'done', current_report_id = ?1, template = COALESCE(template, ?2), updated_at = ?3 WHERE id = ?4 AND user_id = ?5",
        params![report_id, req.template, now, task_id, &user_id.0],
    ) {
        return db_error("create_report task done", e);
    }
    if let Err(e) = tx.commit() {
        return db_error("create_report commit", e);
    }
    let sql = format!("SELECT {REPORT_COLS} FROM factory_reports WHERE id = ?1");
    match db.query_row(&sql, params![report_id], row_to_report) {
        Ok(r) => (StatusCode::OK, Json(json!({ "success": true, "item": r }))),
        Err(e) => db_error("create_report fetch", e),
    }
}

pub async fn retry_job(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let job: Option<FactoryJob> = match db
        .query_row(
            &format!("SELECT {JOB_COLS} FROM factory_jobs WHERE id = ?1 AND user_id = ?2"),
            params![id, &user_id.0],
            row_to_job,
        )
        .optional()
    {
        Ok(v) => v,
        Err(e) => return db_error("retry lookup", e),
    };
    let Some(job) = job else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察工厂 job" })),
        );
    };
    if job.status != "failed" && job.status != "blocked" {
        return (
            StatusCode::CONFLICT,
            Json(json!({ "success": false, "error": "仅 failed/blocked job 可 retry" })),
        );
    }
    match insert_job(
        &db,
        &user_id.0,
        job.task_id,
        "retry",
        &job.provider,
        job.template,
        &job.feedback_note,
        job.parent_report_id,
        Some(job.id),
    ) {
        Ok(new_id) => {
            let now = now_rfc3339();
            if let Err(e) = db.execute(
                "UPDATE factory_tasks SET status = 'pending', updated_at = ?1 WHERE id = ?2 AND user_id = ?3",
                params![now, job.task_id, &user_id.0],
            ) {
                return db_error("retry update task", e);
            }
            fetch_job_by_id(&db, &user_id.0, new_id)
        }
        Err(rusqlite::Error::SqliteFailure(_, _)) => conflict_active_job(),
        Err(e) => db_error("retry insert", e),
    }
}

pub async fn worker_health(user_id: UserId) -> (StatusCode, Json<JsonValue>) {
    let _ = user_id;
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "provider": "codex",
            "status": "placeholder",
            "quotaGate": "not_checked",
            "apiKeyFallback": false
        })),
    )
}

#[derive(Debug, Default, Deserialize)]
pub struct MemoryQuery {
    #[serde(rename = "type")]
    pub memory_type: Option<String>,
    pub enabled: Option<bool>,
}

pub async fn list_memories(
    State(state): State<AppState>,
    user_id: UserId,
    Query(q): Query<MemoryQuery>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let type_filter = match q.memory_type.as_deref() {
        Some(t) if !t.is_empty() => format!(" AND type = '{}'", t.replace('\'', "")),
        _ => String::new(),
    };
    let enabled_filter = match q.enabled {
        Some(true) => " AND enabled = 1".to_string(),
        Some(false) => " AND enabled = 0".to_string(),
        None => String::new(),
    };
    let sql = format!(
        "SELECT id, type, title, body, source, source_ref, enabled, importance, created_at, updated_at
         FROM factory_memories WHERE user_id = ?1{type_filter}{enabled_filter} ORDER BY updated_at DESC"
    );
    let mut stmt = match db.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return db_error("list_memories prepare", e),
    };
    let rows = match stmt.query_map(params![user_id.0], row_to_memory) {
        Ok(r) => r,
        Err(e) => return db_error("list_memories query", e),
    };
    let items: Vec<FactoryMemory> = rows.filter_map(|r| r.ok()).collect();
    (
        StatusCode::OK,
        Json(json!({ "success": true, "items": items, "count": items.len() })),
    )
}

pub async fn create_memory(
    State(state): State<AppState>,
    user_id: UserId,
    Json(req): Json<CreateMemoryRequest>,
) -> (StatusCode, Json<JsonValue>) {
    if !is_valid_memory_type(&req.memory_type) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "invalid memory type" })),
        );
    }
    if req.title.trim().is_empty() || req.body.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "title/body 不能为空" })),
        );
    }
    let db = state.db.lock();
    let now = now_rfc3339();
    let importance = req.importance.unwrap_or(3).clamp(1, 5);
    let enabled = if req.enabled.unwrap_or(true) { 1 } else { 0 };
    if let Err(e) = db.execute(
        "INSERT INTO factory_memories
            (user_id, type, title, body, source, source_ref, importance, enabled, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        params![user_id.0, req.memory_type, req.title, req.body, req.source, req.source_ref, importance, enabled, now],
    ) {
        return db_error("create_memory", e);
    }
    fetch_memory(&db, &user_id.0, db.last_insert_rowid())
}

fn fetch_memory(db: &Connection, user_id: &str, id: i64) -> (StatusCode, Json<JsonValue>) {
    match db.query_row(
        "SELECT id, type, title, body, source, source_ref, enabled, importance, created_at, updated_at
         FROM factory_memories WHERE id = ?1 AND user_id = ?2",
        params![id, user_id],
        row_to_memory,
    ) {
        Ok(m) => (StatusCode::OK, Json(json!({ "success": true, "item": m }))),
        Err(rusqlite::Error::QueryReturnedNoRows) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察工厂记忆" })),
        ),
        Err(e) => db_error("fetch_memory", e),
    }
}

pub async fn update_memory(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
    Json(patch): Json<UpdateMemoryRequest>,
) -> (StatusCode, Json<JsonValue>) {
    if let Some(ref t) = patch.memory_type {
        if !is_valid_memory_type(t) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": "invalid memory type" })),
            );
        }
    }
    let db = state.db.lock();
    let mut sets: Vec<&str> = Vec::new();
    let mut vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    macro_rules! set_text {
        ($field:expr, $value:expr) => {
            if let Some(v) = $value {
                sets.push($field);
                vals.push(Box::new(v));
            }
        };
    }
    set_text!("type = ?", patch.memory_type);
    set_text!("title = ?", patch.title);
    set_text!("body = ?", patch.body);
    set_text!("source = ?", patch.source);
    set_text!("source_ref = ?", patch.source_ref);
    if let Some(v) = patch.importance {
        sets.push("importance = ?");
        vals.push(Box::new(v.clamp(1, 5)));
    }
    if let Some(v) = patch.enabled {
        sets.push("enabled = ?");
        vals.push(Box::new(if v { 1 } else { 0 }));
    }
    if sets.is_empty() {
        return fetch_memory(&db, &user_id.0, id);
    }
    sets.push("updated_at = ?");
    vals.push(Box::new(now_rfc3339()));
    vals.push(Box::new(id));
    vals.push(Box::new(user_id.0.clone()));
    let sql = format!(
        "UPDATE factory_memories SET {} WHERE id = ? AND user_id = ?",
        sets.join(", ")
    );
    let params_ref: Vec<&dyn rusqlite::ToSql> = vals.iter().map(|b| &**b).collect();
    match db.execute(&sql, params_ref.as_slice()) {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察工厂记忆" })),
        ),
        Ok(_) => fetch_memory(&db, &user_id.0, id),
        Err(e) => db_error("update_memory", e),
    }
}

pub async fn delete_memory(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match db.execute(
        "DELETE FROM factory_memories WHERE id = ?1 AND user_id = ?2",
        params![id, user_id.0],
    ) {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察工厂记忆" })),
        ),
        Ok(_) => (StatusCode::OK, Json(json!({ "success": true }))),
        Err(e) => db_error("delete_memory", e),
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

    async fn create_factory_task(app: &axum::Router, cookie: &str, body: &str) -> JsonValue {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/insight-factory/tasks")
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
    async fn create_task_can_create_generate_job_and_blocks_duplicate_active_job() {
        let state = test_state();
        let (_u, token) = create_test_user(&state, "factory-generate", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let j = create_factory_task(
            &app,
            &cookie,
            r#"{"inputContent":"https://example.com/a","createGenerateJob":true}"#,
        )
        .await;
        assert_eq!(j["item"]["inputType"], "url");
        assert_eq!(j["item"]["status"], "pending");
        assert!(j["jobId"].as_i64().is_some());
        let id = j["item"]["id"].as_i64().unwrap();

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insight-factory/tasks/{id}/generate"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn report_write_versions_and_feedback_creates_revise_job() {
        let state = test_state();
        let (_u, token) = create_test_user(&state, "factory-report", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let j = create_factory_task(
            &app,
            &cookie,
            r#"{"inputContent":"研究 AI 搜索","template":"survey","createGenerateJob":true}"#,
        )
        .await;
        let task_id = j["item"]["id"].as_i64().unwrap();
        let job_id = j["jobId"].as_i64().unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insight-factory/tasks/{task_id}/reports"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"jobId":{job_id},"template":"survey","contentMd":"报告 v1","provider":"codex","modelUsed":"codex-cli"}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["version"], 1);
        assert_eq!(j["item"]["contentMd"], "报告 v1");
        let parent_report_id = j["item"]["id"].as_i64().unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/insight-factory/tasks/{task_id}"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["status"], "done");
        assert_eq!(j["item"]["currentReportId"], parent_report_id);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insight-factory/tasks/{task_id}/feedback"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"feedbackNote":"补充政策风险"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["mode"], "revise");
        assert_eq!(j["item"]["feedbackNote"], "补充政策风险");
        assert_eq!(j["item"]["parentReportId"], parent_report_id);
    }

    #[tokio::test]
    async fn retry_failed_job_creates_new_job_without_reusing_original() {
        let state = test_state();
        let (_u, token) = create_test_user(&state, "factory-retry", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state.clone());

        let j = create_factory_task(
            &app,
            &cookie,
            r#"{"inputContent":"重试测试","createGenerateJob":true}"#,
        )
        .await;
        let task_id = j["item"]["id"].as_i64().unwrap();
        let job_id = j["jobId"].as_i64().unwrap();
        {
            let db = state.db.lock();
            db.execute(
                "UPDATE factory_jobs SET status = 'failed', error_message = 'boom' WHERE id = ?1",
                params![job_id],
            )
            .unwrap();
            db.execute(
                "UPDATE factory_tasks SET status = 'failed' WHERE id = ?1",
                params![task_id],
            )
            .unwrap();
        }

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insight-factory/jobs/{job_id}/retry"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["mode"], "retry");
        assert_eq!(j["item"]["retryOfJobId"], job_id);
        assert_ne!(j["item"]["id"], job_id);
    }

    #[tokio::test]
    async fn memories_crud_and_user_isolation() {
        let state = test_state();
        let (_ua, token_a) = create_test_user(&state, "factory-mem-a", "Pa55word1");
        let (_ub, token_b) = create_test_user(&state, "factory-mem-b", "Pa55word1");
        let cookie_a = auth_cookie(&token_a);
        let cookie_b = auth_cookie(&token_b);
        let app = crate::build_app(state);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/insight-factory/memories")
                    .header("Cookie", &cookie_a)
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"type":"project_fact","title":"事实","body":"只属于 A","importance":5}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["type"], "project_fact");
        let id = j["item"]["id"].as_i64().unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/insight-factory/memories/{id}"))
                    .header("Cookie", &cookie_a)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"enabled":false}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["enabled"], false);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/insight-factory/memories")
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
