use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;

use crate::auth::UserId;
use crate::state::AppState;

/// GET /api/admin/dashboard — owner-only usage dashboard
pub async fn dashboard(State(state): State<AppState>, user_id: UserId) -> impl IntoResponse {
    let db = state.db.lock();

    // Check if requesting user has admin role
    let is_admin: bool = db
        .query_row("SELECT role FROM users WHERE id = ?1", [&user_id.0], |r| {
            r.get::<_, String>(0)
        })
        .map(|role| role == "admin")
        .unwrap_or(false);

    if !is_admin {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"success": false, "message": "无权限"})),
        );
    }

    // ── 1) User Activity ──

    let total_users: i64 = db
        .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
        .unwrap_or(0);

    let pending_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM users WHERE status = 'pending'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let dau: i64 = db
        .query_row(
            "SELECT COUNT(DISTINCT user_id) FROM (
                SELECT user_id FROM sessions WHERE created_at >= date('now')
                UNION
                SELECT user_id FROM chat_usage_log WHERE created_at >= date('now')
                UNION
                SELECT user_id FROM todos WHERE created_at >= date('now')
            )",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let wau: i64 = db
        .query_row(
            "SELECT COUNT(DISTINCT user_id) FROM (
                SELECT user_id FROM sessions WHERE created_at >= date('now', '-7 days')
                UNION
                SELECT user_id FROM chat_usage_log WHERE created_at >= date('now', '-7 days')
                UNION
                SELECT user_id FROM todos WHERE created_at >= date('now', '-7 days')
            )",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // Per-user details
    let mut user_list = Vec::new();
    {
        let mut stmt = match db.prepare(
            "SELECT u.username, u.display_name, u.created_at,
                    (SELECT MAX(s.created_at) FROM sessions s WHERE s.user_id = u.id) as last_active,
                    (SELECT COUNT(*) FROM sessions s WHERE s.user_id = u.id) as total_sessions
                FROM users u ORDER BY u.created_at ASC",
        ) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[admin] db error: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "error": "内部错误"})));
            }
        };
        let result = stmt.query_map([], |r| {
            Ok(json!({
                "username": r.get::<_, String>(0)?,
                "display_name": r.get::<_, Option<String>>(1)?,
                "created_at": r.get::<_, String>(2)?,
                "last_active": r.get::<_, Option<String>>(3)?,
                "total_sessions": r.get::<_, i64>(4)?
            }))
        });
        let rows = match result {
            Ok(rows) => rows,
            Err(e) => {
                eprintln!("[admin] db error: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "error": "内部错误"})),
                );
            }
        };
        for row in rows.flatten() {
            user_list.push(row);
        }
    }

    // ── 2) Feature Usage ──

    let features = db
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM todos WHERE deleted=0) as todos,
                (SELECT COUNT(*) FROM todos WHERE deleted=0 AND completed=1) as todos_done,
                (SELECT COUNT(*) FROM routines) as routines,
                (SELECT COUNT(*) FROM reviews) as reviews,
                (SELECT COUNT(*) FROM english_scenarios) as scenarios,
                (SELECT COUNT(*) FROM expense_entries) as expenses,
                (SELECT COUNT(*) FROM trips) as trips,
                (SELECT COUNT(*) FROM conversations) as conversations,
                (SELECT COUNT(*) FROM friendships WHERE status='accepted') as friendships,
                (SELECT COUNT(*) FROM shared_items) as shares",
            [],
            |r| {
                Ok(json!({
                    "todos": r.get::<_, i64>(0)?,
                    "todos_completed": r.get::<_, i64>(1)?,
                    "routines": r.get::<_, i64>(2)?,
                    "reviews": r.get::<_, i64>(3)?,
                    "scenarios": r.get::<_, i64>(4)?,
                    "expenses": r.get::<_, i64>(5)?,
                    "trips": r.get::<_, i64>(6)?,
                    "conversations": r.get::<_, i64>(7)?,
                    "friendships": r.get::<_, i64>(8)?,
                    "shares": r.get::<_, i64>(9)?
                }))
            },
        )
        .unwrap_or_else(|_| json!({}));

    // ── 3) AI Usage ──

    let ai_total = db
        .query_row(
            "SELECT COUNT(*), COUNT(DISTINCT conversation_id),
                COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0),
                COALESCE(SUM(tool_calls),0)
            FROM chat_usage_log",
            [],
            |r| {
                Ok(json!({
                    "messages": r.get::<_, i64>(0)?,
                    "conversations": r.get::<_, i64>(1)?,
                    "input_tokens": r.get::<_, i64>(2)?,
                    "output_tokens": r.get::<_, i64>(3)?,
                    "tool_calls": r.get::<_, i64>(4)?
                }))
            },
        )
        .unwrap_or_else(|_| json!({}));

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let week_ago = (chrono::Utc::now() - chrono::Duration::days(7))
        .format("%Y-%m-%d")
        .to_string();
    let month_ago = (chrono::Utc::now() - chrono::Duration::days(30))
        .format("%Y-%m-%d")
        .to_string();
    let ai_today = query_ai_period(&db, &today);
    let ai_week = query_ai_period(&db, &week_ago);
    let ai_month = query_ai_period(&db, &month_ago);

    // Per-user AI usage
    let mut ai_per_user = Vec::new();
    {
        let mut stmt = match db.prepare(
            "SELECT u.username, u.display_name, COUNT(c.id),
                    COALESCE(SUM(c.input_tokens),0), COALESCE(SUM(c.output_tokens),0),
                    COALESCE(SUM(c.tool_calls),0)
                FROM users u LEFT JOIN chat_usage_log c ON c.user_id = u.id
                GROUP BY u.id
                ORDER BY (COALESCE(SUM(c.input_tokens),0)+COALESCE(SUM(c.output_tokens),0)) DESC",
        ) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[admin] db error: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "error": "内部错误"})),
                );
            }
        };
        let result = stmt.query_map([], |r| {
            Ok(json!({
                "username": r.get::<_, String>(0)?,
                "display_name": r.get::<_, Option<String>>(1)?,
                "messages": r.get::<_, i64>(2)?,
                "input_tokens": r.get::<_, i64>(3)?,
                "output_tokens": r.get::<_, i64>(4)?,
                "tool_calls": r.get::<_, i64>(5)?
            }))
        });
        let rows = match result {
            Ok(rows) => rows,
            Err(e) => {
                eprintln!("[admin] db error: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "error": "内部错误"})),
                );
            }
        };
        for row in rows.flatten() {
            ai_per_user.push(row);
        }
    }

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "users": {
                "total": total_users,
                "dau": dau,
                "wau": wau,
                "pending_count": pending_count,
                "list": user_list
            },
            "features": features,
            "ai": {
                "total": ai_total,
                "today": ai_today,
                "week": ai_week,
                "month": ai_month,
                "per_user": ai_per_user
            }
        })),
    )
}

