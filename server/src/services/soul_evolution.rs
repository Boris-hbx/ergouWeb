use serde_json::Value;

use crate::models::soul_state::clamp_field;
use crate::routes::soul_state::ensure_soul_state;
use crate::state::AppState;

const MAX_DAILY_EVOLUTIONS: i64 = 10;
const MAX_DELTA: f64 = 0.05;

/// Relationship thresholds: (interactions, stage_name)
const THRESHOLDS: &[(i64, &str)] = &[
    (500, "intimate"),
    (150, "close"),
    (50, "familiar"),
    (10, "acquaintance"),
];

/// Async entry point: called via tokio::spawn after chat completes.
pub async fn evolve_after_chat(state: &AppState, user_id: &str, messages: &[Value]) {
    // Step 1: Increment total_interactions
    {
        let db = state.db.lock();
        let now = chrono::Utc::now().to_rfc3339();
        db.execute(
            "UPDATE soul_states SET total_interactions = total_interactions + 1, updated_at = ?1 WHERE user_id = ?2",
            rusqlite::params![now, user_id],
        )
        .ok();
    }

    // Step 2: Check daily evolution count
    let daily_count: i64 = {
        let db = state.db.lock();
        db.query_row(
            "SELECT COUNT(*) FROM soul_evolution_log WHERE user_id = ?1 AND trigger_type = 'auto' AND created_at >= date('now')",
            [user_id],
            |r| r.get(0),
        )
        .unwrap_or(0)
    };

    // Step 3: If under limit, run LLM evolution analysis
    if daily_count < MAX_DAILY_EVOLUTIONS {
        if let Err(e) = run_evolution_analysis(state, user_id, messages).await {
            eprintln!("[soul-evolution] LLM analysis failed for {}: {}", user_id, e);
        }
    }

    // Step 4: Check relationship stage upgrade
    {
        let db = state.db.lock();
        let soul = ensure_soul_state(&db, user_id);
        let new_stage = determine_stage(soul.total_interactions);
        if new_stage != soul.relationship_stage {
            let now = chrono::Utc::now().to_rfc3339();
            db.execute(
                "UPDATE soul_states SET relationship_stage = ?1, updated_at = ?2 WHERE user_id = ?3",
                rusqlite::params![new_stage, now, user_id],
            )
            .ok();
            // Log stage change
            let stage_val = |s: &str| -> f64 {
                match s {
                    "stranger" => 0.0,
                    "acquaintance" => 1.0,
                    "familiar" => 2.0,
                    "close" => 3.0,
                    "intimate" => 4.0,
                    _ => 0.0,
                }
            };
            db.execute(
                "INSERT INTO soul_evolution_log (user_id, parameter, old_value, new_value, trigger_type, created_at) VALUES (?1, 'relationship_stage', ?2, ?3, 'auto', ?4)",
                rusqlite::params![user_id, stage_val(&soul.relationship_stage), stage_val(new_stage), now],
            )
            .ok();
        }
    }
}

fn determine_stage(total: i64) -> &'static str {
    for &(threshold, stage) in THRESHOLDS {
        if total >= threshold {
            return stage;
        }
    }
    "stranger"
}

/// Call LLM to analyze conversation and adjust soul parameters.
async fn run_evolution_analysis(
    state: &AppState,
    user_id: &str,
    messages: &[Value],
) -> Result<(), String> {
    // Build a summary of recent messages (last 6 turns max)
    let recent: Vec<String> = messages
        .iter()
        .rev()
        .take(6)
        .filter_map(|m| {
            let role = m.get("role")?.as_str()?;
            let content = m.get("content")?.as_str()?;
            Some(format!("{}: {}", role, &content[..content.len().min(200)]))
        })
        .collect();

    if recent.is_empty() {
        return Ok(());
    }

    let conversation_text = recent.into_iter().rev().collect::<Vec<_>>().join("\n");

    let analysis_prompt = format!(
        r#"分析以下对话，判断用户与二狗的互动是否应调整以下性格参数。
仅返回需要调整的参数及方向（"+" 或 "-"），幅度由系统控制。
参数：classical_ratio, warmth_level, verbosity_level, proactivity_level, trust_level
对话内容：
{conversation_text}
仅返回 JSON，无其他文字：{{"adjustments": {{"warmth_level": "+", "trust_level": "+"}}}}"#
    );

    // Use Claude API directly (lightweight call)
    let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| "No API key".to_string())?;
    let http = reqwest::Client::new();
    let resp = http
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "model": "claude-haiku-4-5-20251001",
            "max_tokens": 200,
            "messages": [{"role": "user", "content": analysis_prompt}]
        }))
        .send()
        .await
        .map_err(|e| format!("HTTP error: {e}"))?;

    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {e}"))?;

    // Extract text from Claude response
    let text = body["content"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|block| block["text"].as_str())
        .unwrap_or("{}");

    // Try to parse JSON from the response (may have markdown wrapping)
    let json_str = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let parsed: Value = serde_json::from_str(json_str).map_err(|e| format!("Parse error: {e}"))?;

    let adjustments = match parsed.get("adjustments").and_then(|a| a.as_object()) {
        Some(a) => a,
        None => return Ok(()), // No adjustments needed
    };

    if adjustments.is_empty() {
        return Ok(());
    }

    // Apply adjustments
    let db = state.db.lock();
    let soul = ensure_soul_state(&db, user_id);
    let now = chrono::Utc::now().to_rfc3339();

    let params: Vec<(&str, f64)> = vec![
        ("classical_ratio", soul.classical_ratio),
        ("warmth_level", soul.warmth_level),
        ("verbosity_level", soul.verbosity_level),
        ("proactivity_level", soul.proactivity_level),
        ("trust_level", soul.trust_level),
    ];

    for (name, current_val) in &params {
        if let Some(direction) = adjustments.get(*name).and_then(|v| v.as_str()) {
            let delta = match direction {
                "+" => MAX_DELTA,
                "-" => -MAX_DELTA,
                _ => continue,
            };
            let new_val = clamp_field(name, current_val + delta);
            if (new_val - current_val).abs() > f64::EPSILON {
                // Update the field
                let sql = format!(
                    "UPDATE soul_states SET {} = ?1, updated_at = ?2 WHERE user_id = ?3",
                    name
                );
                db.execute(&sql, rusqlite::params![new_val, now, user_id])
                    .ok();
                // Log
                db.execute(
                    "INSERT INTO soul_evolution_log (user_id, parameter, old_value, new_value, trigger_type, created_at) VALUES (?1, ?2, ?3, ?4, 'auto', ?5)",
                    rusqlite::params![user_id, name, current_val, new_val, now],
                )
                .ok();
            }
        }
    }

    Ok(())
}
