//! `/api/sources` — 素材 CRUD + 异步抓取触发(T-105)。
//!
//! 候选池语义(spec 原则 4):
//!   - `insight_id IS NULL` = 未归属候选(刷推随手扔)
//!   - 非空 = 已挂到某个 Insight
//!
//! 抓取(spec § 7.1):
//!   - POST /sources 立刻返回(fetch_status='pending');后台 tokio::spawn 跑 fetch
//!   - 抓成功 → fetch_status='ok' + content/title 填充
//!   - 抓失败 → fetch_status='failed' + fetch_error 填错误
//!   - 用户粘贴纯文本 → fetch_status='manual',跳过抓取

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::auth::UserId;
use crate::models::source::{
    infer_kind, CreateSourceRequest, Source, UpdateSourceRequest,
};
use crate::services::insight_fetcher;
use crate::state::AppState;

// ============ 共用 ============

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn db_error(ctx: &str, e: rusqlite::Error) -> (StatusCode, Json<JsonValue>) {
    error!(target: "sources", "{} db error: {}", ctx, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "error": "内部错误" })),
    )
}

const SELECT_COLS: &str = "id, insight_id, kind, url, title, author, content, note, \
    starred, fetch_status, fetch_error, fetched_at, created_at, updated_at";

fn row_to_source(row: &rusqlite::Row) -> rusqlite::Result<Source> {
    let starred_int: i64 = row.get(8)?;
    Ok(Source {
        id: row.get(0)?,
        insight_id: row.get(1)?,
        kind: row.get(2)?,
        url: row.get(3)?,
        title: row.get(4)?,
        author: row.get(5)?,
        content: row.get(6)?,
        note: row.get(7)?,
        starred: starred_int != 0,
        fetch_status: row.get(9)?,
        fetch_error: row.get(10)?,
        fetched_at: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

// ============ 给 insights::get_insight ?include=sources 复用 ============

pub fn list_for_insight(
    db: &Connection,
    user_id: &str,
    insight_id: Option<i64>,
    unassigned: bool,
) -> Result<Vec<Source>, String> {
    let (sql, params_v): (String, Vec<Box<dyn rusqlite::ToSql>>) = if unassigned {
        (
            format!(
                "SELECT {SELECT_COLS} FROM sources \
                 WHERE user_id = ?1 AND deleted = 0 AND insight_id IS NULL \
                 ORDER BY created_at DESC LIMIT 200"
            ),
            vec![Box::new(user_id.to_string())],
        )
    } else if let Some(iid) = insight_id {
        (
            format!(
                "SELECT {SELECT_COLS} FROM sources \
                 WHERE user_id = ?1 AND deleted = 0 AND insight_id = ?2 \
                 ORDER BY starred DESC, created_at ASC"
            ),
            vec![Box::new(user_id.to_string()), Box::new(iid)],
        )
    } else {
        (
            format!(
                "SELECT {SELECT_COLS} FROM sources \
                 WHERE user_id = ?1 AND deleted = 0 \
                 ORDER BY created_at DESC LIMIT 200"
            ),
            vec![Box::new(user_id.to_string())],
        )
    };
    let mut stmt = db.prepare(&sql).map_err(|e| format!("prepare: {e}"))?;
    let params_ref: Vec<&dyn rusqlite::ToSql> = params_v.iter().map(|b| &**b).collect();
    let rows = stmt
        .query_map(params_ref.as_slice(), row_to_source)
        .map_err(|e| format!("query: {e}"))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

// ============ HTTP handlers ============

#[derive(Debug, Default, Deserialize)]
pub struct ListFilters {
    #[serde(rename = "insight_id")]
    pub insight_id: Option<i64>,
    pub unassigned: Option<i64>, // 1 = 只看未归属
}

pub async fn list_sources(
    State(state): State<AppState>,
    user_id: UserId,
    Query(f): Query<ListFilters>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let unassigned = f.unassigned.unwrap_or(0) == 1;
    match list_for_insight(&db, &user_id.0, f.insight_id, unassigned) {
        Ok(items) => (
            StatusCode::OK,
            Json(json!({ "success": true, "items": items, "count": items.len() })),
        ),
        Err(e) => {
            error!(target: "sources", "list: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "内部错误" })),
            )
        }
    }
}

/// POST /api/sources
/// 三种创建路径:
///   1. url 非空,content 空 → kind 推断;fetch_status='pending';触发异步抓取
///   2. url 空 + content 非空 → kind='text';fetch_status='manual'(粘贴正文)
///   3. url + content 都给 → 视作"已知 URL 但用户也粘了正文",kind 按 URL 推断;fetch_status='manual'(不重抓)
pub async fn create_source(
    State(state): State<AppState>,
    user_id: UserId,
    Json(req): Json<CreateSourceRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let url_trimmed = req.url.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty());
    let has_content = req.content.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false);
    if url_trimmed.is_none() && !has_content {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "需要 url 或 content 之一" })),
        );
    }
    let kind = infer_kind(url_trimmed, has_content);
    let fetch_status = if url_trimmed.is_some() && !has_content {
        "pending"
    } else {
        "manual"
    };
    let initial_content = req.content.clone().unwrap_or_default();
    let initial_title = req.title.clone().unwrap_or_default();
    let now = now_rfc3339();

    // 若 insight_id 给了,必须属于该 user(防越权)
    let insight_id = req.insight_id;
    if let Some(iid) = insight_id {
        let db = state.db.lock();
        let owns: bool = db
            .query_row(
                "SELECT COUNT(*) > 0 FROM insights WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
                params![iid, &user_id.0],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if !owns {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "error": "未找到对应洞察" })),
            );
        }
        drop(db);
    }

    let new_id = {
        let db = state.db.lock();
        let res = db.execute(
            "INSERT INTO sources (user_id, insight_id, kind, url, title, content, fetch_status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                &user_id.0,
                insight_id,
                kind,
                url_trimmed.map(|s| s.to_string()),
                &initial_title,
                &initial_content,
                fetch_status,
                &now,
            ],
        );
        match res {
            Ok(_) => db.last_insert_rowid(),
            Err(e) => return db_error("create insert", e),
        }
    };

    // 异步抓取触发(只 blog 支持;其他 kind 走 manual 兜底)
    if fetch_status == "pending" && kind == "blog" {
        if let Some(url) = url_trimmed.map(|s| s.to_string()) {
            spawn_fetch(state.clone(), user_id.0.clone(), new_id, url);
        }
    } else if fetch_status == "pending" && kind != "blog" {
        // 非 blog kind 暂不支持自动抓取(spec Phase 2);标记 failed 让前端提示粘贴
        let db = state.db.lock();
        let now2 = now_rfc3339();
        db.execute(
            "UPDATE sources SET fetch_status = 'failed', fetch_error = ?1, updated_at = ?2 \
             WHERE id = ?3",
            params![
                format!("kind '{kind}' 暂未实现自动抓取;请粘贴正文(spec Phase 2)"),
                &now2,
                new_id,
            ],
        )
        .ok();
    }

    let db = state.db.lock();
    fetch_source(&db, &user_id.0, new_id)
}

