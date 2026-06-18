use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::auth::UserId;
use crate::state::AppState;

// ===== Models =====

#[derive(Debug, Serialize)]
pub struct Memory {
    pub id: String,
    pub category: String,
    pub content: String,
    pub importance: i64,
    pub created_at: String,
    pub last_accessed_at: String,
    pub access_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateMemoryRequest {
    pub category: String,
    pub content: String,
    #[serde(default = "default_importance")]
    pub importance: i64,
}

fn default_importance() -> i64 {
    3
}

#[derive(Debug, Deserialize)]
pub struct UpdateMemoryRequest {
    pub category: Option<String>,
    pub content: Option<String>,
    pub importance: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct BatchImportRequest {
    pub memories: Vec<CreateMemoryRequest>,
}

#[derive(Debug, Deserialize)]
pub struct ListMemoriesQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default = "default_sort")]
    pub sort: String,
    pub category: Option<String>,
}

fn default_limit() -> i64 {
    20
}

fn default_sort() -> String {
    "recent".to_string()
}

#[derive(Debug, Deserialize)]
pub struct SearchMemoriesQuery {
    pub q: Option<String>,
    #[serde(default = "default_search_limit")]
    pub limit: i64,
}

fn default_search_limit() -> i64 {
    10
}

// ===== Helpers =====

const VALID_CATEGORIES: [&str; 4] = ["fact", "habit", "personality", "intent"];
const MAX_MEMORIES_PER_USER: i64 = 100;

fn is_sensitive_content(content: &str) -> bool {
    let lower = content.to_lowercase();
    let patterns = [
        "密码", "password", "passwd",
        "银行卡", "信用卡", "借记卡", "card number",
        "身份证", "护照", "passport",
        "社保号", "ssn", "social security",
        "pin码", "pin code", "验证码",
        "私钥", "private key", "secret key",
    ];
    patterns.iter().any(|p| lower.contains(p))
}

fn generate_memory_id() -> String {
    format!("mem_{}", &uuid::Uuid::new_v4().to_string()[..12])
}

// ===== Handlers =====

/// POST /api/memories — 创建记忆
pub async fn create_memory(
    State(state): State<AppState>,
    user_id: UserId,
    Json(req): Json<CreateMemoryRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Validate category
    if !VALID_CATEGORIES.contains(&req.category.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid category, must be fact/habit/personality/intent" })),
        );
    }

    // Validate content
    let content = req.content.trim();
    if content.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "content cannot be empty" })),
        );
    }
    if content.chars().count() > 500 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "content exceeds 500 characters" })),
        );
    }

    // Sensitive content check
    if is_sensitive_content(content) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "cannot store sensitive information (passwords, card numbers, IDs, etc.)" })),
        );
    }

    // Validate importance
    let importance = req.importance.clamp(1, 5);

    let db = state.db.lock();

    // Deduplication
    let exists: bool = db
        .query_row(
            "SELECT COUNT(*) > 0 FROM ergou_memories WHERE user_id=?1 AND content=?2",
            rusqlite::params![user_id.0, content],
            |r| r.get(0),
        )
        .unwrap_or(false);
    if exists {
        return (
            StatusCode::CONFLICT,
            Json(json!({ "error": "duplicate memory content" })),
        );
    }

    // Capacity check — auto-evict if at limit
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM ergou_memories WHERE user_id=?1",
            [&user_id.0],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if count >= MAX_MEMORIES_PER_USER {
        db.execute(
            "DELETE FROM ergou_memories WHERE id = (SELECT id FROM ergou_memories WHERE user_id=?1 ORDER BY access_count ASC, last_accessed_at ASC LIMIT 1)",
            [&user_id.0],
        )
        .ok();
    }

    // Insert
    let id = generate_memory_id();
    let now = chrono::Utc::now().to_rfc3339();

    match db.execute(
        "INSERT INTO ergou_memories (id, user_id, category, content, importance, created_at, last_accessed_at, access_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
        rusqlite::params![id, user_id.0, req.category, content, importance, now, now],
    ) {
        Ok(_) => (
            StatusCode::CREATED,
            Json(json!({
                "success": true,
                "memory": {
                    "id": id,
                    "category": req.category,
                    "content": content,
                    "importance": importance,
                    "created_at": now,
                    "last_accessed_at": now,
                    "access_count": 0
                }
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("failed to save memory: {}", e) })),
        ),
    }
}

