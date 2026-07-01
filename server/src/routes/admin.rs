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
    let sort = params
        .get("sort")
        .cloned()
        .unwrap_or_else(|| "created_at".into());
    let order = params
        .get("order")
        .cloned()
        .unwrap_or_else(|| "desc".into());

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
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "内部错误"})),
            );
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

    (
        StatusCode::OK,
        Json(json!({"success": true, "users": rows})),
    )
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
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "角色只能是 admin 或 user"})),
        );
    }

    if target_id == owner.0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "不能修改自己的角色"})),
        );
    }

    let db = state.db.lock();

    // Prevent changing owner's role
    let target_role: String = db
        .query_row("SELECT role FROM users WHERE id = ?1", [&target_id], |r| {
            r.get(0)
        })
        .unwrap_or_default();
    if target_role == "owner" {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"success": false, "message": "无法修改系统所有者角色"})),
        );
    }

    let now = chrono::Utc::now().to_rfc3339();
    let affected = db
        .execute(
            "UPDATE users SET role = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![new_role, now, target_id],
        )
        .unwrap_or(0);

    if affected == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "用户不存在"})),
        );
    }

    let details = format!("{}→{}", target_role, new_role);
    insert_audit_log(
        &db,
        &owner.0,
        "role_change",
        Some(&target_id),
        None,
        Some(&details),
    );

    (StatusCode::OK, Json(json!({"success": true})))
}

/// POST /api/admin/users/{id}/force-logout
pub async fn force_logout(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path(target_id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.lock();

    let deleted = db
        .execute("DELETE FROM sessions WHERE user_id = ?1", [&target_id])
        .unwrap_or(0);

    insert_audit_log(
        &db,
        &admin.0,
        "force_logout",
        Some(&target_id),
        None,
        Some(&format!("{} sessions", deleted)),
    );

    (
        StatusCode::OK,
        Json(json!({"success": true, "deleted_sessions": deleted})),
    )
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
        .query_row("SELECT role FROM users WHERE id = ?1", [&target_id], |r| {
            r.get(0)
        })
        .unwrap_or_default();
    if target_role == "owner" {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"success": false, "message": "无法封禁系统所有者"})),
        );
    }

    let now = chrono::Utc::now().to_rfc3339();
    let affected = db.execute(
        "UPDATE users SET status = 'suspended', updated_at = ?1 WHERE id = ?2 AND status = 'active'",
        rusqlite::params![now, target_id],
    ).unwrap_or(0);

    if affected == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "用户不存在或非活跃状态"})),
        );
    }

    // Invalidate sessions
    db.execute("DELETE FROM sessions WHERE user_id = ?1", [&target_id])
        .ok();

    insert_audit_log(&db, &admin.0, "suspend_user", Some(&target_id), None, None);

    (
        StatusCode::OK,
        Json(json!({"success": true, "message": "已封禁用户"})),
    )
}

// ===== Conversation Monitor =====

/// GET /api/admin/conversations/users — per-user conversation summary
pub async fn conversation_user_summary(
    State(state): State<AppState>,
    admin: AdminUserId,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let _ = admin;
    let db = state.db.lock();

    let date_from = params
        .get("date_from")
        .cloned()
        .unwrap_or_else(|| "2000-01-01".into());
    let date_to = params
        .get("date_to")
        .cloned()
        .unwrap_or_else(|| "2099-12-31".into());

    let mut stmt = match db.prepare(
        "SELECT c.user_id, COALESCE(u.display_name, u.username) as user_name,
                COUNT(c.id) as conv_count,
                (SELECT COUNT(*) FROM chat_messages m WHERE m.conversation_id IN (SELECT c2.id FROM conversations c2 WHERE c2.user_id = c.user_id AND c2.updated_at >= ?1 AND c2.updated_at <= ?2 || 'T23:59:59')) as msg_count,
                COALESCE((SELECT SUM(cl.input_tokens + cl.output_tokens) FROM chat_usage_log cl WHERE cl.conversation_id IN (SELECT c3.id FROM conversations c3 WHERE c3.user_id = c.user_id AND c3.updated_at >= ?1 AND c3.updated_at <= ?2 || 'T23:59:59')), 0) as token_sum,
                MAX(c.updated_at) as last_active
         FROM conversations c
         JOIN users u ON u.id = c.user_id
         WHERE c.updated_at >= ?1 AND c.updated_at <= ?2 || 'T23:59:59'
         GROUP BY c.user_id
         ORDER BY last_active DESC"
    ) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("[admin] conversation_user_summary error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "error": "内部错误"})));
        }
    };

    let users: Vec<serde_json::Value> = stmt
        .query_map(rusqlite::params![date_from, date_to], |r| {
            Ok(json!({
                "user_id": r.get::<_, String>(0)?,
                "user_name": r.get::<_, String>(1)?,
                "conv_count": r.get::<_, i64>(2)?,
                "msg_count": r.get::<_, i64>(3)?,
                "token_sum": r.get::<_, i64>(4)?,
                "last_active": r.get::<_, String>(5)?
            }))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    (
        StatusCode::OK,
        Json(json!({"success": true, "users": users})),
    )
}

/// GET /api/admin/conversations — paginated list
pub async fn list_conversations(
    State(state): State<AppState>,
    admin: AdminUserId,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let _ = admin;
    let db = state.db.lock();

    let user_id_filter = params.get("user_id").cloned().unwrap_or_default();
    let date_from = params
        .get("date_from")
        .cloned()
        .unwrap_or_else(|| "2000-01-01".into());
    let date_to = params
        .get("date_to")
        .cloned()
        .unwrap_or_else(|| "2099-12-31".into());
    let limit: i64 = params
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    let offset: i64 = params
        .get("offset")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

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
        .query_map(
            rusqlite::params![user_id_filter, date_from, date_to, limit, offset],
            |r| {
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
            },
        )
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    // Total count for pagination
    let total: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM conversations c
         WHERE (c.user_id = ?1 OR ?1 = '')
           AND c.updated_at >= ?2 AND c.updated_at <= ?3 || 'T23:59:59'",
            rusqlite::params![user_id_filter, date_from, date_to],
            |r| r.get(0),
        )
        .unwrap_or(0);

    (
        StatusCode::OK,
        Json(json!({"success": true, "conversations": rows, "total": total})),
    )
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
         ORDER BY created_at ASC",
    ) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("[admin] get_messages error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "内部错误"})),
            );
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

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "messages": messages,
            "has_security_events": !flagged_ids.is_empty(),
            "security_event_ids": flagged_ids
        })),
    )
}

// ===== AI Dashboard =====

