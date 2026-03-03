## Why

Next 从个人工具走向多用户，但代码质量和体验一致性未同步跟上。全面审查发现：

- **安全漏洞**: 4 个严重（SQL 注入、路径穿越、XSS）
- **健壮性缺陷**: 12 个重要（unwrap panic、静默吞错、竞态）
- **Guest 模式体验碎片化**: 各模块对 guest 的处理不统一，用户填完表单才发现不能用
- **测试覆盖率 ~20%**: 18 个路由模块零测试

SPEC-050 已从产品层面分析了安全·稳定·可靠的方向，本次 change 聚焦 **代码级的具体问题修复**。

## What Changes

### 安全修复 (Critical)
- 修复 `tool_executor.rs` 和 `admin.rs` 中 3 处 SQL 注入漏洞（用户输入拼接到 SQL）
- 修复 `db.rs` backup 路径穿越风险
- 消除前端 `tasks.js`、`abao.js` 中 innerHTML 拼接导致的 XSS 风险
- 修复 IP 限流绕过：从 `x-forwarded-for` 改用 `Fly-Client-IP`

### Guest 模式体验统一 (Major)
- **前端拦截**：在 trip FAB、expense FAB、AI 分析按钮等处添加 guest 前置检查，避免用户填完表单才报错
- **按钮状态**：guest 受限功能的按钮显示禁用态 + "注册解锁" 提示，而非完全隐藏或假装可用
- **AI 额度可见性**：在所有 AI 功能入口处显示剩余次数，不仅在用完后才提示
- **后端一致性**：
  - `moment.rs` 缺少 guest AI 额度检查 — 补齐
  - `english.rs` AI 响应缺少 `ai_remaining` 字段 — 补齐
  - `shared_inbox` 允许 guest 查看但不能操作 — 统一为拒绝或完整支持
  - `reject_if_guest()` 错误格式被部分调用方覆盖 — 统一

### 健壮性修复 (Major)
- 后端：将关键路径上的 `.unwrap()` 替换为正确的错误处理（涉及 auth.rs, todos.rs, friends.rs, collaborate.rs, admin.rs）
- 后端：消除 `.ok()` 静默吞错，改为日志 + 错误响应
- 前端：修复 10+ 处空 `catch {}` 块，补充错误提示
- 前端：为 `loadItems()` 等高频操作添加请求去重，防止竞态
- 前端：乐观 UI 更新添加失败回滚机制
- 前端：share-modal 好友缓存添加过期策略

### 代码质量 (Minor)
- 运行 `cargo fmt` 修复 7 个文件格式问题
- 统一 API 错误响应格式为 `{"success": false, "error": "..."}`
- 前端 `escapeHtml` 去重，统一使用 `utils.js` 版本
- 清理事件监听器泄漏（abao.js keydown listener）

## Capabilities

### New Capabilities
- `sql-injection-fix`: 参数化所有动态 SQL 查询，消除注入面
- `error-handling-hardening`: 后端 unwrap/ok 替换为结构化错误处理，前端空 catch 补充日志和用户提示
- `frontend-safety`: XSS 防护（innerHTML→DOM API）、请求去重、乐观更新回滚
- `guest-mode-consistency`: Guest 模式前后端体验统一 — 前端前置拦截、按钮禁用态、AI 额度可见性、后端授权补齐

### Modified Capabilities
（无现有 openspec-hw2 spec 需要修改）

## Impact

**后端文件**:
- `server/src/services/tool_executor.rs` — SQL 注入修复
- `server/src/routes/admin.rs` — SQL 注入修复
- `server/src/db.rs` — 路径穿越修复
- `server/src/auth.rs` — unwrap 替换 + 限流 header 修复 + reject_if_guest 统一
- `server/src/routes/todos.rs` — unwrap 替换
- `server/src/routes/friends.rs` — unwrap 替换 + shared_inbox guest 处理
- `server/src/routes/collaborate.rs` — ok() 替换
- `server/src/routes/moment.rs` — 补齐 guest AI 额度检查
- `server/src/routes/english.rs` — AI 响应补齐 ai_remaining
- 所有路由文件 — 错误响应格式统一

**前端文件**:
- `frontend/assets/js/tasks.js` — innerHTML 修复 + 请求去重 + 乐观回滚
- `frontend/assets/js/abao.js` — innerHTML 修复 + 事件监听清理 + guest AI 提示优化
- `frontend/assets/js/trip.js` — guest 前置检查（FAB、分享、AI 分析）
- `frontend/assets/js/expense.js` — guest 前置检查（FAB、AI 分析）+ 空 catch 修复
- `frontend/assets/js/friends.js` — 空 catch 修复 + loadSharedInbox guest 守卫
- `frontend/assets/js/settings.js` — 空 catch 修复 + guest 隐藏项说明文案
- `frontend/assets/js/modal.js` — 空 catch 修复
- `frontend/assets/js/share-modal.js` — 缓存过期
- `frontend/assets/js/api.js` — 响应验证
- `frontend/assets/css/style.css` — 新增 guest 禁用态样式

**依赖**: 无新增依赖
**API**: 错误响应格式统一 + moment/english 端点补齐 ai_remaining，不影响 success 路径
**测试**: 现有 67 个测试不应被破坏
