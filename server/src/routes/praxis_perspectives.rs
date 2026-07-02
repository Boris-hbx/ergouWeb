//! `/api/praxis/perspectives` - Praxis perspectives (视角, T-292).
//!
//! See SPEC: `C:\Project\ergouPM\specs\praxis\spec.md` §11.1
//! 每个视角是一套完全隔离的关系人数据集；删非空视角被拒。

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde_json::{json, Map, Value as JsonValue};
use tracing::error;

use crate::auth::AdminUserId;
use crate::models::praxis_perspective::{
    CreatePerspectiveRequest, PraxisPerspective, UpdatePerspectiveRequest,
};
use crate::state::AppState;

const SELECT_COLS: &str = "id, name, sort_order, created_at, updated_at";

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn validate_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("name must not be empty".into());
    }
    if trimmed.chars().count() > 30 {
        return Err("name must be 30 characters or fewer".into());
    }
    Ok(trimmed.to_string())
}

fn row_to_perspective(row: &rusqlite::Row) -> rusqlite::Result<PraxisPerspective> {
    Ok(PraxisPerspective {
        id: row.get(0)?,
        name: row.get(1)?,
        sort_order: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn load_perspective(db: &Connection, user_id: &str, id: i64) -> Option<PraxisPerspective> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM praxis_perspectives WHERE id = ?1 AND user_id = ?2 AND deleted = 0"
    );
    db.query_row(&sql, params![id, user_id], row_to_perspective)
        .ok()
}

/// 该视角是否属于此用户（未删）。contacts 路由用它校验 perspectiveId 入参。
pub fn perspective_owned(db: &Connection, user_id: &str, id: i64) -> bool {
    load_perspective(db, user_id, id).is_some()
}

/// 返回用户的第一个视角 id；没有则种一个「默认」视角（老数据由 db 迁移回填，
/// 新用户/老前端不带 perspectiveId 时靠这里懒种）。
pub fn ensure_default_perspective(db: &Connection, user_id: &str) -> Result<i64, String> {
    if let Ok(id) = db.query_row(
        "SELECT id FROM praxis_perspectives
          WHERE user_id = ?1 AND deleted = 0
          ORDER BY sort_order ASC, id ASC LIMIT 1",
        params![user_id],
        |r| r.get::<_, i64>(0),
    ) {
        return Ok(id);
    }
    let now = now_rfc3339();
    db.execute(
        "INSERT INTO praxis_perspectives (user_id, name, sort_order, created_at, updated_at)
         VALUES (?1, '默认', 1, ?2, ?2)",
        params![user_id, &now],
    )
    .map_err(|e| format!("seed default perspective: {e}"))?;
    Ok(db.last_insert_rowid())
}

pub fn list_perspectives_impl(
    db: &Connection,
    user_id: &str,
) -> Result<Vec<PraxisPerspective>, String> {
    // 保证至少有一个视角，前端始终有"当前视角"可选。
    ensure_default_perspective(db, user_id)?;
    let sql = format!(
        "SELECT {SELECT_COLS} FROM praxis_perspectives
         WHERE user_id = ?1 AND deleted = 0
         ORDER BY sort_order ASC, id ASC"
    );
    let mut stmt = db.prepare(&sql).map_err(|e| format!("prepare: {e}"))?;
    let rows = stmt
        .query_map(params![user_id], row_to_perspective)
        .map_err(|e| format!("query: {e}"))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub async fn list_perspectives(
    State(state): State<AppState>,
    admin: AdminUserId,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match list_perspectives_impl(&db, &admin.0) {
        Ok(items) => (
            StatusCode::OK,
            Json(json!({ "success": true, "items": items, "count": items.len() })),
        ),
        Err(e) => {
            error!(target: "praxis_perspectives", "list: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "内部错误" })),
            )
        }
    }
}

pub async fn create_perspective(
    State(state): State<AppState>,
    admin: AdminUserId,
    Json(req): Json<CreatePerspectiveRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let name = match validate_name(&req.name) {
        Ok(n) => n,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": e })),
            )
        }
    };
    let db = state.db.lock();
    let sort_order = req.sort_order.unwrap_or_else(|| {
        db.query_row(
            "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM praxis_perspectives WHERE user_id = ?1 AND deleted = 0",
            params![&admin.0],
            |r| r.get(0),
        )
        .unwrap_or(1.0)
    });
    let now = now_rfc3339();
    if let Err(e) = db.execute(
        "INSERT INTO praxis_perspectives (user_id, name, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)",
        params![&admin.0, &name, sort_order, &now],
    ) {
        error!(target: "praxis_perspectives", "create db error: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": "内部错误" })),
        );
    }
    let item = load_perspective(&db, &admin.0, db.last_insert_rowid());
    (
        StatusCode::OK,
        Json(json!({ "success": true, "item": item })),
    )
}

