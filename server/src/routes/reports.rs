//! `/api/insights/:id/reports` â€” æŠ¥å‘Šç‰ˆæœ¬åŒ–ç«¯ç‚¹(T-105)ã€‚
//!
//! Claude Code POST åˆ›å»ºæ–° version;Boris åœ¨ Web ä¸Š PATCH åŒä¸€ version æ”¹ MD(spec åŽŸåˆ™ 3)ã€‚
//! åˆ›å»º report æ—¶(v0.2):status ä»Ž processing â†’ editing(Boris review);
//! current_report_id è‡ªåŠ¨æŒ‡å‘æ–° reportã€‚ä¿®è®¢æ¨¡å¼è¿˜ä¼šæ¸…ç©º insight.pending_revision_noteã€‚

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde_json::{json, Value as JsonValue};
use tracing::error;

use crate::auth::UserId;
use crate::models::report::{CreateReportRequest, Report, UpdateReportRequest};
use crate::state::AppState;

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn db_error(ctx: &str, e: rusqlite::Error) -> (StatusCode, Json<JsonValue>) {
    error!(target: "reports", "{} db error: {}", ctx, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "error": "å†…éƒ¨é”™è¯¯" })),
    )
}

/// v0.2:åŠ  4 å­—æ®µ revision_note / parent_report_id / published_at / retracted_at
/// v0.2.1:åŠ  citations å­—æ®µ(CC è¾“å‡ºçš„å¼•ç”¨æ¡ç›®)
const SELECT_COLS: &str = "id, insight_id, version, template, content_md, source_ids, \
    citations, revision_note, parent_report_id, generated_by, model_used, \
    published_at, retracted_at, created_at, updated_at";

fn row_to_report(row: &rusqlite::Row) -> rusqlite::Result<Report> {
    let src_ids_raw: String = row.get(5)?;
    let source_ids: Vec<i64> = serde_json::from_str(&src_ids_raw).unwrap_or_default();
    let citations_raw: String = row.get(6)?;
    let citations: Vec<crate::models::report::Citation> =
        serde_json::from_str(&citations_raw).unwrap_or_default();
    Ok(Report {
        id: row.get(0)?,
        insight_id: row.get(1)?,
        version: row.get(2)?,
        template: row.get(3)?,
        content_md: row.get(4)?,
        source_ids,
        citations,
        revision_note: row.get(7)?,
        parent_report_id: row.get(8)?,
        generated_by: row.get(9)?,
        model_used: row.get(10)?,
        published_at: row.get(11)?,
        retracted_at: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

/// å†…éƒ¨:ç¡®è®¤ user æ‹¥æœ‰è¯¥ insight
pub fn check_owns_insight(db: &Connection, user_id: &str, insight_id: i64) -> bool {
    db.query_row(
        "SELECT COUNT(*) > 0 FROM insights WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
        params![insight_id, user_id],
        |r| r.get(0),
    )
    .unwrap_or(false)
}

/// POST /api/insights/:id/reports â€” Claude Code æäº¤æ–°æŠ¥å‘Šç‰ˆæœ¬
/// å‰¯ä½œç”¨(v0.2):status ä»Ž processing â†’ editing;current_report_id è‡ªåŠ¨æŒ‡å‘æ–° report;
/// ä¿®è®¢æ¨¡å¼(parent_report_id éžç©º)è¿˜ä¼šæ¸…ç©º insight.pending_revision_noteã€‚
pub async fn create_report(
    State(state): State<AppState>,
    user_id: UserId,
    Path(insight_id): Path<i64>,
    Json(req): Json<CreateReportRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    if !check_owns_insight(&db, &user_id.0, insight_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "æœªæ‰¾åˆ°æ´žå¯Ÿ" })),
        );
    }

    // è®¡ç®—ä¸‹ä¸€ä¸ª version
    let next_version: i64 = db
        .query_row(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM reports WHERE insight_id = ?1",
            params![insight_id],
            |r| r.get(0),
        )
        .unwrap_or(1);

    let source_ids_json = serde_json::to_string(&req.source_ids).unwrap_or_else(|_| "[]".into());
    let citations_json = serde_json::to_string(&req.citations).unwrap_or_else(|_| "[]".into());
    let now = now_rfc3339();
    // v0.2:INSERT åŠ  revision_note å’Œ parent_report_id
    // v0.2.1:INSERT åŠ  citations
    let res = db.execute(
        "INSERT INTO reports (insight_id, version, template, content_md, source_ids, citations,
                              revision_note, parent_report_id,
                              generated_by, model_used, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
        params![
            insight_id,
            next_version,
            &req.template,
            &req.content_md,
            &source_ids_json,
            &citations_json,
            &req.revision_note,
            req.parent_report_id,
            &req.generated_by,
            &req.model_used,
            &now,
        ],
    );
    let new_report_id = match res {
        Ok(_) => db.last_insert_rowid(),
        Err(e) => return db_error("create insert", e),
    };

    // å‰¯ä½œç”¨(v0.2):
    //   - status:processing â†’ 'editing'(v0.1 æ˜¯ 'drafting',v0.2 é‡å‘½å)
    //   - current_report_id:æŒ‡å‘æ–° report
    //   - ä¿®è®¢æ¨¡å¼(parent_report_id éžç©º):æ¸…ç©º pending_revision_note(spec Â§ 6.3.3 / Â§ 8.3)
    let clear_pending_sql = if req.parent_report_id.is_some() {
        ", pending_revision_note = ''"
    } else {
        ""
    };
    let _ = db.execute(
        &format!(
            "UPDATE insights SET status = 'editing', current_report_id = ?1{clear_pending_sql}, updated_at = ?2 \
             WHERE id = ?3 AND user_id = ?4"
        ),
        params![new_report_id, &now, insight_id, &user_id.0],
    );

    fetch_report(&db, insight_id, new_report_id)
}