/// POST /api/memories/batch — 批量导入（迁移用）
pub async fn batch_import(
    State(state): State<AppState>,
    user_id: UserId,
    Json(req): Json<BatchImportRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if req.memories.len() > 200 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "batch import limited to 200 memories" })),
        );
    }

    let total_submitted = req.memories.len();
    let mut imported = 0;
    let mut duplicates_skipped = 0;
    let mut invalid_skipped = 0;

    let db = state.db.lock();
    let now = chrono::Utc::now().to_rfc3339();

    for mem in &req.memories {
        let content = mem.content.trim();

        // Validate
        if !VALID_CATEGORIES.contains(&mem.category.as_str())
            || content.is_empty()
            || content.chars().count() > 500
            || is_sensitive_content(content)
        {
            invalid_skipped += 1;
            continue;
        }

        let importance = mem.importance.clamp(1, 5);

        // Dedup
        let exists: bool = db
            .query_row(
                "SELECT COUNT(*) > 0 FROM ergou_memories WHERE user_id=?1 AND content=?2",
                rusqlite::params![user_id.0, content],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if exists {
            duplicates_skipped += 1;
            continue;
        }

        let id = generate_memory_id();
        if db
            .execute(
                "INSERT INTO ergou_memories (id, user_id, category, content, importance, created_at, last_accessed_at, access_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
                rusqlite::params![id, user_id.0, mem.category, content, importance, now, now],
            )
            .is_ok()
        {
            imported += 1;
        }
    }

    // Evict excess memories beyond the limit
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM ergou_memories WHERE user_id=?1",
            [&user_id.0],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if count > MAX_MEMORIES_PER_USER {
        let excess = count - MAX_MEMORIES_PER_USER;
        db.execute(
            "DELETE FROM ergou_memories WHERE id IN (SELECT id FROM ergou_memories WHERE user_id=?1 ORDER BY access_count ASC, last_accessed_at ASC LIMIT ?2)",
            rusqlite::params![user_id.0, excess],
        )
        .ok();
    }

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "imported": imported,
            "duplicates_skipped": duplicates_skipped,
            "invalid_skipped": invalid_skipped,
            "total_submitted": total_submitted
        })),
    )
}

/// GET /api/memories — 列表查询
pub async fn list_memories(
    State(state): State<AppState>,
    user_id: UserId,
    Query(query): Query<ListMemoriesQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let limit = query.limit.clamp(1, 100);
    let order = match query.sort.as_str() {
        "importance" => "importance DESC, last_accessed_at DESC",
        _ => "last_accessed_at DESC",
    };

    let db = state.db.lock();

    let (sql, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) =
        if let Some(ref cat) = query.category {
            (
                format!(
                    "SELECT id, category, content, importance, created_at, last_accessed_at, access_count FROM ergou_memories WHERE user_id=?1 AND category=?2 ORDER BY {} LIMIT ?3",
                    order
                ),
                vec![
                    Box::new(user_id.0.clone()),
                    Box::new(cat.clone()),
                    Box::new(limit),
                ],
            )
        } else {
            (
                format!(
                    "SELECT id, category, content, importance, created_at, last_accessed_at, access_count FROM ergou_memories WHERE user_id=?1 ORDER BY {} LIMIT ?2",
                    order
                ),
                vec![Box::new(user_id.0.clone()), Box::new(limit)],
            )
        };

    let params_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

    let mut memories: Vec<Memory> = Vec::new();
    if let Ok(mut stmt) = db.prepare(&sql) {
        if let Ok(rows) = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(Memory {
                id: row.get(0)?,
                category: row.get(1)?,
                content: row.get(2)?,
                importance: row.get::<_, i64>(3).unwrap_or(3),
                created_at: row.get(4)?,
                last_accessed_at: row.get(5)?,
                access_count: row.get(6)?,
            })
        }) {
            for row in rows.flatten() {
                memories.push(row);
            }
        }
    }

    // Get total count
    let total: i64 = if let Some(ref cat) = query.category {
        db.query_row(
            "SELECT COUNT(*) FROM ergou_memories WHERE user_id=?1 AND category=?2",
            rusqlite::params![user_id.0, cat],
            |r| r.get(0),
        )
        .unwrap_or(0)
    } else {
        db.query_row(
            "SELECT COUNT(*) FROM ergou_memories WHERE user_id=?1",
            [&user_id.0],
            |r| r.get(0),
        )
        .unwrap_or(0)
    };

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "memories": memories,
            "total": total
        })),
    )
}