/// GET /api/admin/ai-usage — token consumption by model and period
pub async fn ai_usage(State(state): State<AppState>, admin: AdminUserId) -> impl IntoResponse {
    let _ = admin;
    let db = state.db.lock();

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let week_ago = (chrono::Utc::now() - chrono::Duration::days(7))
        .format("%Y-%m-%d")
        .to_string();
    let month_ago = (chrono::Utc::now() - chrono::Duration::days(30))
        .format("%Y-%m-%d")
        .to_string();

    // By model and period
    let query_model_period = |model: &str, since: &str| -> serde_json::Value {
        db.query_row(
            "SELECT COUNT(*), COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0)
             FROM chat_usage_log WHERE model LIKE '%' || ?1 || '%' AND created_at >= ?2",
            rusqlite::params![model, since],
            |r| {
                Ok(json!({
                    "messages": r.get::<_, i64>(0)?,
                    "input_tokens": r.get::<_, i64>(1)?,
                    "output_tokens": r.get::<_, i64>(2)?
                }))
            },
        )
        .unwrap_or_else(|_| json!({"messages":0,"input_tokens":0,"output_tokens":0}))
    };

    let models = ["claude"];
    let periods = [
        ("today", today.as_str()),
        ("week", week_ago.as_str()),
        ("month", month_ago.as_str()),
    ];

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
         ORDER BY (COALESCE(SUM(c.input_tokens),0)+COALESCE(SUM(c.output_tokens),0)) DESC",
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

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "by_model": by_model,
            "totals": totals,
            "per_user": per_user
        })),
    )
}

/// GET /api/admin/ai-usage/providers — provider config status
pub async fn ai_providers(State(state): State<AppState>, admin: AdminUserId) -> impl IntoResponse {
    let _ = (state, admin);

    let providers = json!([
        {
            "name": "Claude",
            "id": "claude",
            "configured": std::env::var("ANTHROPIC_API_KEY").ok().filter(|k| !k.is_empty()).is_some()
        }
    ]);

    (
        StatusCode::OK,
        Json(json!({"success": true, "providers": providers})),
    )
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
    let limit: i64 = params
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50);
    let offset: i64 = params
        .get("offset")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

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
        .query_map(
            rusqlite::params![severity, event_type, user_id_filter, limit, offset],
            |r| {
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
            },
        )
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    // Risk user summary: users with 3+ events
    let risk_users: Vec<serde_json::Value> = db
        .prepare(
            "SELECT se.user_id, COALESCE(u.display_name, u.username), COUNT(*), MAX(se.created_at)
             FROM security_events se JOIN users u ON u.id = se.user_id
             GROUP BY se.user_id HAVING COUNT(*) >= 3
             ORDER BY COUNT(*) DESC",
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

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "events": rows,
            "risk_users": risk_users,
            "suspended_users": suspended
        })),
    )
}

/// POST /api/admin/security-events/{id}/review — mark as reviewed
pub async fn review_security_event(
    State(state): State<AppState>,
    admin: AdminUserId,
    Path(event_id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.lock();

    let affected = db
        .execute(
            "UPDATE security_events SET admin_notified = 1 WHERE id = ?1",
            [&event_id],
        )
        .unwrap_or(0);

    if affected == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "事件不存在"})),
        );
    }

    insert_audit_log(
        &db,
        &admin.0,
        "review_security_event",
        None,
        Some(&event_id),
        None,
    );

    (StatusCode::OK, Json(json!({"success": true})))
}

// ===== System Status =====

/// GET /api/admin/system-status
pub async fn system_status(State(state): State<AppState>, admin: AdminUserId) -> impl IntoResponse {
    let _ = admin;
    let db = state.db.lock();

    // Table row counts
    let tables = [
        "users",
        "todos",
        "conversations",
        "chat_messages",
        "expense_entries",
        "trips",
        "routines",
        "security_events",
    ];
    let mut table_counts = json!({});
    for table in &tables {
        let count: i64 = db
            .query_row(&format!("SELECT COUNT(*) FROM {}", table), [], |r| r.get(0))
            .unwrap_or(0);
        table_counts[*table] = json!(count);
    }

    // DB file size
    let db_path = state.db_path.clone();
    let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    // Upload storage size
    let upload_dir = format!(
        "{}/uploads",
        std::env::var("DATA_DIR").unwrap_or_else(|_| "data".into())
    );
    let (upload_files, upload_size) = count_dir_size(&upload_dir);

    // Recent errors
    let errors_24h: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM client_errors WHERE created_at >= datetime('now', '-1 day')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let errors_7d: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM client_errors WHERE created_at >= datetime('now', '-7 days')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // Uptime
    let uptime_secs = state.start_time.elapsed().as_secs();

    (
        StatusCode::OK,
        Json(json!({
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
        })),
    )
}

fn count_dir_size(path: &str) -> (u64, u64) {
    let mut files = 0u64;
    let mut size = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let ft = entry
                .file_type()
                .unwrap_or_else(|_| entry.file_type().unwrap());
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
    let limit: i64 = params
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50);
    let offset: i64 = params
        .get("offset")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

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
         LIMIT ?3 OFFSET ?4",
    ) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("[admin] audit_log error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "内部错误"})),
            );
        }
    };

    let rows: Vec<serde_json::Value> = stmt
        .query_map(
            rusqlite::params![admin_filter, action_filter, limit, offset],
            |r| {
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
            },
        )
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    (
        StatusCode::OK,
        Json(json!({"success": true, "entries": rows})),
    )
}

// ═══════════════════════════════════════════
// People (人物档案) management
// ═══════════════════════════════════════════

/// GET /api/admin/people?user_id=
pub async fn list_people(
    State(state): State<AppState>,
    _admin: AdminUserId,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let db = state.db.lock();
    let user_id = params.get("user_id").map(|s| s.as_str()).unwrap_or("");

    let mut stmt = match db.prepare(
        "SELECT p.id, p.user_id, p.name, p.relationship, p.nickname, p.attitude, p.notes, p.created_by, p.created_at, p.updated_at, u.username
         FROM ergou_people p
         LEFT JOIN users u ON u.id = p.user_id
         WHERE (?1 = '' OR p.user_id = ?1)
         ORDER BY p.created_at ASC",
    ) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("[admin] list_people error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "error": "内部错误"})));
        }
    };

    let rows: Vec<serde_json::Value> = stmt
        .query_map([user_id], |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "user_id": r.get::<_, String>(1)?,
                "name": r.get::<_, String>(2)?,
                "relationship": r.get::<_, String>(3)?,
                "nickname": r.get::<_, String>(4)?,
                "attitude": r.get::<_, String>(5)?,
                "notes": r.get::<_, String>(6)?,
                "created_by": r.get::<_, String>(7)?,
                "created_at": r.get::<_, String>(8)?,
                "updated_at": r.get::<_, String>(9)?,
                "username": r.get::<_, Option<String>>(10)?
            }))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    (
        StatusCode::OK,
        Json(json!({"success": true, "people": rows})),
    )
}

