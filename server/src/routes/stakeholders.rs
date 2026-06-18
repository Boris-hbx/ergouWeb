//! `/api/work/stakeholders` - CRUD for stakeholder records.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::Utc;
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Map, Value as JsonValue, json};
use tracing::{error, warn};

use crate::auth::UserId;
use crate::models::stakeholder::{CreateStakeholderRequest, Stakeholder, UpdateStakeholderRequest};
use crate::state::AppState;

#[derive(Debug, Default, Deserialize)]
pub struct StakeholderFilters {
    pub q: Option<String>,
    pub team: Option<String>,
    pub region: Option<String>,
}

fn db_error(ctx: &str, e: rusqlite::Error) -> (StatusCode, Json<JsonValue>) {
    error!(target: "stakeholders", "{} db error: {}", ctx, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "error": "内部错误" })),
    )
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn parse_vec(raw: &str, field: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_else(|e| {
        warn!(target: "stakeholders", "{} parse err: {}", field, e);
        Vec::new()
    })
}

fn parse_custom_fields(raw: &str) -> Map<String, JsonValue> {
    match serde_json::from_str::<JsonValue>(raw) {
        Ok(JsonValue::Object(m)) => m,
        Ok(_) => Map::new(),
        Err(e) => {
            warn!(target: "stakeholders", "custom_fields parse err: {}", e);
            Map::new()
        }
    }
}

fn row_to_stakeholder(row: &rusqlite::Row) -> rusqlite::Result<Stakeholder> {
    let duty_raw: String = row.get(4)?;
    let liaison_raw: String = row.get(5)?;
    let method_raw: String = row.get(7)?;
    let custom_raw: String = row.get(12)?;
    Ok(Stakeholder {
        id: row.get(0)?,
        name: row.get(1)?,
        team: row.get(2)?,
        region: row.get(3)?,
        duty: parse_vec(&duty_raw, "duty"),
        liaison: parse_vec(&liaison_raw, "liaison"),
        relation: row.get(6)?,
        method: parse_vec(&method_raw, "method"),
        cadence: row.get(8)?,
        title: row.get(9)?,
        strategy: row.get(10)?,
        notes: row.get(11)?,
        custom_fields: parse_custom_fields(&custom_raw),
        sort_order: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}

const SELECT_COLS: &str = "id, name, team, region, duty, liaison, relation, method, cadence, title, strategy, notes, custom_fields, sort_order, created_at, updated_at";

fn load_stakeholder(db: &Connection, user_id: &str, id: i64) -> Option<Stakeholder> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM stakeholders WHERE id = ?1 AND user_id = ?2 AND deleted = 0"
    );
    db.query_row(&sql, params![id, user_id], row_to_stakeholder)
        .ok()
}

