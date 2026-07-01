//! `/api/work/stakeholder-columns` - per-user column schema for stakeholders.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use rusqlite::{params, Connection};
use serde_json::{json, Value as JsonValue};
use tracing::{error, warn};

use crate::auth::UserId;
use crate::models::stakeholder_column::{
    builtin_seed, BatchSaveStakeholderColumnsRequest, CreateStakeholderColumnRequest,
    StakeholderColumn,
};
use crate::state::AppState;

fn db_error(ctx: &str, e: rusqlite::Error) -> (StatusCode, Json<JsonValue>) {
    error!(target: "stakeholder_columns", "{} db error: {}", ctx, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "error": "内部错误" })),
    )
}

fn row_to_col(row: &rusqlite::Row) -> rusqlite::Result<StakeholderColumn> {
    let opts_raw: String = row.get(3)?;
    let options: Vec<String> = serde_json::from_str(&opts_raw).unwrap_or_else(|e| {
        warn!(target: "stakeholder_columns", "options parse err: {}", e);
        Vec::new()
    });
    let builtin: i32 = row.get(7)?;
    let sys: i32 = row.get(8)?;
    Ok(StakeholderColumn {
        key: row.get(0)?,
        name: row.get(1)?,
        col_type: row.get(2)?,
        options,
        width: row.get(4)?,
        min_width: row.get(5)?,
        position: row.get(6)?,
        builtin: builtin != 0,
        sys: sys != 0,
    })
}

const SELECT_COLS: &str = "key, name, type, options, width, min_width, position, builtin, sys";

fn seed_builtin(db: &Connection, user_id: &str) -> rusqlite::Result<()> {
    let seed = builtin_seed();
    let mut stmt = db.prepare(
        "INSERT OR IGNORE INTO stakeholder_columns
           (user_id, key, name, type, options, width, min_width, position, builtin, sys)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )?;
    for c in seed {
        stmt.execute(params![
            user_id,
            c.key,
            c.name,
            c.col_type,
            serde_json::to_string(&c.options).unwrap_or_else(|_| "[]".into()),
            c.width,
            c.min_width,
            c.position,
            c.builtin as i32,
            c.sys as i32,
        ])?;
    }
    Ok(())
}

fn fetch_all(db: &Connection, user_id: &str) -> rusqlite::Result<Vec<StakeholderColumn>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM stakeholder_columns WHERE user_id = ?1
         ORDER BY position ASC, key ASC"
    );
    let mut stmt = db.prepare(&sql)?;
    let rows = stmt.query_map(params![user_id], row_to_col)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub async fn list_columns(
    State(state): State<AppState>,
    user_id: UserId,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM stakeholder_columns WHERE user_id = ?1",
            params![&user_id.0],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if count == 0 {
        if let Err(e) = seed_builtin(&db, &user_id.0) {
            return db_error("seed_builtin", e);
        }
    } else if let Err(e) = seed_builtin(&db, &user_id.0) {
        warn!(target: "stakeholder_columns", "ensure builtin err: {}", e);
    }
    match fetch_all(&db, &user_id.0) {
        Ok(items) => (
            StatusCode::OK,
            Json(json!({ "success": true, "items": items })),
        ),
        Err(e) => db_error("list", e),
    }
}

pub async fn batch_save_columns(
    State(state): State<AppState>,
    user_id: UserId,
    Json(req): Json<BatchSaveStakeholderColumnsRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let mut db = state.db.lock();
    let tx = match db.transaction() {
        Ok(tx) => tx,
        Err(e) => return db_error("batch tx", e),
    };
    for patch in &req.columns {
        let mut sets: Vec<&str> = Vec::new();
        let mut vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(v) = &patch.name {
            sets.push("name = ?");
            vals.push(Box::new(v.clone()));
        }
        if let Some(v) = &patch.col_type {
            sets.push("type = ?");
            vals.push(Box::new(v.clone()));
        }
        if let Some(v) = &patch.options {
            sets.push("options = ?");
            vals.push(Box::new(
                serde_json::to_string(v).unwrap_or_else(|_| "[]".into()),
            ));
        }
        if let Some(v) = patch.width {
            sets.push("width = ?");
            vals.push(Box::new(v));
        }
        if let Some(v) = patch.min_width {
            sets.push("min_width = ?");
            vals.push(Box::new(v));
        }
        if let Some(v) = patch.position {
            sets.push("position = ?");
            vals.push(Box::new(v));
        }
        if sets.is_empty() {
            continue;
        }
        vals.push(Box::new(user_id.0.clone()));
        vals.push(Box::new(patch.key.clone()));
        let sql = format!(
            "UPDATE stakeholder_columns SET {} WHERE user_id = ? AND key = ?",
            sets.join(", ")
        );
        let params_ref: Vec<&dyn rusqlite::ToSql> = vals.iter().map(|b| &**b).collect();
        if let Err(e) = tx.execute(&sql, params_ref.as_slice()) {
            error!(target: "stakeholder_columns", "batch row {} err: {}", patch.key, e);
        }
    }
    if let Err(e) = tx.commit() {
        return db_error("batch commit", e);
    }
    match fetch_all(&db, &user_id.0) {
        Ok(items) => (
            StatusCode::OK,
            Json(json!({ "success": true, "items": items })),
        ),
        Err(e) => db_error("batch fetch", e),
    }
}