/// Helper: check if user is admin
fn require_admin(
    db: &rusqlite::Connection,
    user_id: &str,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let is_admin: bool = db
        .query_row("SELECT role FROM users WHERE id = ?1", [user_id], |r| {
            r.get::<_, String>(0)
        })
        .map(|role| role == "admin")
        .unwrap_or(false);
    if !is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"success": false, "message": "无权限"})),
        ));
    }
    Ok(())
}

/// GET /api/admin/pending-users
pub async fn pending_users(State(state): State<AppState>, user_id: UserId) -> impl IntoResponse {
    let db = state.db.lock();
    if let Err(e) = require_admin(&db, &user_id.0) {
        return e;
    }

    let mut stmt = match db.prepare(
        "SELECT id, username, display_name, created_at FROM users WHERE status = 'pending' ORDER BY created_at ASC",
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[admin] db error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "error": "内部错误"})));
        }
    };
    let result = stmt.query_map([], |r| {
        Ok(json!({
            "id": r.get::<_, String>(0)?,
            "username": r.get::<_, String>(1)?,
            "display_name": r.get::<_, Option<String>>(2)?,
            "created_at": r.get::<_, String>(3)?
        }))
    });
    let rows: Vec<serde_json::Value> = match result {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            eprintln!("[admin] db error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "内部错误"})),
            );
        }
    };

    (
        StatusCode::OK,
        Json(json!({ "success": true, "users": rows })),
    )
}

/// POST /api/admin/users/{id}/approve
pub async fn approve_user(
    State(state): State<AppState>,
    user_id: UserId,
    Path(target_id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.lock();
    if let Err(e) = require_admin(&db, &user_id.0) {
        return e;
    }

    let now = chrono::Utc::now().to_rfc3339();
    let affected = db
        .execute(
            "UPDATE users SET status = 'active', updated_at = ?1 WHERE id = ?2 AND status = 'pending'",
            rusqlite::params![now, target_id],
        )
        .unwrap_or(0);

    if affected == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "用户不存在或非待审批状态"})),
        );
    }

    // Notify the user
    let notif_id = uuid::Uuid::new_v4().to_string();
    db.execute(
        "INSERT INTO notifications (id, user_id, type, title, body, created_at) VALUES (?1, ?2, 'system', ?3, ?4, ?5)",
        rusqlite::params![notif_id, target_id, "账户已通过审核", "你的账户已通过审核，现在可以正常使用所有功能了。", now],
    )
    .ok();

    (
        StatusCode::OK,
        Json(json!({"success": true, "message": "已通过"})),
    )
}

