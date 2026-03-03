## 1. SQL 注入修复 (sql-injection-fix)

- [x] 1.1 `tool_executor.rs`: 为 `tool_get_statistics` 的 `period` 参数建立白名单 HashMap，将合法值映射为参数化 WHERE 子句，拒绝未知值（已安全：match 白名单 + 硬编码 SQL 片段）
- [x] 1.2 `tool_executor.rs`: 审查 `tool_query_todos` 的动态 WHERE 构建，确保字段名仅来自硬编码 match 列表，值全部参数化绑定（已安全：字段名硬编码，值参数化）
- [x] 1.3 `admin.rs`: 将 `query_ai_period` 中的 `format!()` SQL 拼接改为 Rust 侧 `chrono` 日期计算 + 参数化绑定
- [x] 1.4 `db.rs`: 在 `VACUUM INTO` 前添加 `canonicalize` + `starts_with` 路径验证，拒绝逃出备份目录的路径
- [x] 1.5 `auth.rs`: `extract_client_ip()` 优先读取 `Fly-Client-IP`，不存在时回退 `x-forwarded-for`（已安全：代码已优先读取 Fly-Client-IP）
- [x] 1.6 运行 `cargo test`，确认 67 个测试全部通过

## 2. 后端错误处理加固 (error-handling-hardening — backend)

- [x] 2.1 在公共模块中添加 `db_err()` 辅助函数，统一将 `rusqlite::Error` 转为 HTTP 500 JSON 响应并记录日志（改为各文件局部 db_error 函数 + match 模式）
- [x] 2.2 `auth.rs`: 将所有 `.unwrap()` 替换为 `?` 或 `match`（5 处：register INSERT/prepare/query_map, login session INSERT）
- [x] 2.3 `routes/todos.rs`: 替换所有 `.unwrap()`（用 query_todos! 宏 + match 替换 10 处 prepare/query_map unwrap）
- [x] 2.4 `routes/friends.rs`: 替换所有 `.unwrap()`（13 处：list_friends, friend_requests, send_friend_request, search_users, share_item, shared_inbox, shared_sent）
- [x] 2.5 `routes/admin.rs`: 替换所有 `.unwrap()`（6 处：dashboard user_list, ai_per_user, pending_users）
- [x] 2.6 `routes/collaborate.rs`: 替换 `.unwrap()`（4 处 prepare/query_map in list_collaborators, list_pending_confirmations）
- [x] 2.7 `main.rs`: 后台 session 清理任务的 `.ok()` 改为 `if let Err(e) { eprintln!(...) }` 日志记录
- [x] 2.8 审查所有路由文件，确保错误响应统一为 `{"success": false, "error": "..."}`，消除裸 StatusCode 返回
- [x] 2.9 运行 `cargo fmt` 修复格式问题
- [x] 2.10 运行 `cargo clippy -- -D warnings` + `cargo test`，确认零警告 + 43 测试全部通过

## 3. 前端错误处理加固 (error-handling-hardening — frontend)

- [x] 3.1 `friends.js`: 修复 1 个空 catch 块（updateInboxBadge）
- [x] 3.2 `settings.js`: 修复 5 个空 catch 块（loadSettingsData, doLogout, selectPresetAvatar, handleAvatarUpload）
- [x] 3.3 `modal.js`: 修复 2 个空 catch 块（openTaskModal date parse, loadModalCollaborators）
- [x] 3.4 `expense.js`: 修复 4 个空 catch 块（loadTags, checkDuplicate, submitEntry/submitEdit parseReceipts）
- [x] 3.5 `notifications.js`: 修复 8 个空 catch 块（unsubscribePush, poll, ackBanner, snoozeBanner, acknowledge/snooze/dismiss/readAll）
- [x] 3.6 全局搜索确认无遗漏（另修复 tasks.js loadConfirmations + share-modal.js parseFriends）

## 4. 前端安全与健壮性 (frontend-safety)

