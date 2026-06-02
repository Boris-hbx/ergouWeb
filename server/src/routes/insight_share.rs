//! `/api/insight-tasks/:id/{publish,retract,share}` + 公开 `/r/:token` —— 洞察报告分享(v0.4 / T-126)。
//!
//! 按 v0.3 单表模型(`insight_tasks` + `insight_reports`)恢复 v0.2 分享能力,蓝本 `share_links.rs`。
//! 设计要点(spec § 十二):
//!   - token 32 字节 URL-safe 随机(2^256 不可爆破),绑定到具体 report.id
//!   - **永久有效 + 手动撤销**:撤销只置 `revoked_at`(留历史不物删);撤销 → 公开 GET 返回 410 Gone
//!   - 同一 report 版本最多发一次(发过则 409,需先生成新版本再发);publish auto-revoke 既往 active
//!   - `include_notes=false` → 公开数据里裁掉「思辨」章节(发"干净版");默认 true 保留
//!   - 公开页 `/r/:token` 无 session;**不改 task.status**(v0.3 状态机只有 ready/processing/done,分享正交)
//!   - 公开页带"AI 生成"页脚(前端 share.js 渲染)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Html,
    Json,
};
use chrono::Utc;
use rand::RngCore;
use rusqlite::params;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use tracing::error;

use crate::auth::UserId;
use crate::state::AppState;

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn db_error(ctx: &str, e: rusqlite::Error) -> (StatusCode, Json<JsonValue>) {
    error!(target: "insight_share", "{} db error: {}", ctx, e);
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

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Default)]
pub struct PublishRequest {
    /// 缺省 = task.current_report_id
    #[serde(rename = "reportId", default)]
    pub report_id: Option<i64>,
    /// 默认 true(保留思辨);false = 发"干净版"
    #[serde(rename = "includeNotes", default = "default_true")]
    pub include_notes: bool,
}

/// 从报告 markdown 裁掉「思辨」章节(include_notes=false 时):
/// 找到标题行含"思辨"的那节,删到下一个**同级或更高级**标题为止。
fn strip_sibian(md: &str) -> String {
    let lines: Vec<&str> = md.lines().collect();
    let mut start: Option<usize> = None;
    let mut level = 0usize;
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim_start();
        if t.starts_with('#') && l.contains("思辨") {
            level = t.chars().take_while(|&c| c == '#').count();
            start = Some(i);
            break;
        }
    }
    let Some(s) = start else {
        return md.to_string();
    };
    let mut end = lines.len();
    for (j, l) in lines.iter().enumerate().skip(s + 1) {
        let t = l.trim_start();
        if t.starts_with('#') {
            let lv = t.chars().take_while(|&c| c == '#').count();
            if lv <= level {
                end = j;
                break;
            }
        }
    }
    let mut out: Vec<&str> = Vec::new();
    out.extend_from_slice(&lines[..s]);
    out.extend_from_slice(&lines[end..]);
    out.join("\n").trim_end().to_string()
}

// ============ 作者侧(/api,需 session/PAT) ============

/// POST /api/insight-tasks/:id/publish — 发布只读分享链接。
pub async fn publish(
    State(state): State<AppState>,
    user_id: UserId,
    Path(task_id): Path<i64>,
    Json(req): Json<PublishRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let mut db = state.db.lock();
    let current: Option<i64> = db
        .query_row(
            "SELECT current_report_id FROM insight_tasks \
             WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
            params![task_id, &user_id.0],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    let report_id = match req.report_id.or(current) {
        Some(id) => id,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": "没有可发布的报告 — 先生成 v1" })),
            );
        }
    };
    // report 必须属于该 task + 该 user
    let owns: bool = db
        .query_row(
            "SELECT COUNT(*) > 0 FROM insight_reports r \
             JOIN insight_tasks t ON r.task_id = t.id \
             WHERE r.id = ?1 AND r.task_id = ?2 AND t.user_id = ?3 AND t.deleted = 0",
            params![report_id, task_id, &user_id.0],
            |r| r.get(0),
        )
        .unwrap_or(false);
    if !owns {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到报告" })),
        );
    }
    // 同一 report 版本最多发一次(发过 → 行存在 → 409,即便已撤回)
    let already: bool = db
        .query_row(
            "SELECT COUNT(*) > 0 FROM insight_share_links WHERE report_id = ?1",
            params![report_id],
            |r| r.get(0),
        )
        .unwrap_or(false);
    if already {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": "该报告版本已发布过;撤回后需先生成新版本才能再发"
            })),
        );
    }

    let now = now_rfc3339();
    let token = generate_token();
    let tx = match db.transaction() {
        Ok(t) => t,
        Err(e) => return db_error("tx begin", e),
    };
    // auto-revoke 该 task 既往 active
    if let Err(e) = tx.execute(
        "UPDATE insight_share_links SET revoked_at = ?1 \
         WHERE task_id = ?2 AND revoked_at IS NULL",
        params![&now, task_id],
    ) {
        return db_error("auto-revoke", e);
    }
    if let Err(e) = tx.execute(
        "INSERT INTO insight_share_links (token, task_id, report_id, include_notes, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            &token,
            task_id,
            report_id,
            if req.include_notes { 1 } else { 0 },
            &now
        ],
    ) {
        return db_error("insert share", e);
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
                "taskId": task_id,
                "reportId": report_id,
                "includeNotes": req.include_notes,
                "createdAt": now,
            }
        })),
    )
}

