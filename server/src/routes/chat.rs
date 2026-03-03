use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::auth::{check_guest_ai_quota, ActiveUserId};
use crate::services::{context, llm::LlmClient, tool_executor};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub page_context: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_remaining: Option<i32>,
}

/// POST /api/chat — send message, get SSE stream response
pub async fn chat_handler(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Json(req): Json<ChatRequest>,
) -> impl IntoResponse {
    // Input validation
    let message = req.message.trim().to_string();
    if message.is_empty() || message.len() > 4000 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ChatResponse {
                success: false,
                message: Some("消息不能为空且不超过4000字符".into()),
                conversation_id: None,
                reply: None,
                tool_calls: None,
                ai_remaining: None,
            }),
        )
            .into_response();
    }

    // Guest AI quota check
    let guest_ai_remaining = match check_guest_ai_quota(&state, &user_id.0) {
        Ok(remaining) => Some(remaining),
        Err(err_resp) => return err_resp.into_response(),
    };

    // Rate limiting: 5 per minute per user
    {
        let db = state.db.lock();

        let one_min_ago = (chrono::Utc::now() - chrono::Duration::minutes(1)).to_rfc3339();
        let recent_count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM chat_messages WHERE conversation_id IN (SELECT id FROM conversations WHERE user_id=?1) AND role='user' AND created_at > ?2",
                rusqlite::params![user_id.0, one_min_ago],
                |r| r.get(0),
            )
            .unwrap_or(0);

        if recent_count >= 5 {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(ChatResponse {
                    success: false,
                    message: Some("你发得太快了，歇一会儿".into()),
                    conversation_id: None,
                    reply: None,
                    tool_calls: None,
                    ai_remaining: None,
                }),
            )
                .into_response();
        }
    }

    // Initialize LLM client based on user preference
    let llm = match LlmClient::for_user(&state.db.lock(), &user_id.0) {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ChatResponse {
                    success: false,
                    message: Some("AI 服务未配置".into()),
                    conversation_id: None,
                    reply: None,
                    tool_calls: None,
                    ai_remaining: None,
                }),
            )
                .into_response()
        }
    };

    // Get or create conversation
    let conversation_id;
    let mut history_messages: Vec<serde_json::Value> = Vec::new();

    {
        let db = state.db.lock();

        if let Some(conv_id) = &req.conversation_id {
            // Verify conversation belongs to user
            let exists: bool = db
                .query_row(
                    "SELECT COUNT(*) > 0 FROM conversations WHERE id=?1 AND user_id=?2",
                    rusqlite::params![conv_id, user_id.0],
                    |r| r.get(0),
                )
                .unwrap_or(false);
            if !exists {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ChatResponse {
                        success: false,
                        message: Some("对话不存在".into()),
                        conversation_id: None,
                        reply: None,
                        tool_calls: None,
                        ai_remaining: None,
                    }),
                )
                    .into_response();
            }
            conversation_id = conv_id.clone();

            // Load history (last 20 messages)
            if let Ok(mut stmt) = db.prepare(
                "SELECT role, content_text, content_json FROM chat_messages WHERE conversation_id=?1 ORDER BY sequence DESC LIMIT 20",
            ) {
                if let Ok(rows) = stmt.query_map([&conversation_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                }) {
                    let mut msgs: Vec<serde_json::Value> = Vec::new();
                    for r in rows.flatten() {
                        let (role, content_text, content_json) = r;
                        if let Some(cj) = content_json {
                            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&cj) {
                                msgs.push(json!({"role": role, "content": parsed}));
                                continue;
                            }
                        }
                        if let Some(ct) = content_text {
                            msgs.push(json!({"role": role, "content": ct}));
                        }
                    }
                    msgs.reverse();
                    history_messages = msgs;
                }
            }
        } else {
            // Create new conversation
            conversation_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now().to_rfc3339();
            let title = if message.chars().count() > 30 {
                format!("{}...", message.chars().take(30).collect::<String>())
            } else {
                message.clone()
            };
            db.execute(
                "INSERT INTO conversations (id, user_id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![conversation_id, user_id.0, title, now, now],
            )
            .ok();
        }

        // Save user message
        let msg_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let seq: i64 = db
            .query_row(
                "SELECT COALESCE(MAX(sequence), 0) + 1 FROM chat_messages WHERE conversation_id=?1",
                [&conversation_id],
                |r| r.get(0),
            )
            .unwrap_or(1);
        db.execute(
            "INSERT INTO chat_messages (id, conversation_id, role, content_text, created_at, sequence) VALUES (?1, ?2, 'user', ?3, ?4, ?5)",
            rusqlite::params![msg_id, conversation_id, message, now, seq],
        )
        .ok();
    }

    // Add current message to history
    history_messages.push(json!({"role": "user", "content": message}));

    // Read user timezone
    let user_timezone = {
        let db = state.db.lock();
        db.query_row(
            "SELECT timezone FROM user_settings WHERE user_id = ?1",
            [&user_id.0],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| "America/Toronto".to_string())
    };

    // Build system prompt with page context
    let system_prompt = {
        let db = state.db.lock();
        context::build_system_prompt_with_page(
            &db,
            &user_id.0,
            req.page_context.as_ref(),
            &user_timezone,
        )
    };

    let tools = tool_executor::tool_definitions();

    // Clone state for tool execution
    let tool_state = state.clone();
    let tool_user_id = user_id.0.clone();
    let conv_id_clone = conversation_id.clone();
    let start = std::time::Instant::now();

    // Call LLM with tool use loop
    let result = llm
        .chat(&system_prompt, history_messages, &tools, |name, input| {
            let db = tool_state.db.lock();
            tool_executor::execute_tool(&db, &tool_user_id, name, input)
        })
        .await;

    let latency_ms = start.elapsed().as_millis() as i64;

    match result {
        Ok(chat_result) => {
            // Save assistant response
            let db = state.db.lock();
            let msg_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now().to_rfc3339();
            let seq: i64 = db
                .query_row(
                    "SELECT COALESCE(MAX(sequence), 0) + 1 FROM chat_messages WHERE conversation_id=?1",
                    [&conv_id_clone],
                    |r| r.get(0),
                )
                .unwrap_or(1);
            db.execute(
                "INSERT INTO chat_messages (id, conversation_id, role, content_text, token_count, created_at, sequence) VALUES (?1, ?2, 'assistant', ?3, ?4, ?5, ?6)",
                rusqlite::params![msg_id, conv_id_clone, chat_result.text, chat_result.output_tokens, now, seq],
            )
            .ok();

            // Update conversation timestamp
            db.execute(
                "UPDATE conversations SET updated_at=?1 WHERE id=?2",
                rusqlite::params![now, conv_id_clone],
            )
            .ok();

            // Log usage
            let usage_id = uuid::Uuid::new_v4().to_string();
            db.execute(
                "INSERT INTO chat_usage_log (id, user_id, conversation_id, model, input_tokens, output_tokens, tool_calls, latency_ms, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![usage_id, tool_user_id, conv_id_clone, "claude-sonnet-4-6-20250514", chat_result.input_tokens, chat_result.output_tokens, chat_result.tool_calls.len() as i64, latency_ms, now],
            )
            .ok();

            let tool_info: Vec<serde_json::Value> = chat_result
                .tool_calls
                .iter()
                .map(|(name, _input, result)| json!({"tool": name, "result": result}))
                .collect();

            (
                StatusCode::OK,
                Json(ChatResponse {
                    success: true,
                    message: None,
                    conversation_id: Some(conv_id_clone),
                    reply: Some(chat_result.text),
                    tool_calls: if tool_info.is_empty() {
                        None
                    } else {
                        Some(tool_info)
                    },
                    ai_remaining: if guest_ai_remaining.unwrap_or(999) < 999 {
                        guest_ai_remaining
                    } else {
                        None
                    },
                }),
            )
                .into_response()
        }
        Err(err) => (
            StatusCode::OK,
            Json(ChatResponse {
                success: false,
                message: Some(err),
                conversation_id: Some(conv_id_clone),
                reply: None,
                tool_calls: None,
                ai_remaining: None,
            }),
        )
            .into_response(),
    }
}
