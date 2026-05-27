use parking_lot::Mutex;
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

pub type MomentCache = HashMap<String, (Vec<String>, chrono::NaiveDate)>;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    /// Path to the SQLite database file (for system status reporting)
    pub db_path: String,
    /// Server start time (for uptime calculation)
    pub start_time: Instant,
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
    /// Client error report rate limiting: IP -> (count, window_start)
    pub error_report_limits: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
    /// Owner recovery (T-096): IP -> (attempts_in_window, window_start)
    /// Independent from login_ip_attempts so attackers can't burn it through normal logins.
    pub recovery_ip_attempts: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
    /// Owner recovery (T-096): IP -> (consecutive_failures, last_failure)
    /// 5 wrong keys (or unknown username) → IP locked out 1 hour.
    pub recovery_ip_lockouts: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
    /// Admin rate limit (T-089): user_id -> (req_count_in_window, window_start)
    /// 30 admin operations / minute / user. Prevents brute-force enumeration & DoS.
    pub admin_user_rate_limits: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
    /// Guest AI aggregate rate limit (T-089): IP -> (calls_in_window, window_start)
    /// Caps total AI calls across all guests from same IP at 21/hour (closes
    /// the "5 guests × 21 calls = 105 free Claude calls" loophole).
    pub guest_ai_ip_aggregate: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
}
