use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;

use crate::auth::{reject_if_guest, ActiveUserId, UserId};
use crate::models::friend::*;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct FriendsResponse {
    pub success: bool,
    pub items: Vec<FriendInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FriendRequestsResponse {
    pub success: bool,
    pub items: Vec<FriendRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SharedItemsResponse {
    pub success: bool,
    pub items: Vec<SharedItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SimpleResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CountResponse {
    pub success: bool,
    pub count: i64,
}

// ─── Friends ───

pub async fn list_friends(
    State(state): State<AppState>,
    user_id: UserId,
) -> (StatusCode, Json<FriendsResponse>) {
    let db = state.db.lock();

    let mut stmt = match db.prepare(
        "SELECT f.id, u.id, u.username, u.display_name
             FROM friendships f
             JOIN users u ON (
                 CASE WHEN f.requester_id = ?1 THEN f.addressee_id ELSE f.requester_id END = u.id
             )
             WHERE (f.requester_id = ?1 OR f.addressee_id = ?1) AND f.status = 'accepted'",
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[friends] db error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(FriendsResponse {
                    success: false,
                    items: vec![],
                    message: Some("内部错误".into()),
                }),
            );
        }
    };

    let result = stmt.query_map([&user_id.0], |row| {
        Ok(FriendInfo {
            friendship_id: row.get(0)?,
            id: row.get(1)?,
            username: row.get(2)?,
            display_name: row.get(3)?,
        })
    });
    let items: Vec<FriendInfo> = match result {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            eprintln!("[friends] db error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(FriendsResponse {
                    success: false,
                    items: vec![],
                    message: Some("内部错误".into()),
                }),
            );
        }
    };

    (
        StatusCode::OK,
        Json(FriendsResponse {
            success: true,
            items,
            message: None,
        }),
    )
}

pub async fn list_friend_requests(
    State(state): State<AppState>,
    user_id: UserId,
) -> (StatusCode, Json<FriendRequestsResponse>) {
    let db = state.db.lock();

    let mut stmt = match db.prepare(
        "SELECT f.id, u.id, u.username, u.display_name, f.status, f.created_at
             FROM friendships f
             JOIN users u ON f.requester_id = u.id
             WHERE f.addressee_id = ?1 AND f.status = 'pending'
             ORDER BY f.created_at DESC",
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[friends] db error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(FriendRequestsResponse {
                    success: false,
                    items: vec![],
                    message: Some("内部错误".into()),
                }),
            );
        }
    };

    let result = stmt.query_map([&user_id.0], |row| {
        Ok(FriendRequest {
            id: row.get(0)?,
            from_user: FriendInfo {
                friendship_id: row.get(0)?,
                id: row.get(1)?,
                username: row.get(2)?,
                display_name: row.get(3)?,
            },
            status: row.get(4)?,
            created_at: row.get(5)?,
        })
    });
    let items: Vec<FriendRequest> = match result {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            eprintln!("[friends] db error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(FriendRequestsResponse {
                    success: false,
                    items: vec![],
                    message: Some("内部错误".into()),
                }),
            );
        }
    };

    (
        StatusCode::OK,
        Json(FriendRequestsResponse {
            success: true,
            items,
            message: None,
        }),
    )
}