/// GET /api/memories/search — 搜索记忆
pub async fn search_memories(
    State(state): State<AppState>,
    user_id: UserId,
    Query(query): Query<SearchMemoriesQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let q = match query.q.as_ref() {
        Some(q) if !q.trim().is_empty() => q.trim().to_string(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "q parameter is required" })),
            );
        }
    };

    let limit = query.limit.clamp(1, 50);
    let pattern = format!("%{}%", q);

    let db = state.db.lock();

    let mut memories: Vec<serde_json::Value> = Vec::new();
    let mut ids: Vec<String> = Vec::new();

    if let Ok(mut stmt) = db.prepare(
        "SELECT id, category, content, importance, access_count FROM ergou_memories WHERE user_id=?1 AND content LIKE ?2 COLLATE NOCASE ORDER BY importance DESC, access_count DESC LIMIT ?3",
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![user_id.0, pattern, limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3).unwrap_or(3),
                row.get::<_, i64>(4)?,
            ))
        }) {
            for row in rows.flatten() {
                ids.push(row.0.clone());
                memories.push(json!({
                    "id": row.0,
                    "category": row.1,
                    "content": row.2,
                    "importance": row.3,
                    "access_count": row.4
                }));
            }
        }
    }

    // Update access tracking for returned memories
    if !ids.is_empty() {
        let now = chrono::Utc::now().to_rfc3339();
        for id in &ids {
            db.execute(
                "UPDATE ergou_memories SET last_accessed_at=?1, access_count=access_count+1 WHERE id=?2",
                rusqlite::params![now, id],
            )
            .ok();
        }
    }

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "memories": memories
        })),
    )
}

/// PUT /api/memories/{id} — 更新记忆
pub async fn update_memory(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<String>,
    Json(req): Json<UpdateMemoryRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Validate category if provided
    if let Some(ref cat) = req.category {
        if !VALID_CATEGORIES.contains(&cat.as_str()) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "invalid category, must be fact/habit/personality/intent" })),
            );
        }
    }

    // Validate content if provided
    if let Some(ref content) = req.content {
        let content = content.trim();
        if content.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "content cannot be empty" })),
            );
        }
        if content.chars().count() > 500 {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "content exceeds 500 characters" })),
            );
        }
        if is_sensitive_content(content) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "cannot store sensitive information (passwords, card numbers, IDs, etc.)" })),
            );
        }
    }

    // Validate importance if provided
    if let Some(imp) = req.importance {
        if !(1..=5).contains(&imp) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "importance must be between 1 and 5" })),
            );
        }
    }

    // Check at least one field to update
    if req.category.is_none() && req.content.is_none() && req.importance.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "at least one field (category, content, importance) must be provided" })),
        );
    }

    let db = state.db.lock();

    // Check memory exists and belongs to user
    let exists: bool = db
        .query_row(
            "SELECT COUNT(*) > 0 FROM ergou_memories WHERE id=?1 AND user_id=?2",
            rusqlite::params![id, user_id.0],
            |r| r.get(0),
        )
        .unwrap_or(false);
    if !exists {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "memory not found or not owned by current user" })),
        );
    }

    // Dedup check if content is being updated
    if let Some(ref content) = req.content {
        let content = content.trim();
        let dup: bool = db
            .query_row(
                "SELECT COUNT(*) > 0 FROM ergou_memories WHERE user_id=?1 AND content=?2 AND id!=?3",
                rusqlite::params![user_id.0, content, id],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if dup {
            return (
                StatusCode::CONFLICT,
                Json(json!({ "error": "duplicate memory content" })),
            );
        }
    }

    // Build dynamic UPDATE
    let mut set_clauses: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(ref cat) = req.category {
        set_clauses.push(format!("category=?{}", idx));
        params.push(Box::new(cat.clone()));
        idx += 1;
    }
    if let Some(ref content) = req.content {
        set_clauses.push(format!("content=?{}", idx));
        params.push(Box::new(content.trim().to_string()));
        idx += 1;
    }
    if let Some(imp) = req.importance {
        set_clauses.push(format!("importance=?{}", idx));
        params.push(Box::new(imp));
        idx += 1;
    }

    let now = chrono::Utc::now().to_rfc3339();
    set_clauses.push(format!("last_accessed_at=?{}", idx));
    params.push(Box::new(now));
    idx += 1;

    let sql = format!(
        "UPDATE ergou_memories SET {} WHERE id=?{} AND user_id=?{}",
        set_clauses.join(", "),
        idx,
        idx + 1
    );
    params.push(Box::new(id.clone()));
    params.push(Box::new(user_id.0.clone()));

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        params.iter().map(|p| p.as_ref()).collect();

    match db.execute(&sql, params_refs.as_slice()) {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "memory not found" })),
        ),
        Ok(_) => {
            // Fetch updated memory to return
            match db.query_row(
                "SELECT id, category, content, importance, created_at, last_accessed_at, access_count FROM ergou_memories WHERE id=?1",
                [&id],
                |row| {
                    Ok(json!({
                        "id": row.get::<_, String>(0)?,
                        "category": row.get::<_, String>(1)?,
                        "content": row.get::<_, String>(2)?,
                        "importance": row.get::<_, i64>(3)?,
                        "created_at": row.get::<_, String>(4)?,
                        "last_accessed_at": row.get::<_, String>(5)?,
                        "access_count": row.get::<_, i64>(6)?
                    }))
                },
            ) {
                Ok(memory) => (
                    StatusCode::OK,
                    Json(json!({ "success": true, "memory": memory })),
                ),
                Err(_) => (
                    StatusCode::OK,
                    Json(json!({ "success": true })),
                ),
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("update failed: {}", e) })),
        ),
    }
}