/// GET /api/insights/:id/reports â€” åŽ†å²ç‰ˆæœ¬åˆ—è¡¨
pub async fn list_reports(
    State(state): State<AppState>,
    user_id: UserId,
    Path(insight_id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    if !check_owns_insight(&db, &user_id.0, insight_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "æœªæ‰¾åˆ°æ´žå¯Ÿ" })),
        );
    }
    let sql =
        format!("SELECT {SELECT_COLS} FROM reports WHERE insight_id = ?1 ORDER BY version DESC");
    let mut stmt = match db.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return db_error("list prepare", e),
    };
    let rows = match stmt.query_map(params![insight_id], row_to_report) {
        Ok(r) => r,
        Err(e) => return db_error("list query", e),
    };
    let items: Vec<Report> = rows.filter_map(|r| r.ok()).collect();
    (
        StatusCode::OK,
        Json(json!({ "success": true, "items": items, "count": items.len() })),
    )
}

/// GET /api/insights/:id/reports/latest(T-107 v0.2) â€” æ‹¿å½“å‰ insight çš„æœ€æ–°ç‰ˆæŠ¥å‘Š
/// Claude Code åˆ¤æ–­ä¿®è®¢æ¨¡å¼æ—¶è°ƒç”¨,æ¯”é€ä¸ªç¿» reports åˆ—è¡¨æ–¹ä¾¿ã€‚
/// æ‰¾ä¸åˆ°ä»»ä½• report â†’ 404(insight è¿˜åœ¨ collecting çŠ¶æ€)ã€‚
pub async fn get_latest_report(
    State(state): State<AppState>,
    user_id: UserId,
    Path(insight_id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    if !check_owns_insight(&db, &user_id.0, insight_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "æœªæ‰¾åˆ°æ´žå¯Ÿ" })),
        );
    }
    let sql = format!(
        "SELECT {SELECT_COLS} FROM reports WHERE insight_id = ?1 ORDER BY version DESC LIMIT 1"
    );
    match db.query_row(&sql, params![insight_id], row_to_report) {
        Ok(t) => (StatusCode::OK, Json(json!({ "success": true, "item": t }))),
        Err(rusqlite::Error::QueryReturnedNoRows) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "å°šæ— æŠ¥å‘Šç‰ˆæœ¬" })),
        ),
        Err(e) => db_error("get_latest", e),
    }
}

