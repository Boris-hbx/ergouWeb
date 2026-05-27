use futures::StreamExt;
use rusqlite::Connection;
use serde_json::{json, Value};
use tokio::sync::mpsc;

const CLAUDE_API_URL: &str = "https://api.anthropic.com/v1/messages";
const CLAUDE_MODEL: &str = "claude-sonnet-4-6";

const MAX_TOOL_ROUNDS: usize = 5;

pub struct LlmClient {
    api_key: String,
    http: reqwest::Client,
}

/// Represents one content block from the LLM response
#[derive(Debug, Clone)]
pub enum ContentBlock {
    Text(String),
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
}

/// Events sent through the SSE stream to the client
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum SseEvent {
    /// Text delta (incremental token)
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    /// A tool is being called (name + input summary)
    #[serde(rename = "tool_start")]
    ToolStart { name: String },
    /// Tool execution completed
    #[serde(rename = "tool_result")]
    ToolResult { name: String, result: Value },
    /// Stream completed — includes final metadata
    #[serde(rename = "done")]
    Done {
        conversation_id: String,
        full_text: String,
        tool_calls: Vec<Value>,
        input_tokens: i64,
        output_tokens: i64,
    },
    /// Error
    #[serde(rename = "error")]
    Error { message: String },
}

/// The result of a complete LLM conversation turn (potentially multi-round with tools)
pub struct ChatResult {
    /// The final text response to show the user
    pub text: String,
    /// Tool calls that were executed (name, input, result)
    pub tool_calls: Vec<(String, Value, Value)>,
    /// Total input tokens used across all rounds
    pub input_tokens: i64,
    /// Total output tokens used across all rounds
    pub output_tokens: i64,
}

