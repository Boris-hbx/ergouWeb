//! `/api/insights/:id/share` + 公开 `/r/:token` — 分享链接(T-105)。
//!
//! 设计要点(spec § 5.5 + 原则 3):
//!   - token 32 字节 URL-safe 随机(2^256 空间,无法爆破)
//!   - 绑定到具体 report.id,分享内容不变
//!   - 撤销 → revoked_at 非 NULL → 公开 GET 返回 **410 Gone**(非 404)
//!   - 公开页 `/r/:token` 无需 session;不在 /api/ 下,避免被通用 API rate limit 误伤
//!   - 公开 HTML 返回最小 MVP 模板:标题 + MD 渲染占位 + 引用列表;前端 share.html/js 渲染 MD

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Html,
    Json,
};
use chrono::Utc;
use rand::RngCore;
use rusqlite::params;
use serde_json::{json, Value as JsonValue};
use tracing::error;

use crate::auth::UserId;
use crate::models::share_link::{CreateShareRequest, ShareLink};
use crate::state::AppState;

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn db_error(ctx: &str, e: rusqlite::Error) -> (StatusCode, Json<JsonValue>) {
    error!(target: "share_links", "{} db error: {}", ctx, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "error": "内部错误" })),
    )
}

/// 32 字节 URL-safe 随机 token(base64url 编码,~43 字符)
fn generate_token() -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// POST /api/insights/:id/publish — T-107 v0.2(替代 v0.1 的 POST /share)
/// 事务:auto-revoke 既往 active token + mint 新 token + status=published
///       + current report.published_at = now
pub async fn publish_insight(
    State(state): State<AppState>,
    user_id: UserId,
    Path(insight_id): Path<i64>,
    Json(req): Json<CreateShareRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let mut db = state.db.lock();
    // 拿 current_report_id(spec § 8.4:publish 用 current,reportId 可选覆盖)
    let current: Option<i64> = db
        .query_row(
            "SELECT current_report_id FROM insights WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
            params![insight_id, &user_id.0],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    let report_id = match req.report_id.or(current) {
        Some(id) => id,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false,
                    "error": "没有可发布的报告 — 先生成 v1"
                })),
            );
        }
    };
    // 验证 report 属于该 insight + 该 user
    let owns: bool = db
        .query_row(
            "SELECT COUNT(*) > 0 FROM reports r
             JOIN insights i ON r.insight_id = i.id
             WHERE r.id = ?1 AND r.insight_id = ?2 AND i.user_id = ?3 AND i.deleted = 0",
            params![report_id, insight_id, &user_id.0],
            |r| r.get(0),
        )
        .unwrap_or(false);
    if !owns {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到报告" })),
        );
    }
    // spec 原则 5:一个 report 版本最多发布一次。若该 report 已经 published_at 非空,拒绝
    let already_published: Option<String> = db
        .query_row(
            "SELECT published_at FROM reports WHERE id = ?1",
            params![report_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    if already_published.is_some() {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": "该 report 版本已发布过;撤回后需要先创建新版本(手改一字也行)才能再发"
            })),
        );
    }

    let now = now_rfc3339();
    let token = generate_token();

    // 事务:auto-revoke 既往 active + insert 新 share_link + 更新 status + 写 report.published_at
    let tx = match db.transaction() {
        Ok(t) => t,
        Err(e) => return db_error("tx begin", e),
    };
    if let Err(e) = tx.execute(
        "UPDATE share_links SET revoked_at = ?1
         WHERE insight_id = ?2 AND revoked_at IS NULL",
        params![&now, insight_id],
    ) {
        return db_error("auto-revoke", e);
    }
    if let Err(e) = tx.execute(
        "INSERT INTO share_links (token, insight_id, report_id, show_notes, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            &token,
            insight_id,
            report_id,
            if req.show_notes { 1 } else { 0 },
            &now,
        ],
    ) {
        return db_error("insert share", e);
    }
    if let Err(e) = tx.execute(
        "UPDATE insights SET status = 'published', updated_at = ?1
         WHERE id = ?2 AND user_id = ?3",
        params![&now, insight_id, &user_id.0],
    ) {
        return db_error("update status", e);
    }
    if let Err(e) = tx.execute(
        "UPDATE reports SET published_at = ?1, updated_at = ?1
         WHERE id = ?2 AND published_at IS NULL",
        params![&now, report_id],
    ) {
        return db_error("write published_at", e);
    }
    if let Err(e) = tx.commit() {
        return db_error("tx commit", e);
    }

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "item": {
                "token": token,
                "url": format!("/r/{}", token),
                "insightId": insight_id,
                "reportId": report_id,
                "showNotes": req.show_notes,
                "createdAt": now,
            }
        })),
    )
}