/// GET /api/insights/:id/reports/:version
pub async fn get_report_by_version(
    State(state): State<AppState>,
    user_id: UserId,
    Path((insight_id, version)): Path<(i64, i64)>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    if !check_owns_insight(&db, &user_id.0, insight_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "æœªæ‰¾åˆ°æ´žå¯Ÿ" })),
        );
    }
    let sql = format!("SELECT {SELECT_COLS} FROM reports WHERE insight_id = ?1 AND version = ?2");
    match db.query_row(&sql, params![insight_id, version], row_to_report) {
        Ok(t) => (StatusCode::OK, Json(json!({ "success": true, "item": t }))),
        Err(rusqlite::Error::QueryReturnedNoRows) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "æœªæ‰¾åˆ°è¯¥ç‰ˆæœ¬" })),
        ),
        Err(e) => db_error("get_report", e),
    }
}

/// PATCH /api/insights/:id/reports/:version â€” Boris æ‰‹æ”¹ MD
///
/// T-107 v0.2 è‡ªåŠ¨ fork(spec Â§ 8.3 + Â§ 5.3.3):
/// å¦‚æžœè¯¥ report ç‰ˆæœ¬ `published_at` éž NULL(å·²å‘å¸ƒè¿‡,æ— è®ºæ˜¯å¦å·²æ’¤å›ž),
/// ç›´æŽ¥ PATCH ä¼šæ”¹å†™åŽ†å²å¿«ç…§ã€‚æ‰€ä»¥æœåŠ¡ç«¯è‡ªåŠ¨:
///   1. æ–°å»º version+1 è¡Œ,parent_report_id=åŽŸç‰ˆ,content_md=æ–°å€¼,generated_by='boris-inline'
///   2. insight.current_report_id åˆ‡åˆ°æ–°ç‰ˆ
///   3. è¿”å›žæ–° report(æ–° id / æ–° version)
///
/// å‰ç«¯åº”åœ¨ç¼–è¾‘å‰é—®"ç¡®å®š?"(spec Â§ 5.3.3),ä½†å³ä½¿å‰ç«¯æ²¡é—®,åŽç«¯ä¹Ÿä¼šåšå¯¹ã€‚
pub async fn update_report(
    State(state): State<AppState>,
    user_id: UserId,
    Path((insight_id, version)): Path<(i64, i64)>,
    Json(patch): Json<UpdateReportRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let mut db = state.db.lock();
    if !check_owns_insight(&db, &user_id.0, insight_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "æœªæ‰¾åˆ°æ´žå¯Ÿ" })),
        );
    }
    let Some(content_md) = patch.content_md else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "需要 contentMd 字段" })),
        );
    };
    // æ‹¿çŽ°ç‰ˆçš„ id / published_at / template / source_ids / citations,
    // ç”¨æ¥å†³å®šèµ° fork è¿˜æ˜¯ inline;fork æ—¶åŒæ—¶æ‹·è´çˆ¶ç‰ˆæœ¬çš„ citations ä¸å˜ã€‚
    let row: Option<(i64, Option<String>, String, String, String)> = db
        .query_row(
            "SELECT id, published_at, template, source_ids, citations FROM reports \
             WHERE insight_id = ?1 AND version = ?2",
            params![insight_id, version],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            },
        )
        .ok();
    let Some((id, published_at, template, source_ids_raw, citations_raw)) = row else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "æœªæ‰¾åˆ°è¯¥ç‰ˆæœ¬" })),
        );
    };

    let now = now_rfc3339();

    // å·²å‘å¸ƒè¿‡ â†’ fork(spec Â§ 5.3.3 / Â§ 8.3)
    // citations ç›´æŽ¥æ‹·è´çˆ¶ç‰ˆæœ¬(boris-inline é€šå¸¸åªæ”¹æ–‡å­—,å¼•ç”¨ç»“æž„ä¸å˜);
    // å¦‚æžœ Boris æ”¹åŠ¨è®©æŸäº› `[^N]` ä¸å†å­˜åœ¨,å‰ç«¯ popover æ‰¾ä¸åˆ°å¯¹åº” citation æ—¶ä¸æ˜¾ç¤ºã€‚
    if published_at.is_some() {
        let next_version: i64 = db
            .query_row(
                "SELECT COALESCE(MAX(version), 0) + 1 FROM reports WHERE insight_id = ?1",
                params![insight_id],
                |r| r.get(0),
            )
            .unwrap_or(version + 1);

        let tx = match db.transaction() {
            Ok(t) => t,
            Err(e) => return db_error("fork tx begin", e),
        };
        if let Err(e) = tx.execute(
            "INSERT INTO reports (insight_id, version, template, content_md, source_ids, citations,
                                  revision_note, parent_report_id,
                                  generated_by, model_used, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, '', ?7, 'boris-inline', '', ?8, ?8)",
            params![
                insight_id,
                next_version,
                &template,
                &content_md,
                &source_ids_raw,
                &citations_raw,
                id,
                &now,
            ],
        ) {
            return db_error("fork insert", e);
        }
        let new_id = tx.last_insert_rowid();
        if let Err(e) = tx.execute(
            "UPDATE insights SET current_report_id = ?1, updated_at = ?2 \
             WHERE id = ?3 AND user_id = ?4",
            params![new_id, &now, insight_id, &user_id.0],
        ) {
            return db_error("fork update current", e);
        }
        if let Err(e) = tx.commit() {
            return db_error("fork tx commit", e);
        }
        return fetch_report(&db, insight_id, new_id);
    }

    // æœªå‘å¸ƒ â†’ åŽŸåœ° PATCH(v0.1 è¡Œä¸º)
    let n = db.execute(
        "UPDATE reports SET content_md = ?1, updated_at = ?2 \
         WHERE insight_id = ?3 AND version = ?4",
        params![&content_md, &now, insight_id, version],
    );
    match n {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "æœªæ‰¾åˆ°è¯¥ç‰ˆæœ¬" })),
        ),
        Ok(_) => {
            let _ = db.execute(
                "UPDATE insights SET updated_at = ?1 WHERE id = ?2",
                params![&now, insight_id],
            );
            fetch_report(&db, insight_id, id)
        }
        Err(e) => db_error("update", e),
    }
}