/// POST /api/admin/people
pub async fn create_person(
    State(state): State<AppState>,
    _admin: AdminUserId,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let db = state.db.lock();

    let user_id = match body["user_id"].as_str() {
        Some(u) if !u.is_empty() => u,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "error": "user_id is required"})),
            )
        }
    };
    let name = match body["name"].as_str() {
        Some(n) if !n.trim().is_empty() => n.trim(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "error": "name is required"})),
            )
        }
    };
    let relationship = match body["relationship"].as_str() {
        Some(r) if !r.trim().is_empty() => r.trim(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "error": "relationship is required"})),
            )
        }
    };

    // Check limit
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM ergou_people WHERE user_id=?1",
            [user_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if count >= 20 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "error": "每个用户最多20个人物"})),
        );
    }

    // Dedup
    let exists: bool = db
        .query_row(
            "SELECT COUNT(*) > 0 FROM ergou_people WHERE user_id=?1 AND name=?2",
            rusqlite::params![user_id, name],
            |r| r.get(0),
        )
        .unwrap_or(false);
    if exists {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "error": format!("已存在名为 {} 的人物", name)})),
        );
    }

    let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let nickname = body["nickname"].as_str().unwrap_or("").trim().to_string();
    let attitude = body["attitude"].as_str().unwrap_or("").trim().to_string();
    let notes = body["notes"].as_str().unwrap_or("").trim().to_string();

    match db.execute(
        "INSERT INTO ergou_people (id, user_id, name, relationship, nickname, attitude, notes, created_by, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'admin', ?8, ?9)",
        rusqlite::params![id, user_id, name, relationship, nickname, attitude, notes, now, now],
    ) {
        Ok(_) => (StatusCode::OK, Json(json!({"success": true, "id": id}))),
        Err(e) => {
            tracing::warn!("[admin] create_person error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "error": "创建失败"})))
        }
    }
}

/// PUT /api/admin/people/{id}
pub async fn update_person(
    State(state): State<AppState>,
    _admin: AdminUserId,
    Path(id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let db = state.db.lock();
    let now = chrono::Utc::now().to_rfc3339();

    // Build update dynamically
    let name = body["name"].as_str().map(|s| s.trim().to_string());
    let relationship = body["relationship"].as_str().map(|s| s.trim().to_string());
    let nickname = body["nickname"].as_str().map(|s| s.trim().to_string());
    let attitude = body["attitude"].as_str().map(|s| s.trim().to_string());
    let notes = body["notes"].as_str().map(|s| s.trim().to_string());

    // Name dedup check
    if let Some(ref new_name) = name {
        // Get user_id for this person
        let user_id: String =
            match db.query_row("SELECT user_id FROM ergou_people WHERE id=?1", [&id], |r| {
                r.get(0)
            }) {
                Ok(u) => u,
                Err(_) => {
                    return (
                        StatusCode::NOT_FOUND,
                        Json(json!({"success": false, "error": "人物不存在"})),
                    )
                }
            };
        let dup: bool = db
            .query_row(
                "SELECT COUNT(*) > 0 FROM ergou_people WHERE user_id=?1 AND name=?2 AND id!=?3",
                rusqlite::params![user_id, new_name, id],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if dup {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "error": format!("已存在名为 {} 的人物", new_name)})),
            );
        }
    }

    match db.execute(
        "UPDATE ergou_people SET name=COALESCE(?2, name), relationship=COALESCE(?3, relationship), nickname=COALESCE(?4, nickname), attitude=COALESCE(?5, attitude), notes=COALESCE(?6, notes), updated_at=?7 WHERE id=?1",
        rusqlite::params![id, name, relationship, nickname, attitude, notes, now],
    ) {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"success": false, "error": "人物不存在"}))),
        Ok(_) => (StatusCode::OK, Json(json!({"success": true}))),
        Err(e) => {
            tracing::warn!("[admin] update_person error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "error": "更新失败"})))
        }
    }
}

/// DELETE /api/admin/people/{id}
pub async fn delete_person(
    State(state): State<AppState>,
    _admin: AdminUserId,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.lock();

    match db.execute("DELETE FROM ergou_people WHERE id=?1", [&id]) {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "error": "人物不存在"})),
        ),
        Ok(_) => (StatusCode::OK, Json(json!({"success": true}))),
        Err(e) => {
            tracing::warn!("[admin] delete_person error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "删除失败"})),
            )
        }
    }
}

// ============ T-115: 一次性回补 todo.content → work_task.desc ============
//
// 背景:T-101 实施时 spec 字段映射漏了 `todo.content → work_task.desc`,导致 Boris 之前
//      用旧规则同步过的 work_task,desc 都是空的。T-114 已修未来同步规则;本端点回补历史。
//
// 流程:扫 admin 调用方自己的 work_tasks,按 title === todos.text 反查 todo;
//      条件满足时把 todo.content 写到 work_task.desc。
//
// 安全约束:
// - 只动调用方自己的 user_id(不跨用户)
// - 已有 desc 的不覆盖(保护手动写的简介)
// - 多匹配(同名 todo > 1)跳过 + warn(不知道哪个对应)
// - 幂等(第二次跑 already_has_desc 拦截)
// - 详细统计 + audit log

#[derive(Debug, serde::Deserialize, Default)]
pub struct MigrateTodoContentParams {
    /// dry_run=1 → 只统计不写入,用于预演;默认 false 实际写入。
    #[serde(default)]
    pub dry_run: Option<i32>,
}