/// POST /api/insights/:id/retract — T-107 v0.2(替代 v0.1 的 DELETE /share/:token)
/// 事务:revoke 当前 active token + status=editing + current report.retracted_at = now
pub async fn retract_insight(
    State(state): State<AppState>,
    user_id: UserId,
    Path(insight_id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let mut db = state.db.lock();
    // 拿当前 active token(spec 原则 5:同一时刻最多 1 个 active)
    let active_token_and_report: Option<(String, i64)> = db
        .query_row(
            "SELECT sl.token, sl.report_id FROM share_links sl
             JOIN insights i ON sl.insight_id = i.id
             WHERE sl.insight_id = ?1 AND sl.revoked_at IS NULL
                   AND i.user_id = ?2 AND i.deleted = 0",
            params![insight_id, &user_id.0],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
        )
        .ok();
    let Some((token, report_id)) = active_token_and_report else {
        // 没有 active 分享 → 要么 insight 不存在,要么 status 不是 published
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": "没有 active 分享可撤回(insight 不在 published 状态,或未找到)"
            })),
        );
    };
    let now = now_rfc3339();
    let tx = match db.transaction() {
        Ok(t) => t,
        Err(e) => return db_error("tx begin", e),
    };
    if let Err(e) = tx.execute(
        "UPDATE share_links SET revoked_at = ?1 WHERE token = ?2",
        params![&now, &token],
    ) {
        return db_error("revoke", e);
    }
    if let Err(e) = tx.execute(
        "UPDATE insights SET status = 'editing', updated_at = ?1 WHERE id = ?2 AND user_id = ?3",
        params![&now, insight_id, &user_id.0],
    ) {
        return db_error("update status", e);
    }
    if let Err(e) = tx.execute(
        "UPDATE reports SET retracted_at = ?1, updated_at = ?1
         WHERE id = ?2 AND retracted_at IS NULL",
        params![&now, report_id],
    ) {
        return db_error("write retracted_at", e);
    }
    if let Err(e) = tx.commit() {
        return db_error("tx commit", e);
    }
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "item": { "token": token, "revokedAt": now }
        })),
    )
}

/// GET /api/insights/:id/share — 列出该 insight 的所有分享(给 Boris 看自己发了几条)
pub async fn list_shares_for_insight(
    State(state): State<AppState>,
    user_id: UserId,
    Path(insight_id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let owns: bool = db
        .query_row(
            "SELECT COUNT(*) > 0 FROM insights WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
            params![insight_id, &user_id.0],
            |r| r.get(0),
        )
        .unwrap_or(false);
    if !owns {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察" })),
        );
    }
    let mut stmt = match db.prepare(
        "SELECT token, insight_id, report_id, show_notes, created_at, revoked_at
         FROM share_links WHERE insight_id = ?1 ORDER BY created_at DESC",
    ) {
        Ok(s) => s,
        Err(e) => return db_error("list_shares prepare", e),
    };
    let rows = match stmt.query_map(params![insight_id], |row| {
        let show_notes_int: i64 = row.get(3)?;
        Ok(ShareLink {
            token: row.get(0)?,
            insight_id: row.get(1)?,
            report_id: row.get(2)?,
            show_notes: show_notes_int != 0,
            created_at: row.get(4)?,
            revoked_at: row.get(5)?,
        })
    }) {
        Ok(r) => r,
        Err(e) => return db_error("list_shares query", e),
    };
    let items: Vec<ShareLink> = rows.filter_map(|r| r.ok()).collect();
    (
        StatusCode::OK,
        Json(json!({ "success": true, "items": items, "count": items.len() })),
    )
}

// ============ 公开 GET /r/:token(无 session) ============

