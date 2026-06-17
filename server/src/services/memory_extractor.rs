use serde::Deserialize;
use serde_json::Value;
use tracing::{info, warn};

use crate::state::AppState;

const MAX_MEMORIES_PER_USER: i64 = 100;
const VALID_CATEGORIES: [&str; 4] = ["fact", "habit", "personality", "intent"];

#[derive(Debug, Deserialize)]
struct ExtractionResult {
    #[serde(default)]
    memories: Vec<ExtractedMemory>,
}

#[derive(Debug, Deserialize)]
struct ExtractedMemory {
    category: String,
    content: String,
    #[serde(default = "default_importance")]
    importance: i64,
}

fn default_importance() -> i64 {
    3
}

/// Async entry point: called via tokio::spawn after chat completes.
/// Extracts memorable facts from the conversation and saves them.
///
/// Trigger conditions (T-077):
/// - user message ≥ 20 chars OR assistant reply ≥ 100 chars
/// - same conversation: at most 1 extraction per 10 minutes
pub async fn extract_after_chat(
    state: &AppState,
    user_id: &str,
    conversation_id: &str,
    user_message: &str,
    assistant_reply: &str,
) {
    // Threshold check: skip trivial exchanges
    let user_len = user_message.chars().count();
    let reply_len = assistant_reply.chars().count();
    if user_len < 20 && reply_len < 100 {
        return;
    }

    // Per-conversation rate limit: 1 extraction per 10 minutes
    let recent: i64 = {
        let db = state.db.lock();
        let ten_min_ago = (chrono::Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
        db.query_row(
            "SELECT COUNT(*) FROM memory_extraction_log WHERE conversation_id = ?1 AND created_at > ?2",
            rusqlite::params![conversation_id, ten_min_ago],
            |r| r.get(0),
        )
        .unwrap_or(0)
    };
    if recent > 0 {
        return;
    }

    match run_extraction(
        state,
        user_id,
        conversation_id,
        user_message,
        assistant_reply,
    )
    .await
    {
        Ok(count) => {
            if count > 0 {
                info!(
                    "[memory-extractor] extracted {} memories for user {} (conv {})",
                    count, user_id, conversation_id
                );
            }
        }
        Err(e) => {
            warn!(
                "[memory-extractor] extraction failed for user {}: {}",
                user_id, e
            );
        }
    }
}

async fn run_extraction(
    state: &AppState,
    user_id: &str,
    conversation_id: &str,
    user_message: &str,
    assistant_reply: &str,
) -> Result<usize, String> {
    // Log the extraction attempt FIRST so the rate limit holds even if LLM fails.
    // We update extracted_count after successful extraction below.
    let log_id: i64 = {
        let db = state.db.lock();
        let now = chrono::Utc::now().to_rfc3339();
        db.execute(
            "INSERT INTO memory_extraction_log (user_id, conversation_id, extracted_count, created_at) VALUES (?1, ?2, 0, ?3)",
            rusqlite::params![user_id, conversation_id, now],
        )
        .map_err(|e| format!("log insert error: {e}"))?;
        db.last_insert_rowid()
    };

    let prompt = format!(
        r#"分析以下对话，提取用户透露的重要个人信息。

规则：
- 只提取用户明确说出的事实，不要推测
- 忽略寒暄和泛泛而谈的内容
- 如果没有值得记住的信息，返回空数组
- 每条记忆要简洁（<20字）

分类说明：
- fact: 客观事实（生日、过敏、地址等）
- habit: 行为习惯（作息、饮食偏好等）
- personality: 性格特征（喜好、厌恶等）
- intent: 意图计划（想做什么、目标等）

用户: {user_message}
二狗: {assistant_reply}

仅返回 JSON，无其他文字：
{{"memories":[{{"category":"fact","content":"简短描述","importance":3}}]}}"#
    );

    let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| "No API key".to_string())?;
    let http = reqwest::Client::new();
    let resp = http
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "model": "claude-haiku-4-5-20251001",
            "max_tokens": 512,
            "temperature": 0.1,
            "messages": [{"role": "user", "content": prompt}]
        }))
        .send()
        .await
        .map_err(|e| format!("HTTP error: {e}"))?;

    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {e}"))?;

    let text = body["content"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|block| block["text"].as_str())
        .unwrap_or("{}");

    // Strip markdown wrapping if present
    let json_str = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let parsed: ExtractionResult =
        serde_json::from_str(json_str).map_err(|e| format!("Parse error: {e}"))?;

    if parsed.memories.is_empty() {
        return Ok(0);
    }

    let db = state.db.lock();
    let now = chrono::Utc::now().to_rfc3339();
    let mut saved = 0usize;

    for mem in &parsed.memories {
        let content = mem.content.trim();

        // Validate
        if content.is_empty() || content.chars().count() > 500 {
            continue;
        }
        if !VALID_CATEGORIES.contains(&mem.category.as_str()) {
            continue;
        }

        let importance = mem.importance.clamp(1, 5);

        // Dedup
        let exists: bool = db
            .query_row(
                "SELECT COUNT(*) > 0 FROM ergou_memories WHERE user_id=?1 AND content=?2",
                rusqlite::params![user_id, content],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if exists {
            continue;
        }

        // Auto-evict if at capacity
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM ergou_memories WHERE user_id=?1",
                [user_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if count >= MAX_MEMORIES_PER_USER {
            db.execute(
                "DELETE FROM ergou_memories WHERE id = (SELECT id FROM ergou_memories WHERE user_id=?1 ORDER BY access_count ASC, last_accessed_at ASC LIMIT 1)",
                [user_id],
            )
            .ok();
        }

        // Insert
        let id = format!("mem_{}", &uuid::Uuid::new_v4().to_string()[..12]);
        if db
            .execute(
                "INSERT INTO ergou_memories (id, user_id, category, content, importance, created_at, last_accessed_at, access_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
                rusqlite::params![id, user_id, mem.category, content, importance, now, now],
            )
            .is_ok()
        {
            saved += 1;
        }
    }

    // Update the rate-limit log row inserted at the start with the actual count.
    db.execute(
        "UPDATE memory_extraction_log SET extracted_count = ?1 WHERE id = ?2",
        rusqlite::params![saved as i64, log_id],
    )
    .ok();

    Ok(saved)
}