pub async fn migrate_todo_content(
    State(state): State<AppState>,
    admin: AdminUserId,
    axum::extract::Query(p): axum::extract::Query<MigrateTodoContentParams>,
) -> impl IntoResponse {
    let dry_run = p.dry_run.unwrap_or(0) != 0;
    let db = state.db.lock();
    let user_id = admin.0.clone();

    // 1) 扫所有未删 work_tasks(自己 user_id 下)
    #[derive(Debug)]
    struct WorkTaskRow {
        id: i64,
        title: String,
        desc: String,
    }
    let mut rows_vec: Vec<WorkTaskRow> = Vec::new();
    {
        let mut stmt = match db
            .prepare("SELECT id, title, desc FROM work_tasks WHERE user_id = ?1 AND deleted = 0")
        {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("[migrate-todo-content] prepare wt: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "error": "内部错误"})),
                );
            }
        };
        let mapped = stmt.query_map(rusqlite::params![&user_id], |r| {
            Ok(WorkTaskRow {
                id: r.get(0)?,
                title: r.get(1)?,
                desc: r.get(2)?,
            })
        });
        if let Ok(iter) = mapped {
            for r in iter.flatten() {
                rows_vec.push(r);
            }
        }
    }

    let scanned = rows_vec.len();
    let mut backfilled: usize = 0;
    let mut skipped_already_has_desc: usize = 0;
    let mut skipped_multi_match: usize = 0;
    let mut skipped_no_todo: usize = 0;
    let mut skipped_empty_content: usize = 0;

    let now = chrono::Utc::now().to_rfc3339();

    for row in &rows_vec {
        // 已有 desc → 不覆盖
        if !row.desc.trim().is_empty() {
            skipped_already_has_desc += 1;
            continue;
        }
        // 反查同 user_id + text 匹配的 todos.content;COALESCE 兜 NULL
        let todos_content: Vec<String> = {
            let mut q = match db.prepare(
                "SELECT COALESCE(content, '') FROM todos \
                 WHERE user_id = ?1 AND text = ?2 AND deleted = 0",
            ) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("[migrate-todo-content] prepare todo: {}", e);
                    continue;
                }
            };
            let mapped = q.query_map(rusqlite::params![&user_id, &row.title], |r| {
                r.get::<_, String>(0)
            });
            match mapped {
                Ok(iter) => iter.flatten().collect(),
                Err(_) => Vec::new(),
            }
        };

        if todos_content.is_empty() {
            skipped_no_todo += 1;
            tracing::info!(
                "[migrate-todo-content] skip wt_id={} title={:?}: no matching todo",
                row.id,
                row.title
            );
            continue;
        }
        if todos_content.len() > 1 {
            skipped_multi_match += 1;
            tracing::warn!(
                "[migrate-todo-content] skip wt_id={} title={:?}: {} todos match (ambiguous)",
                row.id,
                row.title,
                todos_content.len()
            );
            continue;
        }
        let content = &todos_content[0];
        if content.trim().is_empty() {
            skipped_empty_content += 1;
            tracing::info!(
                "[migrate-todo-content] skip wt_id={} title={:?}: todo.content empty",
                row.id,
                row.title
            );
            continue;
        }
        // 回补!(dry_run 时只计数不写入)
        if dry_run {
            backfilled += 1;
            tracing::info!(
                "[migrate-todo-content] [DRY-RUN] would backfill wt_id={} title={:?} ({} chars)",
                row.id,
                row.title,
                content.chars().count()
            );
            continue;
        }
        match db.execute(
            "UPDATE work_tasks SET desc = ?1, updated_at = ?2 \
             WHERE id = ?3 AND user_id = ?4 AND deleted = 0",
            rusqlite::params![content, &now, row.id, &user_id],
        ) {
            Ok(_) => {
                backfilled += 1;
                tracing::info!(
                    "[migrate-todo-content] backfilled wt_id={} title={:?} ({} chars)",
                    row.id,
                    row.title,
                    content.chars().count()
                );
            }
            Err(e) => {
                tracing::warn!(
                    "[migrate-todo-content] update failed wt_id={}: {}",
                    row.id,
                    e
                );
            }
        }
    }

    // 写 audit log 留痕(dry_run 也记,便于追溯演练)
    let summary =
        format!(
        "{}scanned={} backfilled={} already_has_desc={} multi_match={} no_todo={} empty_content={}",
        if dry_run { "[DRY-RUN] " } else { "" },
        scanned, backfilled, skipped_already_has_desc,
        skipped_multi_match, skipped_no_todo, skipped_empty_content
    );
    insert_audit_log(
        &db,
        &user_id,
        if dry_run {
            "migrate_todo_content_dry_run"
        } else {
            "migrate_todo_content"
        },
        None,
        Some("work_tasks"),
        Some(&summary),
    );

    tracing::info!("[migrate-todo-content] done: {}", summary);

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "dry_run": dry_run,
            "stats": {
                "scanned": scanned,
                "backfilled": backfilled,
                "skipped_already_has_desc": skipped_already_has_desc,
                "skipped_multi_match": skipped_multi_match,
                "skipped_no_todo": skipped_no_todo,
                "skipped_empty_content": skipped_empty_content,
            }
        })),
    )
}

// ============ T-122: 洞察 v0.3 数据迁移 ============
//
// 把 v0.2 的 insights(双层模型)迁移到 v0.3 的 insight_tasks(单层)。
// SPEC: specs/insight/spec.md v0.3 § 十一。
//
// 每条 insight:
//   - 新建 insight_task: input_type='topic', input_content=insight.topic,
//     template=insight.template, status(有 report → done,否则 ready)
//   - 关联 sources 的 content 拼到 source_snapshot(带 title,`\n\n---\n\n` 分隔)
//   - 每条 report 迁到 insight_reports(version 保留,content_md 保留)
//   - current_report_id 指向最新(max version)report 的新 id
//   - annotations / share_links 不迁移
//
// 安全约束:
//   - 只迁调用方自己的 user_id(不跨用户)
//   - 幂等:已迁过(同 user + input_content + input_type='topic' 已存在)→ skip
//   - 老表不删(保留只读 30 天)

