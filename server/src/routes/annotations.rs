//! `/api/insights/:id/annotations` + `/api/annotations/:id` —
//! 报告备注(T-107 / SPEC insight v0.2 § 4 + § 8.5)。
//!
//! 设计要点:
//!   - annotation 锚定到具体 report 版本,不跨版本(spec § 4)
//!   - anchor / report_id 不可改;要换锚就删了重建
//!   - DELETE 是软删(deleted=1),保留历史
//!   - Claude Code 修订模式只读 `report_id=<parent> AND status='open' AND deleted=0`

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use rusqlite::params;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use tracing::error;

use crate::auth::UserId;
use crate::models::annotation::{
    is_valid_kind, is_valid_status, Annotation, CreateAnnotationRequest, UpdateAnnotationRequest,
};
use crate::state::AppState;

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn db_error(ctx: &str, e: rusqlite::Error) -> (StatusCode, Json<JsonValue>) {
    error!(target: "annotations", "{} db error: {}", ctx, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "error": "内部错误" })),
    )
}

const SELECT_COLS: &str =
    "id, insight_id, report_id, anchor, body, kind, status, created_at, updated_at";

fn row_to_annotation(row: &rusqlite::Row) -> rusqlite::Result<Annotation> {
    Ok(Annotation {
        id: row.get(0)?,
        insight_id: row.get(1)?,
        report_id: row.get(2)?,
        anchor: row.get(3)?,
        body: row.get(4)?,
        kind: row.get(5)?,
        status: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

// ============ HTTP handlers ============

#[derive(Debug, Default, Deserialize)]
pub struct ListFilters {
    pub report_id: Option<i64>,
    pub status: Option<String>,
    /// 默认不含已软删项;1=含
    pub include_deleted: Option<i64>,
}

/// GET /api/insights/:id/annotations?report_id=&status=&include_deleted=
/// 默认拿 insight.current_report_id 的 open 项
pub async fn list_annotations(
    State(state): State<AppState>,
    user_id: UserId,
    Path(insight_id): Path<i64>,
    Query(f): Query<ListFilters>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();

    // 验证 user 拥有该 insight + 缺省的 report_id 用 current_report_id
    let current: Option<i64> = db
        .query_row(
            "SELECT current_report_id FROM insights WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
            params![insight_id, &user_id.0],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    let _exists: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM insights WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
            params![insight_id, &user_id.0],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if _exists == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到洞察" })),
        );
    }

    let report_id = f.report_id.or(current);
    let include_deleted = f.include_deleted.unwrap_or(0) == 1;

    // 动态 SQL:filter 都可选
    let mut conditions: Vec<String> = vec![
        "user_id = ?1".to_string(),
        "insight_id = ?2".to_string(),
    ];
    let mut params_v: Vec<Box<dyn rusqlite::ToSql>> =
        vec![Box::new(user_id.0.clone()), Box::new(insight_id)];
    let mut idx = 3;

    if !include_deleted {
        conditions.push("deleted = 0".to_string());
    }
    if let Some(rid) = report_id {
        conditions.push(format!("report_id = ?{idx}"));
        params_v.push(Box::new(rid));
        idx += 1;
    }
    if let Some(s) = f.status.as_deref().filter(|s| !s.is_empty()) {
        conditions.push(format!("status = ?{idx}"));
        params_v.push(Box::new(s.to_string()));
    }

    let sql = format!(
        "SELECT {SELECT_COLS} FROM annotations WHERE {} ORDER BY created_at ASC",
        conditions.join(" AND ")
    );
    let mut stmt = match db.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return db_error("list prepare", e),
    };
    let params_ref: Vec<&dyn rusqlite::ToSql> = params_v.iter().map(|b| &**b).collect();
    let rows = match stmt.query_map(params_ref.as_slice(), row_to_annotation) {
        Ok(r) => r,
        Err(e) => return db_error("list query", e),
    };
    let items: Vec<Annotation> = rows.filter_map(|r| r.ok()).collect();
    (
        StatusCode::OK,
        Json(json!({ "success": true, "items": items, "count": items.len() })),
    )
}

/// POST /api/insights/:id/annotations
pub async fn create_annotation(
    State(state): State<AppState>,
    user_id: UserId,
    Path(insight_id): Path<i64>,
    Json(req): Json<CreateAnnotationRequest>,
) -> (StatusCode, Json<JsonValue>) {
    // 字段合法性
    if !is_valid_kind(&req.kind) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": format!("invalid kind: {}", req.kind) })),
        );
    }
    if req.body.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "body 不能为空" })),
        );
    }
    if req.anchor.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "anchor 不能为空" })),
        );
    }

    let db = state.db.lock();

    // 验证 report 属于该 insight 且 insight 属于该 user
    let owns: bool = db
        .query_row(
            "SELECT COUNT(*) > 0 FROM reports r
             JOIN insights i ON r.insight_id = i.id
             WHERE r.id = ?1 AND r.insight_id = ?2 AND i.user_id = ?3 AND i.deleted = 0",
            params![req.report_id, insight_id, &user_id.0],
            |r| r.get(0),
        )
        .unwrap_or(false);
    if !owns {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到对应报告" })),
        );
    }

    let now = now_rfc3339();
    let res = db.execute(
        "INSERT INTO annotations (user_id, insight_id, report_id, anchor, body, kind, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'open', ?7, ?7)",
        params![
            &user_id.0,
            insight_id,
            req.report_id,
            &req.anchor,
            &req.body,
            &req.kind,
            &now,
        ],
    );
    match res {
        Ok(_) => {
            let new_id = db.last_insert_rowid();
            let sql = format!(
                "SELECT {SELECT_COLS} FROM annotations WHERE id = ?1 AND user_id = ?2"
            );
            match db.query_row(&sql, params![new_id, &user_id.0], row_to_annotation) {
                Ok(item) => (
                    StatusCode::OK,
                    Json(json!({ "success": true, "item": item })),
                ),
                Err(e) => db_error("fetch new", e),
            }
        }
        Err(e) => db_error("insert", e),
    }
}

