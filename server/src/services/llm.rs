use rusqlite::Connection;
use serde_json::{json, Value};

const CLAUDE_API_URL: &str = "https://api.anthropic.com/v1/messages";
const CLAUDE_MODEL: &str = "claude-sonnet-4-5-20250929";

const KIMI_API_URL: &str = "https://api.moonshot.cn/v1/chat/completions";
const KIMI_MODEL: &str = "kimi-k2-0711";

const DOUBAO_API_URL: &str = "https://ark.cn-beijing.volces.com/api/v3/chat/completions";
const DOUBAO_MODEL: &str = "ep-20260221192556-jzl2c";

const MAX_TOOL_ROUNDS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LlmProvider {
    Claude,
    Kimi,
    Doubao,
}

/// Configuration for an OpenAI-compatible provider (Kimi, Doubao, etc.)
struct OaiConfig {
    api_url: &'static str,
    model: &'static str,
    label: &'static str,
}

impl LlmProvider {
    fn oai_config(&self) -> Option<OaiConfig> {
        match self {
            LlmProvider::Kimi => Some(OaiConfig {
                api_url: KIMI_API_URL,
                model: KIMI_MODEL,
                label: "Kimi",
            }),
            LlmProvider::Doubao => Some(OaiConfig {
                api_url: DOUBAO_API_URL,
                model: DOUBAO_MODEL,
                label: "Doubao",
            }),
            LlmProvider::Claude => None,
        }
    }
}

pub struct LlmClient {
    provider: LlmProvider,
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
    /// Create a client for the given provider preference.
    /// Fallback: if the preferred provider has no key, try others.
    pub fn new(provider_pref: &str) -> Option<Self> {
        let anthropic_key = std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .filter(|k| !k.is_empty());
        let kimi_key = std::env::var("KIMI_API_KEY").ok().filter(|k| !k.is_empty());
        let doubao_key = std::env::var("ARK_API_KEY").ok().filter(|k| !k.is_empty());

        match provider_pref {
            "claude" => {
                if let Some(key) = anthropic_key {
                    return Some(Self::with_provider(LlmProvider::Claude, key));
                }
                if let Some(key) = doubao_key {
                    return Some(Self::with_provider(LlmProvider::Doubao, key));
                }
                kimi_key.map(|key| Self::with_provider(LlmProvider::Kimi, key))
            }
            "kimi" => {
                if let Some(key) = kimi_key {
                    return Some(Self::with_provider(LlmProvider::Kimi, key));
                }
                if let Some(key) = doubao_key {
                    return Some(Self::with_provider(LlmProvider::Doubao, key));
                }
                anthropic_key.map(|key| Self::with_provider(LlmProvider::Claude, key))
            }
            "doubao" => {
                if let Some(key) = doubao_key {
                    return Some(Self::with_provider(LlmProvider::Doubao, key));
                }
                if let Some(key) = anthropic_key {
                    return Some(Self::with_provider(LlmProvider::Claude, key));
                }
                kimi_key.map(|key| Self::with_provider(LlmProvider::Kimi, key))
            }
            _ => {
                // "auto": prefer Doubao (cheapest), then Kimi, then Claude
                if let Some(key) = doubao_key {
                    return Some(Self::with_provider(LlmProvider::Doubao, key));
                }
                if let Some(key) = kimi_key {
                    return Some(Self::with_provider(LlmProvider::Kimi, key));
                }
                anthropic_key.map(|key| Self::with_provider(LlmProvider::Claude, key))
            }
        }
    }

    fn with_provider(provider: LlmProvider, api_key: String) -> Self {
        Self {
            provider,
            api_key,
            http: reqwest::Client::new(),
        }
    }

