use axum::{extract::State, response::IntoResponse, Json};
use serde_json::json;

use crate::auth::{check_guest_ai_quota, UserId};
use crate::services::{context, llm::LlmClient};
use crate::state::AppState;

/// GET /api/moment — get a pool of one-liners from 二狗 for the header
pub async fn get_moment(State(state): State<AppState>, user_id: UserId) -> impl IntoResponse {
    let uid = user_id.0;

    // Read user timezone
    let user_timezone = {
        let db = state.db.lock();
        db.query_row(
            "SELECT timezone FROM user_settings WHERE user_id = ?1",
            [&uid],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| "America/Toronto".to_string())
    };

    let tz = context::parse_tz(&user_timezone);
    let today = chrono::Utc::now().with_timezone(&tz).date_naive();

    // Check cache — hit if same date
    {
        let cache = state.moment_cache.lock();
        if let Some((pool, cached_date)) = cache.get(&uid) {
            if *cached_date == today {
                return Json(json!({
                    "success": true,
                    "pool": pool,
                    "text": pool.first().unwrap_or(&String::new()),
                    "generated_at": today.format("%Y-%m-%d").to_string(),
                    "cached": true,
                }));
            }
        }
    }

    // Guest AI quota check
    let ai_remaining = match check_guest_ai_quota(&state, &uid) {
        Ok(remaining) => Some(remaining),
        Err(_) => {
            // Quota exhausted — use fallback pool
            let moment_ctx = {
                let db = state.db.lock();
                context::build_moment_context(&db, &uid, &user_timezone)
            };
            let pool = fallback_pool(moment_ctx.hour);
            return Json(json!({
                "success": true,
                "pool": pool,
                "text": pool.first().unwrap_or(&String::new()),
                "generated_at": today.format("%Y-%m-%d").to_string(),
                "cached": false,
                "fallback": true,
                "ai_remaining": 0
            }));
        }
    };

    // Build context from DB
    let moment_ctx = {
        let db = state.db.lock();
        context::build_moment_context(&db, &uid, &user_timezone)
    };

    let system_prompt = context::build_moment_system_prompt();
    let user_message = context::build_moment_user_message(&moment_ctx);

    // Try LLM — generate pool
    let llm_client = LlmClient::for_user(&state.db.lock(), &uid);
    let (pool, is_fallback) = match llm_client {
        Some(client) => {
            match client
                .simple_generate(system_prompt, &user_message, 1500)
                .await
            {
                Ok(raw) => {
                    let parsed = parse_pool_response(&raw);
                    if parsed.len() >= 5 {
                        (parsed, false)
                    } else {
                        eprintln!(
                            "[Moment] Parsed only {} items, using fallback",
                            parsed.len()
                        );
                        (fallback_pool(moment_ctx.hour), true)
                    }
                }
                Err(e) => {
                    eprintln!("[Moment] LLM error: {}", e);
                    (fallback_pool(moment_ctx.hour), true)
                }
            }
        }
        None => (fallback_pool(moment_ctx.hour), true),
    };

    // Store in cache
    {
        let mut cache = state.moment_cache.lock();
        cache.insert(uid, (pool.clone(), today));
    }

    let mut resp = json!({
        "success": true,
        "pool": pool,
        "text": pool.first().unwrap_or(&String::new()),
        "generated_at": today.format("%Y-%m-%d").to_string(),
        "cached": false,
    });
    if is_fallback {
        resp["fallback"] = json!(true);
    }
    if let Some(remaining) = ai_remaining {
        if remaining < 999 {
            resp["ai_remaining"] = json!(remaining);
        }
    }
    Json(resp)
}

/// Parse LLM response into a Vec of strings.
/// Three-layer fallback: JSON array → regex extract → line-by-line.
fn parse_pool_response(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();

    // Layer 1: direct JSON parse
    if let Ok(arr) = serde_json::from_str::<Vec<String>>(trimmed) {
        return arr
            .into_iter()
            .map(|s| truncate_moment(&s))
            .filter(|s| !s.is_empty())
            .collect();
    }

    // Layer 2: regex extract [...] then parse
    if let Some(start) = trimmed.find('[') {
        if let Some(end) = trimmed.rfind(']') {
            let slice = &trimmed[start..=end];
            if let Ok(arr) = serde_json::from_str::<Vec<String>>(slice) {
                return arr
                    .into_iter()
                    .map(|s| truncate_moment(&s))
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }
    }

    // Layer 3: line-by-line extraction
    trimmed
        .lines()
        .map(|line| {
            line.trim()
                .trim_matches(|c: char| c == '"' || c == ',' || c == '[' || c == ']')
                .trim()
                .to_string()
        })
        .map(|s| truncate_moment(&s))
        .filter(|s| !s.is_empty() && s.len() > 1)
        .collect()
}

/// Hard-truncate to ~20 CJK chars, on char boundary
fn truncate_moment(s: &str) -> String {
    let trimmed = s.trim().trim_matches('"');
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() <= 20 {
        return trimmed.to_string();
    }
    chars[..18].iter().collect::<String>() + "…"
}

/// Fallback pool — philosophical short quotes
fn fallback_pool(_hour: u32) -> Vec<String> {
    vec![
        "先完成，再完美",
        "方向比速度重要",
        "想太多不如动一下",
        "做完一件再想下一件",
        "少即是多",
        "别跟自己较劲",
        "今天比昨天强就行",
        "千里之行，始于足下",
        "拖延不会让事情消失",
        "专注于重要的事情",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}