pub async fn send_friend_request(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Json(req): Json<FriendRequestPayload>,
) -> (StatusCode, Json<SimpleResponse>) {
    if reject_if_guest(&state, &user_id.0).is_some() {
        return (
            StatusCode::FORBIDDEN,
            Json(SimpleResponse {
                success: false,
                message: Some("体验模式不支持此功能，注册账户解锁".into()),
            }),
        );
    }
    let db = state.db.lock();

    // Find target user
    let target = db.query_row(
        "SELECT id FROM users WHERE username = ?1",
        [&req.username],
        |row| row.get::<_, String>(0),
    );

    let target_id = match target {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(SimpleResponse {
                    success: false,
                    message: Some("用户不存在".into()),
                }),
            )
        }
    };

    if target_id == user_id.0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(SimpleResponse {
                success: false,
                message: Some("不能添加自己为好友".into()),
            }),
        );
    }

    // Check existing friendship
    let existing: bool = db
        .query_row(
            "SELECT COUNT(*) > 0 FROM friendships WHERE (requester_id = ?1 AND addressee_id = ?2) OR (requester_id = ?2 AND addressee_id = ?1)",
            rusqlite::params![user_id.0, target_id],
            |r| r.get(0),
        )
        .unwrap_or(false);

    if existing {
        return (
            StatusCode::CONFLICT,
            Json(SimpleResponse {
                success: false,
                message: Some("已存在好友关系或待处理请求".into()),
            }),
        );
    }

    let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let now = chrono::Utc::now().to_rfc3339();

    if let Err(e) = db.execute(
        "INSERT INTO friendships (id, requester_id, addressee_id, status, created_at, updated_at) VALUES (?1, ?2, ?3, 'pending', ?4, ?5)",
        rusqlite::params![id, user_id.0, target_id, now, now],
    ) {
        eprintln!("[friends] db error: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SimpleResponse {
                success: false,
                message: Some("内部错误".into()),
            }),
        );
    }

    (
        StatusCode::OK,
        Json(SimpleResponse {
            success: true,
            message: Some("好友请求已发送".into()),
        }),
    )
}

pub async fn accept_friend(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(id): Path<String>,
) -> (StatusCode, Json<SimpleResponse>) {
    if reject_if_guest(&state, &user_id.0).is_some() {
        return (
            StatusCode::FORBIDDEN,
            Json(SimpleResponse {
                success: false,
                message: Some("体验模式不支持此功能，注册账户解锁".into()),
            }),
        );
    }
    let db = state.db.lock();
    let now = chrono::Utc::now().to_rfc3339();

    let rows = db
        .execute(
            "UPDATE friendships SET status = 'accepted', updated_at = ?1 WHERE id = ?2 AND addressee_id = ?3 AND status = 'pending'",
            rusqlite::params![now, id, user_id.0],
        )
        .unwrap_or(0);

    if rows == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(SimpleResponse {
                success: false,
                message: Some("好友请求不存在".into()),
            }),
        );
    }

    // Auto-create contacts for both users
    if let Ok((requester_id, addressee_id)) = db.query_row(
        "SELECT requester_id, addressee_id FROM friendships WHERE id = ?1",
        [&id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    ) {
        // Get display names for both users
        let requester_name: String = db
            .query_row(
                "SELECT COALESCE(display_name, username) FROM users WHERE id = ?1",
                [&requester_id],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "Unknown".to_string());

        let addressee_name: String = db
            .query_row(
                "SELECT COALESCE(display_name, username) FROM users WHERE id = ?1",
                [&addressee_id],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "Unknown".to_string());

        // Create contact for addressee -> requester
        let contact_id1 = uuid::Uuid::new_v4().to_string()[..8].to_string();
        db.execute(
            "INSERT OR IGNORE INTO contacts (id, user_id, name, linked_user_id, friendship_id, note, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, '', ?6, ?7)",
            rusqlite::params![contact_id1, addressee_id, requester_name, requester_id, id, now, now],
        ).ok();

        // Create contact for requester -> addressee
        let contact_id2 = uuid::Uuid::new_v4().to_string()[..8].to_string();
        db.execute(
            "INSERT OR IGNORE INTO contacts (id, user_id, name, linked_user_id, friendship_id, note, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, '', ?6, ?7)",
            rusqlite::params![contact_id2, requester_id, addressee_name, addressee_id, id, now, now],
        ).ok();
    }

    (
        StatusCode::OK,
        Json(SimpleResponse {
            success: true,
            message: Some("已接受".into()),
        }),
    )
}