pub fn query_stakeholders_impl(
    db: &Connection,
    user_id: &str,
    f: &StakeholderFilters,
) -> Result<Vec<Stakeholder>, String> {
    let mut conditions: Vec<String> = vec!["user_id = ?1".into(), "deleted = 0".into()];
    let mut params_v: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(user_id.to_string())];
    let mut idx = 2usize;

    if let Some(q) = f.q.as_deref().filter(|s| !s.trim().is_empty()) {
        conditions.push(format!(
            "(name LIKE ?{idx} OR team LIKE ?{idx} OR region LIKE ?{idx} OR title LIKE ?{idx} OR duty LIKE ?{idx} OR liaison LIKE ?{idx} OR relation LIKE ?{idx} OR strategy LIKE ?{idx} OR notes LIKE ?{idx})"
        ));
        params_v.push(Box::new(format!("%{}%", q.trim())));
        idx += 1;
    }
    if let Some(team) = f.team.as_deref().filter(|s| !s.is_empty()) {
        conditions.push(format!("team = ?{idx}"));
        params_v.push(Box::new(team.to_string()));
        idx += 1;
    }
    if let Some(region) = f.region.as_deref().filter(|s| !s.is_empty()) {
        conditions.push(format!("region = ?{idx}"));
        params_v.push(Box::new(region.to_string()));
    }

    let sql = format!(
        "SELECT {SELECT_COLS} FROM stakeholders WHERE {} ORDER BY sort_order ASC, created_at ASC",
        conditions.join(" AND ")
    );
    let params_ref: Vec<&dyn rusqlite::ToSql> = params_v.iter().map(|b| &**b).collect();
    let mut stmt = db.prepare(&sql).map_err(|e| format!("prepare: {e}"))?;
    let rows = stmt
        .query_map(params_ref.as_slice(), row_to_stakeholder)
        .map_err(|e| format!("query: {e}"))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn create_stakeholder_impl(
    db: &Connection,
    user_id: &str,
    req: &CreateStakeholderRequest,
) -> Result<Stakeholder, String> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err("姓名不能为空".into());
    }
    let now = now_rfc3339();
    let next_sort = req.sort_order.unwrap_or_else(|| {
        db.query_row(
            "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM stakeholders WHERE user_id = ?1",
            params![user_id],
            |r| r.get(0),
        )
        .unwrap_or(0.0)
    });
    let duty = serde_json::to_string(&req.duty).unwrap_or_else(|_| "[]".into());
    let liaison = serde_json::to_string(&req.liaison).unwrap_or_else(|_| "[]".into());
    let method = serde_json::to_string(&req.method).unwrap_or_else(|_| "[]".into());
    let custom_fields = serde_json::to_string(
        &req.custom_fields
            .clone()
            .map(JsonValue::Object)
            .unwrap_or_else(|| JsonValue::Object(Map::new())),
    )
    .unwrap_or_else(|_| "{}".into());

    db.execute(
        "INSERT INTO stakeholders
           (user_id, name, team, region, title, duty, liaison, relation, method, cadence,
            strategy, notes, custom_fields, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)",
        params![
            user_id,
            name,
            &req.team,
            &req.region,
            &req.title,
            &duty,
            &liaison,
            &req.relation,
            &method,
            &req.cadence,
            &req.strategy,
            &req.notes,
            &custom_fields,
            next_sort,
            &now,
        ],
    )
    .map_err(|e| format!("insert: {e}"))?;

    let id = db.last_insert_rowid();
    load_stakeholder(db, user_id, id).ok_or_else(|| "reload failed".into())
}

pub fn update_stakeholder_impl(
    db: &Connection,
    user_id: &str,
    id: i64,
    patch: UpdateStakeholderRequest,
) -> Result<Option<Stakeholder>, String> {
    let existing_custom: Option<String> = db
        .query_row(
            "SELECT custom_fields FROM stakeholders WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
            params![id, user_id],
            |r| r.get(0),
        )
        .ok();
    let Some(existing_custom) = existing_custom else {
        return Ok(None);
    };

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
    push_str!("name", patch.name);
    push_str!("team", patch.team);
    push_str!("region", patch.region);
    push_str!("title", patch.title);
    push_str!("relation", patch.relation);
    push_str!("cadence", patch.cadence);
    push_str!("strategy", patch.strategy);
    push_str!("notes", patch.notes);

    if let Some(duty) = patch.duty {
        sets.push("duty = ?");
        vals.push(Box::new(
            serde_json::to_string(&duty).unwrap_or_else(|_| "[]".into()),
        ));
    }
    if let Some(liaison) = patch.liaison {
        sets.push("liaison = ?");
        vals.push(Box::new(
            serde_json::to_string(&liaison).unwrap_or_else(|_| "[]".into()),
        ));
    }
    if let Some(method) = patch.method {
        sets.push("method = ?");
        vals.push(Box::new(
            serde_json::to_string(&method).unwrap_or_else(|_| "[]".into()),
        ));
    }
    if let Some(custom_patch) = patch.custom_fields {
        let mut merged = parse_custom_fields(&existing_custom);
        for (k, v) in custom_patch {
            merged.insert(k, v);
        }
        sets.push("custom_fields = ?");
        vals.push(Box::new(
            serde_json::to_string(&JsonValue::Object(merged)).unwrap_or_else(|_| "{}".into()),
        ));
    }
    if let Some(sort_order) = patch.sort_order {
        sets.push("sort_order = ?");
        vals.push(Box::new(sort_order));
    }
    if sets.is_empty() {
        return Ok(load_stakeholder(db, user_id, id));
    }

    sets.push("updated_at = ?");
    vals.push(Box::new(now_rfc3339()));
    vals.push(Box::new(id));
    vals.push(Box::new(user_id.to_string()));
    let sql = format!(
        "UPDATE stakeholders SET {} WHERE id = ? AND user_id = ? AND deleted = 0",
        sets.join(", ")
    );
    let params_ref: Vec<&dyn rusqlite::ToSql> = vals.iter().map(|b| &**b).collect();
    db.execute(&sql, params_ref.as_slice())
        .map_err(|e| format!("update: {e}"))?;
    Ok(load_stakeholder(db, user_id, id))
}

