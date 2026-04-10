use std::convert::Infallible;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    Json,
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::auth::{check_guest_ai_quota, ActiveUserId};
use crate::services::{
    context,
    llm::{LlmClient, SseEvent},
    tool_executor,
};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ChatImage {
    pub data: String,
    pub media_type: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub page_context: Option<serde_json::Value>,
    #[serde(default)]
    pub images: Option<Vec<ChatImage>>,
}

const ALLOWED_IMAGE_TYPES: &[&str] = &["image/jpeg", "image/png", "image/webp", "image/gif"];
const MAX_IMAGES: usize = 3;
const MAX_IMAGE_SIZE: usize = 5 * 1024 * 1024; // 5MB raw ≈ 6.7MB base64

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
    pub message_id: Option<String>,
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
                message_id: None,
                tool_calls: None,
                ai_remaining: None,
            }),
        )
            .into_response();
    }

    // Validate images
    let images = req.images.unwrap_or_default();
    if images.len() > MAX_IMAGES {
        return (
            StatusCode::BAD_REQUEST,
            Json(ChatResponse {
                success: false,
                message: Some(format!("最多上传{}张图片", MAX_IMAGES)),
                conversation_id: None,
                reply: None,
                message_id: None,
                tool_calls: None,
                ai_remaining: None,
            }),
        )
            .into_response();
    }
    for img in &images {
        if !ALLOWED_IMAGE_TYPES.contains(&img.media_type.as_str()) {
            return (
                StatusCode::BAD_REQUEST,
                Json(ChatResponse {
                    success: false,
                    message: Some(format!("不支持的图片类型: {}", img.media_type)),
                    conversation_id: None,
                    reply: None,
                    message_id: None,
                    tool_calls: None,
                    ai_remaining: None,
                }),
            )
                .into_response();
        }
        // base64 size is ~4/3 of raw, so check decoded size
        let raw_size = img.data.len() * 3 / 4;
        if raw_size > MAX_IMAGE_SIZE {
            return (
                StatusCode::BAD_REQUEST,
                Json(ChatResponse {
                    success: false,
                    message: Some("单张图片不能超过5MB".into()),
                    conversation_id: None,
                    reply: None,
                    message_id: None,
                    tool_calls: None,
                    ai_remaining: None,
                }),
            )
                .into_response();
        }
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
                    message_id: None,
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
                    message_id: None,
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
                        message_id: None,
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
        let image_uris_val: Option<String> = if images.is_empty() {
            None
        } else {
            Some(format!("[{}张图片]", images.len()))
        };
        db.execute(
            "INSERT INTO chat_messages (id, conversation_id, role, content_text, image_uris, created_at, sequence) VALUES (?1, ?2, 'user', ?3, ?4, ?5, ?6)",
            rusqlite::params![msg_id, conversation_id, message, image_uris_val, now, seq],
        )
        .ok();
    }

    // Add current message to history (with images if present)
    let user_content = build_user_content(&message, &images);
    history_messages.push(json!({"role": "user", "content": user_content}));

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

    // Clone messages for soul evolution (before LLM takes ownership)
    let evo_messages = history_messages.clone();

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
                rusqlite::params![usage_id, tool_user_id, conv_id_clone, "claude", chat_result.input_tokens, chat_result.output_tokens, chat_result.tool_calls.len() as i64, latency_ms, now],
            )
            .ok();

            // Trigger soul evolution asynchronously
            drop(db); // release lock before spawn
            {
                let evo_state = state.clone();
                let evo_user_id = tool_user_id.clone();
                tokio::spawn(async move {
                    crate::services::soul_evolution::evolve_after_chat(
                        &evo_state,
                        &evo_user_id,
                        &evo_messages,
                    )
                    .await;
                });
            }

            // Trigger memory extraction asynchronously
            {
                let mem_state = state.clone();
                let mem_user_id = tool_user_id.clone();
                let mem_conv_id = conv_id_clone.clone();
                let mem_user_msg = message.clone();
                let mem_reply = chat_result.text.clone();
                tokio::spawn(async move {
                    crate::services::memory_extractor::extract_after_chat(
                        &mem_state,
                        &mem_user_id,
                        &mem_conv_id,
                        &mem_user_msg,
                        &mem_reply,
                    )
                    .await;
                });
            }

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
                    message_id: Some(msg_id),
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
                message_id: None,
                tool_calls: None,
                ai_remaining: None,
            }),
        )
            .into_response(),
    }
}

