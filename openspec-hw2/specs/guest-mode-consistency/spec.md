## ADDED Requirements

### Requirement: Guest 受限功能前端前置拦截
所有 guest 不可用的创建/修改操作（差旅创建、记账创建、好友管理、分享、协作编辑）SHALL 在前端点击时立即拦截，显示 `showToast("体验模式不支持此功能，注册账户解锁", "warning")`，SHALL NOT 发起后端 API 请求。

#### Scenario: Guest 点击差旅 FAB 创建按钮
- **WHEN** `window._userStatus === 'guest'` 且用户点击差旅页的 FAB +
- **THEN** 前端立即显示 toast 提示，不打开创建表单，不发起 API 请求

#### Scenario: Guest 点击记账 FAB 创建按钮
- **WHEN** `window._userStatus === 'guest'` 且用户点击记账页的 FAB +
- **THEN** 前端立即显示 toast 提示，不打开记账表单

#### Scenario: Guest 点击分享按钮
- **WHEN** `window._userStatus === 'guest'` 且用户点击差旅详情的分享按钮 👥
- **THEN** 前端显示 toast 提示，不打开分享弹窗

#### Scenario: 注册用户点击同一按钮
- **WHEN** `window._userStatus !== 'guest'` 且用户点击差旅 FAB +
- **THEN** 正常打开创建表单，无拦截

### Requirement: Guest 受限按钮显示禁用态
Guest 模式下不可用的按钮 SHALL 显示视觉禁用态（`opacity: 0.5` + `cursor: not-allowed`），与可用按钮形成明确区分。SHALL NOT 完全隐藏这些按钮。

#### Scenario: 差旅详情页按钮布局
- **WHEN** Guest 用户查看差旅详情页
- **THEN** 编辑按钮 ✏️ 和分享按钮 👥 显示为半透明禁用态，导出按钮 ⬇️ 正常显示

#### Scenario: 设置页好友管理区域
- **WHEN** Guest 用户打开设置页
- **THEN** 好友管理区域显示"注册后可管理好友"说明文案，而非静默隐藏整个区域

### Requirement: Guest AI 额度全局可见
Guest 模式下，所有 AI 功能入口 SHALL 显示当前剩余 AI 次数。显示位置包括：阿宝聊天输入框、记账 AI 分析按钮、差旅 AI 分析按钮、英语场景生成按钮。

#### Scenario: 打开阿宝聊天
- **WHEN** Guest 用户打开阿宝聊天面板
- **THEN** 输入框下方显示"AI 剩余 N 次"

#### Scenario: 进入记账页面
- **WHEN** Guest 用户进入记账页面，查看 AI 分析入口
- **THEN** AI 分析按钮旁显示"剩余 N 次"

#### Scenario: AI 额度降至低位
- **WHEN** Guest 剩余 AI 次数 <= 3
- **THEN** 额度提示变为醒目样式（如橙色），附带"注册解锁无限使用"

#### Scenario: AI 额度耗尽
- **WHEN** Guest 剩余 AI 次数 = 0
- **THEN** 所有 AI 按钮变为禁用态，显示"AI 体验次数已用完 — 注册解锁"

### Requirement: Guest AI 额度跨模块同步
前端 SHALL 维护全局 `window._guestAiRemaining` 变量。任意 AI 端点响应中的 `ai_remaining` 字段 SHALL 更新该全局变量，所有 AI 入口的显示 SHALL 同步刷新。

#### Scenario: 聊天消耗后切换到记账
- **WHEN** Guest 在阿宝聊天消耗 1 次 AI（剩余从 20→19），然后切换到记账页面
- **THEN** 记账页的 AI 分析按钮旁显示"剩余 19 次"

#### Scenario: 多个 AI 端点返回 ai_remaining
- **WHEN** `/api/chat` 返回 `ai_remaining: 15`
- **THEN** `window._guestAiRemaining` 更新为 15，所有已渲染的额度显示同步更新

### Requirement: 后端所有 AI 端点返回 ai_remaining
对于 guest 用户，所有调用 LLM 的 API 端点 SHALL 在成功响应中包含 `ai_remaining` 字段。包括：`/api/chat`、`/api/expenses/{id}/parse`、`/api/trips/analyze`、`/api/english/scenarios/{id}/generate`、`/api/moment`。

#### Scenario: 英语场景生成返回剩余额度
- **WHEN** Guest 调用 `/api/english/scenarios/{id}/generate` 成功
- **THEN** 响应 JSON 包含 `"ai_remaining": N`（当前缺失此字段）

#### Scenario: moment 端点返回剩余额度
- **WHEN** Guest 调用 `/api/moment` 成功
- **THEN** 响应 JSON 包含 `"ai_remaining": N`

#### Scenario: 非 guest 用户调用 AI 端点
- **WHEN** 注册用户调用 `/api/chat`
- **THEN** 响应 SHALL NOT 包含 `ai_remaining` 字段（或包含但值不影响前端行为）

### Requirement: moment 端点执行 Guest AI 额度检查
`/api/moment` 端点 SHALL 在调用 LLM 前检查 guest AI 额度（调用 `check_guest_ai_quota`）。额度不足时 SHALL 返回 HTTP 403 及额度耗尽提示。

#### Scenario: Guest 有剩余额度访问 moment
- **WHEN** Guest 用户调用 `/api/moment`，剩余 AI 次数 > 0
- **THEN** 正常调用 LLM，扣减额度，返回结果和 `ai_remaining`

#### Scenario: Guest 额度耗尽访问 moment
- **WHEN** Guest 用户调用 `/api/moment`，剩余 AI 次数 = 0
- **THEN** 返回 HTTP 403 `{"success": false, "error": "AI_QUOTA_EXCEEDED", "message": "AI 体验次数已用完"}`

### Requirement: reject_if_guest 错误格式统一
所有使用 `reject_if_guest()` 的端点 SHALL 直接返回该函数生成的标准错误响应，SHALL NOT 用自定义消息覆盖。标准格式：`{"success": false, "error": "GUEST_RESTRICTED", "message": "体验模式不支持此功能，注册账户解锁"}`。

#### Scenario: 好友操作被 guest 拒绝
- **WHEN** Guest 调用 `/api/friends/request`
- **THEN** 返回标准 `reject_if_guest()` 格式，`error` 字段为 `"GUEST_RESTRICTED"`

#### Scenario: 协作操作被 guest 拒绝
- **WHEN** Guest 调用 `/api/collaborate/todos/{id}`
- **THEN** 返回相同的标准格式，不使用自定义 message 覆盖

### Requirement: shared_inbox 对 Guest 行为一致
`/api/friends/shared-inbox` 端点 SHALL 对 guest 用户执行 `reject_if_guest()` 检查，与 `accept_shared` 和 `dismiss_shared` 保持一致。

#### Scenario: Guest 访问 shared inbox
- **WHEN** Guest 调用 `/api/friends/shared-inbox`
- **THEN** 返回 `GUEST_RESTRICTED` 错误，与操作类端点行为一致

#### Scenario: 前端 guest 模式不加载 shared inbox
- **WHEN** 前端检测到 `window._userStatus === 'guest'`
- **THEN** `loadSharedInbox()` 跳过执行（添加 guest 守卫），不发起 API 请求