fn fetch_report(db: &Connection, insight_id: i64, id: i64) -> (StatusCode, Json<JsonValue>) {
    let sql = format!("SELECT {SELECT_COLS} FROM reports WHERE id = ?1 AND insight_id = ?2");
    match db.query_row(&sql, params![id, insight_id], row_to_report) {
        Ok(t) => (StatusCode::OK, Json(json!({ "success": true, "item": t }))),
        Err(e) => db_error("fetch_report", e),
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
    async fn create_report_assigns_v1_then_v2() {
        let state = test_state();
        let (_u, token) = create_test_user(&state, "u1", "Pa55word1");
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
                    .body(Body::from(r#"{"title":"x"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let iid = body_json(resp).await["item"]["id"].as_i64().unwrap();

        for expected_v in [1, 2] {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/api/insights/{iid}/reports"))
                        .header("Cookie", &cookie)
                        .header("Content-Type", "application/json")
                        .body(Body::from(
                            r#"{"template":"survey","contentMd":"hi v1","sourceIds":[1,2]}"#,
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            let j = body_json(resp).await;
            assert_eq!(j["item"]["version"], expected_v);
            assert_eq!(j["item"]["sourceIds"], json!([1, 2]));
        }

        // v0.2:status ä»Ž collecting â†’ ready â†’ processing â†’ editing(ç­‰ Boris review)
        let resp = app
            .clone()
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
        assert!(j["item"]["currentReportId"].as_i64().unwrap() > 0);
    }

    #[tokio::test]
    async fn latest_returns_highest_version_or_404() {
        let state = test_state();
        let (_u, token) = create_test_user(&state, "u-latest", "Pa55word1");
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
                    .body(Body::from(r#"{"title":"x"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let iid = body_json(resp).await["item"]["id"].as_i64().unwrap();

        // æ²¡ report â†’ 404
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/insights/{iid}/reports/latest"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        for txt in ["v1 content", "v2 content"] {
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/api/insights/{iid}/reports"))
                        .header("Cookie", &cookie)
                        .header("Content-Type", "application/json")
                        .body(Body::from(format!(
                            r#"{{"template":"survey","contentMd":"{txt}"}}"#
                        )))
                        .unwrap(),
                )
                .await
                .unwrap();
        }
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/insights/{iid}/reports/latest"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["version"], 2);
        assert_eq!(j["item"]["contentMd"], "v2 content");
    }

    #[tokio::test]
    async fn revision_mode_clears_pending_note() {
        let state = test_state();
        let (_u, token) = create_test_user(&state, "u-rev", "Pa55word1");
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
        let v1_id = body_json(resp).await["item"]["id"].as_i64().unwrap();
        // regenerate â†’ pending_revision_note å†™å…¥
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid}/regenerate"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"revisionNote":"åŠ åä¾‹"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        // CC claim
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid}/claim"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // CC æäº¤ v2(parent_report_id = v1_id, revision_note æ‹·è¿‡æ¥)
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid}/reports"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"template":"survey","contentMd":"v2","parentReportId":{v1_id},"revisionNote":"åŠ åä¾‹"}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["version"], 2);
        assert_eq!(j["item"]["parentReportId"], v1_id);
        assert_eq!(j["item"]["revisionNote"], "åŠ åä¾‹");

        // insight.pendingRevisionNote å·²æ¸…ç©º,status=editing
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
        assert_eq!(j["item"]["pendingRevisionNote"], "");
    }

    #[tokio::test]
    async fn patch_published_report_auto_forks() {
        let state = test_state();
        let (_u, token) = create_test_user(&state, "u-fork", "Pa55word1");
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
                    .body(Body::from(r#"{"title":"x"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let iid = body_json(resp).await["item"]["id"].as_i64().unwrap();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid}/reports"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"template":"survey","contentMd":"v1 original"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let v1_id = body_json(resp).await["item"]["id"].as_i64().unwrap();

        // Publish v1
        app.clone()
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

        // PATCH v1(å·²å‘å¸ƒ)â†’ åº”è‡ªåŠ¨ fork å‡º v2
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/insights/{iid}/reports/1"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"contentMd":"v1 edited by Boris"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["version"], 2);
        assert_eq!(j["item"]["parentReportId"], v1_id);
        assert_eq!(j["item"]["generatedBy"], "boris-inline");
        assert_eq!(j["item"]["contentMd"], "v1 edited by Boris");

        // v1 åº”è¯¥è¿˜æ˜¯ "v1 original"(åŽ†å²å¿«ç…§ä¸è¢«æ”¹å†™)
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/insights/{iid}/reports/1"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["contentMd"], "v1 original");

        // insight.current_report_id åˆ‡åˆ° v2
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
        let cur = j["item"]["currentReportId"].as_i64().unwrap();
        assert_ne!(cur, v1_id);
    }

    #[tokio::test]
    async fn update_report_keeps_version() {
        let state = test_state();
        let (_u, token) = create_test_user(&state, "u2", "Pa55word1");
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
                    .body(Body::from(
                        r#"{"template":"survey","contentMd":"original"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/insights/{iid}/reports/1"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"contentMd":"edited by Boris"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["version"], 1); // æ²¡å¼€æ–°ç‰ˆ
        assert_eq!(j["item"]["contentMd"], "edited by Boris");

        // åˆ—è¡¨é‡Œä»ç„¶åªæœ‰ 1 ä¸ªç‰ˆæœ¬
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/insights/{iid}/reports"))
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
