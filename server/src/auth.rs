use axum::extract::FromRequestParts;
use axum::extract::Path;
use axum::http::request::Parts;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use http::HeaderMap;
use serde::{Deserialize, Serialize};
use std::time::Instant;

use crate::state::AppState;

// ─── Rate limiting constants ───
const IP_MAX_ATTEMPTS: u32 = 10;
const IP_WINDOW_SECS: u64 = 60;
const USER_MAX_FAILURES: u32 = 5;
const USER_LOCKOUT_SECS: u64 = 900; // 15 minutes

// ─── Owner recovery (T-096 / ADR-006) constants ───
// 独立的限流体系,避免和普通 login 共享攻击面;详见 decisions/006-password-recovery.md。
const RECOVERY_IP_WINDOW_SECS: u64 = 600; // 10 分钟一个 IP 限流窗口
const RECOVERY_IP_MAX_ATTEMPTS: u32 = 1; // 每窗口仅 1 次请求(强力降速)
const RECOVERY_LOCKOUT_FAILURES: u32 = 5; // 累计 5 次失败(错 key/用户不存在)→ 拉黑
const RECOVERY_LOCKOUT_SECS: u64 = 3600; // 拉黑 1 小时

// ─── T-089 安全加固常量 ───
/// 管理员操作每用户每分钟上限(防暴力枚举 / DoS)
const ADMIN_RATE_LIMIT: u32 = 30;
const ADMIN_WINDOW_SECS: u64 = 60;
/// Guest AI 聚合限流:同一 IP 所有 guest 账号 1 小时总和上限。
/// 原漏洞:每 guest 21 calls × 每 IP 5 guests/小时 = 105 calls/小时免费 Claude → 改成全 IP 21/小时。
const GUEST_AI_IP_AGG_LIMIT: u32 = 21;
const GUEST_AI_IP_AGG_WINDOW_SECS: u64 = 3600;

// ─── Types ───

#[derive(Debug, Clone)]
pub struct UserId(pub String);

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

/// T-096 / ADR-006: POST /api/auth/owner-recovery body.
/// All fields required;`recovery_key` must NEVER appear in query string or URL
/// (audit log / referer leaks).
#[derive(Debug, Deserialize)]
pub struct OwnerRecoveryRequest {
    pub recovery_key: String,
    pub username: String,
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<UserInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_calls_remaining: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAvatarRequest {
    pub avatar: String,
}

// ─── T-116:Bearer 路径辅助函数(spec auth § 12.5)===
// 检查 Authorization: Bearer 头,有效 token 返回 user_id;否则 None(由 caller fallback 到 cookie)。
// 同时更新 last_used_at(尽力而为,不阻塞鉴权)。
fn try_bearer_user_id(parts: &Parts, state: &AppState) -> Option<String> {
    let auth = parts
        .headers
        .get(http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    let token = auth.strip_prefix("Bearer ")?.trim();
    if token.is_empty() {
        return None;
    }
    let hash = crate::routes::auth_tokens::hash_token(token);
    let db = state.db.lock();
    let user_id: Option<String> = db
        .query_row(
            "SELECT user_id FROM personal_tokens
             WHERE token_hash = ?1 AND revoked_at IS NULL
               AND (expires_at IS NULL OR expires_at > datetime('now'))",
            [&hash],
            |r| r.get(0),
        )
        .ok();
    if user_id.is_some() {
        let now = chrono::Utc::now().to_rfc3339();
        let _ = db.execute(
            "UPDATE personal_tokens SET last_used_at = ?1 WHERE token_hash = ?2",
            rusqlite::params![&now, &hash],
        );
    }
    user_id
}

// 拿当前 user 的 role / status 给 ActiveUserId / AdminUserId / OwnerUserId 用
fn fetch_role_and_status(state: &AppState, user_id: &str) -> Option<(String, String)> {
    let db = state.db.lock();
    db.query_row(
        "SELECT COALESCE(role, 'user'), COALESCE(status, 'active') FROM users WHERE id = ?1",
        [user_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )
    .ok()
}

// ─── Session middleware: extract UserId from cookie ───

impl FromRequestParts<AppState> for UserId {
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // T-116:Bearer 优先;fallback 到 cookie
        if let Some(uid) = try_bearer_user_id(parts, state) {
            return Ok(UserId(uid));
        }

        // Extract cookie jar
        let jar = CookieJar::from_request_parts(parts, state)
            .await
            .map_err(|_| unauthorized())?;

        let token = jar
            .get("session")
            .map(|c| c.value().to_string())
            .ok_or_else(unauthorized)?;

        // Validate session
        let db = state.db.lock();
        let result = db.query_row(
            "SELECT user_id FROM sessions WHERE token = ?1 AND expires_at > datetime('now')",
            [&token],
            |row: &rusqlite::Row| row.get::<_, String>(0),
        );

        match result {
            Ok(user_id) => Ok(UserId(user_id)),
            Err(_) => Err(unauthorized()),
        }
    }
}

// ─── ActiveUserId: like UserId but rejects pending/rejected accounts ───

#[derive(Debug, Clone)]
pub struct ActiveUserId(pub String);

impl FromRequestParts<AppState> for ActiveUserId {
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // T-116:Bearer 优先;命中后仍需走 status 校验(suspended 用户即使 PAT 有效也拒)
        if let Some(uid) = try_bearer_user_id(parts, state) {
            let (_, status) = fetch_role_and_status(state, &uid)
                .unwrap_or_else(|| ("user".into(), "active".into()));
            return match status.as_str() {
                "active" | "guest" => Ok(ActiveUserId(uid)),
                "pending" => Err((
                    StatusCode::FORBIDDEN,
                    Json(
                        serde_json::json!({"success": false, "error": "ACCOUNT_PENDING", "message": "账户审核中,暂时无法操作"}),
                    ),
                )),
                "suspended" => Err((
                    StatusCode::FORBIDDEN,
                    Json(
                        serde_json::json!({"success": false, "error": "ACCOUNT_SUSPENDED", "message": "账户已临时挂起,请联系管理员"}),
                    ),
                )),
                _ => Err((
                    StatusCode::FORBIDDEN,
                    Json(
                        serde_json::json!({"success": false, "error": "ACCOUNT_REJECTED", "message": "账户已被拒绝"}),
                    ),
                )),
            };
        }

        // Extract cookie jar
        let jar = CookieJar::from_request_parts(parts, state)
            .await
            .map_err(|_| unauthorized())?;

        let token = jar
            .get("session")
            .map(|c| c.value().to_string())
            .ok_or_else(unauthorized)?;

        // Validate session + check user status
        let db = state.db.lock();
        let result = db.query_row(
            "SELECT s.user_id, COALESCE(u.status, 'active')
             FROM sessions s JOIN users u ON u.id = s.user_id
             WHERE s.token = ?1 AND s.expires_at > datetime('now')",
            [&token],
            |row: &rusqlite::Row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        );

        match result {
            Ok((user_id, status)) => match status.as_str() {
                "active" | "guest" => Ok(ActiveUserId(user_id)),
                "pending" => Err((
                    StatusCode::FORBIDDEN,
                    Json(serde_json::json!({
                        "success": false,
                        "error": "ACCOUNT_PENDING",
                        "message": "账户审核中，暂时无法操作"
                    })),
                )),
                "suspended" => Err((
                    StatusCode::FORBIDDEN,
                    Json(serde_json::json!({
                        "success": false,
                        "error": "ACCOUNT_SUSPENDED",
                        "message": "账户已临时挂起，请联系管理员"
                    })),
                )),
                _ => Err((
                    StatusCode::FORBIDDEN,
                    Json(serde_json::json!({
                        "success": false,
                        "error": "ACCOUNT_REJECTED",
                        "message": "账户已被拒绝"
                    })),
                )),
            },
            Err(_) => Err(unauthorized()),
        }
    }
}

// ─── AdminUserId: requires role=admin or role=owner ───

#[derive(Debug, Clone)]
pub struct AdminUserId(pub String);

impl FromRequestParts<AppState> for AdminUserId {
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // T-116:Bearer 优先;命中后仍走 role 校验
        if let Some(uid) = try_bearer_user_id(parts, state) {
            let role = fetch_role_and_status(state, &uid)
                .map(|(r, _)| r)
                .unwrap_or_else(|| "user".into());
            return if role == "admin" || role == "owner" {
                Ok(AdminUserId(uid))
            } else {
                Err((
                    StatusCode::FORBIDDEN,
                    Json(
                        serde_json::json!({"success": false, "error": "PERMISSION_DENIED", "message": "权限不足"}),
                    ),
                ))
            };
        }