pub async fn update_perspective(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path(id): Path<i64>,
    Json(patch): Json<UpdatePerspectiveRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let Some(mut current) = load_perspective(&db, &admin.0, id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到视角" })),
        );
    };

    if let Some(name) = field_string(&patch.fields, "name") {
        match validate_name(&name) {
            Ok(n) => current.name = n,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "success": false, "error": e })),
                )
            }
        }
    }
    if let Some(sort_order) = patch.fields.get("sortOrder").and_then(|v| v.as_f64()) {
        current.sort_order = sort_order;
    }

    let now = now_rfc3339();
    if let Err(e) = db.execute(
        "UPDATE praxis_perspectives SET name = ?1, sort_order = ?2, updated_at = ?3
         WHERE id = ?4 AND user_id = ?5 AND deleted = 0",
        params![&current.name, current.sort_order, &now, id, &admin.0],
    ) {
        error!(target: "praxis_perspectives", "update db error: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": "内部错误" })),
        );
    }
    (
        StatusCode::OK,
        Json(json!({ "success": true, "item": load_perspective(&db, &admin.0, id) })),
    )
}

pub async fn delete_perspective(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    if !perspective_owned(&db, &admin.0, id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到视角" })),
        );
    }
    // 禁删非空视角（spec §11.1）：其下还有未删除关系人则拒。
    let contact_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM praxis_contacts
              WHERE perspective_id = ?1 AND user_id = ?2 AND deleted = 0",
            params![id, &admin.0],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if contact_count > 0 {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": format!("视角内还有 {contact_count} 位关系人，请先清空再删除")
            })),
        );
    }
    let now = now_rfc3339();
    match db.execute(
        "UPDATE praxis_perspectives SET deleted = 1, updated_at = ?1
         WHERE id = ?2 AND user_id = ?3 AND deleted = 0",
        params![&now, id, &admin.0],
    ) {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到视角" })),
        ),
        Ok(_) => (StatusCode::OK, Json(json!({ "success": true }))),
        Err(e) => {
            error!(target: "praxis_perspectives", "delete db error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "内部错误" })),
            )
        }
    }
}

