## 1. 后端: LlmClient 重构 + Doubao 集成

- [x] 1.1 引入 `ProviderConfig` 结构体，替换 `LlmClient` 中散落的 `api_key` 字段，封装 `api_url` / `model` / `api_key`
- [x] 1.2 添加 `LlmProvider::Doubao` 枚举变体，添加 `DOUBAO_API_URL` / `DOUBAO_MODEL` 常量
- [x] 1.3 将 `simple_generate_kimi` 重构为 `simple_generate_openai_compat`，通过 `ProviderConfig` 参数化 URL/model/key
- [x] 1.4 将 `vision_generate_kimi` 重构为 `vision_generate_openai_compat`
- [x] 1.5 将 `chat_kimi` 重构为 `chat_openai_compat`
- [x] 1.6 更新 `LlmClient::new()` 回退逻辑：读取 `ARK_API_KEY` 环境变量，"auto" 优先级 Doubao → Claude
- [x] 1.7 `cargo test` + `cargo clippy` 确保无回归

## 2. 后端: API 端点更新

- [x] 2.1 `set_ai_model` 合法值列表增加 `"doubao"`

## 3. 前端: 模型切换 UI

- [x] 3.1 替换 `index.html` AI 模型区域：移除 disabled 占位按钮，改为 auto/claude/doubao 三个真实按钮
- [x] 3.2 实现 `settings.js` 中 `loadAiModel()`：页面加载时调用 API 获取当前偏好并高亮
- [x] 3.3 实现 `selectAiModel(model)`：乐观更新高亮 + API 保存 + 失败回退 + toast
- [x] 3.4 实现 `highlightAiModel(model)`：切换 active class 的辅助函数
- [x] 3.5 Guest 模式处理：检测 guest 身份，禁用按钮 + 显示提示文字

## 4. 验证与部署

- [x] 4.1 递增缓存版本号 `?v=`
- [x] 4.2 `cargo test` + `cargo clippy` 全量验证
- [x] 4.3 部署 staging + 手动验证模型切换（auto / claude / doubao 各测一次）
