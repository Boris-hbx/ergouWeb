use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::time::Instant;

use crate::state::AppState;

#[derive(Deserialize)]
pub struct ClientErrorReport {
    pub error_message: String,
    pub stack: Option<String>,
    pub app_version: Option<String>,
    pub url: Option<String>,
    pub user_agent: Option<String>,
    pub screen_size: Option<String>,
    pub network_online: Option<bool>,
    pub user_id: Option<String>,
    pub breadcrumbs: Option<serde_json::Value>,
    pub timestamp: Option<String>,
}

fn extract_client_ip(headers: &HeaderMap) -> String {
    if let Some(val) = headers.get("fly-client-ip") {
        if let Ok(ip) = val.to_str() {
            return ip.trim().to_string();
        }
    }
    if let Some(val) = headers.get("x-forwarded-for") {
        if let Ok(ips) = val.to_str() {
            if let Some(first) = ips.split(',').next() {
                return first.trim().to_string();
            }
        }
    }
    "unknown".to_string()
}

pub async fn report_client_error(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(report): Json<ClientErrorReport>,
) -> impl IntoResponse {
    let ip = extract_client_ip(&headers);

    // IP rate limit: 10 per minute
    {
        let mut limits = state.error_report_limits.lock();
        let entry = limits.entry(ip).or_insert((0, Instant::now()));
        if entry.1.elapsed().as_secs() >= 60 {
            *entry = (1, Instant::now());
        } else {
            entry.0 += 1;
            if entry.0 > 10 {
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(json!({"error": "rate_limited"})),
                )
                    .into_response();
            }
        }
    }

    // Truncate fields to prevent abuse
    let error_message = truncate(&report.error_message, 1000);
    let stack = report.stack.as_deref().map(|s| truncate(s, 4000));
    let app_version = report.app_version.as_deref().map(|s| truncate(s, 50));
    let url = report.url.as_deref().map(|s| truncate(s, 500));
    let user_agent = report.user_agent.as_deref().map(|s| truncate(s, 500));
    let screen_size = report.screen_size.as_deref().map(|s| truncate(s, 20));
    let network_online = report.network_online.unwrap_or(true);
    let user_id = report.user_id.as_deref().map(|s| truncate(s, 100));
    let breadcrumbs = report.breadcrumbs.as_ref().map(|v| v.to_string());
    let now_str = chrono::Utc::now().to_rfc3339();
    let created_at = report.timestamp.as_deref().unwrap_or(&now_str);
    let created_at = truncate(created_at, 50);

    let db = state.db.lock();
    match db.execute(
        "INSERT INTO client_errors (error_message, stack, app_version, url, user_agent, screen_size, network_online, user_id, breadcrumbs, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            error_message,
            stack,
            app_version,
            url,
            user_agent,
            screen_size,
            network_online as i32,
            user_id,
            breadcrumbs,
            created_at,
        ],
    ) {
        Ok(_) => (StatusCode::OK, Json(json!({"success": true}))).into_response(),
        Err(e) => {
            eprintln!("[observability] insert client_error failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false}))).into_response()
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        s[..s.floor_char_boundary(max)].to_string()
    }
}
