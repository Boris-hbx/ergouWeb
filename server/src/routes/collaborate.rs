use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;

use crate::auth::{reject_if_guest, ActiveUserId, UserId};
use crate::models::collaboration::*;
use crate::services::collaboration;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct SimpleResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ConfirmationsResponse {
    pub success: bool,
    pub items: Vec<PendingConfirmation>,
}

#[derive(Debug, Serialize)]
pub struct CollaboratorsResponse {
    pub success: bool,
    pub items: Vec<CollaboratorInfo>,
}

#[derive(Debug, Serialize)]
pub struct CollaboratorInfo {
    pub user_id: String,
    pub display_name: String,
    pub role: String,
    pub status: String,
}

/// POST /api/collaborate/todos/:id - Set a collaborator on a todo
pub async fn set_collaborator(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(todo_id): Path<String>,
    Json(req): Json<SetCollaboratorRequest>,
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

    if !collaboration::check_todo_owner(&db, &todo_id, &user_id.0) {
        return (
            StatusCode::FORBIDDEN,
            Json(SimpleResponse {
                success: false,
                message: Some("只有任务所有者可以添加协作者".into()),
            }),
        );
    }

    if !collaboration::check_friendship(&db, &user_id.0, &req.friend_id) {
        return (
            StatusCode::BAD_REQUEST,
            Json(SimpleResponse {
                success: false,
                message: Some("只能添加好友为协作者".into()),
            }),
        );
    }

    if req.friend_id == user_id.0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(SimpleResponse {
                success: false,
                message: Some("不能添加自己为协作者".into()),
            }),
        );
    }

    let (tab, quadrant): (String, String) = db
        .query_row(
            "SELECT tab, quadrant FROM todos WHERE id = ?1",
            [&todo_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or(("today".into(), "not-important-not-urgent".into()));

    let collab_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let now = chrono::Utc::now().to_rfc3339();

    db.execute(
        "INSERT OR IGNORE INTO todo_collaborators (id, todo_id, user_id, role, tab, quadrant, sort_order, status, created_at) VALUES (?1, ?2, ?3, 'collaborator', ?4, ?5, 0.0, 'active', ?6)",
        rusqlite::params![collab_id, todo_id, req.friend_id, tab, quadrant, now],
    )
    .ok();

    db.execute(
        "UPDATE todos SET is_collaborative = 1, updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, todo_id],
    )
    .ok();

    let friend_name =
        collaboration::get_user_display_name(&db, &req.friend_id).unwrap_or_else(|| "用户".into());

    (
        StatusCode::OK,
        Json(SimpleResponse {
            success: true,
            message: Some(format!("已添加 {} 为协作者", friend_name)),
        }),
    )
}

/// DELETE /api/collaborate/todos/:id - Remove collaborator from a todo
pub async fn remove_collaborator(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(todo_id): Path<String>,
    Json(req): Json<SetCollaboratorRequest>,
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

    if !collaboration::check_todo_owner(&db, &todo_id, &user_id.0) {
        return (
            StatusCode::FORBIDDEN,
            Json(SimpleResponse {
                success: false,
                message: Some("只有任务所有者可以移除协作者".into()),
            }),
        );
    }

    db.execute(
        "UPDATE todo_collaborators SET status = 'removed' WHERE todo_id = ?1 AND user_id = ?2 AND status = 'active'",
        rusqlite::params![todo_id, req.friend_id],
    )
    .ok();

    let count = collaboration::count_active_collaborators(&db, &todo_id);
    if count == 0 {
        let now = chrono::Utc::now().to_rfc3339();
        db.execute(
            "UPDATE todos SET is_collaborative = 0, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, todo_id],
        )
        .ok();
    }

    (
        StatusCode::OK,
        Json(SimpleResponse {
            success: true,
            message: Some("已移除协作者".into()),
        }),
    )
}

/// GET /api/collaborate/todos/:id/collaborators - List collaborators
pub async fn list_collaborators(
    State(state): State<AppState>,
    user_id: UserId,
    Path(todo_id): Path<String>,
) -> (StatusCode, Json<CollaboratorsResponse>) {
    let db = state.db.lock();

    if !collaboration::check_todo_participant(&db, &todo_id, &user_id.0) {
        return (
            StatusCode::FORBIDDEN,
            Json(CollaboratorsResponse {
                success: false,
                items: vec![],
            }),
        );
    }

    let mut items = Vec::new();

    if let Some(owner_id) = collaboration::get_todo_owner(&db, &todo_id) {
        if let Some(name) = collaboration::get_user_display_name(&db, &owner_id) {
            items.push(CollaboratorInfo {
                user_id: owner_id,
                display_name: name,
                role: "owner".into(),
                status: "active".into(),
            });
        }
    }

    let mut stmt = match db.prepare(
        "SELECT tc.user_id, COALESCE(u.display_name, u.username) as name, tc.role, tc.status FROM todo_collaborators tc JOIN users u ON tc.user_id = u.id WHERE tc.todo_id = ?1 AND tc.status = 'active'",
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[collaborate] db error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(CollaboratorsResponse { success: false, items: vec![] }));
        }
    };
    let result = stmt.query_map([&todo_id], |row| {
        Ok(CollaboratorInfo {
            user_id: row.get(0)?,
            display_name: row.get(1)?,
            role: row.get(2)?,
            status: row.get(3)?,
        })
    });
    let collabs: Vec<CollaboratorInfo> = match result {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            eprintln!("[collaborate] db error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CollaboratorsResponse {
                    success: false,
                    items: vec![],
                }),
            );
        }
    };
    items.extend(collabs);

    (
        StatusCode::OK,
        Json(CollaboratorsResponse {
            success: true,
            items,
        }),
    )
}

