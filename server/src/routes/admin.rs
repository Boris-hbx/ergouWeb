use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;

use crate::auth::{AdminUserId, OwnerUserId};
use crate::state::AppState;

/// GET /api/admin/dashboard — admin usage dashboard
pub async fn dashboard(State(state): State<AppState>, admin: AdminUserId) -> impl IntoResponse {
    let _ = admin;
    let db = state.db.lock();

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

/// Insert an audit log entry
fn insert_audit_log(
    db: &rusqlite::Connection,
    admin_user_id: &str,
    action_type: &str,
    target_user_id: Option<&str>,
    target_resource: Option<&str>,
    details: Option<&str>,
) {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    if let Err(e) = db.execute(
        "INSERT INTO admin_audit_log (id, admin_user_id, action_type, target_user_id, target_resource, details, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![id, admin_user_id, action_type, target_user_id, target_resource, details, now],
    ) {
        tracing::warn!("[admin] audit log insert failed: {}", e);
    }
}

/// GET /api/admin/pending-users
pub async fn pending_users(State(state): State<AppState>, admin: AdminUserId) -> impl IntoResponse {
    let _ = admin;
    let db = state.db.lock();

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
    admin: AdminUserId,
    Path(target_id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.lock();

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

    insert_audit_log(&db, &admin.0, "approve_user", Some(&target_id), None, None);

    (
        StatusCode::OK,
        Json(json!({"success": true, "message": "已通过"})),
    )
}

/// POST /api/admin/users/{id}/reject
pub async fn reject_user(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path(target_id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.lock();

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

    insert_audit_log(&db, &admin.0, "reject_user", Some(&target_id), None, None);

    (
        StatusCode::OK,
        Json(json!({"success": true, "message": "已拒绝"})),
    )
}

/// GET /api/admin/security-events — list recent security events
pub async fn security_events(
    State(state): State<AppState>,
    admin: AdminUserId,
) -> impl IntoResponse {
    let _ = admin;
    let db = state.db.lock();

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
    admin: AdminUserId,
    Path(target_id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.lock();

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

    insert_audit_log(&db, &admin.0, "restore_user", Some(&target_id), None, None);

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

// ===== NEW ENDPOINTS =====

/// GET /api/admin/users — full user list with stats
pub async fn list_users(
    State(state): State<AppState>,
    admin: AdminUserId,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let _ = admin;
    let db = state.db.lock();

    let search = params.get("search").cloned().unwrap_or_default();
    let status_filter = params.get("status").cloned().unwrap_or_default();
    let role_filter = params.get("role").cloned().unwrap_or_default();
    let sort = params.get("sort").cloned().unwrap_or_else(|| "created_at".into());
    let order = params.get("order").cloned().unwrap_or_else(|| "desc".into());

    // Build safe ORDER BY
    let sort_col = match sort.as_str() {
        "username" => "u.username",
        "role" => "u.role",
        "status" => "u.status",
        "last_active" => "last_active",
        _ => "u.created_at",
    };
    let sort_dir = if order == "asc" { "ASC" } else { "DESC" };

    let sql = format!(
        "SELECT u.id, u.username, u.display_name, u.role, u.status, u.created_at,
                (SELECT MAX(s.created_at) FROM sessions s WHERE s.user_id = u.id) as last_active,
                (SELECT COUNT(*) FROM todos t WHERE t.user_id = u.id AND t.deleted=0) as todo_count,
                (SELECT COUNT(*) FROM expense_entries e WHERE e.user_id = u.id) as expense_count,
                (SELECT COUNT(*) FROM conversations c WHERE c.user_id = u.id) as chat_count,
                (SELECT COUNT(*) FROM trips tr WHERE tr.user_id = u.id) as trip_count,
                (SELECT COALESCE(SUM(cl.input_tokens + cl.output_tokens), 0) FROM chat_usage_log cl WHERE cl.user_id = u.id) as total_tokens
         FROM users u
         WHERE (u.username LIKE '%' || ?1 || '%' OR COALESCE(u.display_name,'') LIKE '%' || ?1 || '%' OR ?1 = '')
           AND (u.status = ?2 OR ?2 = '')
           AND (u.role = ?3 OR ?3 = '')
         ORDER BY {} {}",
        sort_col, sort_dir
    );

    let mut stmt = match db.prepare(&sql) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("[admin] list_users query error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "error": "内部错误"})));
        }
    };

    let rows: Vec<serde_json::Value> = stmt
        .query_map(rusqlite::params![search, status_filter, role_filter], |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "username": r.get::<_, String>(1)?,
                "display_name": r.get::<_, Option<String>>(2)?,
                "role": r.get::<_, String>(3)?,
                "status": r.get::<_, String>(4)?,
                "created_at": r.get::<_, String>(5)?,
                "last_active": r.get::<_, Option<String>>(6)?,
                "todo_count": r.get::<_, i64>(7)?,
                "expense_count": r.get::<_, i64>(8)?,
                "chat_count": r.get::<_, i64>(9)?,
                "trip_count": r.get::<_, i64>(10)?,
                "total_tokens": r.get::<_, i64>(11)?
            }))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    (StatusCode::OK, Json(json!({"success": true, "users": rows})))
}

/// PUT /api/admin/users/{id}/role — owner-only role change
pub async fn change_role(
    State(state): State<AppState>,
    owner: OwnerUserId,
    Path(target_id): Path<String>,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    let new_role = req.get("role").and_then(|v| v.as_str()).unwrap_or("");
    if new_role != "admin" && new_role != "user" {
        return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "message": "角色只能是 admin 或 user"})));
    }

    if target_id == owner.0 {
        return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "message": "不能修改自己的角色"})));
    }

    let db = state.db.lock();

    // Prevent changing owner's role
    let target_role: String = db
        .query_row("SELECT role FROM users WHERE id = ?1", [&target_id], |r| r.get(0))
        .unwrap_or_default();
    if target_role == "owner" {
        return (StatusCode::FORBIDDEN, Json(json!({"success": false, "message": "无法修改系统所有者角色"})));
    }

    let now = chrono::Utc::now().to_rfc3339();
    let affected = db.execute(
        "UPDATE users SET role = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![new_role, now, target_id],
    ).unwrap_or(0);

    if affected == 0 {
        return (StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "用户不存在"})));
    }

    let details = format!("{}→{}", target_role, new_role);
    insert_audit_log(&db, &owner.0, "role_change", Some(&target_id), None, Some(&details));

    (StatusCode::OK, Json(json!({"success": true})))
}

