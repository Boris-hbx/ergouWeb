//! `/api/praxis/journal` - Praxis daily-operation journal (今日经营记录).
//!
//! See SPEC: `C:\Project\ergouPM\specs\praxis\spec.md` §5

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{NaiveDate, Utc};
use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use tracing::{error, warn};

use crate::auth::AdminUserId;
use crate::models::praxis_journal::{CreateJournalRequest, PraxisJournal, UpdateJournalRequest};
use crate::services::llm::LlmClient;
use crate::state::AppState;

const SELECT_COLS: &str =
    "id, entry_date, raw_text, tags, structured, analyzed_at, created_at, updated_at";

const DEFAULT_LIMIT: i64 = 60;
const RAW_TEXT_MAX: usize = 20_000;

/// System prompt for the structuring call. Demands strict JSON, judgement over
/// flattery, and a fixed schema (spec §5.3 / design doc §3.4 / §8).
const ANALYZE_SYSTEM_PROMPT: &str = r#"你是「Praxis · 今日经营」的结构化助手。用户把自己当一家公司经营，把今天的自然语言记录交给你。
请把它结构化为对长期经营有用的信号，**只输出一个严格的 JSON 对象**，不要任何解释、前后缀或 Markdown 代码块。

JSON 结构（字段固定，可空但必须存在）：
{
  "summary": "一句话概括今天（≤40字，有判断，不灌鸡汤）",
  "events": ["关键事件，≤5条，每条简短"],
  "boards": ["命中的经营板块 key，取值仅限 strategy/skill/ops/fin/brand/rel/cog/risk"],
  "value": "今天的主基调，仅取一个：build(主动建设)|respond(被动响应)|firefight(救火)|maintain(维护)|rest(休整)",
  "risks": ["风险信号，可空数组"],
  "tomorrow": "一个明日调整，小而具体、可执行"
}

要求：
- 基于用户记录本身，不空泛、不编造未提及的事实。
- 区分事实/判断/建议：events 是事实，summary 是判断，tomorrow 是建议。
- 输出短、克制、有信息量。
- boards/value 用上面给定的英文 key，其余字段用中文。"#;

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn today_str() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

fn db_error(ctx: &str, e: rusqlite::Error) -> (StatusCode, Json<JsonValue>) {
    error!(target: "praxis_journal", "{} db error: {}", ctx, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "error": "内部错误" })),
    )
}

fn validate_date(s: &str) -> Result<String, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map(|_| s.to_string())
        .map_err(|_| "entryDate must be YYYY-MM-DD".to_string())
}

/// Parse a JSON-text DB column, falling back to `default` when empty/invalid.
fn parse_json_text(s: &str, default: JsonValue) -> JsonValue {
    let t = s.trim();
    if t.is_empty() {
        return default;
    }
    serde_json::from_str(t).unwrap_or(default)
}

/// Serialize a tags value for storage; coerce non-objects to `{}`.
fn tags_to_text(tags: &JsonValue) -> String {
    if tags.is_object() {
        tags.to_string()
    } else {
        "{}".to_string()
    }
}