/// GET /api/collaborate/confirmations/pending - List pending confirmations
pub async fn list_pending_confirmations(
    State(state): State<AppState>,
    user_id: UserId,
) -> (StatusCode, Json<ConfirmationsResponse>) {
    let db = state.db.lock();

    let sql = "SELECT pc.id, pc.item_type, pc.item_id, pc.action, pc.initiated_by,                u.display_name, u.username, pc.initiated_at, pc.status,                t.text as item_text                FROM pending_confirmations pc                JOIN users u ON pc.initiated_by = u.id                LEFT JOIN todos t ON pc.item_type = 'todo' AND pc.item_id = t.id                WHERE pc.status = 'pending'                AND (pc.initiated_by = ?1                     OR EXISTS (SELECT 1 FROM todo_collaborators tc WHERE tc.todo_id = pc.item_id AND tc.user_id = ?1 AND tc.status = 'active')                     OR EXISTS (SELECT 1 FROM todos t2 WHERE t2.id = pc.item_id AND t2.user_id = ?1))                ORDER BY pc.initiated_at DESC";

    let mut stmt = match db.prepare(sql) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[collaborate] db error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ConfirmationsResponse {
                    success: false,
                    items: vec![],
                }),
            );
        }
    };

    let result = stmt.query_map([&user_id.0], |row| {
        let display_name: Option<String> = row.get(5)?;
        let username: String = row.get(6)?;
        Ok(PendingConfirmation {
            id: row.get(0)?,
            item_type: row.get(1)?,
            item_id: row.get(2)?,
            action: row.get(3)?,
            initiated_by: row.get(4)?,
            initiated_at: row.get(7)?,
            status: row.get(8)?,
            resolved_at: None,
            initiator_name: Some(display_name.unwrap_or_else(|| username.clone())),
            initiator_username: Some(username),
            item_text: row.get(9)?,
        })
    });
    let items: Vec<PendingConfirmation> = match result {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            eprintln!("[collaborate] db error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ConfirmationsResponse {
                    success: false,
                    items: vec![],
                }),
            );
        }
    };

    (
        StatusCode::OK,
        Json(ConfirmationsResponse {
            success: true,
            items,
        }),
    )
}