pub async fn decline_friend(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(id): Path<String>,
) -> (StatusCode, Json<SimpleResponse>) {
    if reject_if_guest(&state, &user_id.0).is_some() {
        return (
            StatusCode::FORBIDDEN,
            Json(SimpleResponse {
                success: false,
                message: Some("体验模式不支持此功能，注册账户解锁".into()),
            }),
        );
    }
    let db = state.db.lock();
    let now = chrono::Utc::now().to_rfc3339();

    let rows = db
        .execute(
            "UPDATE friendships SET status = 'declined', updated_at = ?1 WHERE id = ?2 AND addressee_id = ?3 AND status = 'pending'",
            rusqlite::params![now, id, user_id.0],
        )
        .unwrap_or(0);

    if rows == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(SimpleResponse {
                success: false,
                message: Some("好友请求不存在".into()),
            }),
        );
    }

    (
        StatusCode::OK,
        Json(SimpleResponse {
            success: true,
            message: Some("已拒绝".into()),
        }),
    )
}

pub async fn delete_friend(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(id): Path<String>,
) -> (StatusCode, Json<SimpleResponse>) {
    if reject_if_guest(&state, &user_id.0).is_some() {
        return (
            StatusCode::FORBIDDEN,
            Json(SimpleResponse {
                success: false,
                message: Some("体验模式不支持此功能，注册账户解锁".into()),
            }),
        );
    }
    let db = state.db.lock();

    // Get the other user's id before deleting
    let other_user_id: Option<String> = db
        .query_row(
            "SELECT CASE WHEN requester_id = ?1 THEN addressee_id ELSE requester_id END FROM friendships WHERE id = ?2 AND (requester_id = ?1 OR addressee_id = ?1)",
            rusqlite::params![user_id.0, id],
            |row| row.get(0),
        )
        .ok();

    let rows = db
        .execute(
            "DELETE FROM friendships WHERE id = ?1 AND (requester_id = ?2 OR addressee_id = ?2)",
            rusqlite::params![id, user_id.0],
        )
        .unwrap_or(0);

    if rows == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(SimpleResponse {
                success: false,
                message: Some("好友关系不存在".into()),
            }),
        );
    }

    // Cascade: clean up collaboration and contacts
    if let Some(ref other_id) = other_user_id {
        // Mark todo collaborations as 'left'
        db.execute(
            "UPDATE todo_collaborators SET status = 'left' WHERE (user_id = ?1 AND todo_id IN (SELECT id FROM todos WHERE user_id = ?2)) OR (user_id = ?2 AND todo_id IN (SELECT id FROM todos WHERE user_id = ?1))",
            rusqlite::params![user_id.0, other_id],
        ).ok();

        // Mark routine collaborations as 'left'
        db.execute(
            "UPDATE routine_collaborators SET status = 'left' WHERE (user_id = ?1 AND routine_id IN (SELECT id FROM routines WHERE user_id = ?2)) OR (user_id = ?2 AND routine_id IN (SELECT id FROM routines WHERE user_id = ?1))",
            rusqlite::params![user_id.0, other_id],
        ).ok();

        // Remove linked contacts for both users
        db.execute(
            "DELETE FROM contacts WHERE friendship_id = ?1",
            rusqlite::params![id],
        )
        .ok();
    }

    (
        StatusCode::OK,
        Json(SimpleResponse {
            success: true,
            message: Some("已删除好友".into()),
        }),
    )
}

pub async fn search_users(
    State(state): State<AppState>,
    user_id: UserId,
    Query(query): Query<SearchQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let db = state.db.lock();
    let keyword = format!("%{}%", query.q);

    let mut stmt = match db.prepare(
        "SELECT id, username, display_name FROM users WHERE username LIKE ?1 AND id != ?2 LIMIT 10",
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[friends] db error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "内部错误"})),
            );
        }
    };

    let result = stmt.query_map(rusqlite::params![keyword, user_id.0], |row| {
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "username": row.get::<_, String>(1)?,
            "display_name": row.get::<_, Option<String>>(2)?
        }))
    });
    let users: Vec<serde_json::Value> = match result {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            eprintln!("[friends] db error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "内部错误"})),
            );
        }
    };

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "items": users
        })),
    )
}

// ─── Sharing ───