fn row_to_journal(row: &rusqlite::Row) -> rusqlite::Result<PraxisJournal> {
    let tags_s: String = row.get(3)?;
    let structured_s: String = row.get(4)?;
    Ok(PraxisJournal {
        id: row.get(0)?,
        entry_date: row.get(1)?,
        raw_text: row.get(2)?,
        tags: parse_json_text(&tags_s, json!({})),
        structured: parse_json_text(&structured_s, JsonValue::Null),
        analyzed_at: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn load_journal(db: &Connection, user_id: &str, id: i64) -> Option<PraxisJournal> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM praxis_journal WHERE id = ?1 AND user_id = ?2 AND deleted = 0"
    );
    db.query_row(&sql, params![id, user_id], row_to_journal)
        .ok()
}

#[derive(Debug, Deserialize, Default)]
pub struct ListQuery {
    pub date: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<i64>,
}

pub fn list_journal_impl(
    db: &Connection,
    user_id: &str,
    q: &ListQuery,
) -> Result<Vec<PraxisJournal>, String> {
    let mut sql =
        format!("SELECT {SELECT_COLS} FROM praxis_journal WHERE user_id = ?1 AND deleted = 0");
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(user_id.to_string())];

    if let Some(date) = q.date.as_deref().filter(|s| !s.is_empty()) {
        let date = validate_date(date)?;
        args.push(Box::new(date));
        sql.push_str(&format!(" AND entry_date = ?{}", args.len()));
    } else {
        if let Some(from) = q.from.as_deref().filter(|s| !s.is_empty()) {
            let from = validate_date(from)?;
            args.push(Box::new(from));
            sql.push_str(&format!(" AND entry_date >= ?{}", args.len()));
        }
        if let Some(to) = q.to.as_deref().filter(|s| !s.is_empty()) {
            let to = validate_date(to)?;
            args.push(Box::new(to));
            sql.push_str(&format!(" AND entry_date <= ?{}", args.len()));
        }
    }

    let limit = q
        .limit
        .filter(|n| *n > 0 && *n <= 500)
        .unwrap_or(DEFAULT_LIMIT);
    sql.push_str(" ORDER BY entry_date DESC, id DESC");
    args.push(Box::new(limit));
    sql.push_str(&format!(" LIMIT ?{}", args.len()));

    let mut stmt = db.prepare(&sql).map_err(|e| format!("prepare: {e}"))?;
    let param_refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref()).collect();
    let rows = stmt
        .query_map(param_refs.as_slice(), row_to_journal)
        .map_err(|e| format!("query: {e}"))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn create_journal_impl(
    db: &Connection,
    user_id: &str,
    req: &CreateJournalRequest,
) -> Result<PraxisJournal, String> {
    let entry_date = match req.entry_date.as_deref().filter(|s| !s.is_empty()) {
        Some(d) => validate_date(d)?,
        None => today_str(),
    };
    if req.raw_text.chars().count() > RAW_TEXT_MAX {
        return Err("rawText too long".into());
    }
    let tags = req.tags.clone().unwrap_or_else(|| json!({}));
    let now = now_rfc3339();

    db.execute(
        "INSERT INTO praxis_journal
           (user_id, entry_date, raw_text, tags, structured, analyzed_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, '', NULL, ?5, ?5)",
        params![
            user_id,
            &entry_date,
            &req.raw_text,
            tags_to_text(&tags),
            &now
        ],
    )
    .map_err(|e| format!("insert: {e}"))?;

    let id = db.last_insert_rowid();
    load_journal(db, user_id, id).ok_or_else(|| "reload failed".into())
}

pub fn update_journal_impl(
    db: &Connection,
    user_id: &str,
    id: i64,
    patch: UpdateJournalRequest,
) -> Result<Option<PraxisJournal>, String> {
    let Some(mut current) = load_journal(db, user_id, id) else {
        return Ok(None);
    };

    if let Some(v) = patch.fields.get("entryDate").and_then(|v| v.as_str()) {
        current.entry_date = validate_date(v)?;
    }
    if let Some(v) = patch.fields.get("rawText").and_then(|v| v.as_str()) {
        if v.chars().count() > RAW_TEXT_MAX {
            return Err("rawText too long".into());
        }
        current.raw_text = v.to_string();
    }
    if let Some(v) = patch.fields.get("tags") {
        current.tags = v.clone();
    }
    // 允许用户修正 AI 归类结果（spec §5.2）。
    if let Some(v) = patch.fields.get("structured") {
        current.structured = v.clone();
    }

    let now = now_rfc3339();
    let structured_text = if current.structured.is_null() {
        String::new()
    } else {
        current.structured.to_string()
    };
    db.execute(
        "UPDATE praxis_journal
         SET entry_date = ?1, raw_text = ?2, tags = ?3, structured = ?4, updated_at = ?5
         WHERE id = ?6 AND user_id = ?7 AND deleted = 0",
        params![
            &current.entry_date,
            &current.raw_text,
            tags_to_text(&current.tags),
            &structured_text,
            &now,
            id,
            user_id,
        ],
    )
    .map_err(|e| format!("update: {e}"))?;

    Ok(load_journal(db, user_id, id))
}

