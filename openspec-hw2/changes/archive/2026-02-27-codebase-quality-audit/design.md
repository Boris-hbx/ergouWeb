## Context

Next 是 Rust (Axum) + SQLite + Vanilla JS 的任务管理应用，从个人工具扩展为多用户产品。代码审计发现 4 类系统性问题：SQL 注入、错误处理缺失、前端安全隐患、guest 模式体验碎片化。本次改动横跨后端 ~10 个文件和前端 ~10 个文件，需要在不破坏现有功能的前提下逐步修复。

当前状态：
- 后端使用 `rusqlite` 直连 SQLite，通过 `Mutex<Connection>` 序列化访问
- 前端为 Vanilla JS，无框架，各模块通过全局函数和 `window.*` 变量通信
- Guest 模式通过 `window._userStatus` 和后端 `reject_if_guest()` 双层控制
- 已有 67 个自动化测试全部通过

## Goals / Non-Goals

**Goals:**
- 消除所有已知 SQL 注入和路径穿越漏洞
- 后端所有数据库操作零 `.unwrap()`，不因 DB 错误 panic
- 前端零空 catch 块，所有错误对用户可见
- Guest 模式在所有模块中行为一致且用户可预期
- 保持 67 个现有测试全部通过

**Non-Goals:**
- 不引入新的 ORM 或查询构建库（改动范围太大）
- 不重构整体架构（保持 Vanilla JS + 全局函数模式）
- 不增加新功能（纯质量改进）
- 不涉及测试覆盖率提升（属于后续工作）
- 不涉及 CSRF 防护、2FA 等 SPEC-050 范畴的安全增强

## Decisions

### D1: SQL 注入修复 — 白名单映射 + 参数化

**选择**: 为 `tool_get_statistics` 的 `period` 参数建立 `HashMap<&str, &str>` 白名单，将合法值映射为参数化 SQL 片段。`tool_query_todos` 中的字段名使用 `match` 硬编码允许列表。

**替代方案**:
- 引入 SQL 构建库（如 `sea-query`）→ 拒绝：依赖过重，改动范围远超本次修复
- 正则过滤输入 → 拒绝：黑名单方式不可靠，容易遗漏边界

**`admin.rs` 的 `query_ai_period`**: 当前将 SQLite 日期函数（如 `date('now', '-7 days')`）通过 `format!()` 拼入 SQL。改为在 Rust 侧用 `chrono` 计算日期字符串，然后作为参数绑定。

**备份路径验证**: 在 `VACUUM INTO` 前，使用 `std::fs::canonicalize` + `starts_with` 验证路径在备份目录内。canonicalize 会解析符号链接和 `..`，是防路径穿越的标准做法。

**IP 限流**: `extract_client_ip()` 优先读取 `Fly-Client-IP`，不存在时回退 `x-forwarded-for`。一行改动，零风险。

### D2: 错误处理 — `?` 操作符 + 辅助宏

**选择**: 将路由处理器的返回类型统一为 `Result<Json<Value>, (StatusCode, Json<Value>)>`（Axum 的 `IntoResponse` 支持此类型）。数据库操作使用 `?` 传播，配合统一的错误转换函数。

**具体模式**:
```rust
fn db_err(e: rusqlite::Error) -> (StatusCode, Json<Value>) {
    eprintln!("[db] error: {}", e);
    (StatusCode::INTERNAL_SERVER_ERROR,
     Json(json!({"success": false, "error": "内部错误"})))
}
```

路由处理器中：`db.prepare("...")?.query_map(...)` — 用 `?` 替代 `.unwrap()`，错误自动转为 500 响应。

**替代方案**:
- 每个 `.unwrap()` 改为 `match` → 拒绝：代码膨胀严重，200+ 处需要修改
- 引入 `anyhow` 或 `thiserror` → 拒绝：为了单一文件引入 crate 过度

**`.ok()` 处理**: 关键路径上的 `.ok()` 改为 `if let Err(e) = ... { eprintln!(...); return err_response; }`。后台任务中的 `.ok()` 改为 `if let Err(e) = ... { eprintln!(...); }` 仅记录日志。

**前端空 catch**: 逐个文件搜索 `catch(function() {})` 和 `catch(e) {}`，补充 `console.error` + `showToast`。模式：
```javascript
.catch(function(e) {
    console.error('[expense]:', e);
    showToast('操作失败', 'error');
})
```

### D3: 前端安全 — innerHTML 拆分模式

**选择**: 对于现有的 `innerHTML +=` 模式，改为"骨架 + textContent 填充"：
1. innerHTML 仅设置结构性 HTML（包含占位的 `<span class="task-title"></span>`）
2. 随后用 `el.querySelector('.task-title').textContent = userInput` 填充用户数据

这保持了现有代码结构（不需要全面重写为 `createElement`），同时消除了 XSS 风险。

**替代方案**:
- 全部改为 `createElement` + `appendChild` → 拒绝：改动量巨大，可读性差
- 引入模板引擎（如 lit-html）→ 拒绝：架构变更过大

**escapeHtml 统一**: 删除 `friends.js` 中的局部定义，改为使用 `utils.js` 的全局版本。验证 `utils.js` 的 `escapeHtml` 在所有 JS 文件加载前就位。