/// PATCH /api/annotations/:id — 改 body / kind / status(anchor 与 report_id 不可改)
pub async fn update_annotation(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
    Json(patch): Json<UpdateAnnotationRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let mut sets: Vec<&str> = Vec::new();
    let mut vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(v) = patch.body {
        sets.push("body = ?");
        vals.push(Box::new(v));
    }
    if let Some(v) = patch.kind {
        if !is_valid_kind(&v) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": "invalid kind" })),
            );
        }
        sets.push("kind = ?");
        vals.push(Box::new(v));
    }
    if let Some(v) = patch.status {
        if !is_valid_status(&v) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": "invalid status" })),
            );
        }
        sets.push("status = ?");
        vals.push(Box::new(v));
    }
    if sets.is_empty() {
        // 没字段改 → 直接返回当前
        let sql = format!(
            "SELECT {SELECT_COLS} FROM annotations WHERE id = ?1 AND user_id = ?2 AND deleted = 0"
        );
        match db.query_row(&sql, params![id, &user_id.0], row_to_annotation) {
            Ok(item) => {
                return (
                    StatusCode::OK,
                    Json(json!({ "success": true, "item": item })),
                );
            }
            Err(_) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({ "success": false, "error": "未找到备注" })),
                );
            }
        }
    }
    sets.push("updated_at = ?");
    vals.push(Box::new(now_rfc3339()));
    vals.push(Box::new(id));
    vals.push(Box::new(user_id.0.clone()));

    let sql = format!(
        "UPDATE annotations SET {} WHERE id = ? AND user_id = ? AND deleted = 0",
        sets.join(", ")
    );
    let params_ref: Vec<&dyn rusqlite::ToSql> = vals.iter().map(|b| &**b).collect();
    match db.execute(&sql, params_ref.as_slice()) {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到备注" })),
        ),
        Ok(_) => {
            let sql = format!(
                "SELECT {SELECT_COLS} FROM annotations WHERE id = ?1 AND user_id = ?2"
            );
            match db.query_row(&sql, params![id, &user_id.0], row_to_annotation) {
                Ok(item) => (
                    StatusCode::OK,
                    Json(json!({ "success": true, "item": item })),
                ),
                Err(e) => db_error("fetch updated", e),
            }
        }
        Err(e) => db_error("update", e),
    }
}

