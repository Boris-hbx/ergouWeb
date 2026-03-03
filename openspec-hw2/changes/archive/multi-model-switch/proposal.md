## Why

用户目前只能使用 Claude 一个 AI 模型。需要支持多模型切换，让用户可以根据场景选择不同的模型（如成本更低的国产模型）。火山引擎/豆包（Doubao）API key 已就绪，需要完成集成并启用设置页面的模型切换 UI。

## What Changes

- **新增 Doubao 模型提供方**: 在 `LlmClient` 中添加 `Doubao` provider，复用 Kimi 的 OpenAI 兼容格式代码路径（火山引擎 API 与 OpenAI 格式兼容）
- **启用设置页模型切换 UI**: 将当前禁用的占位按钮替换为真实的模型选项（自动 / Claude / 豆包），绑定选择逻辑
- **更新后端验证**: `set_ai_model` 接口允许 `"doubao"` 作为合法值
- **自动回退机制**: 当用户选择的模型 API key 不可用时，自动回退到其他可用模型

### 不在本次范围

- Kimi 集成（后续单独添加）
- 模型级别的用量统计或计费

## Capabilities

### New Capabilities

- `multi-model-provider`: 多模型提供方管理——LlmProvider enum 扩展、Doubao API 集成（chat/vision/simple_generate）、provider 选择与回退逻辑
- `model-switch-ui`: 设置页模型切换界面——模型选项按钮、当前选择高亮、切换交互

### Modified Capabilities

（无需修改现有 spec，本次变更是纯新增能力）

## Impact

### 后端

| 文件 | 改动 |
|------|------|
| `server/src/services/llm.rs` | 新增 `LlmProvider::Doubao`、常量 `DOUBAO_API_URL` / `DOUBAO_MODEL`、`chat_doubao()` / `vision_generate_doubao()` / `simple_generate_doubao()` 实现（复用 Kimi 的 OpenAI 兼容格式） |
| `server/src/auth.rs` | `set_ai_model` 允许列表增加 `"doubao"` |

### 前端

| 文件 | 改动 |
|------|------|
| `frontend/index.html` | 替换 AI 模型区域的禁用占位按钮为真实选项 |
| `frontend/assets/js/settings.js` | 实现 `selectAiModel()` / `highlightAiModel()` / `loadAiModel()` |

### 部署

| 项目 | 说明 |
|------|------|
| 环境变量 | 新增 `DOUBAO_API_KEY`（Fly.io secrets） |
| 缓存版本号 | 递增 `?v=` 参数 |

### API Key 安全性

API key **不会被窃取**，安全模型如下：

1. **服务端存储**: 所有 API key（`ANTHROPIC_API_KEY`、`KIMI_API_KEY`、`DOUBAO_API_KEY`）仅存在于服务端环境变量中
2. **前端隔离**: 前端只发送模型偏好字符串（如 `"auto"`、`"claude"`、`"doubao"`），不接触 key
3. **服务端解析**: `LlmClient::new()` 在服务端从 `std::env::var()` 读取 key，构造 HTTP 请求发往 LLM API
4. **传输安全**: 所有 API 调用使用 HTTPS，key 通过 HTTP header（`x-api-key` 或 `Authorization: Bearer`）传输
5. **Fly.io secrets**: 生产环境 key 通过 `fly secrets set` 注入，不写入代码仓库或 Docker 镜像