/// Build the user message for the analyze call from raw text + tags.
fn build_analyze_input(raw_text: &str, tags: &JsonValue) -> String {
    let mut msg = format!("今日原文：\n{}", raw_text.trim());
    if tags.is_object() && !tags.as_object().map(|m| m.is_empty()).unwrap_or(true) {
        msg.push_str(&format!("\n\n快速信号标记：{tags}"));
    }
    msg
}

/// Extract a JSON object from an LLM response (tolerates ```json fences / prose).
fn parse_structured(text: &str) -> Option<JsonValue> {
    let trimmed = text.trim();
    if let Ok(v @ JsonValue::Object(_)) = serde_json::from_str::<JsonValue>(trimmed) {
        return Some(v);
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end <= start {
        return None;
    }
    match serde_json::from_str::<JsonValue>(&trimmed[start..=end]) {
        Ok(v @ JsonValue::Object(_)) => Some(v),
        _ => None,
    }
}

pub async fn list_journal(
    State(state): State<AppState>,
    admin: AdminUserId,
    Query(q): Query<ListQuery>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match list_journal_impl(&db, &admin.0, &q) {
        Ok(items) => (
            StatusCode::OK,
            Json(json!({ "success": true, "items": items, "count": items.len() })),
        ),
        Err(e) if e.contains("must") => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": e })),
        ),
        Err(e) => {
            error!(target: "praxis_journal", "list: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "内部错误" })),
            )
        }
    }
}

pub async fn create_journal(
    State(state): State<AppState>,
    admin: AdminUserId,
    Json(req): Json<CreateJournalRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match create_journal_impl(&db, &admin.0, &req) {
        Ok(item) => (
            StatusCode::OK,
            Json(json!({ "success": true, "item": item })),
        ),
        Err(e) if e.contains("must") || e.contains("too long") => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": e })),
        ),
        Err(e) => {
            error!(target: "praxis_journal", "create: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "内部错误" })),
            )
        }
    }
}

pub async fn update_journal(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path(id): Path<i64>,
    Json(patch): Json<UpdateJournalRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match update_journal_impl(&db, &admin.0, id, patch) {
        Ok(Some(item)) => (
            StatusCode::OK,
            Json(json!({ "success": true, "item": item })),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到记录" })),
        ),
        Err(e) if e.contains("must") || e.contains("too long") => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": e })),
        ),
        Err(e) => {
            error!(target: "praxis_journal", "update: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "内部错误" })),
            )
        }
    }
}

pub async fn delete_journal(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let now = now_rfc3339();
    match db.execute(
        "UPDATE praxis_journal SET deleted = 1, updated_at = ?1
         WHERE id = ?2 AND user_id = ?3 AND deleted = 0",
        params![&now, id, &admin.0],
    ) {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到记录" })),
        ),
        Ok(_) => (StatusCode::OK, Json(json!({ "success": true }))),
        Err(e) => db_error("delete", e),
    }
}