/// POST /api/admin/users/{id}/force-logout
pub async fn force_logout(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path(target_id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.lock();

    let deleted = db.execute("DELETE FROM sessions WHERE user_id = ?1", [&target_id])
        .unwrap_or(0);

    insert_audit_log(&db, &admin.0, "force_logout", Some(&target_id), None, Some(&format!("{} sessions", deleted)));

    (StatusCode::OK, Json(json!({"success": true, "deleted_sessions": deleted})))
}

/// POST /api/admin/users/{id}/suspend
pub async fn suspend_user(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path(target_id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.lock();

    // Prevent suspending owner
    let target_role: String = db
        .query_row("SELECT role FROM users WHERE id = ?1", [&target_id], |r| r.get(0))
        .unwrap_or_default();
    if target_role == "owner" {
        return (StatusCode::FORBIDDEN, Json(json!({"success": false, "message": "无法封禁系统所有者"})));
    }

    let now = chrono::Utc::now().to_rfc3339();
    let affected = db.execute(
        "UPDATE users SET status = 'suspended', updated_at = ?1 WHERE id = ?2 AND status = 'active'",
        rusqlite::params![now, target_id],
    ).unwrap_or(0);

    if affected == 0 {
        return (StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "用户不存在或非活跃状态"})));
    }

    // Invalidate sessions
    db.execute("DELETE FROM sessions WHERE user_id = ?1", [&target_id]).ok();

    insert_audit_log(&db, &admin.0, "suspend_user", Some(&target_id), None, None);

    (StatusCode::OK, Json(json!({"success": true, "message": "已封禁用户"})))
}

// ===== Conversation Monitor =====