/// POST /api/chat/stream — SSE streaming chat endpoint
pub async fn chat_stream_handler(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Json(req): Json<ChatRequest>,
) -> impl IntoResponse {
    // Input validation
    let message = req.message.trim().to_string();
    if message.is_empty() || message.len() > 4000 {
        return Sse::new(futures::stream::once(async {
            Ok::<_, Infallible>(
                Event::default()
                    .event("error")
                    .json_data(json!({"type": "error", "message": "消息不能为空且不超过4000字符"}))
                    .unwrap(),
            )
        }))
        .keep_alive(KeepAlive::default())
        .into_response();
    }

    // Validate images
    let images = req.images.unwrap_or_default();
    if images.len() > MAX_IMAGES {
        return make_sse_error("最多上传3张图片").into_response();
    }
    for img in &images {
        if !ALLOWED_IMAGE_TYPES.contains(&img.media_type.as_str()) {
            return make_sse_error(&format!("不支持的图片类型: {}", img.media_type))
                .into_response();
        }
        let raw_size = img.data.len() * 3 / 4;
        if raw_size > MAX_IMAGE_SIZE {
            return make_sse_error("单张图片不能超过5MB").into_response();
        }
    }

    // Guest AI quota check
    let _guest_ai_remaining = match check_guest_ai_quota(&state, &user_id.0) {
        Ok(remaining) => Some(remaining),
        Err(_) => {
            return Sse::new(futures::stream::once(async {
                Ok::<_, Infallible>(
                    Event::default()
                        .event("error")
                        .json_data(json!({"type": "error", "message": "AI 使用次数已用完"}))
                        .unwrap(),
                )
            }))
            .keep_alive(KeepAlive::default())
            .into_response();
        }
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
            return Sse::new(futures::stream::once(async {
                Ok::<_, Infallible>(
                    Event::default()
                        .event("error")
                        .json_data(json!({"type": "error", "message": "你发得太快了，歇一会儿"}))
                        .unwrap(),
                )
            }))
            .keep_alive(KeepAlive::default())
            .into_response();
        }
    }

    // Initialize LLM client
    let llm = match LlmClient::for_user(&state.db.lock(), &user_id.0) {
        Some(c) => c,
        None => {
            return Sse::new(futures::stream::once(async {
                Ok::<_, Infallible>(
                    Event::default()
                        .event("error")
                        .json_data(json!({"type": "error", "message": "AI 服务未配置"}))
                        .unwrap(),
                )
            }))
            .keep_alive(KeepAlive::default())
            .into_response();
        }
    };

    // Get or create conversation
    let conversation_id;
    let mut history_messages: Vec<serde_json::Value> = Vec::new();

    {
        let db = state.db.lock();

        if let Some(conv_id) = &req.conversation_id {
            let exists: bool = db
                .query_row(
                    "SELECT COUNT(*) > 0 FROM conversations WHERE id=?1 AND user_id=?2",
                    rusqlite::params![conv_id, user_id.0],
                    |r| r.get(0),
                )
                .unwrap_or(false);
            if !exists {
                return Sse::new(futures::stream::once(async {
                    Ok::<_, Infallible>(
                        Event::default()
                            .event("error")
                            .json_data(json!({"type": "error", "message": "对话不存在"}))
                            .unwrap(),
                    )
                }))
                .keep_alive(KeepAlive::default())
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
        let image_uris_val: Option<String> = if images.is_empty() {
            None
        } else {
            Some(format!("[{}张图片]", images.len()))
        };
        db.execute(
            "INSERT INTO chat_messages (id, conversation_id, role, content_text, image_uris, created_at, sequence) VALUES (?1, ?2, 'user', ?3, ?4, ?5, ?6)",
            rusqlite::params![msg_id, conversation_id, message, image_uris_val, now, seq],
        )
        .ok();
    }

    // Add current message to history (with images if present)
    let user_content = build_user_content(&message, &images);
    history_messages.push(json!({"role": "user", "content": user_content}));

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

    // Build system prompt
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
    let tool_state = state.clone();
    let tool_user_id = user_id.0.clone();
    let conv_id_clone = conversation_id.clone();
    let start = std::time::Instant::now();
    let evo_messages = history_messages.clone();
    let message_clone = message.clone();

    // Create channel for SSE events
    let (tx, rx) = tokio::sync::mpsc::channel::<SseEvent>(64);

    // Spawn the LLM streaming task
    tokio::spawn(async move {
        let chat_result = llm
            .chat_stream(
                &system_prompt,
                history_messages,
                &tools,
                |name, input| {
                    let db = tool_state.db.lock();
                    tool_executor::execute_tool(&db, &tool_user_id, name, input)
                },
                tx.clone(),
            )
            .await;

        let latency_ms = start.elapsed().as_millis() as i64;

        // Save assistant response & log usage
        {
            let db = tool_state.db.lock();
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

            db.execute(
                "UPDATE conversations SET updated_at=?1 WHERE id=?2",
                rusqlite::params![now, conv_id_clone],
            )
            .ok();

            let usage_id = uuid::Uuid::new_v4().to_string();
            db.execute(
                "INSERT INTO chat_usage_log (id, user_id, conversation_id, model, input_tokens, output_tokens, tool_calls, latency_ms, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![usage_id, tool_user_id, conv_id_clone, "claude", chat_result.input_tokens, chat_result.output_tokens, chat_result.tool_calls.len() as i64, latency_ms, now],
            )
            .ok();
        }

        // Send done event
        let tool_info: Vec<serde_json::Value> = chat_result
            .tool_calls
            .iter()
            .map(|(name, _input, result)| json!({"tool": name, "result": result}))
            .collect();

        let _ = tx
            .send(SseEvent::Done {
                conversation_id: conv_id_clone.clone(),
                full_text: chat_result.text.clone(),
                tool_calls: tool_info,
                input_tokens: chat_result.input_tokens,
                output_tokens: chat_result.output_tokens,
            })
            .await;

        // Trigger soul evolution asynchronously
        {
            let evo_state = tool_state.clone();
            let evo_user_id = tool_user_id.clone();
            tokio::spawn(async move {
                crate::services::soul_evolution::evolve_after_chat(
                    &evo_state,
                    &evo_user_id,
                    &evo_messages,
                )
                .await;
            });
        }

        // Trigger memory extraction asynchronously
        {
            let mem_state = tool_state.clone();
            let mem_user_id = tool_user_id.clone();
            let mem_conv_id = conv_id_clone.clone();
            let mem_reply = chat_result.text.clone();
            tokio::spawn(async move {
                crate::services::memory_extractor::extract_after_chat(
                    &mem_state,
                    &mem_user_id,
                    &mem_conv_id,
                    &message_clone,
                    &mem_reply,
                )
                .await;
            });
        }
    });

    // Convert mpsc receiver to SSE stream
    let stream = make_sse_stream(rx);

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

// ─── Feedback ───

#[derive(Debug, Deserialize)]
pub struct FeedbackRequest {
    pub feedback: Option<i64>, // 1=like, -1=dislike, null=clear
}

/// PUT /api/chat/messages/:id/feedback
pub async fn message_feedback_handler(
    State(state): State<AppState>,
    user_id: ActiveUserId,
    Path(message_id): Path<String>,
    Json(req): Json<FeedbackRequest>,
) -> impl IntoResponse {
    // Validate feedback value
    if let Some(v) = req.feedback {
        if v != 1 && v != -1 {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "feedback 只能是 1, -1 或 null"})),
            );
        }
    }

    let db = state.db.lock();

    // Verify message belongs to user's conversation
    let owns: bool = db
        .query_row(
            "SELECT COUNT(*) > 0 FROM chat_messages m JOIN conversations c ON m.conversation_id = c.id WHERE m.id = ?1 AND c.user_id = ?2",
            rusqlite::params![message_id, user_id.0],
            |r| r.get(0),
        )
        .unwrap_or(false);

    if !owns {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "消息不存在"})),
        );
    }

    db.execute(
        "UPDATE chat_messages SET feedback = ?1 WHERE id = ?2",
        rusqlite::params![req.feedback, message_id],
    )
    .ok();

    (
        StatusCode::OK,
        Json(json!({"success": true})),
    )
}