        let jar = CookieJar::from_request_parts(parts, state)
            .await
            .map_err(|_| unauthorized())?;

        let token = jar
            .get("session")
            .map(|c| c.value().to_string())
            .ok_or_else(unauthorized)?;

        let db = state.db.lock();
        let result = db.query_row(
            "SELECT s.user_id, COALESCE(u.role, 'user')
             FROM sessions s JOIN users u ON u.id = s.user_id
             WHERE s.token = ?1 AND s.expires_at > datetime('now')",
            [&token],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        );

        match result {
            Ok((user_id, role)) if role == "admin" || role == "owner" => Ok(AdminUserId(user_id)),
            Ok(_) => Err((
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "success": false,
                    "error": "PERMISSION_DENIED",
                    "message": "权限不足"
                })),
            )),
            Err(_) => Err(unauthorized()),
        }
    }
}

// ─── OwnerUserId: requires role=owner ───

#[derive(Debug, Clone)]
pub struct OwnerUserId(pub String);

impl FromRequestParts<AppState> for OwnerUserId {
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // T-116:Bearer 优先;命中后仍走 owner 校验
        if let Some(uid) = try_bearer_user_id(parts, state) {
            let role = fetch_role_and_status(state, &uid)
                .map(|(r, _)| r)
                .unwrap_or_else(|| "user".into());
            return if role == "owner" {
                Ok(OwnerUserId(uid))
            } else {
                Err((
                    StatusCode::FORBIDDEN,
                    Json(
                        serde_json::json!({"success": false, "error": "PERMISSION_DENIED", "message": "仅系统所有者可执行此操作"}),
                    ),
                ))
            };
        }

        let jar = CookieJar::from_request_parts(parts, state)
            .await
            .map_err(|_| unauthorized())?;

        let token = jar
            .get("session")
            .map(|c| c.value().to_string())
            .ok_or_else(unauthorized)?;

        let db = state.db.lock();
        let result = db.query_row(
            "SELECT s.user_id, COALESCE(u.role, 'user')
             FROM sessions s JOIN users u ON u.id = s.user_id
             WHERE s.token = ?1 AND s.expires_at > datetime('now')",
            [&token],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        );

        match result {
            Ok((user_id, role)) if role == "owner" => Ok(OwnerUserId(user_id)),
            Ok(_) => Err((
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "success": false,
                    "error": "PERMISSION_DENIED",
                    "message": "仅系统所有者可执行此操作"
                })),
            )),
            Err(_) => Err(unauthorized()),
        }
    }
}

fn unauthorized() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({
            "success": false,
            "error": "UNAUTHORIZED",
            "message": "请先登录"
        })),
    )
}

// ─── Rate limiting helpers ───

pub(crate) fn extract_client_ip(headers: &HeaderMap) -> String {
    // Fly.io sets Fly-Client-IP
    if let Some(val) = headers.get("fly-client-ip") {
        if let Ok(ip) = val.to_str() {
            return ip.trim().to_string();
        }
    }
    // Fallback to X-Forwarded-For (first IP)
    if let Some(val) = headers.get("x-forwarded-for") {
        if let Ok(ips) = val.to_str() {
            if let Some(first) = ips.split(',').next() {
                return first.trim().to_string();
            }
        }
    }
    "unknown".to_string()
}

fn check_ip_rate_limit(state: &AppState, ip: &str) -> bool {
    let attempts = state.login_ip_attempts.lock();
    if let Some((count, window_start)) = attempts.get(ip) {
        if window_start.elapsed().as_secs() < IP_WINDOW_SECS {
            return *count >= IP_MAX_ATTEMPTS;
        }
    }
    false
}

fn record_ip_attempt(state: &AppState, ip: &str) {
    let mut attempts = state.login_ip_attempts.lock();
    let entry = attempts
        .entry(ip.to_string())
        .or_insert((0, Instant::now()));
    if entry.1.elapsed().as_secs() >= IP_WINDOW_SECS {
        *entry = (1, Instant::now());
    } else {
        entry.0 += 1;
    }
}

fn check_user_lockout(state: &AppState, username: &str) -> Option<u64> {
    let lockouts = state.login_user_lockouts.lock();
    if let Some((count, last_failure)) = lockouts.get(username) {
        if *count >= USER_MAX_FAILURES {
            let elapsed = last_failure.elapsed().as_secs();
            if elapsed < USER_LOCKOUT_SECS {
                return Some(USER_LOCKOUT_SECS - elapsed);
            }
        }
    }
    None
}

fn record_user_failure(state: &AppState, username: &str) {
    let mut lockouts = state.login_user_lockouts.lock();
    let entry = lockouts
        .entry(username.to_string())
        .or_insert((0, Instant::now()));
    if entry.1.elapsed().as_secs() >= USER_LOCKOUT_SECS {
        *entry = (1, Instant::now());
    } else {
        entry.0 += 1;
        entry.1 = Instant::now();
    }
}

fn clear_user_lockout(state: &AppState, username: &str) {
    let mut lockouts = state.login_user_lockouts.lock();
    lockouts.remove(username);
}

// ─── Owner recovery (T-096) rate limiting ───
// 完全独立的两个 map,语义和参数都和上面 login 不同(见常量定义)。

fn check_recovery_ip_rate(state: &AppState, ip: &str) -> bool {
    let attempts = state.recovery_ip_attempts.lock();
    if let Some((count, window_start)) = attempts.get(ip) {
        if window_start.elapsed().as_secs() < RECOVERY_IP_WINDOW_SECS {
            return *count >= RECOVERY_IP_MAX_ATTEMPTS;
        }
    }
    false
}

