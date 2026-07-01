//! `/api/work/columns` — per-user column schema for the work-task table.
//!
//! See SPEC: `C:\Project\ergouPM\specs\work-task-table\spec.md`
//! Task ticket: T-094
//!
//! Lifecycle:
//! - First GET for a user with no rows → seed the 9 built-in columns
//!   (see `models::work_column::builtin_seed`) before returning.
//! - PUT accepts a batch of partial updates (rename / change type / change
//!   options / reorder via `position`).
//! - POST creates one custom column with an auto-generated key like `c1`, `c2`.
//! - DELETE removes a custom column AND strips that key from every task's
//!   `custom_fields` JSON (so we don't leak orphan values).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::{json, Value as JsonValue};
use tracing::{error, warn};

use crate::auth::UserId;
use crate::models::work_column::{
    builtin_seed, BatchSaveColumnsRequest, CreateColumnRequest, WorkColumn,
};
use crate::state::AppState;

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ColumnsResponse {
    pub success: bool,
    pub items: Vec<WorkColumn>,
}

fn db_error(ctx: &str, e: rusqlite::Error) -> (StatusCode, Json<JsonValue>) {
    error!(target: "work_columns", "{} db error: {}", ctx, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "error": "内部错误" })),
    )
}

fn row_to_col(row: &rusqlite::Row) -> rusqlite::Result<WorkColumn> {
    let opts_raw: String = row.get(3)?;
    let options: Vec<String> = serde_json::from_str(&opts_raw).unwrap_or_else(|e| {
        warn!(target: "work_columns", "options parse err: {}", e);
        Vec::new()
    });
    let width: Option<i32> = row.get(4)?;
    let min_width: Option<i32> = row.get(5)?;
    let builtin: i32 = row.get(7)?;
    let sys: i32 = row.get(8)?;
    Ok(WorkColumn {
        key: row.get(0)?,
        name: row.get(1)?,
        col_type: row.get(2)?,
        options,
        width,
        min_width,
        position: row.get(6)?,
        builtin: builtin != 0,
        sys: sys != 0,
    })
}

const SELECT_COLS: &str = "key, name, type, options, width, min_width, position, builtin, sys";

/// Seed the built-in columns for a user that has none yet.
fn seed_builtin(db: &Connection, user_id: &str) -> rusqlite::Result<()> {
    let seed = builtin_seed();
    let mut stmt = db.prepare(
        "INSERT OR IGNORE INTO work_columns
           (user_id, key, name, type, options, width, min_width, position, builtin, sys)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )?;
    for c in seed {
        let opts = serde_json::to_string(&c.options).unwrap_or_else(|_| "[]".into());
        stmt.execute(params![
            user_id,
            c.key,
            c.name,
            c.col_type,
            opts,
            c.width,
            c.min_width,
            c.position,
            c.builtin as i32,
            c.sys as i32,
        ])?;
    }
    Ok(())
}

/// T-108: 对已有 work_columns 行的老用户,补齐 builtin_seed 中新增的内置列。
/// 用 INSERT OR IGNORE 实现幂等:已存在的 key 跳过,缺失的按种子定义插入。
/// 新增内置列时(如 T-108 的「标签」),老用户首次访问 list_columns 时自动补行。
/// 老用户可能已有自定义列占用了 seed 的 position(如 c1 在 position 9),
/// 新内置列(tags 也想要 position 9)会与之共存;fetch_all 用 (position, key)
/// 双键排序,顺序确定;用户可在「列设置」面板拖动调整。
fn ensure_builtin_present(db: &Connection, user_id: &str) -> rusqlite::Result<()> {
    // 与 seed_builtin 共享 INSERT OR IGNORE 语义,只是不再检查 count==0。
    seed_builtin(db, user_id)
}

fn fetch_all(db: &Connection, user_id: &str) -> rusqlite::Result<Vec<WorkColumn>> {
    // T-108: 加 key 做兜底排序,避免同 position 时(老用户的自定义列与新内置列
    // 可能共享 position)顺序不稳定。
    let sql = format!(
        "SELECT {SELECT_COLS} FROM work_columns WHERE user_id = ?1
         ORDER BY position ASC, key ASC"
    );
    let mut stmt = db.prepare(&sql)?;
    let rows = stmt.query_map(params![user_id], row_to_col)?;
    let cols: Vec<WorkColumn> = rows.filter_map(|r| r.ok()).collect();
    Ok(cols)
}