/// POST /api/admin/users/{id}/reject
pub async fn reject_user(
    State(state): State<AppState>,
    user_id: UserId,
    Path(target_id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.lock();
    if let Err(e) = require_admin(&db, &user_id.0) {
        return e;
    }

    let now = chrono::Utc::now().to_rfc3339();
    let affected = db
        .execute(
            "UPDATE users SET status = 'rejected', updated_at = ?1 WHERE id = ?2 AND status = 'pending'",
            rusqlite::params![now, target_id],
        )
        .unwrap_or(0);

    if affected == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "用户不存在或非待审批状态"})),
        );
    }

    // Invalidate all sessions for the rejected user
    db.execute("DELETE FROM sessions WHERE user_id = ?1", [&target_id])
        .ok();

    (
        StatusCode::OK,
        Json(json!({"success": true, "message": "已拒绝"})),
    )
}

/// GET /api/admin/security-events — list recent security events
pub async fn security_events(
    State(state): State<AppState>,
    user_id: UserId,
) -> impl IntoResponse {
    let db = state.db.lock();
    if let Err(e) = require_admin(&db, &user_id.0) {
        return e;
    }

    let mut stmt = match db.prepare(
        "SELECT se.id, se.user_id, COALESCE(u.display_name, u.username) as user_name, se.event_type, se.severity, se.description, se.admin_notified, se.created_at
         FROM security_events se LEFT JOIN users u ON u.id = se.user_id
         ORDER BY se.created_at DESC LIMIT 50",
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[admin] security events db error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "内部错误"})),
            );
        }
    };

    let rows: Vec<serde_json::Value> = match stmt.query_map([], |r| {
        Ok(json!({
            "id": r.get::<_, String>(0)?,
            "user_id": r.get::<_, String>(1)?,
            "user_name": r.get::<_, String>(2)?,
            "event_type": r.get::<_, String>(3)?,
            "severity": r.get::<_, String>(4)?,
            "description": r.get::<_, String>(5)?,
            "admin_notified": r.get::<_, i64>(6)?,
            "created_at": r.get::<_, String>(7)?
        }))
    }) {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            eprintln!("[admin] security events db error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "内部错误"})),
            );
        }
    };

    // Also get suspended users
    let mut suspended = Vec::new();
    if let Ok(mut stmt2) = db.prepare(
        "SELECT id, username, display_name, updated_at FROM users WHERE status = 'suspended' ORDER BY updated_at DESC",
    ) {
        if let Ok(rows2) = stmt2.query_map([], |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "username": r.get::<_, String>(1)?,
                "display_name": r.get::<_, Option<String>>(2)?,
                "suspended_at": r.get::<_, String>(3)?
            }))
        }) {
            for row in rows2.flatten() {
                suspended.push(row);
            }
        }
    }

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "events": rows,
            "suspended_users": suspended
        })),
    )
}

/// POST /api/admin/users/{id}/restore — restore a suspended user
pub async fn restore_user(
    State(state): State<AppState>,
    user_id: UserId,
    Path(target_id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.lock();
    if let Err(e) = require_admin(&db, &user_id.0) {
        return e;
    }

    let now = chrono::Utc::now().to_rfc3339();
    let affected = db
        .execute(
            "UPDATE users SET status = 'active', updated_at = ?1 WHERE id = ?2 AND status = 'suspended'",
            rusqlite::params![now, target_id],
        )
        .unwrap_or(0);

    if affected == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "用户不存在或非挂起状态"})),
        );
    }

    (
        StatusCode::OK,
        Json(json!({"success": true, "message": "已恢复用户"})),
    )
}

fn query_ai_period(db: &rusqlite::Connection, since_date: &str) -> serde_json::Value {
    db.query_row(
        "SELECT COUNT(*), COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0)
         FROM chat_usage_log WHERE created_at >= ?1",
        [since_date],
        |r| {
            Ok(json!({
                "messages": r.get::<_, i64>(0)?,
                "input_tokens": r.get::<_, i64>(1)?,
                "output_tokens": r.get::<_, i64>(2)?
            }))
        },
    )
    .unwrap_or_else(|_| json!({}))
}