/// DELETE /api/annotations/:id — 软删
pub async fn delete_annotation(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let now = now_rfc3339();
    let n = db.execute(
        "UPDATE annotations SET deleted = 1, updated_at = ?1 WHERE id = ?2 AND user_id = ?3 AND deleted = 0",
        params![&now, id, &user_id.0],
    );
    match n {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到备注" })),
        ),
        Ok(_) => (StatusCode::OK, Json(json!({ "success": true }))),
        Err(e) => db_error("delete", e),
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

    async fn make_insight_with_report(
        app: &axum::Router,
        cookie: &str,
    ) -> (i64, i64) {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/insights")
                    .header("Cookie", cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title":"i"}"#))
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
                    .header("Cookie", cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"template":"survey","contentMd":"body"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let rid = body_json(resp).await["item"]["id"].as_i64().unwrap();
        (iid, rid)
    }

    #[tokio::test]
    async fn create_and_list_annotation() {
        let state = test_state();
        let (_u, tok) = create_test_user(&state, "u1", "Pa55word1");
        let cookie = auth_cookie(&tok);
        let app = crate::build_app(state);
        let (iid, rid) = make_insight_with_report(&app, &cookie).await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid}/annotations"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"reportId":{rid},"anchor":"{{\"kind\":\"paragraph\",\"index\":1}}","body":"再加个反例","kind":"suggestion"}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["success"], true);
        assert_eq!(j["item"]["body"], "再加个反例");
        assert_eq!(j["item"]["kind"], "suggestion");
        assert_eq!(j["item"]["status"], "open");

        // 列表
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/insights/{iid}/annotations"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["count"], 1);
    }

    #[tokio::test]
    async fn update_resolved_then_filter_open() {
        let state = test_state();
        let (_u, tok) = create_test_user(&state, "u2", "Pa55word1");
        let cookie = auth_cookie(&tok);
        let app = crate::build_app(state);
        let (iid, rid) = make_insight_with_report(&app, &cookie).await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid}/annotations"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"reportId":{rid},"anchor":"a","body":"q1","kind":"question"}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        let aid = body_json(resp).await["item"]["id"].as_i64().unwrap();

        // 标 resolved
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/annotations/{aid}"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"status":"resolved"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["status"], "resolved");

        // ?status=open → 0 条
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/insights/{iid}/annotations?status=open"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["count"], 0);
    }

    #[tokio::test]
    async fn soft_delete_hides_from_default_list() {
        let state = test_state();
        let (_u, tok) = create_test_user(&state, "u3", "Pa55word1");
        let cookie = auth_cookie(&tok);
        let app = crate::build_app(state);
        let (iid, rid) = make_insight_with_report(&app, &cookie).await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid}/annotations"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"reportId":{rid},"anchor":"a","body":"x"}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        let aid = body_json(resp).await["item"]["id"].as_i64().unwrap();

        // 软删
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/annotations/{aid}"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // 默认 list 不含
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/insights/{iid}/annotations"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["count"], 0);

        // include_deleted=1 含
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/insights/{iid}/annotations?include_deleted=1"))
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["count"], 1);
    }

    #[tokio::test]
    async fn other_user_cannot_create() {
        let state = test_state();
        let (_a, tok_a) = create_test_user(&state, "alice", "Pa55word1");
        let (_b, tok_b) = create_test_user(&state, "bob", "Pa55word1");
        let cookie_a = auth_cookie(&tok_a);
        let cookie_b = auth_cookie(&tok_b);
        let app = crate::build_app(state);
        let (iid, rid) = make_insight_with_report(&app, &cookie_a).await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid}/annotations"))
                    .header("Cookie", &cookie_b)
                    .header("Content-Type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"reportId":{rid},"anchor":"a","body":"x"}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn invalid_kind_rejected() {
        let state = test_state();
        let (_u, tok) = create_test_user(&state, "u4", "Pa55word1");
        let cookie = auth_cookie(&tok);
        let app = crate::build_app(state);
        let (iid, rid) = make_insight_with_report(&app, &cookie).await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/insights/{iid}/annotations"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"reportId":{rid},"anchor":"a","body":"x","kind":"nonsense"}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