**请求去重**: 使用 loading flag 模式（最简单有效）：
```javascript
var _loadingItems = false;
function loadItems() {
    if (_loadingItems) return;
    _loadingItems = true;
    API.getTodos().then(...).finally(function() { _loadingItems = false; });
}
```
不使用 AbortController（兼容性考虑，且 loading flag 已足够）。

**乐观回滚**: 在操作前深拷贝 `allItems`，API 失败时恢复：
```javascript
var snapshot = JSON.parse(JSON.stringify(allItems));
// ... 乐观更新 ...
API.updateTodo(id, data).catch(function(e) {
    allItems = snapshot;
    renderItems();
    showToast('操作失败，请重试', 'error');
});
```

### D4: Guest 模式一致性 — 统一守卫函数

**选择**: 前端新增全局辅助函数 `isGuestRestricted(featureName)`：
```javascript
function isGuestRestricted(feature) {
    if (window._userStatus !== 'guest') return false;
    showToast('体验模式不支持' + feature + '，注册账户解锁', 'warning');
    return true;
}
```

各模块在操作入口处调用：
```javascript
function openTripModal() {
    if (isGuestRestricted('创建差旅')) return;
    // ... 原有逻辑
}
```

**禁用态样式**: 新增 CSS 类 `.guest-disabled`：
```css
.guest-disabled {
    opacity: 0.5;
    cursor: not-allowed;
    pointer-events: none;
}
```
在 guest 模式下，通过 JS 为受限按钮添加此类。注意使用 `pointer-events: none` 时，需在父容器而非按钮本身上绑定点击事件来显示提示。改为不用 `pointer-events: none`，而是在 click handler 中检查 guest 状态。

修正方案：
```css
.guest-disabled {
    opacity: 0.5;
    cursor: not-allowed;
}
```
按钮保持可点击，点击时由 `isGuestRestricted()` 拦截并提示。这样用户点击后能看到引导信息。

**AI 额度同步**: 在 `api.js` 的请求拦截层统一处理 `ai_remaining`：
```javascript
// 在 API._request 中，响应后检查
if (data.ai_remaining !== undefined && window._userStatus === 'guest') {
    window._guestAiRemaining = data.ai_remaining;
    updateAllGuestAiHints();  // 刷新所有 AI 入口的显示
}
```

`updateAllGuestAiHints()` 更新所有已渲染的额度标签（阿宝输入框、记账按钮、差旅按钮等）。使用 `document.querySelectorAll('.guest-ai-hint')` 统一更新。

**后端补齐**:
- `moment.rs`: 在 LLM 调用前添加 `check_guest_ai_quota()` 检查，响应中添加 `ai_remaining`
- `english.rs`: 在 `generate_scenario` 响应中添加 `ai_remaining`
- `friends.rs`: `shared_inbox` 添加 `reject_if_guest()` 检查
- `collaborate.rs`: 统一使用 `reject_if_guest()` 的标准返回值

## Risks / Trade-offs

**[风险] innerHTML 拆分可能引入渲染 bug** → 逐个文件修改，每改一处手动验证页面渲染。优先处理有明确 XSS 风险的位置（tasks.js:111、abao.js），低风险位置可推后。

**[风险] `?` 传播链改变函数签名** → 部分处理器当前返回 `impl IntoResponse`，改为 `Result` 后需确认 Axum 的类型推断。逐个函数改动，每改一个跑测试。

**[风险] Guest 前端拦截与后端双重检查不同步** → 后端 `reject_if_guest()` 保留作为最后防线。前端拦截是 UX 优化，不是安全边界。即使前端绕过，后端仍然拒绝。

**[权衡] 乐观回滚使用深拷贝 `allItems`** → 对于大列表（1000+ 项），`JSON.parse(JSON.stringify(...))` 有性能开销。实际场景中用户待办很少超过 200 项，可接受。

**[权衡] loading flag vs AbortController** → loading flag 更简单但不取消旧请求（仍消耗带宽）。对于当前的单用户低频场景，简单方案足够。

## Migration Plan

分 4 批独立部署，每批只改一个 capability，降低回滚粒度：

1. **Batch 1 — sql-injection-fix**: 后端安全修复，不影响前端。部署后可通过 admin panel 验证 AI 统计是否正常。
2. **Batch 2 — error-handling-hardening**: 后端 unwrap 替换 + 前端 catch 修复。部署后监控 error log 是否有新增日志（说明之前被吞掉的错误现在可见了）。
3. **Batch 3 — frontend-safety**: 前端 XSS 修复 + 去重 + 回滚。需更新缓存版本号。部署后验证任务列表渲染、聊天显示是否正常。
4. **Batch 4 — guest-mode-consistency**: 前后端联动。需同时部署后端和前端。部署后以 guest 身份走一遍所有模块。

回滚策略：每批部署前打 git tag，出问题直接 `fly deploy` 回退到上一个 tag。

## Open Questions

- 设置页好友区域改为"注册后可用"说明文案 vs 保持隐藏但加 tooltip — 哪种 UX 更好？暂定显示说明文案。
- Guest 是否允许导出差旅示例数据？当前方案是允许（作为体验功能），如果要限制需追加拦截。