/// POST /api/collaborate/confirmations/:id/respond - Respond to a confirmation
pub async fn respond_confirmation(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(confirmation_id): Path<String>,
    Json(req): Json<ConfirmationRespondRequest>,
) -> (StatusCode, Json<SimpleResponse>) {
    let db = state.db.lock();

    if req.response != "approve" && req.response != "reject" {
        return (
            StatusCode::BAD_REQUEST,
            Json(SimpleResponse {
                success: false,
                message: Some("回应只能是 approve 或 reject".into()),
            }),
        );
    }

    let confirmation = db.query_row(
        "SELECT id, item_type, item_id, action, initiated_by, status FROM pending_confirmations WHERE id = ?1",
        [&confirmation_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        },
    );

    let (_id, item_type, item_id, action, initiated_by, status) = match confirmation {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(SimpleResponse {
                    success: false,
                    message: Some("确认请求不存在".into()),
                }),
            );
        }
    };

    if status != "pending" {
        return (
            StatusCode::BAD_REQUEST,
            Json(SimpleResponse {
                success: false,
                message: Some("该确认请求已处理".into()),
            }),
        );
    }

    if user_id.0 == initiated_by {
        return (
            StatusCode::BAD_REQUEST,
            Json(SimpleResponse {
                success: false,
                message: Some("发起者不能回应自己的确认请求".into()),
            }),
        );
    }

    if !collaboration::check_todo_participant(&db, &item_id, &user_id.0) {
        return (
            StatusCode::FORBIDDEN,
            Json(SimpleResponse {
                success: false,
                message: Some("你不是该任务的参与者".into()),
            }),
        );
    }

    let resp_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let now = chrono::Utc::now().to_rfc3339();

    db.execute(
        "INSERT OR REPLACE INTO confirmation_responses (id, confirmation_id, user_id, response, responded_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![resp_id, confirmation_id, user_id.0, req.response, now],
    )
    .ok();

    let (all_responded, all_approved) =
        collaboration::check_all_responded(&db, &confirmation_id, &initiated_by, &item_id);

    if all_responded {
        if all_approved {
            collaboration::execute_confirmation_action(&db, &item_type, &item_id, &action);
            db.execute(
                "UPDATE pending_confirmations SET status = 'resolved', resolved_at = ?1 WHERE id = ?2",
                rusqlite::params![now, confirmation_id],
            )
            .ok();

            let action_text = match action.as_str() {
                "complete" => "完成",
                "delete" => "删除",
                _ => "操作",
            };
            return (
                StatusCode::OK,
                Json(SimpleResponse {
                    success: true,
                    message: Some(format!("全员同意，任务已{}", action_text)),
                }),
            );
        } else {
            db.execute(
                "UPDATE pending_confirmations SET status = 'rejected', resolved_at = ?1 WHERE id = ?2",
                rusqlite::params![now, confirmation_id],
            )
            .ok();

            return (
                StatusCode::OK,
                Json(SimpleResponse {
                    success: true,
                    message: Some("有人拒绝，操作已取消".into()),
                }),
            );
        }
    }

    (
        StatusCode::OK,
        Json(SimpleResponse {
            success: true,
            message: Some("已回应，等待其他人确认".into()),
        }),
    )
}

/// POST /api/collaborate/confirmations/:id/withdraw - Withdraw a confirmation
pub async fn withdraw_confirmation(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(confirmation_id): Path<String>,
) -> (StatusCode, Json<SimpleResponse>) {
    let db = state.db.lock();

    let initiated_by: Option<String> = db
        .query_row(
            "SELECT initiated_by FROM pending_confirmations WHERE id = ?1 AND status = 'pending'",
            [&confirmation_id],
            |row| row.get(0),
        )
        .ok();

    match initiated_by {
        Some(ref initiator) if *initiator == user_id.0 => {
            let now = chrono::Utc::now().to_rfc3339();
            db.execute(
                "UPDATE pending_confirmations SET status = 'withdrawn', resolved_at = ?1 WHERE id = ?2",
                rusqlite::params![now, confirmation_id],
            )
            .ok();

            (
                StatusCode::OK,
                Json(SimpleResponse {
                    success: true,
                    message: Some("已撤回确认请求".into()),
                }),
            )
        }
        Some(_) => (
            StatusCode::FORBIDDEN,
            Json(SimpleResponse {
                success: false,
                message: Some("只有发起者可以撤回".into()),
            }),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(SimpleResponse {
                success: false,
                message: Some("确认请求不存在或已处理".into()),
            }),
        ),
    }
}