pub async fn migrate_insight_v0_3(
    State(state): State<AppState>,
    admin: AdminUserId,
) -> impl IntoResponse {
    let db = state.db.lock();
    let user_id = admin.0.clone();
    let now = chrono::Utc::now().to_rfc3339();

    // 1) 扫调用方的 insights(老表)
    struct OldInsight {
        id: i64,
        topic: String,
        template: String,
    }
    let mut insights: Vec<OldInsight> = Vec::new();
    {
        let mut stmt = match db
            .prepare("SELECT id, topic, template FROM insights WHERE user_id = ?1 AND deleted = 0")
        {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("[migrate-insight-v0.3] prepare insights: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "error": "内部错误"})),
                );
            }
        };
        let mapped = stmt.query_map(rusqlite::params![&user_id], |r| {
            Ok(OldInsight {
                id: r.get(0)?,
                topic: r.get(1)?,
                template: r.get(2)?,
            })
        });
        if let Ok(iter) = mapped {
            for r in iter.flatten() {
                insights.push(r);
            }
        }
    }

    let scanned = insights.len();
    let mut migrated: usize = 0;
    let mut skipped: usize = 0;
    let mut errors: Vec<String> = Vec::new();

    for ins in &insights {
        // 幂等:已迁过则跳过
        let already: bool = db
            .query_row(
                "SELECT COUNT(*) > 0 FROM insight_tasks \
                 WHERE user_id = ?1 AND input_type = 'topic' AND input_content = ?2 AND deleted = 0",
                rusqlite::params![&user_id, &ins.topic],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if already {
            skipped += 1;
            continue;
        }

        // 拼接 sources → source_snapshot
        let mut snapshots: Vec<String> = Vec::new();
        if let Ok(mut s) = db.prepare(
            "SELECT COALESCE(title, ''), COALESCE(content, '') FROM sources \
             WHERE insight_id = ?1 AND user_id = ?2 AND deleted = 0 ORDER BY id ASC",
        ) {
            if let Ok(iter) = s.query_map(rusqlite::params![ins.id, &user_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            }) {
                for (title, content) in iter.flatten() {
                    if content.trim().is_empty() {
                        continue;
                    }
                    if title.trim().is_empty() {
                        snapshots.push(content);
                    } else {
                        snapshots.push(format!("## {title}\n\n{content}"));
                    }
                }
            }
        }
        let source_snapshot = snapshots.join("\n\n---\n\n");

        // 读 insight 的 reports
        struct OldReport {
            version: i64,
            content_md: String,
            generated_by: String,
            model_used: String,
            revision_note: String,
            created_at: String,
        }
        let mut reports: Vec<OldReport> = Vec::new();
        if let Ok(mut s) = db.prepare(
            "SELECT version, content_md, COALESCE(generated_by,''), COALESCE(model_used,''), \
                    COALESCE(revision_note,''), created_at \
             FROM reports WHERE insight_id = ?1 ORDER BY version ASC",
        ) {
            if let Ok(iter) = s.query_map(rusqlite::params![ins.id], |r| {
                Ok(OldReport {
                    version: r.get(0)?,
                    content_md: r.get(1)?,
                    generated_by: r.get(2)?,
                    model_used: r.get(3)?,
                    revision_note: r.get(4)?,
                    created_at: r.get(5)?,
                })
            }) {
                for r in iter.flatten() {
                    reports.push(r);
                }
            }
        }

        // 状态映射(spec § 十一):有 report → done(已有可看的报告),否则 ready(待处理)
        let new_status = if reports.is_empty() { "ready" } else { "done" };
        let title = crate::models::insight_task::derive_title(&ins.topic);

        // 建 insight_task
        if let Err(e) = db.execute(
            "INSERT INTO insight_tasks \
                (user_id, title, input_type, input_content, template, status, feedback_note, source_snapshot, created_at, updated_at) \
             VALUES (?1, ?2, 'topic', ?3, ?4, ?5, '', ?6, ?7, ?7)",
            rusqlite::params![&user_id, &title, &ins.topic, &ins.template, new_status, &source_snapshot, &now],
        ) {
            errors.push(format!("insight #{} task insert: {e}", ins.id));
            continue;
        }
        let new_task_id = db.last_insert_rowid();

        // 迁 reports,记录最新版的新 id
        let mut latest_new_report_id: Option<i64> = None;
        for rep in &reports {
            if let Err(e) = db.execute(
                "INSERT INTO insight_reports \
                    (task_id, user_id, version, template, content_md, parent_report_id, revision_note, generated_by, model_used, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    new_task_id,
                    &user_id,
                    rep.version,
                    &ins.template,
                    &rep.content_md,
                    &rep.revision_note,
                    &rep.generated_by,
                    &rep.model_used,
                    &rep.created_at,
                ],
            ) {
                errors.push(format!("insight #{} report v{} insert: {e}", ins.id, rep.version));
                continue;
            }
            latest_new_report_id = Some(db.last_insert_rowid());
        }

        // current_report_id 指向最新版
        if let Some(rid) = latest_new_report_id {
            let _ = db.execute(
                "UPDATE insight_tasks SET current_report_id = ?1 WHERE id = ?2 AND user_id = ?3",
                rusqlite::params![rid, new_task_id, &user_id],
            );
        }

        migrated += 1;
        tracing::info!(
            "[migrate-insight-v0.3] insight #{} → task #{} ({} reports)",
            ins.id,
            new_task_id,
            reports.len()
        );
    }

    let summary = format!(
        "scanned={scanned} migrated={migrated} skipped={skipped} errors={}",
        errors.len()
    );
    insert_audit_log(
        &db,
        &user_id,
        "migrate_insight_v0.3",
        None,
        Some("insight_tasks"),
        Some(&summary),
    );
    tracing::info!("[migrate-insight-v0.3] done: {}", summary);

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "scanned": scanned,
            "migrated": migrated,
            "skipped": skipped,
            "errors": errors,
        })),
    )
}

#[cfg(test)]
mod migrate_tests {
    use crate::test_helpers::{auth_cookie, create_test_user, test_state};
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use serde_json::Value as JsonValue;
    use tower::ServiceExt;