/// 启动一个后台 task 抓取 URL 并写回(spec § 8.5:异步)
fn spawn_fetch(state: AppState, user_id: String, source_id: i64, url: String) {
    tokio::spawn(async move {
        info!(target: "sources", "fetch start: id={} url={}", source_id, url);
        let result = insight_fetcher::fetch_blog(&url).await;
        let now = now_rfc3339();
        let db = state.db.lock();
        match result {
            Ok(r) => {
                let title_to_set = if r.title.trim().is_empty() {
                    None
                } else {
                    Some(r.title.clone())
                };
                if let Err(e) = db.execute(
                    "UPDATE sources SET fetch_status = 'ok', fetched_at = ?1, content = ?2, \
                     title = COALESCE(NULLIF(title, ''), ?3), \
                     fetch_error = NULL, updated_at = ?1 \
                     WHERE id = ?4 AND user_id = ?5",
                    params![&now, &r.content, &title_to_set, source_id, &user_id],
                ) {
                    error!(target: "sources", "fetch write ok: {}", e);
                } else {
                    info!(target: "sources", "fetch ok: id={} title=\"{}\" content={}b",
                          source_id, r.title, r.content.len());
                }
            }
            Err(e) => {
                warn!(target: "sources", "fetch failed: id={} {}", source_id, e);
                let _ = db.execute(
                    "UPDATE sources SET fetch_status = 'failed', fetch_error = ?1, updated_at = ?2 \
                     WHERE id = ?3 AND user_id = ?4",
                    params![&e, &now, source_id, &user_id],
                );
            }
        }
    });
}

