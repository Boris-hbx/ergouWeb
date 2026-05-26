//! `/api/work/tasks` — CRUD for work tasks (independent of personal todos).
//!
//! See SPEC: `C:\Project\ergouPM\specs\work-task-table\spec.md`
//! Task ticket: T-094

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::{json, Map, Value as JsonValue};
use tracing::{error, warn};

use crate::auth::UserId;
use crate::models::work_task::{CreateWorkTaskRequest, UpdateWorkTaskRequest, WorkTask};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct TasksResponse {
    pub success: bool,
    pub items: Vec<WorkTask>,
}

#[derive(Debug, Serialize)]
pub struct TaskResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<WorkTask>,
}

#[derive(Debug, Serialize)]
pub struct SimpleResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

fn db_error(ctx: &str, e: rusqlite::Error) -> (StatusCode, Json<JsonValue>) {
    error!(target: "work_tasks", "{} db error: {}", ctx, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "error": "内部错误" })),
    )
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

/// Parse a `custom_fields` JSON column from DB into a serde Map. Tolerates malformed
/// data by returning empty (with a warning).
fn parse_custom_fields(raw: &str) -> Map<String, JsonValue> {
    match serde_json::from_str::<JsonValue>(raw) {
        Ok(JsonValue::Object(m)) => m,
        Ok(_) => {
            warn!(target: "work_tasks", "custom_fields not an object: {}", raw);
            Map::new()
        }
        Err(e) => {
            warn!(target: "work_tasks", "custom_fields parse error: {} for {}", e, raw);
            Map::new()
        }
    }
}