fn record_recovery_ip_attempt(state: &AppState, ip: &str) {
    let mut attempts = state.recovery_ip_attempts.lock();
    let entry = attempts
        .entry(ip.to_string())
        .or_insert((0, Instant::now()));
    if entry.1.elapsed().as_secs() >= RECOVERY_IP_WINDOW_SECS {
        *entry = (1, Instant::now());
    } else {
        entry.0 += 1;
    }
}

fn check_recovery_lockout(state: &AppState, ip: &str) -> Option<u64> {
    let lockouts = state.recovery_ip_lockouts.lock();
    if let Some((count, last_failure)) = lockouts.get(ip) {
        if *count >= RECOVERY_LOCKOUT_FAILURES {
            let elapsed = last_failure.elapsed().as_secs();
            if elapsed < RECOVERY_LOCKOUT_SECS {
                return Some(RECOVERY_LOCKOUT_SECS - elapsed);
            }
        }
    }
    None
}

fn record_recovery_failure(state: &AppState, ip: &str) {
    let mut lockouts = state.recovery_ip_lockouts.lock();
    let entry = lockouts
        .entry(ip.to_string())
        .or_insert((0, Instant::now()));
    if entry.1.elapsed().as_secs() >= RECOVERY_LOCKOUT_SECS {
        *entry = (1, Instant::now());
    } else {
        entry.0 += 1;
        entry.1 = Instant::now();
    }
}

/// 成功重置后清除该 IP 的失败计数 + 限流计数(合法用途,不留惩罚)。
fn clear_recovery_state(state: &AppState, ip: &str) {
    state.recovery_ip_lockouts.lock().remove(ip);
    state.recovery_ip_attempts.lock().remove(ip);
}

/// Validate password complexity: 8-128 chars, must contain uppercase + lowercase + digit
pub(crate) fn validate_password(password: &str) -> Result<(), &'static str> {
    if password.len() < 8 {
        return Err("密码至少需要 8 个字符");
    }
    if password.len() > 128 {
        return Err("密码不能超过 128 个字符");
    }
    let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
    let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    if !has_upper || !has_lower || !has_digit {
        return Err("密码需要包含大写字母、小写字母和数字");
    }
    Ok(())
}

// ─── Handlers ───

pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    // IP rate limit (shared with login)
    let ip = extract_client_ip(&headers);
    if check_ip_rate_limit(&state, &ip) {
        return (
            jar,
            (
                StatusCode::TOO_MANY_REQUESTS,
                Json(AuthResponse {
                    success: false,
                    user: None,
                    message: Some("请求过于频繁，请稍后再试".into()),
                }),
            ),
        );
    }
    record_ip_attempt(&state, &ip);

    // Validate username
    if req.username.len() < 3 || req.username.len() > 20 {
        return (
            jar,
            (
                StatusCode::BAD_REQUEST,
                Json(AuthResponse {
                    success: false,
                    user: None,
                    message: Some("用户名需要 3-20 个字符".into()),
                }),
            ),
        );
    }
    if !req
        .username
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_')
    {
        return (
            jar,
            (
                StatusCode::BAD_REQUEST,
                Json(AuthResponse {
                    success: false,
                    user: None,
                    message: Some("用户名只能包含字母、数字和下划线".into()),
                }),
            ),
        );
    }

    // Validate password complexity
    if let Err(msg) = validate_password(&req.password) {
        return (
            jar,
            (
                StatusCode::BAD_REQUEST,
                Json(AuthResponse {
                    success: false,
                    user: None,
                    message: Some(msg.into()),
                }),
            ),
        );
    }

    let db = state.db.lock();

    // Check username uniqueness
    let exists: bool = db
        .query_row(
            "SELECT COUNT(*) > 0 FROM users WHERE username = ?1",
            [&req.username],
            |row: &rusqlite::Row| row.get(0),
        )
        .unwrap_or(false);

    if exists {
        return (
            jar,
            (
                StatusCode::CONFLICT,
                Json(AuthResponse {
                    success: false,
                    user: None,
                    message: Some("用户名已被使用".into()),
                }),
            ),
        );
    }

    // Hash password
    let password_hash = match hash_password(&req.password) {
        Ok(h) => h,
        Err(_) => {
            return (
                jar,
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(AuthResponse {
                        success: false,
                        user: None,
                        message: Some("密码加密失败".into()),
                    }),
                ),
            )
        }
    };

    let user_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let display_name = req.display_name.unwrap_or_else(|| req.username.clone());

    // Check daily registration count to decide status
    let today_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM users WHERE created_at >= date('now')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let status = if today_count < 10 {
        "active"
    } else {
        "pending"
    };

    if let Err(e) = db.execute(
        "INSERT INTO users (id, username, password_hash, display_name, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![user_id, req.username, password_hash, display_name, status, now, now],
    ) {
        eprintln!("[auth] register db error: {}", e);
        return (
            jar,
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthResponse {
                    success: false,
                    user: None,
                    message: Some("注册失败".into()),
                }),
            ),
        );
    }

    // Initialize default soul state for new user
    if let Err(e) = db.execute(
        "INSERT INTO soul_states (user_id, updated_at) VALUES (?1, ?2)",
        rusqlite::params![user_id, now],
    ) {
        eprintln!(
            "[auth] soul state init failed (will lazy-create later): {}",
            e
        );
    }

    // If pending, notify all admins
    if status == "pending" {
        let mut admin_stmt = match db.prepare("SELECT id FROM users WHERE role = 'admin'") {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[auth] register db error: {}", e);
                return (
                    jar,
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(AuthResponse {
                            success: false,
                            user: None,
                            message: Some("注册失败".into()),
                        }),
                    ),
                );
            }
        };
        let admin_ids: Vec<String> = match admin_stmt.query_map([], |row| row.get(0)) {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(e) => {
                eprintln!("[auth] register db error: {}", e);
                return (
                    jar,
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(AuthResponse {
                            success: false,
                            user: None,
                            message: Some("注册失败".into()),
                        }),
                    ),
                );
            }
        };
        for admin_id in admin_ids {
            let notif_id = uuid::Uuid::new_v4().to_string();
            db.execute(
                "INSERT INTO notifications (id, user_id, type, title, body, created_at) VALUES (?1, ?2, 'system', ?3, ?4, ?5)",
                rusqlite::params![
                    notif_id,
                    admin_id,
                    "新用户待审批",
                    format!("用户 {} 注册待审批", req.username),
                    now
                ],
            )
            .ok();
        }
    }

    // Auto-login: create session
    let token = generate_session_token();
    let expires = (chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339();
    if let Err(e) = db.execute(
        "INSERT INTO sessions (token, user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![token, user_id, now, expires],
    ) {
        eprintln!("[auth] register session db error: {}", e);
        return (
            jar,
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthResponse {
                    success: false,
                    user: None,
                    message: Some("注册失败".into()),
                }),
            ),
        );
    }

    let jar = jar.add(make_session_cookie(token));

    let message = if status == "pending" {
        "注册成功，账户待审核"
    } else {
        "注册成功"
    };

    (
        jar,
        (
            StatusCode::OK,
            Json(AuthResponse {
                success: true,
                user: Some(UserInfo {
                    id: user_id,
                    username: req.username,
                    display_name: Some(display_name),
                    avatar: None,
                    status: Some(status.to_string()),
                    role: None,
                    ai_calls_remaining: None,
                }),
                message: Some(message.into()),
            }),
        ),
    )
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    // IP rate limit
    let ip = extract_client_ip(&headers);
    if check_ip_rate_limit(&state, &ip) {
        return (
            jar,
            (
                StatusCode::TOO_MANY_REQUESTS,
                Json(AuthResponse {
                    success: false,
                    user: None,
                    message: Some("请求过于频繁，请稍后再试".into()),
                }),
            ),
        );
    }
    record_ip_attempt(&state, &ip);

    // User lockout check
    if let Some(remaining) = check_user_lockout(&state, &req.username) {
        let mins = remaining.div_ceil(60);
        return (
            jar,
            (
                StatusCode::TOO_MANY_REQUESTS,
                Json(AuthResponse {
                    success: false,
                    user: None,
                    message: Some(format!("账户已锁定，请 {} 分钟后再试", mins)),
                }),
            ),
        );
    }

    let db = state.db.lock();

    // Find user
    let user_row = db.query_row(
        "SELECT id, username, password_hash, display_name, COALESCE(avatar,''), COALESCE(status,'active'), COALESCE(role,'user') FROM users WHERE username = ?1",
        [&req.username],
        |row: &rusqlite::Row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        },
    );

    let (user_id, username, password_hash, display_name, avatar, status, role) = match user_row {
        Ok(r) => r,
        Err(_) => {
            // Timing attack mitigation: run dummy hash even when user not found
            let _ = verify_password("dummy", "$argon2id$v=19$m=19456,t=2,p=1$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
            record_user_failure(&state, &req.username);
            return (
                jar,
                (
                    StatusCode::UNAUTHORIZED,
                    Json(AuthResponse {
                        success: false,
                        user: None,
                        message: Some("用户名或密码错误".into()),
                    }),
                ),
            );
        }
    };

    // Verify password
    if !verify_password(&req.password, &password_hash) {
        record_user_failure(&state, &req.username);
        return (
            jar,
            (
                StatusCode::UNAUTHORIZED,
                Json(AuthResponse {
                    success: false,
                    user: None,
                    message: Some("用户名或密码错误".into()),
                }),
            ),
        );
    }

    // Login success — clear lockout
    clear_user_lockout(&state, &req.username);

    // Create session
    let token = generate_session_token();
    let now = chrono::Utc::now().to_rfc3339();
    let expires = (chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339();
    if let Err(e) = db.execute(
        "INSERT INTO sessions (token, user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![token, user_id, now, expires],
    ) {
        eprintln!("[auth] login session db error: {}", e);
        return (
            jar,
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthResponse {
                    success: false,
                    user: None,
                    message: Some("登录失败".into()),
                }),
            ),
        );
    }

    // Limit to 5 sessions per user
    db.execute(
        "DELETE FROM sessions WHERE user_id = ?1 AND token NOT IN (SELECT token FROM sessions WHERE user_id = ?1 ORDER BY created_at DESC LIMIT 5)",
        [&user_id],
    )
    .ok();

    let jar = jar.add(make_session_cookie(token));

    // Reject login for rejected users
    if status == "rejected" {
        return (
            jar,
            (
                StatusCode::FORBIDDEN,
                Json(AuthResponse {
                    success: false,
                    user: None,
                    message: Some("账户已被拒绝，无法登录".into()),
                }),
            ),
        );
    }

    (
        jar,
        (
            StatusCode::OK,
            Json(AuthResponse {
                success: true,
                user: Some(UserInfo {
                    id: user_id,
                    username,
                    display_name,
                    avatar: if avatar.is_empty() {
                        None
                    } else {
                        Some(avatar)
                    },
                    status: Some(status),
                    role: Some(role),
                    ai_calls_remaining: None,
                }),
                message: Some("登录成功".into()),
            }),
        ),
    )
}

pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(cookie) = jar.get("session") {
        let token = cookie.value().to_string();
        let db = state.db.lock();
        db.execute("DELETE FROM sessions WHERE token = ?1", [&token])
            .ok();
    }

    let jar = jar.remove(Cookie::from("session"));

    (jar, Json(serde_json::json!({ "success": true })))
}

pub async fn me(State(state): State<AppState>, user_id: UserId) -> impl IntoResponse {
    let db = state.db.lock();

    let result = db.query_row(
        "SELECT id, username, display_name, COALESCE(avatar,''), COALESCE(status,'active'), ai_calls_remaining, COALESCE(role,'user') FROM users WHERE id = ?1",
        [&user_id.0],
        |row: &rusqlite::Row| {
            let av: String = row.get(3)?;
            let st: String = row.get(4)?;
            let ai_remaining: Option<i32> = row.get(5)?;
            let rl: String = row.get(6)?;
            Ok(UserInfo {
                id: row.get(0)?,
                username: row.get(1)?,
                display_name: row.get(2)?,
                avatar: if av.is_empty() { None } else { Some(av) },
                ai_calls_remaining: if st == "guest" { ai_remaining } else { None },
                status: Some(st),
                role: Some(rl),
            })
        },
    );

    match result {
        Ok(user) => (
            StatusCode::OK,
            Json(AuthResponse {
                success: true,
                user: Some(user),
                message: None,
            }),
        ),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(AuthResponse {
                success: false,
                user: None,
                message: Some("用户不存在".into()),
            }),
        ),
    }
}

pub async fn change_password(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    jar: CookieJar,
    Json(req): Json<ChangePasswordRequest>,
) -> impl IntoResponse {
    // Validate new password complexity
    if let Err(msg) = validate_password(&req.new_password) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "message": msg
            })),
        );
    }

    let db = state.db.lock();

    // Get current password hash
    let current_hash = match db.query_row(
        "SELECT password_hash FROM users WHERE id = ?1",
        [&user_id.0],
        |row: &rusqlite::Row| row.get::<_, String>(0),
    ) {
        Ok(h) => h,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "success": false,
                    "message": "用户不存在"
                })),
            );
        }
    };

    // Verify old password
    if !verify_password(&req.old_password, &current_hash) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "message": "当前密码不正确"
            })),
        );
    }

    // Hash new password
    let new_hash = match hash_password(&req.new_password) {
        Ok(h) => h,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "message": "密码加密失败"
                })),
            );
        }
    };

    // Update password
    let now = chrono::Utc::now().to_rfc3339();
    match db.execute(
        "UPDATE users SET password_hash = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![new_hash, now, user_id.0],
    ) {
        Ok(_) => {
            // Invalidate all other sessions (keep current)
            let current_token = jar.get("session").map(|c| c.value().to_string());
            if let Some(token) = current_token {
                db.execute(
                    "DELETE FROM sessions WHERE user_id = ?1 AND token != ?2",
                    rusqlite::params![user_id.0, token],
                )
                .ok();
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": true,
                    "message": "密码修改成功，其他设备已自动登出"
                })),
            )
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "message": "密码更新失败"
            })),
        ),
    }
}