/// GET /api/work/columns — seeds on first access, returns ordered list.
pub async fn list_columns(
    State(state): State<AppState>,
    user_id: UserId,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();

    // Seed if empty (new user); otherwise ensure any newly-added builtin column
    // (e.g. T-108 「标签」) is present for existing users — INSERT OR IGNORE is idempotent.
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM work_columns WHERE user_id = ?1",
            params![&user_id.0],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if count == 0 {
        if let Err(e) = seed_builtin(&db, &user_id.0) {
            return db_error("seed_builtin", e);
        }
    } else if let Err(e) = ensure_builtin_present(&db, &user_id.0) {
        // Non-fatal: log and continue with whatever columns are present.
        warn!(target: "work_columns", "ensure_builtin_present err: {}", e);
    }

    match fetch_all(&db, &user_id.0) {
        Ok(cols) => (
            StatusCode::OK,
            Json(json!({ "success": true, "items": cols })),
        ),
        Err(e) => db_error("list_columns", e),
    }
}

/// PUT /api/work/columns — batch save.
/// Each `ColumnPatch` must include `key` (target);
/// other fields are optional and only update what's set.
/// Position values are accepted as-is (frontend renumbers after reorder).
pub async fn batch_save_columns(
    State(state): State<AppState>,
    user_id: UserId,
    Json(req): Json<BatchSaveColumnsRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let mut db = state.db.lock();

    let tx = match db.transaction() {
        Ok(t) => t,
        Err(e) => return db_error("batch_save tx", e),
    };

    for patch in &req.columns {
        // Build dynamic UPDATE
        let mut sets: Vec<&str> = Vec::new();
        let mut vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(n) = &patch.name {
            sets.push("name = ?");
            vals.push(Box::new(n.clone()));
        }
        if let Some(t) = &patch.col_type {
            sets.push("type = ?");
            vals.push(Box::new(t.clone()));
        }
        if let Some(o) = &patch.options {
            sets.push("options = ?");
            vals.push(Box::new(
                serde_json::to_string(o).unwrap_or_else(|_| "[]".into()),
            ));
        }
        if let Some(w) = patch.width {
            sets.push("width = ?");
            vals.push(Box::new(w));
        }
        if let Some(mw) = patch.min_width {
            sets.push("min_width = ?");
            vals.push(Box::new(mw));
        }
        if let Some(p) = patch.position {
            sets.push("position = ?");
            vals.push(Box::new(p));
        }
        if sets.is_empty() {
            continue;
        }
        vals.push(Box::new(user_id.0.clone()));
        vals.push(Box::new(patch.key.clone()));

        let sql = format!(
            "UPDATE work_columns SET {} WHERE user_id = ? AND key = ?",
            sets.join(", ")
        );
        let params_ref: Vec<&dyn rusqlite::ToSql> = vals.iter().map(|b| &**b).collect();
        if let Err(e) = tx.execute(&sql, params_ref.as_slice()) {
            error!(target: "work_columns", "batch_save row {} err: {}", patch.key, e);
        }
    }

    if let Err(e) = tx.commit() {
        return db_error("batch_save commit", e);
    }

    match fetch_all(&db, &user_id.0) {
        Ok(cols) => (
            StatusCode::OK,
            Json(json!({ "success": true, "items": cols })),
        ),
        Err(e) => db_error("batch_save fetch", e),
    }
}

/// POST /api/work/columns — create a custom column.
/// Auto-generates key like `c1`, `c2`, … (max existing + 1).
pub async fn create_column(
    State(state): State<AppState>,
    user_id: UserId,
    Json(req): Json<CreateColumnRequest>,
) -> (StatusCode, Json<JsonValue>) {
    if req.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "列名不能为空" })),
        );
    }
    let db = state.db.lock();

    // Find next custom key (c1, c2, ...). Look at existing keys matching ^c\d+$.
    let next_key = next_custom_key(&db, &user_id.0).unwrap_or_else(|| "c1".to_string());

    // Next position = current max + 1
    let next_pos: i32 = db
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM work_columns WHERE user_id = ?1",
            params![&user_id.0],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let opts_str = serde_json::to_string(&req.options).unwrap_or_else(|_| "[]".into());

    let res = db.execute(
        "INSERT INTO work_columns
           (user_id, key, name, type, options, width, min_width, position, builtin, sys)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, 0)",
        params![
            &user_id.0,
            &next_key,
            &req.name.trim(),
            &req.col_type,
            &opts_str,
            req.width.unwrap_or(140),
            req.min_width.unwrap_or(80),
            next_pos,
        ],
    );
    if let Err(e) = res {
        return db_error("create_column insert", e);
    }

    match fetch_all(&db, &user_id.0) {
        Ok(cols) => (
            StatusCode::OK,
            Json(json!({ "success": true, "items": cols, "createdKey": next_key })),
        ),
        Err(e) => db_error("create_column fetch", e),
    }
}