pub async fn share_item(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Json(req): Json<SharePayload>,
) -> (StatusCode, Json<SimpleResponse>) {
    if reject_if_guest(&state, &user_id.0).is_some() {
        return (
            StatusCode::FORBIDDEN,
            Json(SimpleResponse {
                success: false,
                message: Some("体验模式不支持此功能，注册账户解锁".into()),
            }),
        );
    }
    let db = state.db.lock();

    // Verify friendship
    let is_friend: bool = db
        .query_row(
            "SELECT COUNT(*) > 0 FROM friendships WHERE status = 'accepted' AND ((requester_id = ?1 AND addressee_id = ?2) OR (requester_id = ?2 AND addressee_id = ?1))",
            rusqlite::params![user_id.0, req.friend_id],
            |r| r.get(0),
        )
        .unwrap_or(false);

    if !is_friend {
        return (
            StatusCode::FORBIDDEN,
            Json(SimpleResponse {
                success: false,
                message: Some("对方不是你的好友".into()),
            }),
        );
    }

    // Build snapshot based on item_type
    let snapshot = match req.item_type.as_str() {
        "todo" => {
            db.query_row(
                "SELECT id, text, content, tab, quadrant, progress, due_date, assignee, tags FROM todos WHERE id = ?1 AND user_id = ?2 AND deleted = 0",
                rusqlite::params![req.item_id, user_id.0],
                |row| {
                    Ok(json!({
                        "id": row.get::<_, String>(0)?,
                        "text": row.get::<_, String>(1)?,
                        "content": row.get::<_, String>(2).unwrap_or_default(),
                        "tab": row.get::<_, String>(3)?,
                        "quadrant": row.get::<_, String>(4)?,
                        "progress": row.get::<_, i64>(5)?,
                        "due_date": row.get::<_, Option<String>>(6)?,
                        "assignee": row.get::<_, String>(7).unwrap_or_default(),
                        "tags": row.get::<_, String>(8).unwrap_or_else(|_| "[]".into())
                    }))
                },
            )
            .ok()
        }
        "review" => {
            db.query_row(
                "SELECT id, text, frequency, frequency_config, notes, category FROM reviews WHERE id = ?1 AND user_id = ?2",
                rusqlite::params![req.item_id, user_id.0],
                |row| {
                    Ok(json!({
                        "id": row.get::<_, String>(0)?,
                        "text": row.get::<_, String>(1)?,
                        "frequency": row.get::<_, String>(2)?,
                        "frequency_config": row.get::<_, String>(3).unwrap_or_else(|_| "{}".into()),
                        "notes": row.get::<_, String>(4).unwrap_or_default(),
                        "category": row.get::<_, String>(5).unwrap_or_default()
                    }))
                },
            )
            .ok()
        }
        "scenario" => {
            db.query_row(
                "SELECT id, title, title_en, description, icon, content, COALESCE(category, '英语'), COALESCE(notes, '') FROM english_scenarios WHERE id = ?1 AND user_id = ?2",
                rusqlite::params![req.item_id, user_id.0],
                |row| {
                    Ok(json!({
                        "id": row.get::<_, String>(0)?,
                        "title": row.get::<_, String>(1)?,
                        "title_en": row.get::<_, String>(2).unwrap_or_default(),
                        "description": row.get::<_, String>(3).unwrap_or_default(),
                        "icon": row.get::<_, String>(4).unwrap_or_default(),
                        "content": row.get::<_, String>(5).unwrap_or_default(),
                        "category": row.get::<_, String>(6).unwrap_or_else(|_| "英语".into()),
                        "notes": row.get::<_, String>(7).unwrap_or_default()
                    }))
                },
            )
            .ok()
        }
        "routine" => {
            db.query_row(
                "SELECT id, text FROM routines WHERE id = ?1 AND user_id = ?2",
                rusqlite::params![req.item_id, user_id.0],
                |row| {
                    Ok(json!({
                        "id": row.get::<_, String>(0)?,
                        "text": row.get::<_, String>(1)?
                    }))
                },
            )
            .ok()
        }
        "expense" => {
            db.query_row(
                "SELECT id, amount, date, notes, tags, currency FROM expense_entries WHERE id = ?1 AND user_id = ?2",
                rusqlite::params![req.item_id, user_id.0],
                |row| {
                    Ok(json!({
                        "id": row.get::<_, String>(0)?,
                        "amount": row.get::<_, f64>(1)?,
                        "date": row.get::<_, String>(2)?,
                        "notes": row.get::<_, String>(3).unwrap_or_default(),
                        "tags": row.get::<_, String>(4).unwrap_or_else(|_| "[]".into()),
                        "currency": row.get::<_, String>(5).unwrap_or_else(|_| "CAD".into())
                    }))
                },
            )
            .ok()
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(SimpleResponse {
                    success: false,
                    message: Some("不支持的分享类型".into()),
                }),
            )
        }
    };

    let snapshot = match snapshot {
        Some(s) => s,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(SimpleResponse {
                    success: false,
                    message: Some("要分享的内容不存在".into()),
                }),
            )
        }
    };

    let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let snapshot_str = serde_json::to_string(&snapshot).unwrap_or_default();
    let message = req.message.unwrap_or_default();

    if let Err(e) = db.execute(
        "INSERT INTO shared_items (id, sender_id, recipient_id, item_type, item_id, item_snapshot, message, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'unread', ?8)",
        rusqlite::params![id, user_id.0, req.friend_id, req.item_type, req.item_id, snapshot_str, message, now],
    ) {
        eprintln!("[friends] db error: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SimpleResponse {
                success: false,
                message: Some("内部错误".into()),
            }),
        );
    }

    (
        StatusCode::OK,
        Json(SimpleResponse {
            success: true,
            message: Some("分享成功".into()),
        }),
    )
}