pub async fn update_avatar(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Json(req): Json<UpdateAvatarRequest>,
) -> impl IntoResponse {
    // Limit avatar data size (256KB max for base64 images)
    if req.avatar.len() > 256 * 1024 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "message": "头像数据太大"
            })),
        );
    }

    let db = state.db.lock();

    let now = chrono::Utc::now().to_rfc3339();
    match db.execute(
        "UPDATE users SET avatar = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![req.avatar, now, user_id.0],
    ) {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true
            })),
        ),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "message": "保存头像失败"
            })),
        ),
    }
}

// ─── Owner recovery (T-096 / ADR-006) ───
//
// POST /api/auth/owner-recovery
//   { "recovery_key": "<64-hex>", "username": "...", "new_password": "..." }
//
// 紧急自救通道:owner 忘密码时,凭 OWNER_RECOVERY_KEY(部署时存到 Fly secret,绝不入 git)
// 重置密码 + 吊销该用户的所有 session(含可能已被劫持的)。
// 详细设计 / 三方案对比 见 ergouPM/decisions/006-password-recovery.md。
//
// 安全护栏(全部在本函数实现,缺一不可,见任务令 T-096 § 安全护栏):
//   1. 常数时间比较 recovery_key (subtle::ConstantTimeEq)
//   2. IP 限流:1 次 / 600 秒 / IP(独立 map,不挤占 login 限流)
//   3. 累计 5 次失败 → IP 拉黑 1 小时
//   4. 先查 role 再校验 key —— role≠owner 直接 403(不消耗 key 验证次数,但 IP 计数已记)
//   5. 复用 validate_password / hash_password
//   6. 成功后 DELETE FROM sessions WHERE user_id = ?(全部吊销,比 change_password 更激进)
//   7. 审计日志 eprintln!("[owner-recovery] ip=… result=…"),永不记录 key 或密码明文
//   8. 响应不返回 owner 列表 / 计数 / 用户存在性信息(401 文案统一)
//   9. recovery_key 只接受 POST body,不接受 query string(防 access log 泄露 —— Axum Json 提取本身就只读 body)
pub async fn owner_recovery(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<OwnerRecoveryRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let ip = extract_client_ip(&headers);

    // 1) 功能必须配置(护栏 § 配置)
    let configured_key = match std::env::var("OWNER_RECOVERY_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            eprintln!("[owner-recovery] ip={} result=disabled (env not set)", ip);
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "success": false,
                    "error": "RECOVERY_DISABLED"
                })),
            );
        }
    };

    // 2) IP 限流(护栏 § 2)—— 1 次/600s
    if check_recovery_ip_rate(&state, &ip) {
        eprintln!("[owner-recovery] ip={} result=rate_limited", ip);
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "success": false,
                "error": "RATE_LIMITED"
            })),
        );
    }
    // 3) IP 拉黑(护栏 § 3)—— 5 次失败/1h
    if let Some(remaining) = check_recovery_lockout(&state, &ip) {
        eprintln!(
            "[owner-recovery] ip={} result=locked_out remaining={}s",
            ip, remaining
        );
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "success": false,
                "error": "RATE_LIMITED"
            })),
        );
    }

    // 计入 IP 尝试(无论后续结果如何)—— 即"角色不对" / "用户不存在"也会消耗 IP 配额
    record_recovery_ip_attempt(&state, &ip);

    // 4) 新密码复杂度(护栏 § 5 复用 validate_password)
    if let Err(msg) = validate_password(&req.new_password) {
        eprintln!(
            "[owner-recovery] ip={} username={} result=weak_password",
            ip, req.username
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": "WEAK_PASSWORD",
                "message": msg
            })),
        );
    }

    // 5) 查用户 + 角色(护栏 § 4 —— 先 role,后 key)
    let db = state.db.lock();
    let row = db.query_row(
        "SELECT id, COALESCE(role, 'user') FROM users WHERE username = ?1",
        [&req.username],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    );

    let (user_id, role) = match row {
        Ok(t) => t,
        Err(_) => {
            // 用户名不存在 → 401(文案与"key 错"统一,防探测枚举)。
            // 计入 IP 失败计数(防止用脚本扫描 owner 用户名)。
            drop(db);
            record_recovery_failure(&state, &ip);
            eprintln!(
                "[owner-recovery] ip={} username={} result=unknown_user",
                ip, req.username
            );
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "success": false,
                    "error": "INVALID_CREDENTIALS"
                })),
            );
        }
    };

    // role ≠ owner → 403。不消耗"key 验证次数"(护栏 § 4),IP 计数已经记过了。
    if role != "owner" {
        eprintln!(
            "[owner-recovery] ip={} username={} role={} result=not_owner",
            ip, req.username, role
        );
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "success": false,
                "error": "NOT_OWNER"
            })),
        );
    }

    // 6) 常数时间比较 recovery_key(护栏 § 1)
    use subtle::ConstantTimeEq;
    let configured = configured_key.as_bytes();
    let provided = req.recovery_key.as_bytes();
    // ct_eq 要求长度一致才有意义;长度不同时 ct_eq 自动返回 false,但仍走常数时间路径。
    if !bool::from(configured.ct_eq(provided)) {
        drop(db);
        record_recovery_failure(&state, &ip);
        eprintln!(
            "[owner-recovery] ip={} username={} result=wrong_key",
            ip, req.username
        );
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "success": false,
                "error": "INVALID_CREDENTIALS"
            })),
        );
    }

    // 7) 密码 hash + 持久化
    let new_hash = match hash_password(&req.new_password) {
        Ok(h) => h,
        Err(_) => {
            eprintln!(
                "[owner-recovery] ip={} username={} result=hash_error",
                ip, req.username
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": "INTERNAL"
                })),
            );
        }
    };
    let now = chrono::Utc::now().to_rfc3339();
    if let Err(e) = db.execute(
        "UPDATE users SET password_hash = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![new_hash, now, user_id],
    ) {
        eprintln!(
            "[owner-recovery] ip={} username={} result=db_error err={}",
            ip, req.username, e
        );
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "error": "INTERNAL"
            })),
        );
    }
    // 8) 吊销该用户全部 session(护栏 § 6)
    let _ = db.execute("DELETE FROM sessions WHERE user_id = ?1", [&user_id]);
    drop(db);

    // 合法成功的 IP,清除其失败计数 + 限流计数(不留惩罚)
    clear_recovery_state(&state, &ip);

    eprintln!(
        "[owner-recovery] ip={} username={} result=success",
        ip, req.username
    );
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "密码已重置，请重新登录"
        })),
    )
}

// ─── Guest mode helpers ───

/// Check if user is a guest and reject if so (for social/collab routes)
pub fn reject_if_guest(
    state: &AppState,
    user_id: &str,
) -> Option<(StatusCode, Json<serde_json::Value>)> {
    let db = state.db.lock();
    let status: String = db
        .query_row(
            "SELECT COALESCE(status,'active') FROM users WHERE id = ?1",
            [user_id],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "active".to_string());
    if status == "guest" {
        Some((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "success": false,
                "error": "GUEST_RESTRICTED",
                "message": "体验模式不支持此功能，注册账户解锁"
            })),
        ))
    } else {
        None
    }
}