/// GET /api/admin/conversations — paginated list
pub async fn list_conversations(
    State(state): State<AppState>,
    admin: AdminUserId,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let _ = admin;
    let db = state.db.lock();

    let user_id_filter = params.get("user_id").cloned().unwrap_or_default();
    let date_from = params.get("date_from").cloned().unwrap_or_else(|| "2000-01-01".into());
    let date_to = params.get("date_to").cloned().unwrap_or_else(|| "2099-12-31".into());
    let limit: i64 = params.get("limit").and_then(|v| v.parse().ok()).unwrap_or(20);
    let offset: i64 = params.get("offset").and_then(|v| v.parse().ok()).unwrap_or(0);

    let mut stmt = match db.prepare(
        "SELECT c.id, c.title, c.user_id, COALESCE(u.display_name, u.username) as user_name,
                (SELECT COUNT(*) FROM chat_messages m WHERE m.conversation_id = c.id) as msg_count,
                (SELECT COALESCE(SUM(cl.input_tokens + cl.output_tokens), 0) FROM chat_usage_log cl WHERE cl.conversation_id = c.id) as token_sum,
                c.created_at, c.updated_at
         FROM conversations c
         JOIN users u ON u.id = c.user_id
         WHERE (c.user_id = ?1 OR ?1 = '')
           AND c.updated_at >= ?2 AND c.updated_at <= ?3 || 'T23:59:59'
         ORDER BY c.updated_at DESC
         LIMIT ?4 OFFSET ?5"
    ) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("[admin] list_conversations error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "error": "内部错误"})));
        }
    };

    let rows: Vec<serde_json::Value> = stmt
        .query_map(rusqlite::params![user_id_filter, date_from, date_to, limit, offset], |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "title": r.get::<_, Option<String>>(1)?,
                "user_id": r.get::<_, String>(2)?,
                "user_name": r.get::<_, String>(3)?,
                "message_count": r.get::<_, i64>(4)?,
                "token_sum": r.get::<_, i64>(5)?,
                "created_at": r.get::<_, String>(6)?,
                "updated_at": r.get::<_, String>(7)?
            }))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    // Total count for pagination
    let total: i64 = db.query_row(
        "SELECT COUNT(*) FROM conversations c
         WHERE (c.user_id = ?1 OR ?1 = '')
           AND c.updated_at >= ?2 AND c.updated_at <= ?3 || 'T23:59:59'",
        rusqlite::params![user_id_filter, date_from, date_to],
        |r| r.get(0),
    ).unwrap_or(0);

    (StatusCode::OK, Json(json!({"success": true, "conversations": rows, "total": total})))
}