    /// Query user's ai_model preference from DB and create the corresponding client.
    pub fn for_user(db: &Connection, user_id: &str) -> Option<Self> {
        let model_pref = db
            .query_row(
                "SELECT ai_model FROM user_settings WHERE user_id = ?1",
                [user_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap_or_else(|_| "auto".to_string());
        Self::new(&model_pref)
    }

    // ─── Chat with tool use loop ───

    pub async fn chat(
        &self,
        system: &str,
        messages: Vec<Value>,
        tools: &[Value],
        mut execute_tool: impl FnMut(&str, &Value) -> Value,
    ) -> Result<ChatResult, String> {
        match self.provider {
            LlmProvider::Claude => {
                self.chat_claude(system, messages, tools, &mut execute_tool)
                    .await
            }
            LlmProvider::Kimi | LlmProvider::Doubao => {
                let cfg = self.provider.oai_config().unwrap();
                self.chat_openai_compat(&cfg, system, messages, tools, &mut execute_tool)
                    .await
            }
        }
    }

    async fn chat_claude(
        &self,
        system: &str,
        messages: Vec<Value>,
        tools: &[Value],
        execute_tool: &mut impl FnMut(&str, &Value) -> Value,
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

    async fn chat_openai_compat(
        &self,
        cfg: &OaiConfig,
        system: &str,
        messages: Vec<Value>,
        tools: &[Value],
        execute_tool: &mut impl FnMut(&str, &Value) -> Value,
    ) -> Result<ChatResult, String> {
        // Convert Claude-format messages to OpenAI format
        let mut oai_messages: Vec<Value> = Vec::new();
        oai_messages.push(json!({"role": "system", "content": system}));
        for msg in &messages {
            oai_messages.push(Self::claude_msg_to_openai(msg));
        }

        // Convert Claude tools to OpenAI format
        let oai_tools: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t["name"],
                        "description": t["description"],
                        "parameters": t["input_schema"],
                    }
                })
            })
            .collect();

        let mut total_input = 0i64;
        let mut total_output = 0i64;
        let mut tool_calls_log: Vec<(String, Value, Value)> = Vec::new();

        for round in 0..MAX_TOOL_ROUNDS {
            if round == 0 {
                eprintln!(
                    "[{}] Sending {} messages, {} tools",
                    cfg.label,
                    oai_messages.len(),
                    oai_tools.len()
                );
            }

            let mut body = json!({
                "model": cfg.model,
                "max_tokens": 2048,
                "messages": oai_messages,
            });
            if !oai_tools.is_empty() {
                body["tools"] = json!(oai_tools);
                body["tool_choice"] = json!("auto");
            }

            let resp = self
                .http
                .post(cfg.api_url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("content-type", "application/json")
                .json(&body)
                .timeout(std::time::Duration::from_secs(90))
                .send()
                .await
                .map_err(|e| {
                    eprintln!("[{}] request error: {}", cfg.label, e);
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
                eprintln!("[{}] API error {}: {}", cfg.label, status.as_u16(), text);
                return Err("AI 服务暂时不可用，请稍后重试".into());
            }

            let resp_json: Value = resp.json().await.map_err(|e| {
                eprintln!("[{}] Failed to parse response: {}", cfg.label, e);
                "AI 服务响应异常，请稍后重试".to_string()
            })?;

            if let Some(usage) = resp_json.get("usage") {
                total_input += usage["prompt_tokens"].as_i64().unwrap_or(0);
                total_output += usage["completion_tokens"].as_i64().unwrap_or(0);
            }

            let choice = &resp_json["choices"][0];
            let message = &choice["message"];
            let finish_reason = choice["finish_reason"].as_str().unwrap_or("stop");
            eprintln!(
                "[{}] Round {}: finish_reason={}",
                cfg.label, round, finish_reason
            );

            if finish_reason == "tool_calls" {
                // Add assistant message with tool_calls to conversation
                oai_messages.push(message.clone());

                if let Some(tool_calls) = message["tool_calls"].as_array() {
                    for tc in tool_calls {
                        let fn_name = tc["function"]["name"].as_str().unwrap_or("");
                        let fn_args_str = tc["function"]["arguments"].as_str().unwrap_or("{}");
                        let fn_args: Value =
                            serde_json::from_str(fn_args_str).unwrap_or(json!({}));
                        let tc_id = tc["id"].as_str().unwrap_or("");

                        let result = execute_tool(fn_name, &fn_args);
                        tool_calls_log.push((fn_name.to_string(), fn_args, result.clone()));

                        oai_messages.push(json!({
                            "role": "tool",
                            "tool_call_id": tc_id,
                            "content": serde_json::to_string(&result).unwrap_or_default(),
                        }));
                    }
                }
                continue;
            }

            // Normal text response
            let text = message["content"].as_str().unwrap_or("").to_string();

            return Ok(ChatResult {
                text,
                tool_calls: tool_calls_log,
                input_tokens: total_input,
                output_tokens: total_output,
            });
        }

        Err("操作太复杂，请简化请求".into())
    }

    /// Convert a Claude-format message to OpenAI format
    fn claude_msg_to_openai(msg: &Value) -> Value {
        let role = msg["role"].as_str().unwrap_or("user");

        // If content is a simple string
        if let Some(text) = msg["content"].as_str() {
            return json!({"role": role, "content": text});
        }

        // If content is an array of blocks (Claude format)
        if let Some(blocks) = msg["content"].as_array() {
            let mut parts: Vec<Value> = Vec::new();
            for block in blocks {
                match block["type"].as_str() {
                    Some("text") => {
                        parts.push(json!({
                            "type": "text",
                            "text": block["text"]
                        }));
                    }
                    Some("image") => {
                        // Convert Claude image format to OpenAI image_url format
                        if let Some(source) = block.get("source") {
                            let media_type = source["media_type"].as_str().unwrap_or("image/jpeg");
                            let data = source["data"].as_str().unwrap_or("");
                            parts.push(json!({
                                "type": "image_url",
                                "image_url": {
                                    "url": format!("data:{};base64,{}", media_type, data)
                                }
                            }));
                        }
                    }
                    _ => {}
                }
            }
            if parts.len() == 1 && parts[0]["type"].as_str() == Some("text") {
                // Simplify single text block
                return json!({"role": role, "content": parts[0]["text"]});
            }
            return json!({"role": role, "content": parts});
        }

        json!({"role": role, "content": ""})
    }

    // ─── Vision generation ───

    pub async fn vision_generate(
        &self,
        system: &str,
        images: Vec<(String, String)>, // (base64_data, media_type)
        user_message: &str,
        max_tokens: u32,
    ) -> Result<String, String> {
        match self.provider {
            LlmProvider::Claude => {
                self.vision_generate_claude(system, images, user_message, max_tokens)
                    .await
            }
            LlmProvider::Kimi | LlmProvider::Doubao => {
                let cfg = self.provider.oai_config().unwrap();
                self.vision_generate_openai_compat(&cfg, system, images, user_message, max_tokens)
                    .await
            }
        }
    }

    async fn vision_generate_claude(
        &self,
        system: &str,
        images: Vec<(String, String)>,
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

    async fn vision_generate_openai_compat(
        &self,
        cfg: &OaiConfig,
        system: &str,
        images: Vec<(String, String)>,
        user_message: &str,
        max_tokens: u32,
    ) -> Result<String, String> {
        let mut content_parts: Vec<Value> = Vec::new();
        for (b64, mime) in &images {
            content_parts.push(json!({
                "type": "image_url",
                "image_url": {
                    "url": format!("data:{};base64,{}", mime, b64)
                }
            }));
        }
        content_parts.push(json!({"type": "text", "text": user_message}));

        let body = json!({
            "model": cfg.model,
            "max_tokens": max_tokens,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": content_parts}
            ],
        });

        let resp = self
            .http
            .post(cfg.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| {
                eprintln!("[{}] vision request error: {}", cfg.label, e);
                "AI 服务连接失败，请稍后重试".to_string()
            })?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            eprintln!("[{}] vision_generate API error: {}", cfg.label, text);
            return Err("AI 服务暂时不可用，请稍后重试".into());
        }

        let resp_json: Value = resp.json().await.map_err(|e| {
            eprintln!("[{}] vision_generate parse error: {}", cfg.label, e);
            "AI 服务响应异常".to_string()
        })?;

        resp_json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.trim().to_string())
            .ok_or_else(|| format!("No text in {} response", cfg.label))
    }

    // ─── Simple one-shot generation ───

    pub async fn simple_generate(
        &self,
        system: &str,
        user_message: &str,
        max_tokens: u32,
    ) -> Result<String, String> {
        match self.provider {
            LlmProvider::Claude => {
                self.simple_generate_claude(system, user_message, max_tokens)
                    .await
            }
            LlmProvider::Kimi | LlmProvider::Doubao => {
                let cfg = self.provider.oai_config().unwrap();
                self.simple_generate_openai_compat(&cfg, system, user_message, max_tokens)
                    .await
            }
        }
    }

    async fn simple_generate_claude(
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

    async fn simple_generate_openai_compat(
        &self,
        cfg: &OaiConfig,
        system: &str,
        user_message: &str,
        max_tokens: u32,
    ) -> Result<String, String> {
        let body = json!({
            "model": cfg.model,
            "max_tokens": max_tokens,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user_message}
            ],
        });

        let resp = self
            .http
            .post(cfg.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| {
                eprintln!("[{}] simple_generate request error: {}", cfg.label, e);
                "AI 服务连接失败，请稍后重试".to_string()
            })?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            eprintln!("[{}] simple_generate API error: {}", cfg.label, text);
            return Err("AI 服务暂时不可用，请稍后重试".into());
        }

        let resp_json: Value = resp.json().await.map_err(|e| {
            eprintln!("[{}] simple_generate parse error: {}", cfg.label, e);
            "AI 服务响应异常，请稍后重试".to_string()
        })?;

        resp_json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.trim().to_string())
            .ok_or_else(|| format!("No text in {} response", cfg.label))
    }
}