/// Check and decrement AI quota for guest users. Returns remaining count.
/// Non-guest users pass through with Ok(999).
pub fn check_guest_ai_quota(
    state: &AppState,
    user_id: &str,
    ip: &str,
) -> Result<i32, (StatusCode, Json<serde_json::Value>)> {
    let db = state.db.lock();
    let row = db.query_row(
        "SELECT COALESCE(status,'active'), ai_calls_remaining FROM users WHERE id = ?1",
        [user_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<i32>>(1)?)),
    );
    match row {
        Ok((status, remaining)) => {
            if status != "guest" {
                return Ok(999);
            }

            // T-089 块3:guest 用户先过 IP 聚合限流(同 IP 所有 guest 合计 ≤ 21/小时)。
            //   - 只对 guest 计数,普通注册用户不入此 map。
            //   - 先查后扣:若已超限,直接拒绝,不消耗个人配额(扣了也白搭)。
            //   - `ip == ""`(extractor 拿不到 client IP 的边缘场景)→ 降级跳过聚合,
            //     仍走个人配额。这样不会因为 IP 解析失败就给 attacker 留口子,但也不
            //     会因为 IP 拿不到就误锁 legit user。
            drop(db);
            if !ip.is_empty() {
                let mut agg = state.guest_ai_ip_aggregate.lock();
                let entry = agg.entry(ip.to_string()).or_insert((0, Instant::now()));
                if entry.1.elapsed().as_secs() >= GUEST_AI_IP_AGG_WINDOW_SECS {
                    *entry = (1, Instant::now());
                } else if entry.0 >= GUEST_AI_IP_AGG_LIMIT {
                    eprintln!(
                        "[guest-ai] ip={} aggregate exhausted ({}/{})",
                        ip, entry.0, GUEST_AI_IP_AGG_LIMIT
                    );
                    return Err((
                        StatusCode::FORBIDDEN,
                        Json(serde_json::json!({
                            "success": false,
                            "error": "GUEST_AI_EXHAUSTED",
                            "message": "本网络的 AI 体验额度已用尽,请注册账户解锁",
                            "ai_remaining": 0
                        })),
                    ));
                } else {
                    entry.0 += 1;
                }
            }

            // 再过个人配额
            let db = state.db.lock();
            let remaining = remaining.unwrap_or(0);
            if remaining <= 0 {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(serde_json::json!({
                        "success": false,
                        "error": "GUEST_AI_EXHAUSTED",
                        "message": "AI 体验次数已用完，注册解锁无限使用",
                        "ai_remaining": 0
                    })),
                ));
            }
            // Decrement
            db.execute(
                "UPDATE users SET ai_calls_remaining = ai_calls_remaining - 1 WHERE id = ?1",
                [user_id],
            )
            .ok();
            Ok(remaining - 1)
        }
        Err(_) => Ok(999),
    }
}

// T-089 块2:Admin 限流中间件
// 用法:`admin_routes.layer(axum::middleware::from_fn(admin_rate_limit_middleware))`
//   ——middleware 通过 State<AppState> + UserId 抽取器自动拿到状态和用户,
//   不需要 `from_fn_with_state`(因为 Axum 0.8 的 router 会在外层 `.with_state()` 时统一注入)。
pub async fn admin_rate_limit_middleware(
    State(state): State<AppState>,
    user_id: UserId,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    {
        let mut limits = state.admin_user_rate_limits.lock();
        let entry = limits
            .entry(user_id.0.clone())
            .or_insert((0, Instant::now()));
        if entry.1.elapsed().as_secs() >= ADMIN_WINDOW_SECS {
            // 新窗口
            *entry = (1, Instant::now());
        } else {
            entry.0 += 1;
            if entry.0 > ADMIN_RATE_LIMIT {
                eprintln!(
                    "[admin rate-limit] user={} hits={}/min — 429",
                    user_id.0, entry.0
                );
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(serde_json::json!({
                        "success": false,
                        "error": "RATE_LIMITED",
                        "message": "管理操作过于频繁,请稍后再试"
                    })),
                )
                    .into_response();
            }
        }
    }
    next.run(req).await
}

// ─── Guest login ───

const GUEST_IP_MAX: u32 = 5;
const GUEST_IP_WINDOW_SECS: u64 = 3600; // 1 hour
const GUEST_MAX_ACTIVE: i64 = 50;
const GUEST_AI_QUOTA: i32 = 21;

pub async fn guest_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
) -> impl IntoResponse {
    let ip = extract_client_ip(&headers);

    // IP rate limit for guest creation
    {
        let limits = state.guest_ip_rate_limits.lock();
        if let Some((count, window_start)) = limits.get(&ip) {
            if window_start.elapsed().as_secs() < GUEST_IP_WINDOW_SECS && *count >= GUEST_IP_MAX {
                return (
                    jar,
                    (
                        StatusCode::TOO_MANY_REQUESTS,
                        Json(AuthResponse {
                            success: false,
                            user: None,
                            message: Some("体验账户创建过于频繁，请稍后再试".into()),
                        }),
                    ),
                );
            }
        }
    }

    let db = state.db.lock();

    // Check global guest count
    let active_guests: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM users u WHERE u.status = 'guest' \
             AND EXISTS (SELECT 1 FROM sessions s WHERE s.user_id = u.id AND s.expires_at > datetime('now'))",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if active_guests >= GUEST_MAX_ACTIVE {
        return (
            jar,
            (
                StatusCode::TOO_MANY_REQUESTS,
                Json(AuthResponse {
                    success: false,
                    user: None,
                    message: Some("体验名额已满，请稍后再试或直接注册".into()),
                }),
            ),
        );
    }

    // Generate guest user
    let hex8: String = {
        use rand::RngCore;
        let mut bytes = [0u8; 4];
        rand::rng().fill_bytes(&mut bytes);
        hex::encode(bytes)
    };
    let user_id = uuid::Uuid::new_v4().to_string();
    let username = format!("guest_{}", hex8);
    let now = chrono::Utc::now().to_rfc3339();

    // Create user with dummy password hash (guests can't login with password)
    db.execute(
        "INSERT INTO users (id, username, password_hash, display_name, status, ai_calls_remaining, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'guest', ?5, ?6, ?7)",
        rusqlite::params![user_id, username, "guest_no_password", "访客", GUEST_AI_QUOTA, now, now],
    )
    .unwrap();

    // Seed demo data
    drop(db); // release lock before seeding (seed will re-acquire)
    crate::services::guest_seed::seed_guest_demo_data(&state, &user_id);

    let db = state.db.lock();

    // Create 24h session
    let token = generate_session_token();
    let expires = (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339();
    db.execute(
        "INSERT INTO sessions (token, user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![token, user_id, now, expires],
    )
    .unwrap();

    // Record IP attempt
    drop(db);
    {
        let mut limits = state.guest_ip_rate_limits.lock();
        let entry = limits.entry(ip).or_insert((0, Instant::now()));
        if entry.1.elapsed().as_secs() >= GUEST_IP_WINDOW_SECS {
            *entry = (1, Instant::now());
        } else {
            entry.0 += 1;
        }
    }

    let jar = jar.add(make_session_cookie(token));

    (
        jar,
        (
            StatusCode::OK,
            Json(AuthResponse {
                success: true,
                user: Some(UserInfo {
                    id: user_id,
                    username,
                    display_name: Some("访客".into()),
                    avatar: None,
                    status: Some("guest".into()),
                    role: None,
                    ai_calls_remaining: Some(GUEST_AI_QUOTA),
                }),
                message: Some("体验模式已开启".into()),
            }),
        ),
    )
}