pub async fn create_column(
    State(state): State<AppState>,
    user_id: UserId,
    Json(req): Json<CreateStakeholderColumnRequest>,
) -> (StatusCode, Json<JsonValue>) {
    if req.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "列名不能为空" })),
        );
    }
    let db = state.db.lock();
    let next_key = next_custom_key(&db, &user_id.0);
    let next_pos: i32 = db
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM stakeholder_columns WHERE user_id = ?1",
            params![&user_id.0],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let opts = serde_json::to_string(&req.options).unwrap_or_else(|_| "[]".into());
    if let Err(e) = db.execute(
        "INSERT INTO stakeholder_columns
           (user_id, key, name, type, options, width, min_width, position, builtin, sys)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, 0)",
        params![
            &user_id.0,
            &next_key,
            req.name.trim(),
            &req.col_type,
            &opts,
            req.width.unwrap_or(140),
            req.min_width.unwrap_or(80),
            next_pos,
        ],
    ) {
        return db_error("create insert", e);
    }
    match fetch_all(&db, &user_id.0) {
        Ok(items) => (
            StatusCode::OK,
            Json(json!({ "success": true, "items": items, "createdKey": next_key })),
        ),
        Err(e) => db_error("create fetch", e),
    }
}

fn next_custom_key(db: &Connection, user_id: &str) -> String {
    let mut max_n = 0i64;
    if let Ok(mut stmt) =
        db.prepare("SELECT key FROM stakeholder_columns WHERE user_id = ?1 AND key GLOB 'c[0-9]*'")
    {
        if let Ok(rows) = stmt.query_map(params![user_id], |r| r.get::<_, String>(0)) {
            for key in rows.flatten() {
                if let Some(rest) = key.strip_prefix('c') {
                    if let Ok(n) = rest.parse::<i64>() {
                        max_n = max_n.max(n);
                    }
                }
            }
        }
    }
    format!("c{}", max_n + 1)
}

pub async fn delete_column(
    State(state): State<AppState>,
    user_id: UserId,
    Path(key): Path<String>,
) -> (StatusCode, Json<JsonValue>) {
    let mut db = state.db.lock();
    let builtin: Option<i32> = db
        .query_row(
            "SELECT builtin FROM stakeholder_columns WHERE user_id = ?1 AND key = ?2",
            params![&user_id.0, &key],
            |r| r.get(0),
        )
        .ok();
    let Some(builtin) = builtin else {
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
        Ok(tx) => tx,
        Err(e) => return db_error("delete tx", e),
    };
    if let Err(e) = tx.execute(
        "DELETE FROM stakeholder_columns WHERE user_id = ?1 AND key = ?2",
        params![&user_id.0, &key],
    ) {
        return db_error("delete row", e);
    }
    let escaped = key.replace('"', "\\\"");
    let strip_sql = format!(
        "UPDATE stakeholders
         SET custom_fields = COALESCE(json_remove(custom_fields, '$.\"{}\"'), '{{}}'),
             updated_at = ?1
         WHERE user_id = ?2 AND deleted = 0
           AND json_extract(custom_fields, '$.\"{}\"') IS NOT NULL",
        escaped, escaped
    );
    if let Err(e) = tx.execute(
        &strip_sql,
        params![chrono::Utc::now().to_rfc3339(), &user_id.0],
    ) {
        warn!(target: "stakeholder_columns", "strip custom_fields err: {}", e);
    }
    if let Err(e) = tx.commit() {
        return db_error("delete commit", e);
    }
    match fetch_all(&db, &user_id.0) {
        Ok(items) => (
            StatusCode::OK,
            Json(json!({ "success": true, "items": items })),
        ),
        Err(e) => db_error("delete fetch", e),
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
    async fn list_seeds_builtin_stakeholder_columns() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "sc_alice", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/work/stakeholder-columns")
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        let items = j["items"].as_array().unwrap();
        assert_eq!(items.len(), 11);
        assert_eq!(items[0]["key"], "name");
        let method = items.iter().find(|c| c["key"] == "method").unwrap();
        assert_eq!(method["type"], "multi");
        assert_eq!(method["builtin"], true);
        let cadence = items.iter().find(|c| c["key"] == "cadence").unwrap();
        assert_eq!(cadence["options"].as_array().unwrap().len(), 4);
    }

    #[tokio::test]
    async fn custom_column_create_delete_strips_stakeholder_values() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "sc_bob", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/work/stakeholder-columns")
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/stakeholder-columns")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"name":"影响力","type":"text"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["createdKey"], "c1");

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/stakeholders")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"name":"李四","customFields":{"c1":"高"}}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["item"]["customFields"]["c1"], "高");

        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/work/stakeholder-columns/c1")
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/work/stakeholders")
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert!(j["items"][0]["customFields"]
            .as_object()
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn delete_builtin_column_is_refused() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "sc_builtin", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/work/stakeholder-columns")
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
                    .uri("/api/work/stakeholder-columns/name")
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