/// POST /journal/:id/analyze — call the existing chat LLM (same API key) to
/// structure `raw_text` into JSON, write back `structured` + `analyzed_at`.
/// On LLM/parse failure the row is left untouched (structured stays empty,
/// retryable) — never blocks the already-saved raw text (spec §5.3).
pub async fn analyze_journal(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    // 1. Load raw text + tags (short lock, dropped before the await).
    let (raw_text, tags) = {
        let db = state.db.lock();
        match load_journal(&db, &admin.0, id) {
            Some(j) => (j.raw_text, j.tags),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({ "success": false, "error": "未找到记录" })),
                )
            }
        }
    };

    if raw_text.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "原文为空，无法分析" })),
        );
    }

    // 2. Build a client with the user's configured key (same as 二狗对话).
    let client = match LlmClient::for_user(&state.db.lock(), &admin.0) {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "success": false, "error": "AI 服务未配置" })),
            )
        }
    };

    // 3. Call the LLM (no DB lock held across the await).
    let user_msg = build_analyze_input(&raw_text, &tags);
    let raw_reply = match client
        .simple_generate(ANALYZE_SYSTEM_PROMPT, &user_msg, 1024)
        .await
    {
        Ok(t) => t,
        Err(e) => {
            warn!(target: "praxis_journal", "analyze llm error id={}: {}", id, e);
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "success": false, "error": e, "retryable": true })),
            );
        }
    };

    // 4. Parse strict JSON; on failure leave the row untouched, allow retry.
    let structured = match parse_structured(&raw_reply) {
        Some(v) => v,
        None => {
            warn!(target: "praxis_journal", "analyze parse fail id={}", id);
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "success": false,
                    "error": "AI 返回格式异常，可重试",
                    "retryable": true
                })),
            );
        }
    };

    // 5. Write back structured + analyzed_at.
    let now = now_rfc3339();
    {
        let db = state.db.lock();
        if let Err(e) = db.execute(
            "UPDATE praxis_journal SET structured = ?1, analyzed_at = ?2, updated_at = ?2
             WHERE id = ?3 AND user_id = ?4 AND deleted = 0",
            params![structured.to_string(), &now, id, &admin.0],
        ) {
            return db_error("analyze write", e);
        }
    }

    let db = state.db.lock();
    match load_journal(&db, &admin.0, id) {
        Some(item) => (
            StatusCode::OK,
            Json(json!({ "success": true, "item": item })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到记录" })),
        ),
    }
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

    #[test]
    fn parse_structured_tolerates_fences() {
        let fenced = "```json\n{\"summary\":\"ok\",\"events\":[]}\n```";
        let v = parse_structured(fenced).expect("should parse");
        assert_eq!(v["summary"], "ok");
        assert!(parse_structured("not json at all").is_none());
        assert!(parse_structured("[1,2,3]").is_none());
    }

    #[tokio::test]
    async fn journal_requires_admin() {
        let state = test_state();
        let (_user_id, user_token) = create_test_user(&state, "pj-user", "Pa55word1");
        let app = crate::build_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/praxis/journal")
                    .header("Cookie", auth_cookie(&user_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn journal_crud_and_per_user_isolation() {
        let state = test_state();
        let (_admin_id, admin_token) = create_admin_user(&state, "pj-admin", "Pa55word1");
        let (_other_id, other_token) = create_admin_user(&state, "pj-other", "Pa55word1");
        let app = crate::build_app(state);

        // create
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/praxis/journal")
                    .header("Cookie", auth_cookie(&admin_token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"entryDate":"2026-06-30","rawText":"复盘了定位","tags":{"energy":"high"}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["success"], true);
        assert_eq!(j["item"]["entryDate"], "2026-06-30");
        assert_eq!(j["item"]["tags"]["energy"], "high");
        assert_eq!(j["item"]["structured"], JsonValue::Null);
        let id = j["item"]["id"].as_i64().unwrap();

        // other admin cannot see it
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/praxis/journal")
                    .header("Cookie", auth_cookie(&other_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["count"], 0);

        // patch: user fixes structured classification
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/praxis/journal/{id}"))
                    .header("Cookie", auth_cookie(&admin_token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"rawText":"复盘了定位与能力","structured":{"summary":"聚焦","boards":["strategy"]}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["rawText"], "复盘了定位与能力");
        assert_eq!(j["item"]["structured"]["boards"][0], "strategy");

        // date filter
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/praxis/journal?date=2026-06-30")
                    .header("Cookie", auth_cookie(&admin_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["count"], 1);

        // delete
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/praxis/journal/{id}"))
                    .header("Cookie", auth_cookie(&admin_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["success"], true);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/praxis/journal")
                    .header("Cookie", auth_cookie(&admin_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["count"], 0);
    }

    #[tokio::test]
    async fn journal_validates_date_and_analyze_guards() {
        let state = test_state();
        let (_admin_id, admin_token) = create_admin_user(&state, "pj-admin-2", "Pa55word1");
        let app = crate::build_app(state);

        // bad date → 400
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/praxis/journal")
                    .header("Cookie", auth_cookie(&admin_token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"entryDate":"06/30/2026","rawText":"x"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // create empty-raw entry, analyze → 400 (empty raw, before LLM)
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/praxis/journal")
                    .header("Cookie", auth_cookie(&admin_token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"rawText":"   "}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = body_json(resp).await["item"]["id"].as_i64().unwrap();

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/praxis/journal/{id}/analyze"))
                    .header("Cookie", auth_cookie(&admin_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