    async fn body_json(resp: axum::response::Response) -> JsonValue {
        let body = to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    async fn admin_setup() -> (crate::state::AppState, String, String) {
        // 第一个注册用户被 migration 自动设为 owner (db.rs run_migrations) → AdminUserId 通过
        let state = test_state();
        let (uid, token) = create_test_user(&state, "boris_test", "Pa55word1");
        // 显式确保 role=owner
        {
            let db = state.db.lock();
            db.execute(
                "UPDATE users SET role = 'owner' WHERE id = ?1",
                rusqlite::params![&uid],
            )
            .unwrap();
        }
        let cookie = auth_cookie(&token);
        (state, uid, cookie)
    }

    async fn create_todo(state: &crate::state::AppState, user_id: &str, text: &str, content: &str) {
        let db = state.db.lock();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        db.execute(
            "INSERT INTO todos (id, user_id, text, content, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            rusqlite::params![id, user_id, text, content, now],
        )
        .unwrap();
    }

    async fn create_work_task(
        state: &crate::state::AppState,
        user_id: &str,
        title: &str,
        desc: &str,
    ) -> i64 {
        let db = state.db.lock();
        let now = chrono::Utc::now().to_rfc3339();
        db.execute(
            "INSERT INTO work_tasks (user_id, title, desc, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?4)",
            rusqlite::params![user_id, title, desc, now],
        )
        .unwrap();
        db.last_insert_rowid()
    }

    async fn run_migrate(app: &axum::Router, cookie: &str) -> JsonValue {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/admin/migrate-todo-content")
                    .header("Cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        body_json(resp).await
    }

    #[tokio::test]
    async fn backfills_single_match_with_content() {
        let (state, uid, cookie) = admin_setup().await;
        create_todo(&state, &uid, "院评委会准备", "PPT 大纲 + 三处数据").await;
        let wt_id = create_work_task(&state, &uid, "院评委会准备", "").await;
        let app = crate::build_app(state.clone());

        let j = run_migrate(&app, &cookie).await;
        assert_eq!(j["success"], true);
        assert_eq!(j["stats"]["scanned"], 1);
        assert_eq!(j["stats"]["backfilled"], 1);

        // 验证 desc 被写入
        let db = state.db.lock();
        let desc: String = db
            .query_row(
                "SELECT desc FROM work_tasks WHERE id = ?1",
                rusqlite::params![wt_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(desc, "PPT 大纲 + 三处数据");
    }

    #[tokio::test]
    async fn skips_when_work_task_already_has_desc() {
        let (state, uid, cookie) = admin_setup().await;
        create_todo(&state, &uid, "X", "todo content").await;
        let wt_id = create_work_task(&state, &uid, "X", "已有简介").await;
        let app = crate::build_app(state.clone());

        let j = run_migrate(&app, &cookie).await;
        assert_eq!(j["stats"]["skipped_already_has_desc"], 1);
        assert_eq!(j["stats"]["backfilled"], 0);

        let db = state.db.lock();
        let desc: String = db
            .query_row(
                "SELECT desc FROM work_tasks WHERE id = ?1",
                rusqlite::params![wt_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(desc, "已有简介"); // 未被覆盖
    }

    #[tokio::test]
    async fn skips_multi_match_with_warning() {
        let (state, uid, cookie) = admin_setup().await;
        create_todo(&state, &uid, "同名", "content A").await;
        create_todo(&state, &uid, "同名", "content B").await;
        create_work_task(&state, &uid, "同名", "").await;
        let app = crate::build_app(state);

        let j = run_migrate(&app, &cookie).await;
        assert_eq!(j["stats"]["skipped_multi_match"], 1);
        assert_eq!(j["stats"]["backfilled"], 0);
    }

    #[tokio::test]
    async fn skips_when_no_matching_todo() {
        let (state, uid, cookie) = admin_setup().await;
        create_work_task(&state, &uid, "孤儿任务", "").await;
        let app = crate::build_app(state);

        let j = run_migrate(&app, &cookie).await;
        assert_eq!(j["stats"]["skipped_no_todo"], 1);
        assert_eq!(j["stats"]["backfilled"], 0);
    }

    #[tokio::test]
    async fn skips_when_todo_content_empty() {
        let (state, uid, cookie) = admin_setup().await;
        create_todo(&state, &uid, "空内容 todo", "").await;
        create_work_task(&state, &uid, "空内容 todo", "").await;
        let app = crate::build_app(state);

        let j = run_migrate(&app, &cookie).await;
        assert_eq!(j["stats"]["skipped_empty_content"], 1);
        assert_eq!(j["stats"]["backfilled"], 0);
    }

    #[tokio::test]
    async fn idempotent_second_run_does_nothing() {
        let (state, uid, cookie) = admin_setup().await;
        create_todo(&state, &uid, "幂等测试", "回补一次").await;
        create_work_task(&state, &uid, "幂等测试", "").await;
        let app = crate::build_app(state);

        let j1 = run_migrate(&app, &cookie).await;
        assert_eq!(j1["stats"]["backfilled"], 1);

        // 第二次:work_task.desc 已有内容 → 走 already_has_desc 跳过
        let j2 = run_migrate(&app, &cookie).await;
        assert_eq!(j2["stats"]["backfilled"], 0);
        assert_eq!(j2["stats"]["skipped_already_has_desc"], 1);
    }

    #[tokio::test]
    async fn user_isolation_does_not_touch_other_users() {
        let (state, uid_admin, cookie) = admin_setup().await;
        // 另一个用户的 todo + work_task,不应被 admin migrate 触碰
        let (uid_other, _) = create_test_user(&state, "other_user", "Pa55word1");
        create_todo(&state, &uid_other, "他人任务", "他人内容").await;
        let wt_other = create_work_task(&state, &uid_other, "他人任务", "").await;
        // admin 自己也有一条匹配
        create_todo(&state, &uid_admin, "我的任务", "我的内容").await;
        create_work_task(&state, &uid_admin, "我的任务", "").await;
        let app = crate::build_app(state.clone());

        let j = run_migrate(&app, &cookie).await;
        assert_eq!(j["stats"]["scanned"], 1, "只扫 admin 自己的 work_tasks");
        assert_eq!(j["stats"]["backfilled"], 1);

        // 他人的 work_task.desc 没动
        let db = state.db.lock();
        let desc: String = db
            .query_row(
                "SELECT desc FROM work_tasks WHERE id = ?1",
                rusqlite::params![wt_other],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(desc, "", "他人 work_task 不应被触碰");
    }

    // ============ T-122: 洞察 v0.3 迁移测试 ============

    fn seed_old_insight(
        state: &crate::state::AppState,
        user_id: &str,
        topic: &str,
        template: &str,
        status: &str,
    ) -> i64 {
        let db = state.db.lock();
        let now = chrono::Utc::now().to_rfc3339();
        db.execute(
            "INSERT INTO insights (user_id, title, topic, template, status, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            rusqlite::params![user_id, topic, topic, template, status, now],
        )
        .unwrap();
        db.last_insert_rowid()
    }

    fn seed_old_source(
        state: &crate::state::AppState,
        user_id: &str,
        insight_id: i64,
        title: &str,
        content: &str,
    ) {
        let db = state.db.lock();
        let now = chrono::Utc::now().to_rfc3339();
        db.execute(
            "INSERT INTO sources (user_id, insight_id, kind, title, content, created_at, updated_at) \
             VALUES (?1, ?2, 'text', ?3, ?4, ?5, ?5)",
            rusqlite::params![user_id, insight_id, title, content, now],
        )
        .unwrap();
    }

    fn seed_old_report(
        state: &crate::state::AppState,
        insight_id: i64,
        version: i64,
        content_md: &str,
    ) {
        let db = state.db.lock();
        let now = chrono::Utc::now().to_rfc3339();
        db.execute(
            "INSERT INTO reports (insight_id, version, template, content_md, generated_by, model_used, created_at, updated_at) \
             VALUES (?1, ?2, 'survey', ?3, 'claude-code', 'claude-opus', ?4, ?4)",
            rusqlite::params![insight_id, version, content_md, now],
        )
        .unwrap();
    }

    async fn run_migrate_insight(app: &axum::Router, cookie: &str) -> JsonValue {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/admin/migrate-insight-v0.3")
                    .header("Cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        body_json(resp).await
    }

    #[tokio::test]
    async fn migrates_insight_with_sources_and_reports() {
        let (state, uid, cookie) = admin_setup().await;
        let ins_id = seed_old_insight(&state, &uid, "Gemini 对比", "survey", "published");
        seed_old_source(&state, &uid, ins_id, "源A", "内容甲");
        seed_old_source(&state, &uid, ins_id, "源B", "内容乙");
        seed_old_report(&state, ins_id, 1, "报告 v1");
        seed_old_report(&state, ins_id, 2, "报告 v2");
        let app = crate::build_app(state.clone());

        let j = run_migrate_insight(&app, &cookie).await;
        assert_eq!(j["success"], true);
        assert_eq!(j["scanned"], 1);
        assert_eq!(j["migrated"], 1);

        let db = state.db.lock();
        // 新 task 建好
        let (task_id, input_type, status, snapshot, cur_report): (
            i64,
            String,
            String,
            String,
            Option<i64>,
        ) = db
            .query_row(
                "SELECT id, input_type, status, source_snapshot, current_report_id \
                 FROM insight_tasks WHERE user_id = ?1 AND input_content = 'Gemini 对比'",
                rusqlite::params![&uid],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(input_type, "topic");
        assert_eq!(status, "done");
        assert!(snapshot.contains("内容甲") && snapshot.contains("内容乙"));
        assert!(snapshot.contains("\n\n---\n\n"), "多源用分隔符");

        // 两条 report 迁过来,current 指向 v2
        let rep_count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM insight_reports WHERE task_id = ?1",
                rusqlite::params![task_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rep_count, 2);
        let cur_version: i64 = db
            .query_row(
                "SELECT version FROM insight_reports WHERE id = ?1",
                rusqlite::params![cur_report.unwrap()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cur_version, 2, "current_report_id 指向最新版");
    }

    #[tokio::test]
    async fn migrate_is_idempotent() {
        let (state, uid, cookie) = admin_setup().await;
        seed_old_insight(&state, &uid, "幂等主题", "survey", "collecting");
        let app = crate::build_app(state.clone());

        let j1 = run_migrate_insight(&app, &cookie).await;
        assert_eq!(j1["migrated"], 1);

        // 第二次:已迁过 → skipped,不重复建
        let j2 = run_migrate_insight(&app, &cookie).await;
        assert_eq!(j2["migrated"], 0);
        assert_eq!(j2["skipped"], 1);

        let db = state.db.lock();
        let n: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM insight_tasks WHERE user_id = ?1 AND input_content = '幂等主题'",
                rusqlite::params![&uid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "不重复建 task");
    }

    #[tokio::test]
    async fn migrate_no_report_maps_to_ready() {
        let (state, uid, cookie) = admin_setup().await;
        seed_old_insight(&state, &uid, "空报告主题", "decision", "collecting");
        let app = crate::build_app(state.clone());

        let j = run_migrate_insight(&app, &cookie).await;
        assert_eq!(j["migrated"], 1);

        let db = state.db.lock();
        let status: String = db
            .query_row(
                "SELECT status FROM insight_tasks WHERE user_id = ?1 AND input_content = '空报告主题'",
                rusqlite::params![&uid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "ready", "无 report → ready");
    }
}

// ===== Behavior Analytics (T-218 / SPEC analytics) =====
//
// Admin-only aggregation over `behavior_events`. Every endpoint supports an
// optional `user_id` filter (= owner / a user / everyone) and a `from`/`to`
// RFC3339 time range (default: last 7 days). `user_id` is the analysis key here,
// NOT a trust boundary — these handlers are guarded by `AdminUserId`.

type AnalyticsParams = std::collections::HashMap<String, String>;

/// Parse `(from, to, user_id)`; defaults to the last 7 days, empty user_id = all.
fn analytics_range(params: &AnalyticsParams) -> (String, String, String) {
    let to = params
        .get("to")
        .cloned()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    let from = params
        .get("from")
        .cloned()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339());
    let user_id = params.get("user_id").cloned().unwrap_or_default();
    (from, to, user_id)
}

fn analytics_db_error(ctx: &str, e: rusqlite::Error) -> (StatusCode, Json<serde_json::Value>) {
    tracing::warn!("[admin] analytics {} error: {}", ctx, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "error": "内部错误" })),
    )
}

/// GET /api/admin/analytics/overview — totals + hour/day distribution.
pub async fn analytics_overview(
    State(state): State<AppState>,
    admin: AdminUserId,
    axum::extract::Query(params): axum::extract::Query<AnalyticsParams>,
) -> impl IntoResponse {
    let _ = admin;
    let (from, to, user_id) = analytics_range(&params);
    let db = state.db.lock();

    let (total_events, active_users, sessions): (i64, i64, i64) = match db.query_row(
        "SELECT COUNT(*), COUNT(DISTINCT user_id), COUNT(DISTINCT session_id)
         FROM behavior_events
         WHERE created_at >= ?1 AND created_at <= ?2 AND (?3 = '' OR user_id = ?3)",
        rusqlite::params![from, to, user_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    ) {
        Ok(t) => t,
        Err(e) => return analytics_db_error("overview counts", e),
    };

    // Hour-of-day distribution from client_ts (user local clock), 24 buckets.
    let mut by_hour = vec![0i64; 24];
    if let Ok(mut stmt) = db.prepare(
        "SELECT CAST(substr(client_ts, 12, 2) AS INTEGER) AS hr, COUNT(*)
         FROM behavior_events
         WHERE created_at >= ?1 AND created_at <= ?2 AND (?3 = '' OR user_id = ?3)
         GROUP BY hr",
    ) {
        let rows = stmt
            .query_map(rusqlite::params![from, to, user_id], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
            })
            .map(|rows| rows.filter_map(|x| x.ok()).collect::<Vec<_>>())
            .unwrap_or_default();
        for (hr, c) in rows {
            if (0..24).contains(&hr) {
                by_hour[hr as usize] = c;
            }
        }
    }

    // Day distribution from client_ts date.
    let by_day: Vec<serde_json::Value> = db
        .prepare(
            "SELECT substr(client_ts, 1, 10) AS day, COUNT(*)
             FROM behavior_events
             WHERE created_at >= ?1 AND created_at <= ?2 AND (?3 = '' OR user_id = ?3)
             GROUP BY day ORDER BY day ASC",
        )
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![from, to, user_id], |r| {
                Ok(json!({ "day": r.get::<_, String>(0)?, "count": r.get::<_, i64>(1)? }))
            })
            .map(|rows| rows.filter_map(|x| x.ok()).collect())
        })
        .unwrap_or_default();

    let events_per_user = if active_users > 0 {
        (total_events as f64 / active_users as f64 * 100.0).round() / 100.0
    } else {
        0.0
    };

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "total_events": total_events,
            "active_users": active_users,
            "sessions": sessions,
            "events_per_user": events_per_user,
            "by_hour": by_hour,
            "by_day": by_day,
        })),
    )
}