/// GET /api/admin/conversations/{id}/messages
pub async fn get_conversation_messages(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path(conv_id): Path<String>,
) -> impl IntoResponse {
    let _ = admin;
    let db = state.db.lock();

    let mut stmt = match db.prepare(
        "SELECT id, role, content_text, token_count, created_at
         FROM chat_messages WHERE conversation_id = ?1
         ORDER BY created_at ASC"
    ) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("[admin] get_messages error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "error": "内部错误"})));
        }
    };

    let messages: Vec<serde_json::Value> = stmt
        .query_map(rusqlite::params![conv_id], |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "role": r.get::<_, String>(1)?,
                "content": r.get::<_, Option<String>>(2)?,
                "token_count": r.get::<_, Option<i64>>(3)?,
                "created_at": r.get::<_, String>(4)?
            }))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    // Check if any security events reference this conversation
    let flagged_ids: Vec<String> = db
        .prepare("SELECT id FROM security_events WHERE conversation_id = ?1")
        .and_then(|mut s| {
            s.query_map([&conv_id], |r| r.get::<_, String>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    (StatusCode::OK, Json(json!({
        "success": true,
        "messages": messages,
        "has_security_events": !flagged_ids.is_empty(),
        "security_event_ids": flagged_ids
    })))
}

// ===== AI Dashboard =====

/// GET /api/admin/ai-usage — token consumption by model and period
pub async fn ai_usage(
    State(state): State<AppState>,
    admin: AdminUserId,
) -> impl IntoResponse {
    let _ = admin;
    let db = state.db.lock();

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let week_ago = (chrono::Utc::now() - chrono::Duration::days(7)).format("%Y-%m-%d").to_string();
    let month_ago = (chrono::Utc::now() - chrono::Duration::days(30)).format("%Y-%m-%d").to_string();

    // By model and period
    let query_model_period = |model: &str, since: &str| -> serde_json::Value {
        db.query_row(
            "SELECT COUNT(*), COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0)
             FROM chat_usage_log WHERE model LIKE '%' || ?1 || '%' AND created_at >= ?2",
            rusqlite::params![model, since],
            |r| Ok(json!({
                "messages": r.get::<_, i64>(0)?,
                "input_tokens": r.get::<_, i64>(1)?,
                "output_tokens": r.get::<_, i64>(2)?
            })),
        ).unwrap_or_else(|_| json!({"messages":0,"input_tokens":0,"output_tokens":0}))
    };

    let models = ["claude", "kimi", "doubao"];
    let periods = [("today", today.as_str()), ("week", week_ago.as_str()), ("month", month_ago.as_str())];

    let mut by_model = json!({});
    for model in &models {
        let mut model_data = json!({});
        for (period_name, since) in &periods {
            model_data[period_name] = query_model_period(model, since);
        }
        by_model[model] = model_data;
    }

    // Totals
    let mut totals = json!({});
    for (period_name, since) in &periods {
        totals[period_name] = query_ai_period(&db, since);
    }

    // Per-user ranking
    let mut per_user = Vec::new();
    if let Ok(mut stmt) = db.prepare(
        "SELECT u.id, u.username, u.display_name, COUNT(c.id),
                COALESCE(SUM(c.input_tokens),0), COALESCE(SUM(c.output_tokens),0),
                COALESCE(SUM(c.tool_calls),0),
                (SELECT c2.model FROM chat_usage_log c2 WHERE c2.user_id = u.id
                 GROUP BY c2.model ORDER BY COUNT(*) DESC LIMIT 1) as primary_model
         FROM users u LEFT JOIN chat_usage_log c ON c.user_id = u.id
         GROUP BY u.id
         HAVING COUNT(c.id) > 0
         ORDER BY (COALESCE(SUM(c.input_tokens),0)+COALESCE(SUM(c.output_tokens),0)) DESC"
    ) {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok(json!({
                "user_id": r.get::<_, String>(0)?,
                "username": r.get::<_, String>(1)?,
                "display_name": r.get::<_, Option<String>>(2)?,
                "messages": r.get::<_, i64>(3)?,
                "input_tokens": r.get::<_, i64>(4)?,
                "output_tokens": r.get::<_, i64>(5)?,
                "tool_calls": r.get::<_, i64>(6)?,
                "primary_model": r.get::<_, Option<String>>(7)?
            }))
        }) {
            for row in rows.flatten() {
                per_user.push(row);
            }
        }
    }

    (StatusCode::OK, Json(json!({
        "success": true,
        "by_model": by_model,
        "totals": totals,
        "per_user": per_user
    })))
}

/// GET /api/admin/ai-usage/providers — provider config status
pub async fn ai_providers(
    State(state): State<AppState>,
    admin: AdminUserId,
) -> impl IntoResponse {
    let _ = (state, admin);

    let providers = json!([
        {
            "name": "Claude",
            "id": "claude",
            "configured": std::env::var("ANTHROPIC_API_KEY").ok().filter(|k| !k.is_empty()).is_some()
        },
        {
            "name": "Kimi",
            "id": "kimi",
            "configured": std::env::var("KIMI_API_KEY").ok().filter(|k| !k.is_empty()).is_some()
        },
        {
            "name": "Doubao",
            "id": "doubao",
            "configured": std::env::var("ARK_API_KEY").ok().filter(|k| !k.is_empty()).is_some()
        }
    ]);

    (StatusCode::OK, Json(json!({"success": true, "providers": providers})))
}

// ===== Enhanced Security Events =====