/// GET /r/:token — 公开分享页。
/// 返回最小 HTML 框架,JS 再 fetch /r/:token/data 拿 JSON 数据后渲染 MD。
/// 撤销 → 410 Gone(明确告知"已撤销",非 404 模糊语义)。
pub async fn public_share_page(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> (StatusCode, Html<String>) {
    let db = state.db.lock();
    let row: Option<(Option<String>,)> = db
        .query_row(
            "SELECT revoked_at FROM share_links WHERE token = ?1",
            params![&token],
            |r| Ok((r.get::<_, Option<String>>(0)?,)),
        )
        .ok();
    match row {
        None => (
            StatusCode::NOT_FOUND,
            Html("<!doctype html><html><body><h1>404 — 链接不存在</h1></body></html>".into()),
        ),
        Some((Some(_),)) => (
            StatusCode::GONE,
            Html(SHARE_410_HTML.to_string()),
        ),
        Some((None,)) => (
            StatusCode::OK,
            // 实际渲染由前端 share.html / share.js 完成(T-106 实现);
            // 后端只保证返回外壳,数据走 /r/:token/data。
            Html(SHARE_SHELL_HTML.replace("{TOKEN}", &token)),
        ),
    }
}

/// GET /r/:token/data — 公开 JSON(给 share.js fetch)。
/// 撤销 → 410 Gone(JSON)。
pub async fn public_share_data(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let row: Option<(i64, i64, i64, Option<String>)> = db
        .query_row(
            "SELECT insight_id, report_id, show_notes, revoked_at \
             FROM share_links WHERE token = ?1",
            params![&token],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .ok();
    let (insight_id, report_id, show_notes_int, revoked_at) = match row {
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "error": "链接不存在" })),
            );
        }
        Some(r) => r,
    };
    if revoked_at.is_some() {
        return (
            StatusCode::GONE,
            Json(json!({ "success": false, "error": "已撤销" })),
        );
    }
    // 拉 insight + report + 引用的 sources
    let insight: Option<(String, String, String, String)> = db
        .query_row(
            "SELECT title, topic, template, created_at FROM insights WHERE id = ?1 AND deleted = 0",
            params![insight_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )
        .ok();
    let (title, topic, template, created_at) = match insight {
        Some(v) => v,
        None => {
            return (
                StatusCode::GONE,
                Json(json!({ "success": false, "error": "源洞察已删除" })),
            );
        }
    };
    // T-106 P3 / v0.2.1:同时拉 citations(让分享页 popover 用)
    let report: Option<(i64, String, String, String, String, String)> = db
        .query_row(
            "SELECT version, content_md, source_ids, generated_by, created_at, citations \
             FROM reports WHERE id = ?1",
            params![report_id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                ))
            },
        )
        .ok();
    let (version, content_md, source_ids_raw, generated_by, report_created, citations_raw) = match report {
        Some(v) => v,
        None => {
            return (
                StatusCode::GONE,
                Json(json!({ "success": false, "error": "报告已删除" })),
            );
        }
    };
    let source_ids: Vec<i64> = serde_json::from_str(&source_ids_raw).unwrap_or_default();
    let citations: serde_json::Value = serde_json::from_str(&citations_raw)
        .unwrap_or_else(|_| serde_json::Value::Array(vec![]));

    // 拉引用的 sources(标题、url、author)
    let show_notes = show_notes_int != 0;
    let mut sources_out: Vec<JsonValue> = Vec::new();
    for sid in &source_ids {
        let s: Option<(String, Option<String>, String, String)> = db
            .query_row(
                "SELECT title, url, author, note FROM sources \
                 WHERE id = ?1 AND deleted = 0",
                params![sid],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                },
            )
            .ok();
        if let Some((stitle, surl, sauthor, snote)) = s {
            let mut obj = json!({
                "id": sid,
                "title": stitle,
                "url": surl,
                "author": sauthor,
            });
            if show_notes && !snote.is_empty() {
                obj["note"] = json!(snote);
            }
            sources_out.push(obj);
        }
    }

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "insight": {
                "title": title,
                "topic": topic,
                "template": template,
                "createdAt": created_at,
            },
            "report": {
                "version": version,
                "contentMd": content_md,
                "generatedBy": generated_by,
                "createdAt": report_created,
                "citations": citations,
            },
            "sources": sources_out,
            "showNotes": show_notes,
        })),
    )
}

// ============ Public HTML 外壳 ============
// 不挂任何模块代码;只引 share.css + share.js(T-106 写)。

const SHARE_SHELL_HTML: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<meta name="robots" content="noindex,nofollow">
<title>洞察分享</title>
<link rel="stylesheet" href="/assets/css/share.css">
</head>
<body>
<div id="share-app" data-token="{TOKEN}">
  <div class="share-loading">加载中…</div>
</div>
<script src="/assets/js/share.js"></script>
</body>
</html>"#;