pub async fn shared_inbox(
    State(state): State<AppState>,
    user_id: UserId,
    Query(params): Query<HashMap<String, String>>,
) -> (StatusCode, Json<SharedItemsResponse>) {
    if reject_if_guest(&state, &user_id.0).is_some() {
        return (
            StatusCode::FORBIDDEN,
            Json(SharedItemsResponse {
                success: false,
                items: vec![],
                message: Some("体验模式不支持此功能，注册账户解锁".into()),
            }),
        );
    }
    let db = state.db.lock();
    let item_type_filter = params.get("type").cloned().unwrap_or_default();

    let (sql, use_filter) = if item_type_filter.is_empty() {
        (
            "SELECT s.id, s.sender_id, u.display_name, u.username, s.recipient_id, s.item_type, s.item_id, s.item_snapshot, s.message, s.status, s.created_at
             FROM shared_items s
             JOIN users u ON s.sender_id = u.id
             WHERE s.recipient_id = ?1 AND s.status IN ('unread', 'read')
             ORDER BY s.created_at DESC".to_string(),
            false,
        )
    } else {
        (
            "SELECT s.id, s.sender_id, u.display_name, u.username, s.recipient_id, s.item_type, s.item_id, s.item_snapshot, s.message, s.status, s.created_at
             FROM shared_items s
             JOIN users u ON s.sender_id = u.id
             WHERE s.recipient_id = ?1 AND s.status IN ('unread', 'read') AND s.item_type = ?2
             ORDER BY s.created_at DESC".to_string(),
            true,
        )
    };

    let mut stmt = match db.prepare(&sql) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[friends] db error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SharedItemsResponse {
                    success: false,
                    items: vec![],
                    message: Some("内部错误".into()),
                }),
            );
        }
    };

    let row_mapper = |row: &rusqlite::Row| -> rusqlite::Result<SharedItem> {
        let display_name: Option<String> = row.get(2)?;
        let username: String = row.get(3)?;
        let snapshot_str: String = row.get(7)?;
        let snapshot: serde_json::Value = serde_json::from_str(&snapshot_str).unwrap_or(json!({}));

        Ok(SharedItem {
            id: row.get(0)?,
            sender_id: row.get(1)?,
            sender_name: Some(display_name.unwrap_or(username)),
            recipient_id: row.get(4)?,
            item_type: row.get(5)?,
            item_id: row.get(6)?,
            item_snapshot: snapshot,
            message: row.get::<_, String>(8).unwrap_or_default(),
            status: row.get(9)?,
            created_at: row.get(10)?,
        })
    };

    let items: Vec<SharedItem> = if use_filter {
        let result = stmt.query_map(rusqlite::params![user_id.0, item_type_filter], &row_mapper);
        match result {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(e) => {
                eprintln!("[friends] db error: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(SharedItemsResponse {
                        success: false,
                        items: vec![],
                        message: Some("内部错误".into()),
                    }),
                );
            }
        }
    } else {
        let result = stmt.query_map([&user_id.0], &row_mapper);
        match result {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(e) => {
                eprintln!("[friends] db error: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(SharedItemsResponse {
                        success: false,
                        items: vec![],
                        message: Some("内部错误".into()),
                    }),
                );
            }
        }
    };

    (
        StatusCode::OK,
        Json(SharedItemsResponse {
            success: true,
            items,
            message: None,
        }),
    )
}