fn field_string(fields: &Map<String, JsonValue>, key: &str) -> Option<String> {
    fields
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
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

    async fn request(
        app: &axum::Router,
        method: &str,
        uri: String,
        token: &str,
        body: Option<&'static str>,
    ) -> axum::response::Response {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("Cookie", auth_cookie(token));
        let body = match body {
            Some(b) => {
                builder = builder.header("Content-Type", "application/json");
                Body::from(b)
            }
            None => Body::empty(),
        };
        app.clone()
            .oneshot(builder.body(body).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn perspectives_require_admin() {
        let state = test_state();
        let (_uid, user_token) = create_test_user(&state, "pp-user", "Pa55word1");
        let app = crate::build_app(state);
        let resp = request(
            &app,
            "GET",
            "/api/praxis/perspectives".into(),
            &user_token,
            None,
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn perspectives_seed_crud_isolation_and_nonempty_delete_guard() {
        let state = test_state();
        let (_aid, admin_token) = create_admin_user(&state, "pp-admin", "Pa55word1");
        let (_oid, other_token) = create_admin_user(&state, "pp-other", "Pa55word1");
        let app = crate::build_app(state);

        // 首次 GET 懒种「默认」视角
        let resp = request(
            &app,
            "GET",
            "/api/praxis/perspectives".into(),
            &admin_token,
            None,
        )
        .await;
        let j = body_json(resp).await;
        assert_eq!(j["count"], 1);
        assert_eq!(j["items"][0]["name"], "默认");

        // 新建视角 + 校验空名 400
        let resp = request(
            &app,
            "POST",
            "/api/praxis/perspectives".into(),
            &admin_token,
            Some(r#"{"name":"  工作  "}"#),
        )
        .await;
        let j = body_json(resp).await;
        assert_eq!(j["item"]["name"], "工作");
        let work_id = j["item"]["id"].as_i64().unwrap();

        let resp = request(
            &app,
            "POST",
            "/api/praxis/perspectives".into(),
            &admin_token,
            Some(r#"{"name":"   "}"#),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // PATCH 改名/排序
        let resp = request(
            &app,
            "PATCH",
            format!("/api/praxis/perspectives/{work_id}"),
            &admin_token,
            Some(r#"{"name":"人生建议","sortOrder":5.0}"#),
        )
        .await;
        let j = body_json(resp).await;
        assert_eq!(j["item"]["name"], "人生建议");
        assert_eq!(j["item"]["sortOrder"], 5.0);

        // 用户间隔离:other 只看到自己的默认视角,PATCH 别人的视角 404
        let resp = request(
            &app,
            "GET",
            "/api/praxis/perspectives".into(),
            &other_token,
            None,
        )
        .await;
        let j = body_json(resp).await;
        assert_eq!(j["count"], 1);
        assert_ne!(j["items"][0]["id"].as_i64().unwrap(), work_id);
        let resp = request(
            &app,
            "PATCH",
            format!("/api/praxis/perspectives/{work_id}"),
            &other_token,
            Some(r#"{"name":"hack"}"#),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        // 视角里放一个关系人 → 删除被拒(409)
        let resp = request(
            &app,
            "POST",
            "/api/praxis/contacts".into(),
            &admin_token,
            Some(r#"{"name":"Mentor","layer":"core"}"#),
        )
        .await;
        let j = body_json(resp).await;
        let default_id = j["item"]["perspectiveId"].as_i64().unwrap();
        assert_ne!(default_id, work_id);
        let resp = request(
            &app,
            "DELETE",
            format!("/api/praxis/perspectives/{default_id}"),
            &admin_token,
            None,
        )
        .await;
        assert_eq!(resp.status(), StatusCode::CONFLICT);

        // 空视角可删
        let resp = request(
            &app,
            "DELETE",
            format!("/api/praxis/perspectives/{work_id}"),
            &admin_token,
            None,
        )
        .await;
        assert_eq!(body_json(resp).await["success"], true);
        let resp = request(
            &app,
            "GET",
            "/api/praxis/perspectives".into(),
            &admin_token,
            None,
        )
        .await;
        assert_eq!(body_json(resp).await["count"], 1);
    }

    #[tokio::test]
    async fn migration_backfills_existing_contacts_into_default_perspective() {
        let state = test_state();
        let (aid, admin_token) = create_admin_user(&state, "pp-mig", "Pa55word1");
        {
            // 模拟 T-292 之前的存量关系人:perspective_id 为 NULL,用户还没有任何视角
            let db = state.db.lock();
            db.execute(
                "INSERT INTO praxis_contacts (user_id, name, layer, sort_order, created_at, updated_at)
                 VALUES (?1, '老关系人', 'core', 1, '2026-01-01', '2026-01-01')",
                params![&aid],
            )
            .unwrap();
            // 重跑迁移(幂等):应种「默认」视角并回填 perspective_id
            crate::db::init_connection(&db);
        }
        let app = crate::build_app(state);
        let resp = request(
            &app,
            "GET",
            "/api/praxis/contacts".into(),
            &admin_token,
            None,
        )
        .await;
        let j = body_json(resp).await;
        assert_eq!(j["count"], 1);
        assert_eq!(j["items"][0]["name"], "老关系人");
        assert!(j["items"][0]["perspectiveId"].as_i64().is_some());
        let resp = request(
            &app,
            "GET",
            "/api/praxis/perspectives".into(),
            &admin_token,
            None,
        )
        .await;
        let j = body_json(resp).await;
        assert_eq!(j["count"], 1);
        assert_eq!(j["items"][0]["name"], "默认");
    }
}
