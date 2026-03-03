use parking_lot::Mutex;
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

pub type MomentCache = HashMap<String, (Vec<String>, chrono::NaiveDate)>;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    /// Cache for moment pool: user_id -> (pool, date)
    pub moment_cache: Arc<Mutex<MomentCache>>,
    /// Login rate limiting: IP -> (attempt_count, window_start)
    pub login_ip_attempts: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
    /// Login user lockout: username -> (failed_count, last_failure)
    pub login_user_lockouts: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
    /// AI scenario generation rate limiting: user_id -> last_generation_time
    pub ai_rate_limits: Arc<Mutex<HashMap<String, Instant>>>,
    /// Guest login rate limiting: IP -> (count, window_start)
    pub guest_ip_rate_limits: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
}
