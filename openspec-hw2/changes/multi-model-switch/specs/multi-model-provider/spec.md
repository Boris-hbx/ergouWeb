## ADDED Requirements

### Requirement: Doubao provider integration

The system SHALL support Volcengine Doubao as an LLM provider, using the OpenAI-compatible chat completions API.

- API URL: `https://ark.cn-beijing.volces.com/api/v3/chat/completions`
- Model endpoint ID: `ep-20260221192556-jzl2c`
- API key 环境变量: `ARK_API_KEY`
- 认证方式: `Authorization: Bearer <key>`
- 请求/响应格式: OpenAI 兼容（与 Kimi 相同）

`LlmProvider` enum SHALL 新增 `Doubao` 变体。`LlmClient` SHALL 实现 `chat_doubao()`、`vision_generate_doubao()`、`simple_generate_doubao()` 三个方法，复用 Kimi 的 OpenAI 兼容格式逻辑。

#### Scenario: Doubao simple_generate 成功
- **WHEN** 用户模型偏好为 "doubao" 且 `ARK_API_KEY` 环境变量存在
- **THEN** `LlmClient::new("doubao")` 返回 `Some(client)` 且 `client.provider == Doubao`，`simple_generate()` 调用火山引擎 API 并返回文本结果

#### Scenario: Doubao vision_generate 成功
- **WHEN** 用户请求差旅票据分析（带图片）且模型为 Doubao
- **THEN** `vision_generate()` 将图片以 `image_url` + base64 data URI 格式发送至火山引擎 API，返回分析结果

#### Scenario: Doubao chat with tools 成功
- **WHEN** 用户使用记账 AI 分析（需要 tool use）且模型为 Doubao
- **THEN** `chat()` 以 OpenAI function calling 格式发送 tools，正确处理 `tool_calls` 响应并执行多轮对话

### Requirement: Provider fallback mechanism

当用户选择的模型 API key 不可用时，系统 SHALL 自动回退到其他可用模型，确保 AI 功能不中断。

回退优先级：
- `"doubao"` → 尝试 Doubao，无 key 则回退到 Claude
- `"claude"` → 尝试 Claude，无 key 则回退到 Doubao
- `"auto"` → 按优先级尝试：Doubao → Claude（Doubao 成本更低）

#### Scenario: 指定模型不可用时回退
- **WHEN** 用户偏好为 "doubao" 但 `ARK_API_KEY` 环境变量未设置
- **THEN** `LlmClient::new("doubao")` 回退到 Claude（如果 `ANTHROPIC_API_KEY` 可用），AI 功能正常工作

#### Scenario: 所有 key 都不可用
- **WHEN** 没有任何 LLM API key 配置
- **THEN** `LlmClient::new()` 返回 `None`，各路由使用各自的 fallback 逻辑（如 moment 使用 `fallback_greeting()`）

### Requirement: Model validation endpoint update

`PUT /api/settings/ai-model` 的合法值列表 SHALL 包含 `"doubao"`。

合法值: `["auto", "claude", "kimi", "doubao"]`

保留 `"kimi"` 以兼容未来扩展，但当前 `"kimi"` 选择时因无 key 会回退。

#### Scenario: 设置 doubao 为偏好模型
- **WHEN** 用户发送 `PUT /api/settings/ai-model` 且 body 为 `{"model": "doubao"}`
- **THEN** 服务端返回 `{"success": true}`，`user_settings.ai_model` 更新为 "doubao"

#### Scenario: 无效模型值被拒绝
- **WHEN** 用户发送 `PUT /api/settings/ai-model` 且 body 为 `{"model": "gpt4"}`
- **THEN** 服务端返回 400 `{"success": false, "message": "无效的模型选择"}`
