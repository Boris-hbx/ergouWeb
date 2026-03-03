## Use Cases

### Use Case: 切换 AI 模型

**Primary Actor:** 已登录用户
**Scope:** Next 应用
**Level:** User goal

**Stakeholders and Interests:**
- 用户 — 选择适合自己的 AI 模型（成本、速度、质量）
- 系统管理员 — 确保 API key 安全，模型可用

**Preconditions:**
- 用户已登录（非 guest）
- 至少有一个模型的 API key 在服务端可用

**Success Guarantee (Postconditions):**
- 用户的模型偏好已持久化到 `user_settings.ai_model`
- 后续所有 AI 调用使用新选择的模型
- 设置页 UI 反映当前选择

**Trigger:** 用户打开设置页，点击 AI 模型区域的模型按钮

**Main Success Scenario:**
1. 用户打开设置页，系统加载当前模型偏好并高亮对应按钮
2. 用户点击目标模型按钮（如"豆包"）
3. 系统立即高亮新选择（乐观更新），发送 PUT 请求保存
4. 服务端验证模型值合法，写入数据库，返回成功
5. 用户后续使用 AI 功能（记账分析、差旅分析、阿宝一句话等）均走新模型

**Extensions:**
- 3a. 网络请求失败：系统回退高亮到之前的选择，显示 toast "保存失败"
- 4a. 模型值不合法：服务端返回 400，前端回退并提示错误
- 5a. 用户选择的模型 API key 不可用：服务端自动回退到其他可用模型，AI 功能正常工作（用户无感知）

---

### Use Case: 自动选择模型

**Primary Actor:** 系统
**Scope:** Next 后端
**Level:** Subfunction

**Stakeholders and Interests:**
- 用户 — AI 功能始终可用，不因模型配置问题中断

**Preconditions:**
- 用户发起了一个需要 AI 的请求（记账、差旅、英语、moment 等）

**Success Guarantee (Postconditions):**
- 成功创建 `LlmClient` 并调用对应模型 API
- 如果首选模型不可用，透明回退到备选模型

**Trigger:** 任何需要 AI 的后端请求触发 `LlmClient::for_user()`

**Main Success Scenario:**
1. 系统从 `user_settings` 读取用户的 `ai_model` 偏好
2. 偏好为 "auto"：系统按优先级尝试可用模型（Doubao → Claude）
3. 找到可用 key，创建对应 provider 的 `LlmClient`
4. 调用 LLM API 完成任务

**Extensions:**
- 1a. 用户无 `user_settings` 记录：使用默认值 "auto"
- 2a. 偏好为具体模型（如 "doubao"）但该 key 不可用：回退到其他有 key 的模型
- 2b. 偏好为具体模型且 key 可用：直接使用该模型
- 4a. LLM API 返回 429（限流）：重试最多 2 次，间隔指数递增
- 4b. LLM API 返回其他错误：返回用户友好的错误信息

---

### Use Case: Guest 用户查看模型设置

**Primary Actor:** Guest 用户
**Scope:** Next 前端
**Level:** Subfunction

**Stakeholders and Interests:**
- Guest 用户 — 了解功能存在但无法修改

**Preconditions:**
- 用户以 guest 身份访问

**Success Guarantee (Postconditions):**
- 模型切换 UI 显示但处于禁用状态
- 不发送任何 API 请求

**Trigger:** Guest 用户打开设置页

**Main Success Scenario:**
1. 设置页加载，检测到 guest 身份
2. AI 模型按钮显示为禁用状态，附提示"注册后可切换模型"
3. Guest 点击按钮无反应

**Extensions:**
- （无）