/// POST /api/insight-tasks/:id/retract — 撤回当前 active 分享。
pub async fn retract(
    State(state): State<AppState>,
    user_id: UserId,
    Path(task_id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let active: Option<String> = db
        .query_row(
            "SELECT sl.token FROM insight_share_links sl \
             JOIN insight_tasks t ON sl.task_id = t.id \
             WHERE sl.task_id = ?1 AND sl.revoked_at IS NULL \
                   AND t.user_id = ?2 AND t.deleted = 0",
            params![task_id, &user_id.0],
            |r| r.get(0),
        )
        .ok();
    let Some(token) = active else {
        return (
            StatusCode::CONFLICT,
            Json(json!({ "success": false, "error": "没有 active 分享可撤回" })),
        );
    };
    let now = now_rfc3339();
    match db.execute(
        "UPDATE insight_share_links SET revoked_at = ?1 WHERE token = ?2",
        params![&now, &token],
    ) {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({ "success": true, "item": { "token": token, "revokedAt": now } })),
        ),
        Err(e) => db_error("retract", e),
    }
}

/// GET /api/insight-tasks/:id/share — 列出该任务所有分享(active + revoked)。
pub async fn list_shares(
    State(state): State<AppState>,
    user_id: UserId,
    Path(task_id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let owns: bool = db
        .query_row(
            "SELECT COUNT(*) > 0 FROM insight_tasks WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
            params![task_id, &user_id.0],
            |r| r.get(0),
        )
        .unwrap_or(false);
    if !owns {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察任务" })),
        );
    }
    let mut stmt = match db.prepare(
        "SELECT token, report_id, include_notes, created_at, revoked_at \
         FROM insight_share_links WHERE task_id = ?1 ORDER BY created_at DESC",
    ) {
        Ok(s) => s,
        Err(e) => return db_error("list prepare", e),
    };
    let rows = match stmt.query_map(params![task_id], |row| {
        let include_notes: i64 = row.get(2)?;
        Ok(json!({
            "token": row.get::<_, String>(0)?,
            "reportId": row.get::<_, i64>(1)?,
            "includeNotes": include_notes != 0,
            "createdAt": row.get::<_, String>(3)?,
            "revokedAt": row.get::<_, Option<String>>(4)?,
        }))
    }) {
        Ok(r) => r,
        Err(e) => return db_error("list query", e),
    };
    let items: Vec<JsonValue> = rows.filter_map(|r| r.ok()).collect();
    (
        StatusCode::OK,
        Json(json!({ "success": true, "items": items, "count": items.len() })),
    )
}

// ============ 公开侧(/r/:token,无 session) ============