// ─── Timezone Settings ───

#[derive(Debug, Deserialize)]
pub struct SetTimezoneRequest {
    pub timezone: String,
}

/// GET /api/settings/timezone
pub async fn get_timezone(State(state): State<AppState>, user_id: UserId) -> impl IntoResponse {
    let db = state.db.lock();
    let tz = db
        .query_row(
            "SELECT timezone FROM user_settings WHERE user_id = ?1",
            [&user_id.0],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| "America/Toronto".to_string());

    Json(serde_json::json!({
        "success": true,
        "timezone": tz,
    }))
}

/// PUT /api/settings/timezone
pub async fn set_timezone(
    State(state): State<AppState>,
    user_id: UserId,
    Json(body): Json<SetTimezoneRequest>,
) -> impl IntoResponse {
    let valid = ["America/Toronto", "America/Vancouver", "Asia/Shanghai"];
    if !valid.contains(&body.timezone.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "message": "无效的时区选择",
            })),
        );
    }

    let db = state.db.lock();
    let now = chrono::Utc::now().to_rfc3339();

    // Upsert into user_settings
    db.execute(
        "INSERT INTO user_settings (user_id, timezone, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(user_id) DO UPDATE SET timezone = ?2, updated_at = ?3",
        rusqlite::params![user_id.0, body.timezone, now],
    )
    .ok();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
        })),
    )
}

// ─── Memory management endpoints ───

/// GET /api/settings/memories — list all memories for current user
pub async fn list_memories(State(state): State<AppState>, user_id: UserId) -> impl IntoResponse {
    let db = state.db.lock();
    let mut stmt = match db.prepare(
        "SELECT id, category, content, created_at, last_accessed_at, access_count FROM ergou_memories WHERE user_id=?1 ORDER BY last_accessed_at DESC",
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[memories] db error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"success": false, "message": "查询失败"})),
            );
        }
    };

    let rows: Vec<serde_json::Value> = match stmt.query_map([&user_id.0], |r| {
        Ok(serde_json::json!({
            "id": r.get::<_, String>(0)?,
            "category": r.get::<_, String>(1)?,
            "content": r.get::<_, String>(2)?,
            "created_at": r.get::<_, String>(3)?,
            "last_accessed_at": r.get::<_, String>(4)?,
            "access_count": r.get::<_, i64>(5)?
        }))
    }) {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            eprintln!("[memories] db error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"success": false, "message": "查询失败"})),
            );
        }
    };

    (
        StatusCode::OK,
        Json(serde_json::json!({"success": true, "memories": rows, "count": rows.len()})),
    )
}

/// DELETE /api/settings/memories/{id} — delete a single memory
pub async fn delete_memory_by_id(
    State(state): State<AppState>,
    user_id: UserId,
    Path(memory_id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.lock();
    match db.execute(
        "DELETE FROM ergou_memories WHERE id=?1 AND user_id=?2",
        rusqlite::params![memory_id, user_id.0],
    ) {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "message": "记忆不存在"})),
        ),
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => {
            eprintln!("[memories] delete error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"success": false, "message": "删除失败"})),
            )
        }
    }
}

/// DELETE /api/settings/memories — delete all memories for current user
pub async fn delete_all_memories(
    State(state): State<AppState>,
    user_id: UserId,
) -> impl IntoResponse {
    let db = state.db.lock();
    match db.execute("DELETE FROM ergou_memories WHERE user_id=?1", [&user_id.0]) {
        Ok(count) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "deleted": count})),
        ),
        Err(e) => {
            eprintln!("[memories] delete all error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"success": false, "message": "清空失败"})),
            )
        }
    }
}

// ─── Helpers ───

pub(crate) fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    use argon2::password_hash::SaltString;
    use argon2::{Argon2, PasswordHasher};
    use rand::RngCore;

    // Generate salt bytes using rand, then encode as SaltString
    let mut salt_bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut salt_bytes);
    let salt = SaltString::encode_b64(&salt_bytes)?;

    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

