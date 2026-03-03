use crate::state::AppState;
use parking_lot::Mutex;
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::Arc;

/// Create an AppState backed by an in-memory SQLite database with full schema.
pub fn test_state() -> AppState {
    let conn = Connection::open_in_memory().expect("Failed to open in-memory DB");
    crate::db::init_connection(&conn);

    AppState {
        db: Arc::new(Mutex::new(conn)),
        db_path: ":memory:".to_string(),
        start_time: std::time::Instant::now(),
        moment_cache: Arc::new(Mutex::new(HashMap::new())),
        login_ip_attempts: Arc::new(Mutex::new(HashMap::new())),
        login_user_lockouts: Arc::new(Mutex::new(HashMap::new())),
        ai_rate_limits: Arc::new(Mutex::new(HashMap::new())),
        guest_ip_rate_limits: Arc::new(Mutex::new(HashMap::new())),
        error_report_limits: Arc::new(Mutex::new(HashMap::new())),
    }
}

/// Create a test user with a real Argon2 password hash and an active session.
/// Returns `(user_id, session_token)`.
pub fn create_test_user(state: &AppState, username: &str, password: &str) -> (String, String) {
    let db = state.db.lock();
    let user_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let password_hash = crate::auth::hash_password(password).expect("hash failed");

    db.execute(
        "INSERT INTO users (id, username, password_hash, display_name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![user_id, username, password_hash, username, now, now],
    )
    .expect("insert user failed");

    let token = hex::encode(&uuid::Uuid::new_v4().to_string().as_bytes()[..16]);
    let expires = (chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339();
    db.execute(
        "INSERT INTO sessions (token, user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![token, user_id, now, expires],
    )
    .expect("insert session failed");

    (user_id, token)
}

/// Create a test user with a specific status (active, pending, rejected).
/// Returns `(user_id, session_token)`.
pub fn create_test_user_with_status(
    state: &AppState,
    username: &str,
    password: &str,
    status: &str,
) -> (String, String) {
    let db = state.db.lock();
    let user_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let password_hash = crate::auth::hash_password(password).expect("hash failed");

    db.execute(
        "INSERT INTO users (id, username, password_hash, display_name, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![user_id, username, password_hash, username, status, now, now],
    )
    .expect("insert user failed");

    let token = hex::encode(&uuid::Uuid::new_v4().to_string().as_bytes()[..16]);
    let expires = (chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339();
    db.execute(
        "INSERT INTO sessions (token, user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![token, user_id, now, expires],
    )
    .expect("insert session failed");

    (user_id, token)
}

/// Create a test user with admin role. Returns `(user_id, session_token)`.
pub fn create_admin_user(state: &AppState, username: &str, password: &str) -> (String, String) {
    let (user_id, token) = create_test_user(state, username, password);
    let db = state.db.lock();
    db.execute("UPDATE users SET role = 'admin' WHERE id = ?1", [&user_id])
        .expect("set admin failed");
    (user_id, token)
}

/// Build a `Cookie: session=<token>` header value string.
pub fn auth_cookie(token: &str) -> String {
    format!("session={}", token)
}