/// GET /r/:token — 公开分享页外壳。撤销→410,不存在→404。
pub async fn public_page(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> (StatusCode, Html<String>) {
    let db = state.db.lock();
    let row: Option<Option<String>> = db
        .query_row(
            "SELECT revoked_at FROM insight_share_links WHERE token = ?1",
            params![&token],
            |r| r.get::<_, Option<String>>(0),
        )
        .ok();
    match row {
        None => (
            StatusCode::NOT_FOUND,
            Html("<!doctype html><html><body><h1>404 — 链接不存在</h1></body></html>".into()),
        ),
        Some(Some(_)) => (StatusCode::GONE, Html(SHARE_410_HTML.to_string())),
        Some(None) => (
            StatusCode::OK,
            Html(SHARE_SHELL_HTML.replace("{TOKEN}", &token)),
        ),
    }
}

/// GET /r/:token/data — 公开 JSON(给 share.js)。撤销→410。include_notes=false 裁掉思辨。
pub async fn public_data(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let row: Option<(i64, i64, i64, Option<String>)> = db
        .query_row(
            "SELECT task_id, report_id, include_notes, revoked_at \
             FROM insight_share_links WHERE token = ?1",
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
    let (task_id, report_id, include_notes_int, revoked_at) = match row {
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
    let include_notes = include_notes_int != 0;

    let task: Option<(String, String, String)> = db
        .query_row(
            "SELECT title, input_type, created_at FROM insight_tasks \
             WHERE id = ?1 AND deleted = 0",
            params![task_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .ok();
    let (title, input_type, task_created) = match task {
        Some(v) => v,
        None => {
            return (
                StatusCode::GONE,
                Json(json!({ "success": false, "error": "源任务已删除" })),
            );
        }
    };
    let report: Option<(i64, String, String, String, String)> = db
        .query_row(
            "SELECT version, content_md, template, model_used, created_at \
             FROM insight_reports WHERE id = ?1",
            params![report_id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            },
        )
        .ok();
    let (version, content_md, template, model_used, report_created) = match report {
        Some(v) => v,
        None => {
            return (
                StatusCode::GONE,
                Json(json!({ "success": false, "error": "报告已删除" })),
            );
        }
    };
    let out_md = if include_notes {
        content_md
    } else {
        strip_sibian(&content_md)
    };
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "task": {
                "title": title,
                "inputType": input_type,
                "createdAt": task_created,
            },
            "report": {
                "version": version,
                "contentMd": out_md,
                "template": template,
                "modelUsed": model_used,
                "createdAt": report_created,
            },
            "includeNotes": include_notes,
        })),
    )
}

// ============ Public HTML 外壳 ============

const SHARE_SHELL_HTML: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<meta name="robots" content="noindex,nofollow">
<title>洞察分享</title>
<link rel="stylesheet" href="/assets/css/share.css?v=20260601a">
</head>
<body>
<div id="share-app" data-token="{TOKEN}">
  <div class="share-loading">加载中…</div>
</div>
<script src="/assets/js/share.js?v=20260601a"></script>
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
        let body = to_bytes(resp.into_body(), 2_000_000).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    /// 建任务 + 写一版报告,返回 (task_id, report_id)。
    async fn setup_task_with_report(
        app: &axum::Router,
        cookie: &str,
        content_md: &str,
    ) -> (i64, i64) {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/insight-tasks")
                    .header("Cookie", cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"inputContent":"洞察一下 X","inputType":"topic"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let tid = body_json(resp).await["item"]["id"].as_i64().unwrap();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insight-tasks/{tid}/reports"))
                    .header("Cookie", cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"template":"survey","contentMd":{}}}"#,
                        serde_json::to_string(content_md).unwrap()
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        let rid = body_json(resp).await["item"]["id"].as_i64().unwrap();
        (tid, rid)
    }

    async fn publish(app: &axum::Router, cookie: &str, tid: i64, body: &str) -> JsonValue {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insight-tasks/{tid}/publish"))
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
    async fn publish_then_public_get() {
        let state = test_state();
        let (_u, tok) = create_test_user(&state, "s1", "Pa55word1");
        let cookie = auth_cookie(&tok);
        let app = crate::build_app(state);
        let (tid, _rid) =
            setup_task_with_report(&app, &cookie, "正文\n\n## 5. 思辨\n质疑\n\n## 6. 来源\nx")
                .await;
        let token = publish(&app, &cookie, tid, "{}").await["item"]["token"]
            .as_str()
            .unwrap()
            .to_string();

        // 公开 /r/:token(无 cookie)→ 200
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

        // /data 默认含思辨
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
        let j = body_json(resp).await;
        assert_eq!(j["success"], true);
        assert!(j["report"]["contentMd"].as_str().unwrap().contains("思辨"));
    }

    #[tokio::test]
    async fn include_notes_false_strips_sibian() {
        let state = test_state();
        let (_u, tok) = create_test_user(&state, "s2", "Pa55word1");
        let cookie = auth_cookie(&tok);
        let app = crate::build_app(state);
        let (tid, _rid) = setup_task_with_report(
            &app,
            &cookie,
            "正文\n\n## 5. 思辨\n质疑内容\n\n## 6. 来源\nx",
        )
        .await;
        let token = publish(&app, &cookie, tid, r#"{"includeNotes":false}"#).await["item"]["token"]
            .as_str()
            .unwrap()
            .to_string();
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
        let md = j["report"]["contentMd"].as_str().unwrap();
        assert!(!md.contains("质疑内容"), "思辨章节应被裁掉");
        assert!(md.contains("## 6. 来源"), "思辨后的章节应保留");
    }

    #[tokio::test]
    async fn retract_returns_410() {
        let state = test_state();
        let (_u, tok) = create_test_user(&state, "s3", "Pa55word1");
        let cookie = auth_cookie(&tok);
        let app = crate::build_app(state);
        let (tid, _rid) = setup_task_with_report(&app, &cookie, "hi").await;
        let token = publish(&app, &cookie, tid, "{}").await["item"]["token"]
            .as_str()
            .unwrap()
            .to_string();
        // retract
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insight-tasks/{tid}/retract"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        // 公开 → 410
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/r/{token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::GONE);
    }

    #[tokio::test]
    async fn double_publish_same_report_409() {
        let state = test_state();
        let (_u, tok) = create_test_user(&state, "s4", "Pa55word1");
        let cookie = auth_cookie(&tok);
        let app = crate::build_app(state);
        let (tid, _rid) = setup_task_with_report(&app, &cookie, "hi").await;
        publish(&app, &cookie, tid, "{}").await;
        // retract
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insight-tasks/{tid}/retract"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // 再发同一版 → 409
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insight-tasks/{tid}/publish"))
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
    async fn token_is_random_base64url() {
        let t1 = generate_token();
        let t2 = generate_token();
        assert_ne!(t1, t2);
        assert!(t1.len() >= 40);
        assert!(t1
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }
}