impl LlmClient {
    /// Create a Claude client. Returns None if ANTHROPIC_API_KEY is not set.
    pub fn new(_provider_pref: &str) -> Option<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .filter(|k| !k.is_empty())?;
        Some(Self {
            api_key,
            http: reqwest::Client::new(),
        })
    }

    /// Create a client for the given user (reads preference from DB, but always uses Claude).
    pub fn for_user(_db: &Connection, _user_id: &str) -> Option<Self> {
        Self::new("claude")
    }

    // ─── Chat with tool use loop ───

    pub async fn chat(
        &self,
        system: &str,
        messages: Vec<Value>,
        tools: &[Value],
        mut execute_tool: impl FnMut(&str, &Value) -> Value,
    ) -> Result<ChatResult, String> {
        let mut all_messages = messages;
        let mut total_input = 0i64;
        let mut total_output = 0i64;
        let mut tool_calls_log: Vec<(String, Value, Value)> = Vec::new();

        for round in 0..MAX_TOOL_ROUNDS {
            let mut body = json!({
                "model": CLAUDE_MODEL,
                "max_tokens": 2048,
                "system": system,
                "tools": tools,
                "messages": all_messages,
            });
            if round == 0 {
                body["tool_choice"] = json!({"type": "auto"});
            }

            if round == 0 {
                eprintln!(
                    "[Claude] Sending {} messages, {} tools",
                    all_messages.len(),
                    tools.len()
                );
            }

            let resp = self
                .http
                .post(CLAUDE_API_URL)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&body)
                .timeout(std::time::Duration::from_secs(90))
                .send()
                .await
                .map_err(|e| {
                    eprintln!("[Claude] request error: {}", e);
                    "AI 服务连接失败，请稍后重试".to_string()
                })?;

            let status = resp.status();
            if status.as_u16() == 429 {
                if round < 2 {
                    tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(round as u32)))
                        .await;
                    continue;
                }
                return Err("二狗太忙了，请稍后再试".into());
            }
            if !status.is_success() {
                let text = resp.text().await.unwrap_or_default();
                eprintln!("[Claude] API error {}: {}", status.as_u16(), text);
                return Err("AI 服务暂时不可用，请稍后重试".into());
            }

            let resp_json: Value = resp.json().await.map_err(|e| {
                eprintln!("[Claude] Failed to parse response: {}", e);
                "AI 服务响应异常，请稍后重试".to_string()
            })?;

            if let Some(usage) = resp_json.get("usage") {
                total_input += usage["input_tokens"].as_i64().unwrap_or(0);
                total_output += usage["output_tokens"].as_i64().unwrap_or(0);
            }

            let content = resp_json["content"].as_array().cloned().unwrap_or_default();
            let stop_reason = resp_json["stop_reason"].as_str().unwrap_or("end_turn");
            eprintln!(
                "[Claude] Round {}: stop_reason={}, blocks={}",
                round,
                stop_reason,
                content.len()
            );

            let mut blocks: Vec<ContentBlock> = Vec::new();
            for block in &content {
                match block["type"].as_str() {
                    Some("text") => {
                        if let Some(text) = block["text"].as_str() {
                            blocks.push(ContentBlock::Text(text.to_string()));
                        }
                    }
                    Some("tool_use") => {
                        blocks.push(ContentBlock::ToolUse {
                            id: block["id"].as_str().unwrap_or("").to_string(),
                            name: block["name"].as_str().unwrap_or("").to_string(),
                            input: block["input"].clone(),
                        });
                    }
                    _ => {}
                }
            }

            if stop_reason == "tool_use" {
                all_messages.push(json!({
                    "role": "assistant",
                    "content": content,
                }));

                let mut tool_results = Vec::new();
                for block in &blocks {
                    if let ContentBlock::ToolUse { id, name, input } = block {
                        let result = execute_tool(name, input);
                        tool_calls_log.push((name.clone(), input.clone(), result.clone()));
                        tool_results.push(json!({
                            "type": "tool_result",
                            "tool_use_id": id,
                            "content": serde_json::to_string(&result).unwrap_or_default(),
                        }));
                    }
                }

                all_messages.push(json!({
                    "role": "user",
                    "content": tool_results,
                }));

                continue;
            }

            let mut final_text = String::new();
            for block in &blocks {
                if let ContentBlock::Text(t) = block {
                    if !final_text.is_empty() {
                        final_text.push('\n');
                    }
                    final_text.push_str(t);
                }
            }

            return Ok(ChatResult {
                text: final_text,
                tool_calls: tool_calls_log,
                input_tokens: total_input,
                output_tokens: total_output,
            });
        }

        Err("操作太复杂，请简化请求".into())
    }

    // ─── Streaming chat with tool use loop ───

    /// Stream chat responses via SSE. Sends SseEvents through the channel.
    /// The execute_tool closure runs tool calls between streaming rounds.
    pub async fn chat_stream(
        &self,
        system: &str,
        messages: Vec<Value>,
        tools: &[Value],
        mut execute_tool: impl FnMut(&str, &Value) -> Value,
        tx: mpsc::Sender<SseEvent>,
    ) -> ChatResult {
        let mut all_messages = messages;
        let mut total_input = 0i64;
        let mut total_output = 0i64;
        let mut tool_calls_log: Vec<(String, Value, Value)> = Vec::new();
        let mut final_text = String::new();

        for round in 0..MAX_TOOL_ROUNDS {
            let mut body = json!({
                "model": CLAUDE_MODEL,
                "max_tokens": 2048,
                "system": system,
                "tools": tools,
                "messages": all_messages,
                "stream": true,
            });
            if round == 0 {
                body["tool_choice"] = json!({"type": "auto"});
            }

            if round == 0 {
                tracing::info!(
                    "[Claude stream] Sending {} messages, {} tools",
                    all_messages.len(),
                    tools.len()
                );
            }

            let resp = match self
                .http
                .post(CLAUDE_API_URL)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&body)
                .timeout(std::time::Duration::from_secs(90))
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("[Claude stream] request error: {}", e);
                    let _ = tx
                        .send(SseEvent::Error {
                            message: "AI 服务连接失败，请稍后重试".into(),
                        })
                        .await;
                    return ChatResult {
                        text: final_text,
                        tool_calls: tool_calls_log,
                        input_tokens: total_input,
                        output_tokens: total_output,
                    };
                }
            };

            let status = resp.status();
            if status.as_u16() == 429 {
                if round < 2 {
                    tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(round as u32)))
                        .await;
                    continue;
                }
                let _ = tx
                    .send(SseEvent::Error {
                        message: "二狗太忙了，请稍后再试".into(),
                    })
                    .await;
                return ChatResult {
                    text: final_text,
                    tool_calls: tool_calls_log,
                    input_tokens: total_input,
                    output_tokens: total_output,
                };
            }
            if !status.is_success() {
                let text = resp.text().await.unwrap_or_default();
                tracing::error!("[Claude stream] API error {}: {}", status.as_u16(), text);
                let _ = tx
                    .send(SseEvent::Error {
                        message: "AI 服务暂时不可用，请稍后重试".into(),
                    })
                    .await;
                return ChatResult {
                    text: final_text,
                    tool_calls: tool_calls_log,
                    input_tokens: total_input,
                    output_tokens: total_output,
                };
            }

            // Parse SSE stream from Claude
            let mut byte_stream = resp.bytes_stream();
            let mut buffer = String::new();
            let mut round_text = String::new();
            let mut stop_reason = String::from("end_turn");
            let mut content_blocks: Vec<Value> = Vec::new();
            // Track tool_use blocks being built incrementally
            let mut tool_blocks: std::collections::HashMap<usize, (String, String, String)> =
                std::collections::HashMap::new(); // index -> (id, name, input_json_str)

            while let Some(chunk_result) = byte_stream.next().await {
                let chunk = match chunk_result {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("[Claude stream] chunk error: {}", e);
                        break;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&chunk));

                // Process complete SSE lines
                while let Some(pos) = buffer.find("\n\n") {
                    let event_block = buffer[..pos].to_string();
                    buffer = buffer[pos + 2..].to_string();

                    // Parse event type and data
                    let mut event_type = String::new();
                    let mut data = String::new();
                    for line in event_block.lines() {
                        if let Some(et) = line.strip_prefix("event: ") {
                            event_type = et.to_string();
                        } else if let Some(d) = line.strip_prefix("data: ") {
                            data = d.to_string();
                        }
                    }

                    if data.is_empty() {
                        continue;
                    }

                    let parsed: Value = match serde_json::from_str(&data) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    match event_type.as_str() {
                        "message_start" => {
                            if let Some(usage) = parsed.get("message").and_then(|m| m.get("usage"))
                            {
                                total_input += usage["input_tokens"].as_i64().unwrap_or(0);
                            }
                        }
                        "content_block_start" => {
                            let idx = parsed["index"].as_u64().unwrap_or(0) as usize;
                            let block = &parsed["content_block"];
                            if block["type"].as_str() == Some("tool_use") {
                                let id =
                                    block["id"].as_str().unwrap_or_default().to_string();
                                let name =
                                    block["name"].as_str().unwrap_or_default().to_string();
                                tool_blocks.insert(idx, (id, name.clone(), String::new()));
                                let _ = tx.send(SseEvent::ToolStart { name }).await;
                            }
                        }
                        "content_block_delta" => {
                            let idx = parsed["index"].as_u64().unwrap_or(0) as usize;
                            let delta = &parsed["delta"];
                            match delta["type"].as_str() {
                                Some("text_delta") => {
                                    if let Some(text) = delta["text"].as_str() {
                                        round_text.push_str(text);
                                        let _ = tx
                                            .send(SseEvent::TextDelta {
                                                text: text.to_string(),
                                            })
                                            .await;
                                    }
                                }
                                Some("input_json_delta") => {
                                    if let Some(json_str) = delta["partial_json"].as_str() {
                                        if let Some(tb) = tool_blocks.get_mut(&idx) {
                                            tb.2.push_str(json_str);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        "content_block_stop" => {
                            let idx = parsed["index"].as_u64().unwrap_or(0) as usize;
                            if let Some((id, name, input_str)) = tool_blocks.remove(&idx) {
                                let input: Value =
                                    serde_json::from_str(&input_str).unwrap_or(json!({}));
                                content_blocks.push(json!({
                                    "type": "tool_use",
                                    "id": id,
                                    "name": name,
                                    "input": input,
                                }));
                            } else if !round_text.is_empty() {
                                content_blocks.push(json!({
                                    "type": "text",
                                    "text": round_text.clone(),
                                }));
                            }
                        }
                        "message_delta" => {
                            if let Some(sr) = parsed["delta"]["stop_reason"].as_str() {
                                stop_reason = sr.to_string();
                            }
                            if let Some(usage) = parsed.get("usage") {
                                total_output += usage["output_tokens"].as_i64().unwrap_or(0);
                            }
                        }
                        "message_stop" | "error" => {
                            if event_type == "error" {
                                tracing::error!(
                                    "[Claude stream] API error event: {}",
                                    data
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }

            tracing::info!(
                "[Claude stream] Round {}: stop_reason={}, blocks={}",
                round,
                stop_reason,
                content_blocks.len()
            );

            if stop_reason == "tool_use" {
                // Build assistant message with all content blocks
                all_messages.push(json!({
                    "role": "assistant",
                    "content": content_blocks,
                }));

                // Execute tools
                let mut tool_results = Vec::new();
                // Collect tool_use blocks from content_blocks
                let tool_use_blocks: Vec<_> = content_blocks
                    .iter()
                    .filter(|b| b["type"].as_str() == Some("tool_use"))
                    .cloned()
                    .collect();

                for block in &tool_use_blocks {
                    let id = block["id"].as_str().unwrap_or_default();
                    let name = block["name"].as_str().unwrap_or_default();
                    let input = &block["input"];

                    let result = execute_tool(name, input);
                    tool_calls_log.push((name.to_string(), input.clone(), result.clone()));

                    let _ = tx
                        .send(SseEvent::ToolResult {
                            name: name.to_string(),
                            result: result.clone(),
                        })
                        .await;

                    tool_results.push(json!({
                        "type": "tool_result",
                        "tool_use_id": id,
                        "content": serde_json::to_string(&result).unwrap_or_default(),
                    }));
                }

                all_messages.push(json!({
                    "role": "user",
                    "content": tool_results,
                }));

                // Reset for next round
                content_blocks = Vec::new();
                round_text = String::new();
                continue;
            }

            // Final round — collect text
            if !round_text.is_empty() {
                final_text.push_str(&round_text);
            }
            break;
        }

        ChatResult {
            text: final_text,
            tool_calls: tool_calls_log,
            input_tokens: total_input,
            output_tokens: total_output,
        }
    }

    // ─── Vision generation ───

    pub async fn vision_generate(
        &self,
        system: &str,
        images: Vec<(String, String)>, // (base64_data, media_type)
        user_message: &str,
        max_tokens: u32,
    ) -> Result<String, String> {
        let mut content = Vec::new();
        for (b64, mime) in &images {
            content.push(json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": mime,
                    "data": b64,
                }
            }));
        }
        content.push(json!({"type": "text", "text": user_message}));

        let body = json!({
            "model": CLAUDE_MODEL,
            "max_tokens": max_tokens,
            "system": system,
            "messages": [{"role": "user", "content": content}],
        });

        let resp = self
            .http
            .post(CLAUDE_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| {
                eprintln!("[Claude] vision request error: {}", e);
                "AI 服务连接失败，请稍后重试".to_string()
            })?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            eprintln!("[Claude] vision_generate API error: {}", text);
            return Err("AI 服务暂时不可用，请稍后重试".into());
        }

        let resp_json: Value = resp.json().await.map_err(|e| {
            eprintln!("[Claude] vision_generate parse error: {}", e);
            "AI 服务响应异常".to_string()
        })?;

        if let Some(content) = resp_json["content"].as_array() {
            for block in content {
                if block["type"].as_str() == Some("text") {
                    if let Some(text) = block["text"].as_str() {
                        return Ok(text.trim().to_string());
                    }
                }
            }
        }

        Err("No text in Claude response".into())
    }

    // ─── Simple one-shot generation ───

    pub async fn simple_generate(
        &self,
        system: &str,
        user_message: &str,
        max_tokens: u32,
    ) -> Result<String, String> {
        let body = json!({
            "model": CLAUDE_MODEL,
            "max_tokens": max_tokens,
            "system": system,
            "messages": [{"role": "user", "content": user_message}],
        });

        let resp = self
            .http
            .post(CLAUDE_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| {
                eprintln!("[Claude] simple_generate request error: {}", e);
                "AI 服务连接失败，请稍后重试".to_string()
            })?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            eprintln!("[Claude] simple_generate API error: {}", text);
            return Err("AI 服务暂时不可用，请稍后重试".into());
        }

        let resp_json: Value = resp.json().await.map_err(|e| {
            eprintln!("[Claude] simple_generate parse error: {}", e);
            "AI 服务响应异常，请稍后重试".to_string()
        })?;

        if let Some(content) = resp_json["content"].as_array() {
            for block in content {
                if block["type"].as_str() == Some("text") {
                    if let Some(text) = block["text"].as_str() {
                        return Ok(text.trim().to_string());
                    }
                }
            }
        }

        Err("No text in Claude response".into())
    }
}