/// GET /api/admin/security-events-v2 — with filters and pagination
pub async fn security_events_v2(
    State(state): State<AppState>,
    admin: AdminUserId,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let _ = admin;
    let db = state.db.lock();

    let severity = params.get("severity").cloned().unwrap_or_default();
    let event_type = params.get("event_type").cloned().unwrap_or_default();
    let user_id_filter = params.get("user_id").cloned().unwrap_or_default();
    let limit: i64 = params.get("limit").and_then(|v| v.parse().ok()).unwrap_or(50);
    let offset: i64 = params.get("offset").and_then(|v| v.parse().ok()).unwrap_or(0);

    let mut stmt = match db.prepare(
        "SELECT se.id, se.user_id, COALESCE(u.display_name, u.username) as user_name,
                se.event_type, se.severity, se.description, se.conversation_id,
                se.admin_notified, se.created_at
         FROM security_events se LEFT JOIN users u ON u.id = se.user_id
         WHERE (se.severity = ?1 OR ?1 = '')
           AND (se.event_type = ?2 OR ?2 = '')
           AND (se.user_id = ?3 OR ?3 = '')
         ORDER BY CASE se.severity WHEN 'high' THEN 0 WHEN 'medium' THEN 1 ELSE 2 END, se.created_at DESC
         LIMIT ?4 OFFSET ?5"
    ) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("[admin] security_events_v2 error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "error": "内部错误"})));
        }
    };

    let rows: Vec<serde_json::Value> = stmt
        .query_map(rusqlite::params![severity, event_type, user_id_filter, limit, offset], |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "user_id": r.get::<_, String>(1)?,
                "user_name": r.get::<_, String>(2)?,
                "event_type": r.get::<_, String>(3)?,
                "severity": r.get::<_, String>(4)?,
                "description": r.get::<_, String>(5)?,
                "conversation_id": r.get::<_, Option<String>>(6)?,
                "admin_notified": r.get::<_, i64>(7)?,
                "created_at": r.get::<_, String>(8)?
            }))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    // Risk user summary: users with 3+ events
    let risk_users: Vec<serde_json::Value> = db
        .prepare(
            "SELECT se.user_id, COALESCE(u.display_name, u.username), COUNT(*), MAX(se.created_at)
             FROM security_events se JOIN users u ON u.id = se.user_id
             GROUP BY se.user_id HAVING COUNT(*) >= 3
             ORDER BY COUNT(*) DESC"
        )
        .and_then(|mut s| {
            s.query_map([], |r| {
                Ok(json!({
                    "user_id": r.get::<_, String>(0)?,
                    "user_name": r.get::<_, String>(1)?,
                    "event_count": r.get::<_, i64>(2)?,
                    "last_event": r.get::<_, String>(3)?
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    // Suspended users
    let suspended: Vec<serde_json::Value> = db
        .prepare("SELECT id, username, display_name, updated_at FROM users WHERE status = 'suspended' ORDER BY updated_at DESC")
        .and_then(|mut s| {
            s.query_map([], |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?,
                    "username": r.get::<_, String>(1)?,
                    "display_name": r.get::<_, Option<String>>(2)?,
                    "suspended_at": r.get::<_, String>(3)?
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    (StatusCode::OK, Json(json!({
        "success": true,
        "events": rows,
        "risk_users": risk_users,
        "suspended_users": suspended
    })))
}

/// POST /api/admin/security-events/{id}/review — mark as reviewed
pub async fn review_security_event(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path(event_id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.lock();

    let affected = db.execute(
        "UPDATE security_events SET admin_notified = 1 WHERE id = ?1",
        [&event_id],
    ).unwrap_or(0);

    if affected == 0 {
        return (StatusCode::NOT_FOUND, Json(json!({"success": false, "message": "事件不存在"})));
    }

    insert_audit_log(&db, &admin.0, "review_security_event", None, Some(&event_id), None);

    (StatusCode::OK, Json(json!({"success": true})))
}

// ===== System Status =====

/// GET /api/admin/system-status
pub async fn system_status(
    State(state): State<AppState>,
    admin: AdminUserId,
) -> impl IntoResponse {
    let _ = admin;
    let db = state.db.lock();

    // Table row counts
    let tables = ["users", "todos", "conversations", "chat_messages", "expense_entries", "trips", "routines", "security_events"];
    let mut table_counts = json!({});
    for table in &tables {
        let count: i64 = db.query_row(
            &format!("SELECT COUNT(*) FROM {}", table),
            [],
            |r| r.get(0),
        ).unwrap_or(0);
        table_counts[*table] = json!(count);
    }

    // DB file size
    let db_path = state.db_path.clone();
    let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    // Upload storage size
    let upload_dir = format!("{}/uploads", std::env::var("DATA_DIR").unwrap_or_else(|_| "data".into()));
    let (upload_files, upload_size) = count_dir_size(&upload_dir);

    // Recent errors
    let errors_24h: i64 = db.query_row(
        "SELECT COUNT(*) FROM client_errors WHERE created_at >= datetime('now', '-1 day')",
        [], |r| r.get(0),
    ).unwrap_or(0);
    let errors_7d: i64 = db.query_row(
        "SELECT COUNT(*) FROM client_errors WHERE created_at >= datetime('now', '-7 days')",
        [], |r| r.get(0),
    ).unwrap_or(0);

    // Uptime
    let uptime_secs = state.start_time.elapsed().as_secs();

    (StatusCode::OK, Json(json!({
        "success": true,
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_secs": uptime_secs,
        "database": {
            "file_size": db_size,
            "tables": table_counts
        },
        "storage": {
            "upload_files": upload_files,
            "upload_size": upload_size
        },
        "errors": {
            "last_24h": errors_24h,
            "last_7d": errors_7d
        }
    })))
}

fn count_dir_size(path: &str) -> (u64, u64) {
    let mut files = 0u64;
    let mut size = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let ft = entry.file_type().unwrap_or_else(|_| entry.file_type().unwrap());
            if ft.is_file() {
                files += 1;
                size += entry.metadata().map(|m| m.len()).unwrap_or(0);
            } else if ft.is_dir() {
                let (f, s) = count_dir_size(&entry.path().to_string_lossy());
                files += f;
                size += s;
            }
        }
    }
    (files, size)
}

// ===== Audit Log =====

/// GET /api/admin/audit-log — paginated
pub async fn audit_log(
    State(state): State<AppState>,
    admin: AdminUserId,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let _ = admin;
    let db = state.db.lock();

    let admin_filter = params.get("admin_user").cloned().unwrap_or_default();
    let action_filter = params.get("action_type").cloned().unwrap_or_default();
    let limit: i64 = params.get("limit").and_then(|v| v.parse().ok()).unwrap_or(50);
    let offset: i64 = params.get("offset").and_then(|v| v.parse().ok()).unwrap_or(0);

    let mut stmt = match db.prepare(
        "SELECT a.id, a.admin_user_id, COALESCE(u.display_name, u.username) as admin_name,
                a.action_type, a.target_user_id,
                COALESCE(tu.display_name, tu.username) as target_name,
                a.target_resource, a.details, a.created_at
         FROM admin_audit_log a
         JOIN users u ON u.id = a.admin_user_id
         LEFT JOIN users tu ON tu.id = a.target_user_id
         WHERE (a.admin_user_id = ?1 OR ?1 = '')
           AND (a.action_type = ?2 OR ?2 = '')
         ORDER BY a.created_at DESC
         LIMIT ?3 OFFSET ?4"
    ) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("[admin] audit_log error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "error": "内部错误"})));
        }
    };

    let rows: Vec<serde_json::Value> = stmt
        .query_map(rusqlite::params![admin_filter, action_filter, limit, offset], |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "admin_user_id": r.get::<_, String>(1)?,
                "admin_name": r.get::<_, String>(2)?,
                "action_type": r.get::<_, String>(3)?,
                "target_user_id": r.get::<_, Option<String>>(4)?,
                "target_name": r.get::<_, Option<String>>(5)?,
                "target_resource": r.get::<_, Option<String>>(6)?,
                "details": r.get::<_, Option<String>>(7)?,
                "created_at": r.get::<_, String>(8)?
            }))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    (StatusCode::OK, Json(json!({"success": true, "entries": rows})))
}