/// GET /api/admin/analytics/top-targets — most-clicked elements.
pub async fn analytics_top_targets(
    State(state): State<AppState>,
    admin: AdminUserId,
    axum::extract::Query(params): axum::extract::Query<AnalyticsParams>,
) -> impl IntoResponse {
    let _ = admin;
    let (from, to, user_id) = analytics_range(&params);
    let limit: i64 = params
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(20)
        .clamp(1, 200);
    let db = state.db.lock();

    let items: Vec<serde_json::Value> = match db.prepare(&format!(
        "SELECT target_id, MAX(target_label) AS label, COUNT(*) AS clicks
         FROM behavior_events
         WHERE event_type = 'click' AND target_id IS NOT NULL AND target_id != ''
           AND created_at >= ?1 AND created_at <= ?2 AND (?3 = '' OR user_id = ?3)
         GROUP BY target_id ORDER BY clicks DESC LIMIT {limit}"
    )) {
        Ok(mut stmt) => stmt
            .query_map(rusqlite::params![from, to, user_id], |r| {
                Ok(json!({
                    "target_id": r.get::<_, String>(0)?,
                    "target_label": r.get::<_, Option<String>>(1)?,
                    "clicks": r.get::<_, i64>(2)?,
                }))
            })
            .map(|rows| rows.filter_map(|x| x.ok()).collect())
            .unwrap_or_default(),
        Err(e) => return analytics_db_error("top_targets", e),
    };

    (
        StatusCode::OK,
        Json(json!({ "success": true, "items": items })),
    )
}