pub(crate) fn verify_password(password: &str, hash: &str) -> bool {
    use argon2::password_hash::PasswordHash;
    use argon2::{Argon2, PasswordVerifier};

    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

fn generate_session_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn make_session_cookie(token: String) -> Cookie<'static> {
    Cookie::build(("session", token))
        .path("/")
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::days(30))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── validate_password tests ──

    #[test]
    fn test_password_too_short() {
        assert!(validate_password("Ab1").is_err());
        assert!(validate_password("Abcdef1").is_err()); // 7 chars
    }

    #[test]
    fn test_password_no_uppercase() {
        assert!(validate_password("abcdefg1").is_err());
    }

    #[test]
    fn test_password_no_lowercase() {
        assert!(validate_password("ABCDEFG1").is_err());
    }

    #[test]
    fn test_password_no_digit() {
        assert!(validate_password("Abcdefgh").is_err());
    }

    #[test]
    fn test_password_too_long() {
        let long = "A".repeat(100) + &"a".repeat(20) + "1234567890";
        assert!(validate_password(&long).is_err());
    }

    #[test]
    fn test_password_valid() {
        assert!(validate_password("Abcdefg1").is_ok());
        assert!(validate_password("Test1234").is_ok());
        assert!(validate_password("P@ssw0rd").is_ok());
    }

    // ── hash + verify tests ──

    #[test]
    fn test_hash_and_verify_correct() {
        let hash = hash_password("Correct1").expect("hash should succeed");
        assert!(verify_password("Correct1", &hash));
    }

    #[test]
    fn test_hash_and_verify_wrong() {
        let hash = hash_password("Correct1").expect("hash should succeed");
        assert!(!verify_password("Wrong123", &hash));
    }

    // ────── Owner recovery tests (T-096 / ADR-006) ──────
    //
    // 注:OWNER_RECOVERY_KEY 是进程级环境变量,改它会影响整个进程内所有并行测试。
    // 用静态 Mutex 把 recovery 相关测试串行化(只串这一组,不影响其它 tests 并行)。

    use crate::test_helpers::{create_test_user, test_state};
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use serde_json::Value as JsonValue;
    use tower::ServiceExt;

    static RECOVERY_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// 一个合法长度的测试用 key(64-hex,256-bit 熵)。
    const TEST_KEY: &str = "deadbeefcafef00ddeadbeefcafef00ddeadbeefcafef00ddeadbeefcafef00d";

    async fn body_json(resp: axum::response::Response) -> (StatusCode, JsonValue) {
        let status = resp.status();
        let body = to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        let v: JsonValue = serde_json::from_slice(&body).unwrap_or_else(|e| {
            panic!(
                "parse fail status={} body={:?} err={}",
                status,
                String::from_utf8_lossy(&body),
                e
            )
        });
        (status, v)
    }

    /// 推升 owner role(test_helpers 默认创建的是 'user')
    fn promote_to_owner(state: &AppState, user_id: &str) {
        let db = state.db.lock();
        db.execute("UPDATE users SET role = 'owner' WHERE id = ?1", [user_id])
            .expect("promote owner");
    }

    fn recovery_req(ip: &str, body: &str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/api/auth/owner-recovery")
            .header("fly-client-ip", ip)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    #[tokio::test]
    async fn recovery_disabled_without_env_returns_503() {
        let _g = RECOVERY_TEST_LOCK.lock().unwrap();
        std::env::remove_var("OWNER_RECOVERY_KEY");
        let state = test_state();
        let app = crate::build_app(state);
        let resp = app
            .oneshot(recovery_req(
                "10.0.0.1",
                r#"{"recovery_key":"x","username":"a","new_password":"Strong1A"}"#,
            ))
            .await
            .unwrap();
        let (status, body) = body_json(resp).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"], "RECOVERY_DISABLED");
    }

    #[tokio::test]
    async fn recovery_weak_new_password_returns_400() {
        let _g = RECOVERY_TEST_LOCK.lock().unwrap();
        std::env::set_var("OWNER_RECOVERY_KEY", TEST_KEY);
        let state = test_state();
        let (uid, _) = create_test_user(&state, "boris_dev", "OldPass1");
        promote_to_owner(&state, &uid);
        let app = crate::build_app(state);
        let resp = app.oneshot(recovery_req("10.0.0.2",
            r#"{"recovery_key":"deadbeefcafef00ddeadbeefcafef00ddeadbeefcafef00ddeadbeefcafef00d","username":"boris_dev","new_password":"weak"}"#)).await.unwrap();
        let (status, body) = body_json(resp).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "WEAK_PASSWORD");
        std::env::remove_var("OWNER_RECOVERY_KEY");
    }

    #[tokio::test]
    async fn recovery_unknown_username_returns_401_same_as_wrong_key() {
        let _g = RECOVERY_TEST_LOCK.lock().unwrap();
        std::env::set_var("OWNER_RECOVERY_KEY", TEST_KEY);
        let state = test_state();
        let app = crate::build_app(state);
        let resp = app
            .oneshot(recovery_req(
                "10.0.0.3",
                &format!(
                    r#"{{"recovery_key":"{}","username":"nobody","new_password":"NewPass1A"}}"#,
                    TEST_KEY
                ),
            ))
            .await
            .unwrap();
        let (status, body) = body_json(resp).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"], "INVALID_CREDENTIALS");
        // 与"key 错"返回一致(防探测枚举)
        std::env::remove_var("OWNER_RECOVERY_KEY");
    }

    #[tokio::test]
    async fn recovery_non_owner_role_returns_403() {
        let _g = RECOVERY_TEST_LOCK.lock().unwrap();
        std::env::set_var("OWNER_RECOVERY_KEY", TEST_KEY);
        let state = test_state();
        // 默认创建的 user role = 'user',不提升
        let (_uid, _) = create_test_user(&state, "alice", "OldPass1");
        let app = crate::build_app(state);
        let resp = app
            .oneshot(recovery_req(
                "10.0.0.4",
                &format!(
                    r#"{{"recovery_key":"{}","username":"alice","new_password":"NewPass1A"}}"#,
                    TEST_KEY
                ),
            ))
            .await
            .unwrap();
        let (status, body) = body_json(resp).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["error"], "NOT_OWNER");
        std::env::remove_var("OWNER_RECOVERY_KEY");
    }

    #[tokio::test]
    async fn recovery_wrong_key_returns_401() {
        let _g = RECOVERY_TEST_LOCK.lock().unwrap();
        std::env::set_var("OWNER_RECOVERY_KEY", TEST_KEY);
        let state = test_state();
        let (uid, _) = create_test_user(&state, "boris2", "OldPass1");
        promote_to_owner(&state, &uid);
        let app = crate::build_app(state);
        // key 错(长度/内容都不对)
        let resp = app
            .oneshot(recovery_req(
                "10.0.0.5",
                r#"{"recovery_key":"wrong","username":"boris2","new_password":"NewPass1A"}"#,
            ))
            .await
            .unwrap();
        let (status, body) = body_json(resp).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"], "INVALID_CREDENTIALS");
        std::env::remove_var("OWNER_RECOVERY_KEY");
    }

    #[tokio::test]
    async fn recovery_success_resets_password_and_revokes_all_sessions() {
        let _g = RECOVERY_TEST_LOCK.lock().unwrap();
        std::env::set_var("OWNER_RECOVERY_KEY", TEST_KEY);
        let state = test_state();
        let (uid, _token) = create_test_user(&state, "boris3", "OldPass1");
        promote_to_owner(&state, &uid);

        // 给该 owner 再造一个 session,以验证"全部吊销"
        {
            let db = state.db.lock();
            let now = chrono::Utc::now().to_rfc3339();
            let expires = (chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339();
            db.execute(
                "INSERT INTO sessions (token, user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params!["extra_session_token", uid, now, expires],
            ).unwrap();
            let cnt: i64 = db
                .query_row(
                    "SELECT COUNT(*) FROM sessions WHERE user_id=?1",
                    [&uid],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(cnt, 2, "应有 2 个 session(原 + 额外)");
        }

        let app = crate::build_app(state.clone());
        let resp = app
            .oneshot(recovery_req(
                "10.0.0.6",
                &format!(
                    r#"{{"recovery_key":"{}","username":"boris3","new_password":"NewPass1A"}}"#,
                    TEST_KEY
                ),
            ))
            .await
            .unwrap();
        let (status, body) = body_json(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["success"], true);

        // 验证:旧密码失败 + 新密码可登 + sessions 表中该用户行清空
        let db = state.db.lock();
        let new_hash: String = db
            .query_row(
                "SELECT password_hash FROM users WHERE id = ?1",
                [&uid],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!verify_password("OldPass1", &new_hash), "旧密码应失效");
        assert!(verify_password("NewPass1A", &new_hash), "新密码应可用");
        let session_cnt: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE user_id = ?1",
                [&uid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(session_cnt, 0, "所有 session 应被吊销");
        std::env::remove_var("OWNER_RECOVERY_KEY");
    }

    #[tokio::test]
    async fn recovery_same_ip_second_call_within_window_returns_429() {
        let _g = RECOVERY_TEST_LOCK.lock().unwrap();
        std::env::set_var("OWNER_RECOVERY_KEY", TEST_KEY);
        let state = test_state();
        let app = crate::build_app(state);

        // 第 1 次:用户不存在 → 401(但占了 IP 配额 1/1)
        let resp1 = app
            .clone()
            .oneshot(recovery_req(
                "10.0.0.7",
                r#"{"recovery_key":"x","username":"nobody","new_password":"NewPass1A"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(resp1.status(), StatusCode::UNAUTHORIZED);

        // 第 2 次同 IP(立即):应被限流 → 429
        let resp2 = app
            .oneshot(recovery_req(
                "10.0.0.7",
                r#"{"recovery_key":"x","username":"nobody","new_password":"NewPass1A"}"#,
            ))
            .await
            .unwrap();
        let (status, body) = body_json(resp2).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["error"], "RATE_LIMITED");
        std::env::remove_var("OWNER_RECOVERY_KEY");
    }
}