const SHARE_410_HTML: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>链接已撤销</title>
<style>
  body { font-family: -apple-system, BlinkMacSystemFont, "PingFang SC", sans-serif;
         max-width: 560px; margin: 14vh auto; padding: 0 24px; color: #1F2937; }
  h1 { font-size: 22px; margin-bottom: 12px; }
  p  { color: #6B7280; line-height: 1.6; }
  .icon { font-size: 48px; margin-bottom: 16px; }
</style>
</head>
<body>
  <div class="icon">🔗</div>
  <h1>这个分享链接已经撤销</h1>
  <p>洞察作者已经把这份链接撤掉了。如果你确实需要内容,请直接找作者。</p>
</body>
</html>"#;

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
    async fn body_text(resp: axum::response::Response) -> String {
        let body = to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        String::from_utf8_lossy(&body).to_string()
    }

    async fn setup_published(app: &axum::Router, cookie: &str) -> (i64, i64, String) {
        // Create insight
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/insights")
                    .header("Cookie", cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"x"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let iid = body_json(resp).await["item"]["id"].as_i64().unwrap();
        // Create report
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid}/reports"))
                    .header("Cookie", cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"template":"survey","contentMd":"Hi"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let rid = body_json(resp).await["item"]["id"].as_i64().unwrap();
        // T-107 v0.2:publish 替代 v0.1 的 POST /share
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid}/publish"))
                    .header("Cookie", cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let token = body_json(resp).await["item"]["token"].as_str().unwrap().to_string();
        (iid, rid, token)
    }

    #[tokio::test]
    async fn share_then_public_get() {
        let state = test_state();
        let (_u, tok) = create_test_user(&state, "u1", "Pa55word1");
        let cookie = auth_cookie(&tok);
        let app = crate::build_app(state);

        let (_iid, _rid, token) = setup_published(&app, &cookie).await;

        // 公开 GET /r/:token (无 cookie)
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/r/{token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let html = body_text(resp).await;
        assert!(html.contains(&token));

        // /r/:token/data 拿数据
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/r/{token}/data"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["success"], true);
        assert_eq!(j["report"]["contentMd"], "Hi");
    }

    #[tokio::test]
    async fn retract_returns_410_on_public() {
        let state = test_state();
        let (_u, tok) = create_test_user(&state, "u2", "Pa55word1");
        let cookie = auth_cookie(&tok);
        let app = crate::build_app(state);

        let (iid, _rid, token) = setup_published(&app, &cookie).await;

        // T-107 v0.2:用 /retract 撤回
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid}/retract"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // 再访问 → 410(HTML)
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/r/{token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::GONE);

        // JSON /data 也 410
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/r/{token}/data"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::GONE);

        // 撤回后 insight.status = editing
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/insights/{iid}"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["status"], "editing");
    }

    #[tokio::test]
    async fn double_publish_same_report_rejected() {
        let state = test_state();
        let (_u, tok) = create_test_user(&state, "u-dp", "Pa55word1");
        let cookie = auth_cookie(&tok);
        let app = crate::build_app(state);

        let (iid, _rid, _t1) = setup_published(&app, &cookie).await;
        // 撤回
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid}/retract"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // 再发同一版 → 409(spec 原则 5:同 report 版本最多发一次)
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid}/publish"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn publish_auto_revokes_previous_active() {
        let state = test_state();
        let (_u, tok) = create_test_user(&state, "u-rev2", "Pa55word1");
        let cookie = auth_cookie(&tok);
        let app = crate::build_app(state);

        // Setup: insight + 2 reports;publish v1,然后 publish v2 → v1 应自动撤回
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
        // v1
        let resp = app
            .clone()
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
        let rid_v1 = body_json(resp).await["item"]["id"].as_i64().unwrap();
        // publish v1
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid}/publish"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(format!(r#"{{"reportId":{rid_v1}}}"#)))
                    .unwrap(),
            )
            .await
            .unwrap();
        let t1 = body_json(resp).await["item"]["token"].as_str().unwrap().to_string();

        // v2
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid}/reports"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"template":"survey","contentMd":"v2"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let rid_v2 = body_json(resp).await["item"]["id"].as_i64().unwrap();
        // publish v2 (auto-revokes t1)
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid}/publish"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(format!(r#"{{"reportId":{rid_v2}}}"#)))
                    .unwrap(),
            )
            .await
            .unwrap();
        let t2 = body_json(resp).await["item"]["token"].as_str().unwrap().to_string();
        assert_ne!(t1, t2);

        // t1 应该已撤回 → 410
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/r/{t1}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::GONE);

        // t2 应该 200
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/r/{t2}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn token_is_random_base64url() {
        let t1 = generate_token();
        let t2 = generate_token();
        assert_ne!(t1, t2);
        assert!(t1.len() >= 40); // 32 bytes base64url ≈ 43 chars
        assert!(t1.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[tokio::test]
    async fn other_user_cannot_retract() {
        let state = test_state();
        let (_a, tok_a) = create_test_user(&state, "alice", "Pa55word1");
        let (_b, tok_b) = create_test_user(&state, "bob", "Pa55word1");
        let cookie_a = auth_cookie(&tok_a);
        let cookie_b = auth_cookie(&tok_b);
        let app = crate::build_app(state);

        let (iid, _rid, _token) = setup_published(&app, &cookie_a).await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid}/retract"))
                    .header("Cookie", &cookie_b)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // bob 看不到这个 insight,没 active 分享给他 → 409
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }
}