/// DELETE /api/memories/{id} — 删除单条
pub async fn delete_memory(
    State(state): State<AppState>,
    user_id: UserId,
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let db = state.db.lock();

    match db.execute(
        "DELETE FROM ergou_memories WHERE id=?1 AND user_id=?2",
        rusqlite::params![id, user_id.0],
    ) {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "memory not found or not owned by current user" })),
        ),
        Ok(_) => (StatusCode::OK, Json(json!({ "success": true }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("delete failed: {}", e) })),
        ),
    }
}

/// DELETE /api/memories — 清空所有记忆
pub async fn clear_memories(
    State(state): State<AppState>,
    user_id: UserId,
) -> (StatusCode, Json<serde_json::Value>) {
    let db = state.db.lock();

    match db.execute(
        "DELETE FROM ergou_memories WHERE user_id=?1",
        [&user_id.0],
    ) {
        Ok(count) => (
            StatusCode::OK,
            Json(json!({ "success": true, "deleted_count": count })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("clear failed: {}", e) })),
        ),
    }
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::{auth_cookie, create_test_user, test_state};
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use serde_json::{Value as JsonValue, json};
    use tower::ServiceExt;

    async fn body_json(resp: axum::response::Response) -> JsonValue {
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn api_memories_put_updates_memory_via_build_app() {
        let state = test_state();
        let (_uid, token) = create_test_user(&state, "memory_editor", "Pa55word1");
        let cookie = auth_cookie(&token);
        let app = crate::build_app(state);

        let create_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/memories")
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        json!({
                            "category": "fact",
                            "content": "用户喜欢绿茶",
                            "importance": 3
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_resp.status(), StatusCode::CREATED);
        let created = body_json(create_resp).await;
        let id = created["memory"]["id"].as_str().unwrap();

        let update_resp = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/memories/{id}"))
                    .header("Cookie", &cookie)
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        json!({
                            "category": "habit",
                            "content": "用户每天喝绿茶",
                            "importance": 5
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(update_resp.status(), StatusCode::OK);
        let updated = body_json(update_resp).await;
        assert_eq!(updated["success"], true);
        assert_eq!(updated["memory"]["id"], id);
        assert_eq!(updated["memory"]["category"], "habit");
        assert_eq!(updated["memory"]["content"], "用户每天喝绿茶");
        assert_eq!(updated["memory"]["importance"], 5);
    }
}