pub async fn shared_inbox_count(
    State(state): State<AppState>,
    user_id: UserId,
) -> (StatusCode, Json<CountResponse>) {
    let db = state.db.lock();

    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM shared_items WHERE recipient_id = ?1 AND status = 'unread'",
            [&user_id.0],
            |r| r.get(0),
        )
        .unwrap_or(0);

    (
        StatusCode::OK,
        Json(CountResponse {
            success: true,
            count,
        }),
    )
}

/// Accept shared item — returns new_id + item_type for navigation
pub async fn accept_shared(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Some(e) = reject_if_guest(&state, &user_id.0) {
        return e;
    }
    let db = state.db.lock();

    let result = db.query_row(
        "SELECT item_type, item_snapshot FROM shared_items WHERE id = ?1 AND recipient_id = ?2 AND status IN ('unread', 'read')",
        rusqlite::params![id, user_id.0],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    );

    let (item_type, snapshot_str) = match result {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "message": "分享不存在" })),
            )
        }
    };

    let snapshot: serde_json::Value = serde_json::from_str(&snapshot_str).unwrap_or(json!({}));
    let new_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let now = chrono::Utc::now().to_rfc3339();

    match item_type.as_str() {
        "todo" => {
            let text = snapshot["text"].as_str().unwrap_or("(分享的任务)");
            let content = snapshot["content"].as_str().unwrap_or("");
            let tab = snapshot["tab"].as_str().unwrap_or("today");
            let quadrant = snapshot["quadrant"]
                .as_str()
                .unwrap_or("not-important-not-urgent");
            let tags = snapshot["tags"].as_str().unwrap_or("[]");

            db.execute(
                "INSERT INTO todos (id, user_id, text, content, tab, quadrant, progress, completed, tags, sort_order, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, ?7, 0.0, ?8, ?9)",
                rusqlite::params![new_id, user_id.0, text, content, tab, quadrant, tags, now, now],
            )
            .ok();
        }
        "review" => {
            let text = snapshot["text"].as_str().unwrap_or("(分享的例行)");
            let frequency = snapshot["frequency"].as_str().unwrap_or("weekly");
            let freq_config = snapshot["frequency_config"].as_str().unwrap_or("{}");
            let notes = snapshot["notes"].as_str().unwrap_or("");
            let category = snapshot["category"].as_str().unwrap_or("");

            db.execute(
                "INSERT INTO reviews (id, user_id, text, frequency, frequency_config, notes, category, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![new_id, user_id.0, text, frequency, freq_config, notes, category, now, now],
            )
            .ok();
        }
        "scenario" => {
            let title = snapshot["title"].as_str().unwrap_or("(分享的场景)");
            let title_en = snapshot["title_en"].as_str().unwrap_or("");
            let description = snapshot["description"].as_str().unwrap_or("");
            let icon = snapshot["icon"].as_str().unwrap_or("📖");
            let content = snapshot["content"].as_str().unwrap_or("");
            let category = snapshot["category"].as_str().unwrap_or("英语");
            let notes = snapshot["notes"].as_str().unwrap_or("");

            db.execute(
                "INSERT INTO english_scenarios (id, user_id, title, title_en, description, icon, content, status, archived, created_at, updated_at, category, notes) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'ready', 0, ?8, ?9, ?10, ?11)",
                rusqlite::params![new_id, user_id.0, title, title_en, description, icon, content, now, now, category, notes],
            )
            .ok();
        }
        "routine" => {
            let text = snapshot["text"].as_str().unwrap_or("(分享的例行)");
            db.execute(
                "INSERT INTO routines (id, user_id, text, completed_today, created_at) VALUES (?1, ?2, ?3, 0, ?4)",
                rusqlite::params![new_id, user_id.0, text, now],
            )
            .ok();
        }
        "expense" => {
            let amount = snapshot["amount"].as_f64().unwrap_or(0.0);
            let date = snapshot["date"].as_str().unwrap_or(&now);
            let notes = snapshot["notes"].as_str().unwrap_or("");
            let tags = snapshot["tags"].as_str().unwrap_or("[]");
            let currency = snapshot["currency"].as_str().unwrap_or("CAD");
            db.execute(
                "INSERT INTO expense_entries (id, user_id, amount, date, notes, tags, currency, ai_processed, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9)",
                rusqlite::params![new_id, user_id.0, amount, date, notes, tags, currency, now, now],
            )
            .ok();
        }
        _ => {}
    }

    db.execute(
        "UPDATE shared_items SET status = 'accepted' WHERE id = ?1",
        [&id],
    )
    .ok();

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "已收下",
            "new_id": new_id,
            "item_type": item_type
        })),
    )
}