/// GET /api/admin/analytics/feature-usage — pageviews + dwell per route.
pub async fn analytics_feature_usage(
    State(state): State<AppState>,
    admin: AdminUserId,
    axum::extract::Query(params): axum::extract::Query<AnalyticsParams>,
) -> impl IntoResponse {
    let _ = admin;
    let (from, to, user_id) = analytics_range(&params);
    let db = state.db.lock();

    let items: Vec<serde_json::Value> = match db.prepare(
        "SELECT route,
                SUM(CASE WHEN event_type = 'pageview' THEN 1 ELSE 0 END) AS pageviews,
                COALESCE(SUM(CASE WHEN event_type = 'dwell' THEN dwell_ms ELSE 0 END), 0) AS total_dwell,
                SUM(CASE WHEN event_type = 'dwell' AND dwell_ms IS NOT NULL THEN 1 ELSE 0 END) AS dwell_samples
         FROM behavior_events
         WHERE route IS NOT NULL AND route != ''
           AND created_at >= ?1 AND created_at <= ?2 AND (?3 = '' OR user_id = ?3)
         GROUP BY route ORDER BY pageviews DESC, total_dwell DESC",
    ) {
        Ok(mut stmt) => stmt
            .query_map(rusqlite::params![from, to, user_id], |r| {
                let total_dwell: i64 = r.get(2)?;
                let samples: i64 = r.get(3)?;
                let avg = if samples > 0 { total_dwell / samples } else { 0 };
                Ok(json!({
                    "route": r.get::<_, String>(0)?,
                    "pageviews": r.get::<_, i64>(1)?,
                    "total_dwell_ms": total_dwell,
                    "avg_dwell_ms": avg,
                }))
            })
            .map(|rows| rows.filter_map(|x| x.ok()).collect())
            .unwrap_or_default(),
        Err(e) => return analytics_db_error("feature_usage", e),
    };

    (
        StatusCode::OK,
        Json(json!({ "success": true, "items": items })),
    )
}

/// GET /api/admin/analytics/dwell — avg/median dwell per route (median in Rust).
pub async fn analytics_dwell(
    State(state): State<AppState>,
    admin: AdminUserId,
    axum::extract::Query(params): axum::extract::Query<AnalyticsParams>,
) -> impl IntoResponse {
    let _ = admin;
    let (from, to, user_id) = analytics_range(&params);
    let db = state.db.lock();

    let rows: Vec<(String, i64)> = match db.prepare(
        "SELECT route, dwell_ms
         FROM behavior_events
         WHERE event_type = 'dwell' AND dwell_ms IS NOT NULL AND route IS NOT NULL AND route != ''
           AND created_at >= ?1 AND created_at <= ?2 AND (?3 = '' OR user_id = ?3)",
    ) {
        Ok(mut stmt) => stmt
            .query_map(rusqlite::params![from, to, user_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })
            .map(|rows| rows.filter_map(|x| x.ok()).collect())
            .unwrap_or_default(),
        Err(e) => return analytics_db_error("dwell", e),
    };

    let mut by_route: std::collections::HashMap<String, Vec<i64>> =
        std::collections::HashMap::new();
    for (route, ms) in rows {
        by_route.entry(route).or_default().push(ms);
    }
    let mut items: Vec<serde_json::Value> = by_route
        .into_iter()
        .map(|(route, mut samples)| {
            samples.sort_unstable();
            let n = samples.len();
            let sum: i64 = samples.iter().sum();
            let avg = if n > 0 { sum / n as i64 } else { 0 };
            let median = if n == 0 {
                0
            } else if n % 2 == 1 {
                samples[n / 2]
            } else {
                (samples[n / 2 - 1] + samples[n / 2]) / 2
            };
            json!({
                "route": route,
                "avg_dwell_ms": avg,
                "median_dwell_ms": median,
                "samples": n as i64,
            })
        })
        .collect();
    items.sort_by(|a, b| b["samples"].as_i64().cmp(&a["samples"].as_i64()));

    (
        StatusCode::OK,
        Json(json!({ "success": true, "items": items })),
    )
}

/// GET /api/admin/analytics/trail — event timeline for a session (or a user's recent events).
pub async fn analytics_trail(
    State(state): State<AppState>,
    admin: AdminUserId,
    axum::extract::Query(params): axum::extract::Query<AnalyticsParams>,
) -> impl IntoResponse {
    let _ = admin;
    let db = state.db.lock();
    let session_id = params.get("session_id").cloned().unwrap_or_default();
    let user_id = params.get("user_id").cloned().unwrap_or_default();
    if session_id.is_empty() && user_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "需要 session_id 或 user_id" })),
        );
    }
    let limit: i64 = params
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(200)
        .clamp(1, 1000);

    let sql = format!(
        "SELECT event_type, target_id, target_label, route, dwell_ms, client_ts, session_id
         FROM behavior_events
         WHERE (?1 = '' OR session_id = ?1) AND (?2 = '' OR user_id = ?2)
         ORDER BY client_ts ASC LIMIT {limit}"
    );
    let items: Vec<serde_json::Value> = match db.prepare(&sql) {
        Ok(mut stmt) => stmt
            .query_map(rusqlite::params![session_id, user_id], |r| {
                Ok(json!({
                    "event_type": r.get::<_, String>(0)?,
                    "target_id": r.get::<_, Option<String>>(1)?,
                    "target_label": r.get::<_, Option<String>>(2)?,
                    "route": r.get::<_, Option<String>>(3)?,
                    "dwell_ms": r.get::<_, Option<i64>>(4)?,
                    "client_ts": r.get::<_, String>(5)?,
                    "session_id": r.get::<_, String>(6)?,
                }))
            })
            .map(|rows| rows.filter_map(|x| x.ok()).collect())
            .unwrap_or_default(),
        Err(e) => return analytics_db_error("trail", e),
    };

    (
        StatusCode::OK,
        Json(json!({ "success": true, "items": items })),
    )
}

/// GET /api/admin/analytics/users — per-user activity cohort.
pub async fn analytics_users(
    State(state): State<AppState>,
    admin: AdminUserId,
    axum::extract::Query(params): axum::extract::Query<AnalyticsParams>,
) -> impl IntoResponse {
    let _ = admin;
    let (from, to, _user_id) = analytics_range(&params);
    let db = state.db.lock();

    let items: Vec<serde_json::Value> = match db.prepare(
        "SELECT b.user_id, COALESCE(u.display_name, u.username) AS name, COALESCE(u.role, 'user') AS role,
                COUNT(*) AS events, COUNT(DISTINCT b.session_id) AS sessions, MAX(b.created_at) AS last_active
         FROM behavior_events b
         JOIN users u ON u.id = b.user_id
         WHERE b.created_at >= ?1 AND b.created_at <= ?2
         GROUP BY b.user_id ORDER BY events DESC",
    ) {
        Ok(mut stmt) => stmt
            .query_map(rusqlite::params![from, to], |r| {
                Ok(json!({
                    "user_id": r.get::<_, String>(0)?,
                    "display_name": r.get::<_, String>(1)?,
                    "role": r.get::<_, String>(2)?,
                    "events": r.get::<_, i64>(3)?,
                    "sessions": r.get::<_, i64>(4)?,
                    "last_active": r.get::<_, String>(5)?,
                }))
            })
            .map(|rows| rows.filter_map(|x| x.ok()).collect())
            .unwrap_or_default(),
        Err(e) => return analytics_db_error("users", e),
    };

    (
        StatusCode::OK,
        Json(json!({ "success": true, "items": items })),
    )
}
