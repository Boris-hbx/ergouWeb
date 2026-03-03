## Context

`LlmClient` 已有 Claude + Kimi 双 provider 架构，Kimi 使用 OpenAI 兼容格式。火山引擎/豆包 API 同样兼容 OpenAI 格式，因此可以复用 Kimi 的请求构造逻辑。

现状：
- `LlmProvider` enum: `Claude`, `Kimi`
- DB `user_settings.ai_model` 列已存在，默认 `"auto"`
- API endpoints `GET/PUT /api/settings/ai-model` 已实现
- 前端 `API.getAiModel()` / `API.setAiModel()` 已实现
- 设置页 UI 有占位按钮（disabled），JS 有注释掉的 stubs

约 85% 的基础设施已就绪，主要工作是添加 Doubao provider 实现 + 启用前端 UI。

## Goals / Non-Goals

**Goals:**
- 添加 Doubao 作为第三个 LLM provider
- 启用设置页模型切换 UI
- 保持现有 Claude 功能不变（回归安全）

**Non-Goals:**
- Kimi 模型的实际接入（保留代码但不启用 UI 选项）
- 用量统计或计费
- 模型能力差异的 UI 提示（如某模型不支持 vision）
- Provider 级别的超时/重试策略差异化

## Decisions

### 1. 复用 OpenAI 兼容格式，提取公共方法

**决定**: 将 Kimi 的 `chat_kimi` / `vision_generate_kimi` / `simple_generate_kimi` 重构为通用的 `chat_openai_compat` / `vision_generate_openai_compat` / `simple_generate_openai_compat`，Doubao 和 Kimi 共用。

**理由**: Doubao 和 Kimi 的请求格式完全相同（OpenAI compatible），仅 URL / model / key 不同。重复代码约 200 行，提取后差异通过参数传递。

**替代方案**: 复制 Kimi 方法改名为 `_doubao` 版本 → 代码重复，维护成本高。

### 2. Provider 配置用结构体而非散落常量

**决定**: 引入 `ProviderConfig` 结构体，将 URL / model / key 封装：

```rust
struct ProviderConfig {
    api_url: &'static str,
    model: &'static str,
    api_key: String,
}
```

`LlmClient` 存储 `provider: LlmProvider` + `config: ProviderConfig`，替代现有的 `api_key: String`。

**理由**: 当前 `CLAUDE_API_URL` / `KIMI_API_URL` 等常量 + match 分支的模式在 3 个 provider 时变得冗长。结构体让每个 provider 的配置自包含。

### 3. "auto" 优先级: Doubao → Claude

**决定**: `"auto"` 模式优先使用 Doubao（成本最低），回退到 Claude。

**理由**: Doubao 调用成本显著低于 Claude，对于日常记账/差旅分析等任务质量足够。用户需要 Claude 质量时可手动选择。

### 4. 前端不区分模型能力

**决定**: 所有模型按钮平等展示，不标注"不支持 vision"等差异。

**理由**: 火山引擎 Doubao 支持 vision（多模态），与 Claude 能力对等。如果未来加入不支持 vision 的模型再考虑 UI 差异化。

## Risks / Trade-offs

**[Risk] Doubao vision 质量不如 Claude** → 用户可随时切回 Claude。"auto" 选 Doubao 是成本优化取舍，后续可根据用户反馈调整优先级。

**[Risk] 重构 Kimi 方法为公共方法可能引入回归** → Kimi 当前无 key 未实际使用，风险可控。重构后 Kimi 代码路径仍保留，未来接入只需配置 key。

**[Risk] 火山引擎 API 限流策略未知** → 复用现有指数退避重试逻辑（最多 2 次），与 Claude/Kimi 一致。

## Migration Plan

1. 后端: 添加 Doubao provider + 重构 OpenAI 兼容方法 → `cargo test` + `cargo clippy`
2. 前端: 启用模型切换 UI + 实现交互逻辑
3. 部署 staging: `fly secrets set ARK_API_KEY="..." --config fly.staging.toml` → 部署 → 手动验证各模型
4. 部署 production: `fly secrets set ARK_API_KEY="..."` → 部署
5. **回滚**: 如果 Doubao 有问题，用户切回 Claude 即可；紧急情况下可删除 `ARK_API_KEY` secret 让 auto 回退到 Claude