fn next_custom_key(db: &Connection, user_id: &str) -> Option<String> {
    // Pull all keys matching c\d+, find max suffix
    let mut max_n: i64 = 0;
    if let Ok(mut stmt) =
        db.prepare("SELECT key FROM work_columns WHERE user_id = ?1 AND key GLOB 'c[0-9]*'")
    {
        if let Ok(rows) = stmt.query_map(params![user_id], |r| r.get::<_, String>(0)) {
            for key in rows.flatten() {
                if let Some(rest) = key.strip_prefix('c') {
                    if let Ok(n) = rest.parse::<i64>() {
                        if n > max_n {
                            max_n = n;
                        }
                    }
                }
            }
        }
    }
    Some(format!("c{}", max_n + 1))
}

/// DELETE /api/work/columns/{key} — remove a custom column.
/// Built-in columns refuse to delete. Custom column values are also stripped
/// from every task's `custom_fields` JSON (no orphan data left behind).
pub async fn delete_column(
    State(state): State<AppState>,
    user_id: UserId,
    Path(key): Path<String>,
) -> (StatusCode, Json<JsonValue>) {
    let mut db = state.db.lock();

    // Check exists + not builtin
    let row: Option<i32> = db
        .query_row(
            "SELECT builtin FROM work_columns WHERE user_id = ?1 AND key = ?2",
            params![&user_id.0, &key],
            |r| r.get(0),
        )
        .ok();
    let Some(builtin) = row else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到该列" })),
        );
    };
    if builtin != 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "内置列不能删除" })),
        );
    }

    let tx = match db.transaction() {
        Ok(t) => t,
        Err(e) => return db_error("delete_column tx", e),
    };

    // 1) Delete the column row
    if let Err(e) = tx.execute(
        "DELETE FROM work_columns WHERE user_id = ?1 AND key = ?2",
        params![&user_id.0, &key],
    ) {
        return db_error("delete_column delete", e);
    }

    // 2) Strip the key from every task's custom_fields JSON.
    //    Use SQLite's json_remove (available in the bundled SQLite for rusqlite).
    let strip_sql = format!(
        "UPDATE work_tasks
         SET custom_fields = COALESCE(json_remove(custom_fields, '$.\"{}\"'), '{{}}'),
             updated_at = ?1
         WHERE user_id = ?2 AND deleted = 0
           AND json_extract(custom_fields, '$.\"{}\"') IS NOT NULL",
        key.replace('"', "\\\""),
        key.replace('"', "\\\""),
    );
    if let Err(e) = tx.execute(
        &strip_sql,
        params![chrono::Utc::now().to_rfc3339(), &user_id.0],
    ) {
        // Non-fatal: column row already gone, orphan values are cosmetic.
        warn!(target: "work_columns", "delete_column strip custom_fields err: {}", e);
    }

    if let Err(e) = tx.commit() {
        return db_error("delete_column commit", e);
    }

    match fetch_all(&db, &user_id.0) {
        Ok(cols) => (
            StatusCode::OK,
            Json(json!({ "success": true, "items": cols })),
        ),
        Err(e) => db_error("delete_column fetch", e),
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
        let body = to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn list_seeds_12_builtin_cols_on_first_access() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "alice", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/work/columns")
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        let items = j["items"].as_array().unwrap();
        assert_eq!(items.len(), 12);
        // First seeded col is "title" by position
        assert_eq!(items[0]["key"], "title");
        assert_eq!(items[0]["name"], "任务");
        assert_eq!(items[0]["builtin"], true);
        // status + priority are sys
        let status_col = items.iter().find(|c| c["key"] == "status").unwrap();
        assert_eq!(status_col["sys"], true);
        let engage_col = items.iter().find(|c| c["key"] == "engage").unwrap();
        assert_eq!(engage_col["type"], "select");
        assert_eq!(engage_col["sys"], true);
        let area_col = items.iter().find(|c| c["key"] == "area").unwrap();
        assert_eq!(area_col["type"], "select");
        assert_eq!(area_col["sys"], false);
        // T-108: tags column is seeded (multi type, empty options)
        let tags_col = items.iter().find(|c| c["key"] == "tags").unwrap();
        assert_eq!(tags_col["name"], "标签");
        assert_eq!(tags_col["type"], "multi");
        assert_eq!(tags_col["builtin"], true);
        assert_eq!(tags_col["sys"], false);
        let opts = tags_col["options"].as_array().unwrap();
        assert!(opts.is_empty(), "tags options should start empty");
    }

    /// T-108: 模拟"老用户"场景 — 数据库里已经有 9 个旧版内置列,
    /// 不该重复 seed,但应该自动把缺失的 tags 列补上。
    #[tokio::test]
    async fn list_backfills_new_builtin_columns_for_existing_users_idempotently() {
        let state = test_state();
        let (uid, token) = create_test_user(&state, "old_user", "Pa55word1");
        let cookie = auth_cookie(&token);

        // 手工插入 9 个旧版内置列(模拟 T-094 时代的老用户数据)
        {
            let db = state.db.lock();
            let legacy_seed: Vec<(&str, &str, &str, i32)> = vec![
                ("title", "任务", "text", 0),
                ("desc", "简介", "longtext", 1),
                ("assignee", "责任人", "text", 2),
                ("level", "层级", "select", 3),
                ("freq", "频率", "select", 4),
                ("status", "状态", "status", 5),
                ("priority", "优先级", "select", 6),
                ("due", "截止日", "date", 7),
                ("progress", "进度", "percent", 8),
            ];
            for (key, name, typ, pos) in legacy_seed {
                let sys = if key == "status" || key == "priority" {
                    1
                } else {
                    0
                };
                db.execute(
                    "INSERT INTO work_columns(user_id, key, name, type, options,
                       width, min_width, position, builtin, sys)
                     VALUES (?1, ?2, ?3, ?4, '[]', 140, 80, ?5, 1, ?6)",
                    params![&uid, key, name, typ, pos, sys],
                )
                .unwrap();
            }
        }

        let app = crate::build_app(state);

        // 第一次访问:tags 应该被补进去
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/work/columns")
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        let items = j["items"].as_array().unwrap();
        assert_eq!(items.len(), 12, "new builtin columns should be backfilled");
        let tags = items.iter().find(|c| c["key"] == "tags").unwrap();
        assert_eq!(tags["type"], "multi");
        assert_eq!(tags["builtin"], true);
        assert!(items.iter().any(|c| c["key"] == "area"));
        assert!(items
            .iter()
            .any(|c| c["key"] == "engage" && c["sys"] == true));

        // 第二次访问:幂等,仍是 10 列(不重复插入)
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/work/columns")
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        let items = j["items"].as_array().unwrap();
        assert_eq!(items.len(), 12, "second access should be idempotent");
    }

    #[tokio::test]
    async fn create_custom_column_assigns_c1_key() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "bob", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        // First seed via GET
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/work/columns")
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Create custom column
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/columns")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"name":"涉及部门","type":"multi"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["success"], true);
        assert_eq!(j["createdKey"], "c1");
        let items = j["items"].as_array().unwrap();
        // 10 builtin (incl. T-108 tags) + 1 custom = 11
        assert_eq!(items.len(), 13);
        let new_col = items.iter().find(|c| c["key"] == "c1").unwrap();
        assert_eq!(new_col["name"], "涉及部门");
        assert_eq!(new_col["type"], "multi");
        assert_eq!(new_col["builtin"], false);
    }

    #[tokio::test]
    async fn delete_builtin_column_is_refused() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "carol", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        // seed
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/work/columns")
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let resp = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/work/columns/title")
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = resp.status();
        let j = body_json(resp).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(j["success"], false);
    }

    #[tokio::test]
    async fn delete_custom_column_strips_its_values_from_tasks() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "dan", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        // 1) seed columns
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/work/columns")
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // 2) create custom col c1
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/columns")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"name":"foo","type":"text"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        // 3) create a task with c1 = "X"
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/tasks")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"t","customFields":{"c1":"X"}}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            body_json(resp).await["item"]["customFields"]["c1"],
            json!("X")
        );

        // 4) delete c1
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/work/columns/c1")
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // 5) list tasks: c1 should be gone from customFields
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/work/tasks")
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        let task = &j["items"][0];
        let cf = task["customFields"].as_object().unwrap();
        assert!(
            !cf.contains_key("c1"),
            "c1 should be stripped, got {:?}",
            cf
        );
    }
}