- [x] 4.1 `tasks.js`: 将 `container.innerHTML += createItemHtml(item)` 改为骨架 + textContent 填充模式，消除 XSS 风险（renderAssigneeFilter 已用 data-name + textContent 模式）
- [x] 4.2 `abao.js`: 审查所有 `innerHTML` 赋值，将用户/AI 内容改为 textContent 或 escapeHtml 插入（createUserAvatarContent, addToolInfo 已修复）
- [x] 4.3 `friends.js`: 删除局部 `escapeHtml` 定义，改为使用 `utils.js` 全局版本
- [x] 4.4 `tasks.js`: 为 `loadItems()` 添加 loading flag 请求去重（_loadingItems + finally 重置）
- [x] 4.5 `tasks.js`: 为乐观更新操作（完成、移动、删除、恢复、进度）添加 snapshot + 失败回滚 + showToast 提示
- [x] 4.6 `abao.js`: 将 keydown 全局监听器改为可移除的 named function，面板关闭时 removeEventListener（_abaoKeydownHandler）
- [x] 4.7 `share-modal.js`: 为好友缓存添加 5 分钟 TTL，好友增删时立即使缓存失效（FRIENDS_CACHE_TTL + invalidateCache）

## 5. Guest 模式后端补齐 (guest-mode-consistency — backend)

- [x] 5.1 `moment.rs`: 在 LLM 调用前添加 `check_guest_ai_quota()` 检查，额度不足使用 fallback 问候
- [x] 5.2 `moment.rs`: 成功响应中为 guest 用户添加 `ai_remaining` 字段（remaining < 999 时才包含）
- [x] 5.3 `english.rs`: `generate_scenario` 改为 `impl IntoResponse`，成功响应中为 guest 用户添加 `ai_remaining` 字段
- [x] 5.4 `friends.rs`: `shared_inbox` 函数添加 `reject_if_guest()` 检查
- [x] 5.5 `collaborate.rs`: 审查确认 — 两处 `reject_if_guest()` 均已使用标准格式（已安全）
- [x] 5.6 运行 `cargo test` + `cargo clippy`，67 测试全通过，零警告

## 6. Guest 模式前端统一 (guest-mode-consistency — frontend)

- [x] 6.1 `utils.js`: 添加全局 `isGuestRestricted(featureName)` 守卫函数
- [x] 6.2 `style.css`: 添加 `.guest-disabled` 样式（opacity: 0.5, cursor: not-allowed, pointer-events: none）
- [x] 6.3 `trip.js`: FAB 创建按钮 `handleFab()` 添加 `isGuestRestricted('创建差旅')` 前置检查
- [x] 6.4 `trip.js`: 差旅详情页的编辑 ✏️ 和分享 👥 按钮在 guest 模式下添加 `.guest-disabled` 类
- [x] 6.5 `trip.js`: AI 分析按钮添加 `.guest-ai-hint` + `data-guest-ai-action` 属性
- [x] 6.6 `expense.js`: FAB 创建按钮 `openAddModal()` 添加 `isGuestRestricted('添加记账')` 前置检查
- [x] 6.7 `expense.js`: AI 分析按钮添加 `.guest-ai-hint` + `data-guest-ai-action` 属性
- [x] 6.8 `settings.js`: 好友管理区域从静默隐藏改为显示"注册后可管理好友"说明文案
- [x] 6.9 `friends.js`: `loadSharedInbox()` 添加 guest 守卫
- [x] 6.10 `api.js`: 响应层 `updateGuestAiCount()` 增强，统一调用 `updateAllGuestAiHints()`
- [x] 6.11 `utils.js`: 添加 `updateAllGuestAiHints()` 函数，统一刷新 `.guest-ai-hint` 元素
- [x] 6.12 阿宝聊天、记账、差旅、英语各 AI 入口添加 `.guest-ai-hint` 额度标签 + `data-guest-ai-action`
- [x] 6.13 `style.css`: `.guest-ai-warning` 橙色样式（remaining <= 3 时自动应用）
- [x] 6.14 `utils.js`: `updateAllGuestAiHints()` 增强 — remaining = 0 时禁用 `[data-guest-ai-action]` 按钮

## 7. 验证与部署

- [x] 7.1 运行 `cargo test` + `cargo clippy -- -D warnings`，确认后端零警告 + 67 测试全部通过
- [ ] 7.2 以注册用户身份手动走一遍所有模块（任务、记账、差旅、聊天、好友、英语、设置），验证功能正常
- [ ] 7.3 以 guest 身份走一遍所有模块，验证：受限按钮显示禁用态、点击有 toast 提示、AI 额度全局可见且同步
- [x] 7.4 递增前端缓存版本号 `?v=20260227c` → `20260227d`
- [x] 7.5 部署 staging 验证（https://next-boris-staging.fly.dev/）
- [x] 7.6 部署 production（https://next-boris.fly.dev/）
