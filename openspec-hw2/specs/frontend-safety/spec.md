## ADDED Requirements

### Requirement: 用户内容通过安全 DOM API 插入
所有用户提供的文本（任务标题、备注、AI 回复、共享内容）SHALL 通过 `textContent`、`escapeHtml()` 或 `createElement` 插入 DOM。SHALL NOT 将用户内容直接拼接到 `innerHTML` 字符串中。

#### Scenario: 渲染包含 HTML 标签的任务标题
- **WHEN** 任务标题为 `<script>alert(1)</script>`
- **THEN** 页面显示原始文本 `<script>alert(1)</script>`，无脚本执行

#### Scenario: 渲染 AI 聊天回复
- **WHEN** AI 回复内容包含 `<img src=x onerror=alert(1)>`
- **THEN** 聊天气泡显示转义后的文本，不触发 onerror 事件

#### Scenario: innerHTML 用于结构性 HTML
- **WHEN** 代码使用 innerHTML 构建任务卡片的布局骨架
- **THEN** innerHTML 仅包含硬编码的 HTML 常量（class、结构标签），用户数据通过后续的 `el.textContent = userInput` 填入

### Requirement: escapeHtml 统一实现
项目 SHALL 使用 `utils.js` 中的 `escapeHtml()` 作为唯一的 HTML 转义函数。其他模块（如 `friends.js`）中的局部 `escapeHtml` 定义 SHALL 被移除，改为引用 `utils.js` 版本。

#### Scenario: friends.js 调用 escapeHtml
- **WHEN** `friends.js` 需要转义用户名显示
- **THEN** 调用全局 `escapeHtml()`（来自 `utils.js`），不使用模块内局部定义

### Requirement: 数据加载请求去重
前端数据加载函数（`loadItems`、`loadExpenses` 等）SHALL 实现请求去重：同一操作类型同时只允许一个请求在途。重复调用 SHALL 被跳过或取消前一个请求。

#### Scenario: 快速切换标签页触发重复加载
- **WHEN** `loadItems()` 正在执行中，用户切换标签后返回再次触发 `loadItems()`
- **THEN** 第二次调用被跳过（loading flag 拦截），第一个请求完成后正常渲染

#### Scenario: 筛选条件变更时取消旧请求
- **WHEN** 用户切换筛选条件，旧的 `loadItems()` 请求尚未返回
- **THEN** 旧请求的响应被忽略（通过 generation counter），新请求正常发起并渲染

### Requirement: 乐观更新失败时回滚
前端对任务的乐观 UI 更新（完成、移动、删除等）SHALL 在 API 调用失败时回滚到操作前状态，并显示错误提示。

#### Scenario: 任务完成操作 API 失败
- **WHEN** 用户勾选任务完成，UI 立即反映变更，但 API 返回错误
- **THEN** UI 恢复任务到未完成状态，显示 `showToast("操作失败，请重试", "error")`

#### Scenario: 任务完成操作 API 成功
- **WHEN** 用户勾选任务完成，API 返回成功
- **THEN** 乐观状态确认为正式状态，不发生回滚

### Requirement: 事件监听器正确清理
动态添加的全局事件监听器（如 `document.addEventListener`）SHALL 在相关功能关闭或页面切换时被移除，防止重复绑定和内存泄漏。

#### Scenario: abao 面板关闭后键盘监听
- **WHEN** abao 聊天面板关闭
- **THEN** 面板注册的 `keydown` 监听器被移除，按 B 键不再触发面板

#### Scenario: 多次打开关闭面板
- **WHEN** 用户打开→关闭→打开 abao 面板 3 次
- **THEN** 只有 1 个 `keydown` 监听器处于活跃状态，不会触发 3 次回调

### Requirement: SW error capture
Service Worker catch blocks SHALL log errors with `console.error('[SW]', error)` instead of silently swallowing them. The SW SHALL also listen for `error` and `unhandledrejection` events.

#### Scenario: SW fetch handler error
- **WHEN** the SW fetch handler encounters an error
- **THEN** the catch block logs `console.error('[SW]', error)` before falling back

#### Scenario: SW global error
- **WHEN** an uncaught error occurs in the SW scope
- **THEN** the `error` event listener logs it with `console.error('[SW] uncaught:', event.error)`

### Requirement: 好友缓存设置过期策略
share-modal 的好友列表缓存 SHALL 设置 5 分钟 TTL。超过 TTL 后重新请求服务端数据。好友增删操作 SHALL 立即使缓存失效。

#### Scenario: 缓存未过期时打开分享弹窗
- **WHEN** 用户在 3 分钟内第二次打开分享弹窗
- **THEN** 使用缓存的好友列表，不发起新请求

#### Scenario: 缓存过期后打开分享弹窗
- **WHEN** 用户在 6 分钟后打开分享弹窗
- **THEN** 重新从服务端获取好友列表

#### Scenario: 添加好友后打开分享弹窗
- **WHEN** 用户添加了一个新好友，然后打开分享弹窗
- **THEN** 缓存已被好友操作失效，分享弹窗显示包含新好友的最新列表