pub async fn dismiss_shared(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(id): Path<String>,
) -> (StatusCode, Json<SimpleResponse>) {
    if reject_if_guest(&state, &user_id.0).is_some() {
        return (
            StatusCode::FORBIDDEN,
            Json(SimpleResponse {
                success: false,
                message: Some("体验模式不支持此功能，注册账户解锁".into()),
            }),
        );
    }
    let db = state.db.lock();

    let rows = db
        .execute(
            "UPDATE shared_items SET status = 'dismissed' WHERE id = ?1 AND recipient_id = ?2",
            rusqlite::params![id, user_id.0],
        )
        .unwrap_or(0);

    if rows == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(SimpleResponse {
                success: false,
                message: Some("分享不存在".into()),
            }),
        );
    }

    (
        StatusCode::OK,
        Json(SimpleResponse {
            success: true,
            message: Some("已忽略".into()),
        }),
    )
}

/// GET /api/share/sent?item_type=scenario&item_id=xxx
/// Returns list of recipient_ids this item has been shared to (for duplicate detection)
pub async fn shared_sent(
    State(state): State<AppState>,
    user_id: UserId,
    Query(params): Query<HashMap<String, String>>,
) -> (StatusCode, Json<serde_json::Value>) {
    let item_type = params.get("item_type").cloned().unwrap_or_default();
    let item_id = params.get("item_id").cloned().unwrap_or_default();

    let db = state.db.lock();

    let mut stmt = match db.prepare(
        "SELECT s.recipient_id, u.username, u.display_name, s.status, s.created_at
             FROM shared_items s
             JOIN users u ON s.recipient_id = u.id
             WHERE s.sender_id = ?1 AND s.item_type = ?2 AND s.item_id = ?3
             ORDER BY s.created_at DESC",
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[friends] db error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "内部错误"})),
            );
        }
    };

    let result = stmt.query_map(rusqlite::params![user_id.0, item_type, item_id], |row| {
        Ok(json!({
            "recipient_id": row.get::<_, String>(0)?,
            "username": row.get::<_, String>(1)?,
            "display_name": row.get::<_, Option<String>>(2)?,
            "status": row.get::<_, String>(3)?,
            "created_at": row.get::<_, String>(4)?
        }))
    });
    let items: Vec<serde_json::Value> = match result {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            eprintln!("[friends] db error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "内部错误"})),
            );
        }
    };

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "items": items
        })),
    )
}