/// PATCH /api/sources/:id
pub async fn update_source(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
    Json(patch): Json<UpdateSourceRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let mut sets: Vec<&str> = Vec::new();
    let mut vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(v) = patch.title {
        sets.push("title = ?");
        vals.push(Box::new(v));
    }
    if let Some(v) = patch.author {
        sets.push("author = ?");
        vals.push(Box::new(v));
    }
    if let Some(v) = patch.content {
        sets.push("content = ?");
        vals.push(Box::new(v));
        // 用户手粘正文 → 标记为 manual(覆盖之前的 ok/failed)
        sets.push("fetch_status = 'manual'");
    }
    if let Some(v) = patch.note {
        sets.push("note = ?");
        vals.push(Box::new(v));
    }
    if let Some(v) = patch.starred {
        sets.push("starred = ?");
        vals.push(Box::new(if v { 1 } else { 0 }));
    }
    // insight_id 拖动归属:Some(Some(id))=归属;Some(None)=取消归属;None=不改
    if let Some(maybe_iid) = patch.insight_id {
        match maybe_iid {
            Some(iid) => {
                // 验证 user 拥有该 insight
                let owns: bool = db
                    .query_row(
                        "SELECT COUNT(*) > 0 FROM insights WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
                        params![iid, &user_id.0],
                        |r| r.get(0),
                    )
                    .unwrap_or(false);
                if !owns {
                    return (
                        StatusCode::NOT_FOUND,
                        Json(json!({ "success": false, "error": "未找到对应洞察" })),
                    );
                }
                sets.push("insight_id = ?");
                vals.push(Box::new(iid));
            }
            None => {
                sets.push("insight_id = NULL");
            }
        }
    }
    if sets.is_empty() {
        return fetch_source(&db, &user_id.0, id);
    }
    sets.push("updated_at = ?");
    vals.push(Box::new(now_rfc3339()));
    vals.push(Box::new(id));
    vals.push(Box::new(user_id.0.clone()));
    let sql = format!(
        "UPDATE sources SET {} WHERE id = ? AND user_id = ? AND deleted = 0",
        sets.join(", ")
    );
    let params_ref: Vec<&dyn rusqlite::ToSql> = vals.iter().map(|b| &**b).collect();
    match db.execute(&sql, params_ref.as_slice()) {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到素材" })),
        ),
        Ok(_) => fetch_source(&db, &user_id.0, id),
        Err(e) => db_error("update", e),
    }
}

/// DELETE /api/sources/:id
pub async fn delete_source(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let now = now_rfc3339();
    let res = db.execute(
        "UPDATE sources SET deleted = 1, updated_at = ?1 WHERE id = ?2 AND user_id = ?3 AND deleted = 0",
        params![&now, id, &user_id.0],
    );
    match res {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到素材" })),
        ),
        Ok(_) => (StatusCode::OK, Json(json!({ "success": true }))),
        Err(e) => db_error("delete", e),
    }
}