/// Helper: build a single-event SSE error response
fn make_sse_error(msg: &str) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let payload = json!({"type": "error", "message": msg});
    Sse::new(futures::stream::once(async move {
        Ok::<_, Infallible>(
            Event::default()
                .event("error")
                .json_data(payload)
                .unwrap(),
        )
    }))
    .keep_alive(KeepAlive::default())
}

/// Build Claude API content array for a user message (text + optional images)
fn build_user_content(message: &str, images: &[ChatImage]) -> serde_json::Value {
    if images.is_empty() {
        return json!(message);
    }
    let mut content = Vec::new();
    for img in images {
        content.push(json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": img.media_type,
                "data": img.data,
            }
        }));
    }
    content.push(json!({"type": "text", "text": message}));
    json!(content)
}

fn make_sse_stream(
    mut rx: tokio::sync::mpsc::Receiver<SseEvent>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    async_stream::stream! {
        while let Some(event) = rx.recv().await {
            let event_name = match &event {
                SseEvent::TextDelta { .. } => "text_delta",
                SseEvent::ToolStart { .. } => "tool_start",
                SseEvent::ToolResult { .. } => "tool_result",
                SseEvent::Done { .. } => "done",
                SseEvent::Error { .. } => "error",
            };
            if let Ok(data) = serde_json::to_string(&event) {
                yield Ok(Event::default().event(event_name).data(data));
            }
            // Stop after done or error
            if matches!(event, SseEvent::Done { .. } | SseEvent::Error { .. }) {
                break;
            }
        }
    }
}