pub async fn list_stakeholders(
    State(state): State<AppState>,
    user_id: UserId,
    Query(filters): Query<StakeholderFilters>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match query_stakeholders_impl(&db, &user_id.0, &filters) {
        Ok(items) => (
            StatusCode::OK,
            Json(json!({ "success": true, "items": items, "count": items.len() })),
        ),
        Err(e) => {
            error!(target: "stakeholders", "list: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "内部错误" })),
            )
        }
    }
}

pub async fn create_stakeholder(
    State(state): State<AppState>,
    user_id: UserId,
    Json(req): Json<CreateStakeholderRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match create_stakeholder_impl(&db, &user_id.0, &req) {
        Ok(item) => (
            StatusCode::OK,
            Json(json!({ "success": true, "item": item })),
        ),
        Err(e) if e == "姓名不能为空" => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": e })),
        ),
        Err(e) => {
            error!(target: "stakeholders", "create: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "内部错误" })),
            )
        }
    }
}

pub async fn update_stakeholder(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
    Json(patch): Json<UpdateStakeholderRequest>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    match update_stakeholder_impl(&db, &user_id.0, id, patch) {
        Ok(Some(item)) => (
            StatusCode::OK,
            Json(json!({ "success": true, "item": item })),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到干系人" })),
        ),
        Err(e) => {
            error!(target: "stakeholders", "update: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "内部错误" })),
            )
        }
    }
}

pub async fn delete_stakeholder(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<i64>,
) -> (StatusCode, Json<JsonValue>) {
    let db = state.db.lock();
    let now = now_rfc3339();
    match db.execute(
        "UPDATE stakeholders SET deleted = 1, deleted_at = ?1, updated_at = ?1
         WHERE id = ?2 AND user_id = ?3 AND deleted = 0",
        params![&now, id, &user_id.0],
    ) {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "未找到干系人" })),
        ),
        Ok(_) => (StatusCode::OK, Json(json!({ "success": true }))),
        Err(e) => db_error("delete", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{auth_cookie, create_test_user, test_state};
    use axum::body::{Body, to_bytes};
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

    #[tokio::test]
    async fn stakeholder_crud_filters_and_soft_delete() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "stake_alice", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/stakeholders")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"name":"张三","team":"平台","region":"北京","title":"秘书","duty":["预算"],"method":["定期汇报"],"customFields":{"c1":"A"}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["success"], true);
        assert_eq!(j["item"]["name"], "张三");
        assert_eq!(j["item"]["duty"][0], "预算");
        let id = j["item"]["id"].as_i64().unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/work/stakeholders/{id}"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"region":"上海","liaison":["采购"],"customFields":{"c2":"B"}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["item"]["region"], "上海");
        assert_eq!(j["item"]["liaison"][0], "采购");
        assert_eq!(j["item"]["customFields"]["c1"], "A");
        assert_eq!(j["item"]["customFields"]["c2"], "B");

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/work/stakeholders?q=%E9%87%87%E8%B4%AD&region=%E4%B8%8A%E6%B5%B7")
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let j = body_json(resp).await;
        assert_eq!(j["count"], 1);

        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/work/stakeholders/{id}"))
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
        assert_eq!(body_json(resp).await["count"], 0);
    }

    #[tokio::test]
    async fn stakeholders_are_per_user_isolated() {
        let state = test_state();
        let (_alice, alice_token) = create_test_user(&state, "stake_a", "Pa55word1");
        let (_bob, bob_token) = create_test_user(&state, "stake_b", "Pa55word1");
        let alice_cookie = auth_cookie(&alice_token);
        let bob_cookie = auth_cookie(&bob_token);
        let app = crate::build_app(state);

        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/stakeholders")
                    .header("Cookie", &alice_cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"name":"只属于 Alice"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/work/stakeholders")
                    .header("Cookie", &bob_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["count"], 0);
    }

    #[tokio::test]
    async fn create_requires_name() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "stake_empty", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/work/stakeholders")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"name":"  "}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