/// POST /api/sources/:id/refetch — 手动重试
pub async fn refetch_source(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let (url, kind) = {
        let db = state.db.lock();
        let row: Option<(Option<String>, String)> = db
            .query_row(
                "SELECT url, kind FROM sources WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
                params![id, &user_id.0],
                |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, String>(1)?)),
            )
            .ok();
        match row {
            Some(r) => r,
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({ "success": false, "error": "未找到素材" })),
                );
            }
        }
    };

    let url = match url {
        Some(u) if !u.trim().is_empty() => u,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": "该素材没有 url(粘贴文本无法重抓)" })),
            );
        }
    };
    if kind != "blog" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": format!("kind '{}' 暂未实现自动抓取(spec Phase 2)", kind)
            })),
        );
    }

    // 重置为 pending 并触发抓取
    {
        let db = state.db.lock();
        let now = now_rfc3339();
        db.execute(
            "UPDATE sources SET fetch_status = 'pending', fetch_error = NULL, updated_at = ?1 \
             WHERE id = ?2 AND user_id = ?3",
            params![&now, id, &user_id.0],
        )
        .ok();
    }
    spawn_fetch(state.clone(), user_id.0.clone(), id, url);

    let db = state.db.lock();
    fetch_source(&db, &user_id.0, id)
}

// ============ helper ============

fn fetch_source(db: &Connection, user_id: &str, id: i64) -> (StatusCode, Json<JsonValue>) {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM sources WHERE id = ?1 AND user_id = ?2 AND deleted = 0"
    );
    match db.query_row(&sql, params![id, user_id], row_to_source) {
        Ok(t) => (
            StatusCode::OK,
            Json(json!({ "success": true, "item": t })),
        ),
        Err(rusqlite::Error::QueryReturnedNoRows) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到素材" })),
        ),
        Err(e) => db_error("fetch_source", e),
    }
}

// 让 Arc<AppState> 等场景需要时能用(目前 spawn_fetch 用 AppState 直接 clone 即可)
#[allow(dead_code)]
fn _arc_compatible(_: Arc<AppState>) {}

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
    async fn create_text_source_manual_status() {
        let state = test_state();
        let (_u, token) = create_test_user(&state, "u1", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sources")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"content":"paste text body","title":"我的笔记"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["success"], true);
        assert_eq!(j["item"]["kind"], "text");
        assert_eq!(j["item"]["fetchStatus"], "manual");
        assert_eq!(j["item"]["content"], "paste text body");
    }

    #[tokio::test]
    async fn create_youtube_url_failed_not_implemented() {
        let state = test_state();
        let (_u, token) = create_test_user(&state, "u2", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sources")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"url":"https://www.youtube.com/watch?v=abc"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["kind"], "youtube");
        // 因为非 blog,立刻标记为 failed
        assert_eq!(j["item"]["fetchStatus"], "failed");
    }

    #[tokio::test]
    async fn unassigned_filter() {
        let state = test_state();
        let (_u, token) = create_test_user(&state, "u3", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        // 2 个未归属
        for _ in 0..2 {
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/sources")
                        .header("Cookie", &cookie)
                        .header("Content-Type", "application/json")
                        .body(Body::from(r#"{"content":"x"}"#))
                        .unwrap(),
                )
                .await
                .unwrap();
        }
        // 1 个挂到 insight
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/insights")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"i1"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let iid = body_json(resp).await["item"]["id"].as_i64().unwrap();
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sources")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(format!(r#"{{"content":"y","insightId":{iid}}}"#)))
                    .unwrap(),
            )
            .await
            .unwrap();

        // unassigned=1 → 2 条
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/sources?unassigned=1")
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["count"], 2);

        // insight_id=iid → 1 条
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/sources?insight_id={iid}"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["count"], 1);
    }
}