/// Map a SELECT row to a `WorkTask`. Column order MUST match the SELECT statement
/// used by all readers in this file (see `SELECT_COLS`).
fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<WorkTask> {
    let custom_raw: String = row.get(11)?;
    Ok(WorkTask {
        id: row.get(0)?,
        title: row.get(1)?,
        desc: row.get(2)?,
        assignee: row.get(3)?,
        level: row.get(4)?,
        freq: row.get(5)?,
        status: row.get(6)?,
        priority: row.get(7)?,
        due_date: row.get(8)?,
        progress: row.get(9)?,
        sort_order: row.get(10)?,
        custom_fields: parse_custom_fields(&custom_raw),
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

const SELECT_COLS: &str =
    "id, title, desc, assignee, level, freq, status, priority, due_date, progress, sort_order, custom_fields, created_at, updated_at";

/// GET /api/work/tasks
pub async fn list_tasks(
    State(state): State<AppState>,
    user_id: UserId,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let sql = format!(
        "SELECT {SELECT_COLS} FROM work_tasks \
         WHERE user_id = ?1 AND deleted = 0 \
         ORDER BY sort_order ASC, created_at ASC"
    );
    let mut stmt = match db.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return db_error("list_tasks prepare", e),
    };
    let rows = match stmt.query_map(params![&user_id.0], row_to_task) {
        Ok(r) => r,
        Err(e) => return db_error("list_tasks query", e),
    };
    let items: Vec<WorkTask> = rows.filter_map(|r| r.ok()).collect();
    (
        StatusCode::OK,
        Json(json!({ "success": true, "items": items })),
    )
}

/// POST /api/work/tasks
pub async fn create_task(
    State(state): State<AppState>,
    user_id: UserId,
    Json(req): Json<CreateWorkTaskRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let now = now_rfc3339();
    let custom_str = serde_json::to_string(
        &req.custom_fields
            .map(JsonValue::Object)
            .unwrap_or_else(|| JsonValue::Object(Map::new())),
    )
    .unwrap_or_else(|_| "{}".to_string());

    // Get next sort_order = max + 1 (so new row appears at end by default).
    let next_sort: f64 = db
        .query_row(
            "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM work_tasks WHERE user_id = ?1",
            params![&user_id.0],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    let res = db.execute(
        "INSERT INTO work_tasks
           (user_id, title, desc, assignee, level, freq, status, priority,
            due_date, progress, custom_fields, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)",
        params![
            &user_id.0,
            &req.title,
            &req.desc,
            &req.assignee,
            &req.level,
            &req.freq,
            &req.status,
            &req.priority,
            &req.due_date,
            req.progress,
            &custom_str,
            next_sort,
            &now,
        ],
    );
    match res {
        Ok(_) => {
            let new_id = db.last_insert_rowid();
            fetch_task(&db, &user_id.0, new_id)
        }
        Err(e) => db_error("create_task insert", e),
    }
}

/// PATCH /api/work/tasks/{id}
pub async fn update_task(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
    Json(patch): Json<UpdateWorkTaskRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();

    // Load existing row so we can merge custom_fields and apply auto-progress rule.
    let existing: Option<(String, String)> = db
        .query_row(
            "SELECT status, custom_fields FROM work_tasks \
             WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
            params![id, &user_id.0],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .ok();
    let Some((old_status, old_custom_raw)) = existing else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到任务" })),
        );
    };

    // Build SET clause dynamically from the patch fields.
    let mut sets: Vec<&str> = Vec::new();
    let mut vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    macro_rules! push_str {
        ($field:literal, $opt:expr) => {
            if let Some(v) = $opt {
                sets.push(concat!($field, " = ?"));
                vals.push(Box::new(v));
            }
        };
    }
    push_str!("title", patch.title);
    push_str!("desc", patch.desc);
    push_str!("assignee", patch.assignee);
    push_str!("level", patch.level);
    push_str!("freq", patch.freq);

    // status with auto-progress side effect
    let new_status = patch.status.clone();
    if let Some(s) = patch.status {
        sets.push("status = ?");
        vals.push(Box::new(s));
    }
    push_str!("priority", patch.priority);

    // due_date: empty string → NULL; non-empty → store
    if let Some(due) = patch.due_date {
        sets.push("due_date = ?");
        if due.is_empty() {
            vals.push(Box::new(Option::<String>::None));
        } else {
            vals.push(Box::new(Some(due)));
        }
    }

    // progress: explicit only; auto-set to 100 if status flipped to 'done' AND
    // caller didn't explicitly set progress.
    let mut effective_progress = patch.progress;
    if let Some(s) = new_status.as_deref() {
        if s == "done" && effective_progress.is_none() {
            effective_progress = Some(100);
        }
    } else if old_status == "done" {
        // status unchanged; nothing to auto-do
    }
    if let Some(p) = effective_progress {
        sets.push("progress = ?");
        vals.push(Box::new(p.clamp(0, 100)));
    }

    // custom_fields: merge over existing
    if let Some(patch_custom) = patch.custom_fields {
        let mut merged = parse_custom_fields(&old_custom_raw);
        for (k, v) in patch_custom {
            merged.insert(k, v);
        }
        let s = serde_json::to_string(&JsonValue::Object(merged)).unwrap_or_else(|_| "{}".into());
        sets.push("custom_fields = ?");
        vals.push(Box::new(s));
    }

    if let Some(so) = patch.sort_order {
        sets.push("sort_order = ?");
        vals.push(Box::new(so));
    }

    if sets.is_empty() {
        // No-op patch — just return the current task
        return fetch_task(&db, &user_id.0, id);
    }

    sets.push("updated_at = ?");
    vals.push(Box::new(now_rfc3339()));

    // WHERE bindings
    vals.push(Box::new(id));
    vals.push(Box::new(user_id.0.clone()));

    let sql = format!(
        "UPDATE work_tasks SET {} WHERE id = ? AND user_id = ? AND deleted = 0",
        sets.join(", ")
    );
    let params_ref: Vec<&dyn rusqlite::ToSql> = vals.iter().map(|b| &**b).collect();
    let res = db.execute(&sql, params_ref.as_slice());
    match res {
        Ok(_) => fetch_task(&db, &user_id.0, id),
        Err(e) => db_error("update_task", e),
    }
}

/// DELETE /api/work/tasks/{id} — soft delete
pub async fn delete_task(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let now = now_rfc3339();
    let res = db.execute(
        "UPDATE work_tasks SET deleted = 1, deleted_at = ?1, updated_at = ?1 \
         WHERE id = ?2 AND user_id = ?3 AND deleted = 0",
        params![&now, id, &user_id.0],
    );
    match res {
        Ok(n) if n == 0 => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到任务" })),
        ),
        Ok(_) => (StatusCode::OK, Json(json!({ "success": true }))),
        Err(e) => db_error("delete_task", e),
    }
}

/// Helper: load one task by id (after create / patch) and return it.
fn fetch_task(db: &Connection, user_id: &str, id: i64) -> (StatusCode, Json<JsonValue>) {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM work_tasks WHERE id = ?1 AND user_id = ?2 AND deleted = 0"
    );
    let res = db.query_row(&sql, params![id, user_id], row_to_task);
    match res {
        Ok(t) => (
            StatusCode::OK,
            Json(json!({ "success": true, "item": t })),
        ),
        Err(rusqlite::Error::QueryReturnedNoRows) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到任务" })),
        ),
        Err(e) => db_error("fetch_task", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{auth_cookie, create_test_user, test_state};
    use axum::body::to_bytes;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn body_json(resp: axum::response::Response) -> JsonValue {
        let status = resp.status();
        let body = to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        let body_str = String::from_utf8_lossy(&body);
        serde_json::from_slice(&body)
            .unwrap_or_else(|e| panic!("parse fail status={} body={:?} err={}", status, body_str, e))
    }

    #[tokio::test]
    async fn create_then_list_returns_task() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "alice", "Pa55word1");
        let session_cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        // Create
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"院年度规划","level":"院"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["success"], true);
        assert_eq!(j["item"]["title"], "院年度规划");
        assert_eq!(j["item"]["status"], "todo");

        // List
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/work/tasks")
                    .header("Cookie", &session_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["items"].as_array().unwrap().len(), 1);
        assert_eq!(j["items"][0]["title"], "院年度规划");
    }

    #[tokio::test]
    async fn patch_status_done_auto_sets_progress_100() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "bob", "Pa55word1");
        let session_cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        // Create
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"x"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = body_json(resp).await["item"]["id"].as_i64().unwrap();

        // PATCH status=done WITHOUT progress
        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/work/tasks/{id}"))
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"status":"done"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["status"], "done");
        assert_eq!(j["item"]["progress"], 100);
    }

    #[tokio::test]
    async fn delete_soft_removes_from_list() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "carol", "Pa55word1");
        let session_cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"y"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = body_json(resp).await["item"]["id"].as_i64().unwrap();

        // Delete
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/work/tasks/{id}"))
                    .header("Cookie", &session_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["success"], true);

        // List → empty
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/work/tasks")
                    .header("Cookie", &session_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["items"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn custom_fields_patch_merges() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "dan", "Pa55word1");
        let session_cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        // Create with custom field a=1
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"z","customFields":{"a":"1"}}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = body_json(resp).await["item"]["id"].as_i64().unwrap();

        // PATCH custom b=2 — should merge, not replace
        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/work/tasks/{id}"))
                    .header("Cookie", &session_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"customFields":{"b":"2"}}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        let cf = j["item"]["customFields"].as_object().unwrap();
        assert_eq!(cf.get("a"), Some(&json!("1"))); // preserved
        assert_eq!(cf.get("b"), Some(&json!("2"))); // added
    }
}
