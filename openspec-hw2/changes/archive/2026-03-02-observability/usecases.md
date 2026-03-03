## Use Cases

### Use Case: Diagnose user-reported frontend error

**Primary Actor:** Developer (Boris)
**Scope:** Next 应用（前端 + 后端 + SQLite）
**Level:** User goal

**Stakeholders and Interests:**
- Developer — 快速定位用户遇到的问题根因
- User — 问题被修复

**Preconditions:**
- 前端全局错误拦截已部署
- `client_errors` 表已创建
- 用户正常使用应用

**Success Guarantee (Postconditions):**
- 错误信息、堆栈、操作轨迹已存入 `client_errors` 表
- Developer 可通过 SQL 查询定位根因

**Trigger:** 用户报告"某功能不好使"

**Main Success Scenario:**
1. 用户操作触发未捕获的 JS 错误
2. 系统拦截错误，收集错误信息 + 堆栈 + 最近 20 条操作轨迹
3. 系统将错误数据 POST 到 `/api/client-errors`
4. 后端将错误存入 `client_errors` 表
5. Developer 通过 `fly ssh` + `sqlite3` 查询最近错误
6. Developer 根据 breadcrumbs 还原用户操作路径，定位根因

**Extensions:**
- 3a. 网络不可用：系统将错误存入 localStorage 离线缓冲（最多 5 条），下次页面加载时补发
- 3b. 同一 error_message 已上报过：系统去重，不重复上报
- 3c. 当前会话已上报 10 条：系统停止上报，避免洪泛
- 4a. IP 限流触发（>10条/分钟）：后端返回 429，前端静默忽略

### Use Case: Debug issue on mobile device

**Primary Actor:** Developer (Boris)
**Scope:** Next 前端（手机浏览器）
**Level:** User goal

**Stakeholders and Interests:**
- Developer — 在手机上实时查看 console/network/DOM

**Preconditions:**
- Eruda.js 已自托管在 `assets/vendor/eruda.min.js`
- 用户已登录

**Success Guarantee (Postconditions):**
- Eruda DevTools 面板在手机浏览器中显示
- Developer 可查看 console、network、elements、storage

**Trigger:** Developer 需要在手机上调试问题

**Main Success Scenario:**
1. Developer 在手机浏览器中打开应用，URL 加上 `?debug=1`
2. 系统动态加载 Eruda.js 并初始化
3. 系统在 localStorage 存入 `eruda_enabled=1`，刷新后保持
4. Developer 使用 Eruda 面板调试问题
5. Developer 完成调试，访问 `?debug=0` 关闭

**Extensions:**
- 1a. 使用隐藏手势：Developer 连续点击版本号 5 次，等效触发
- 2a. Eruda.js 加载失败（离线）：静默忽略，不影响正常使用
- 5a. 再次 5 连击：等效关闭 Eruda

### Use Case: Review backend request logs

**Primary Actor:** Developer (Boris)
**Scope:** Next 后端（Axum）
**Level:** User goal

**Stakeholders and Interests:**
- Developer — 查看请求级日志定位后端问题

**Preconditions:**
- `tracing` + `TraceLayer` 已配置

**Success Guarantee (Postconditions):**
- 每个 HTTP 请求自动记录方法、路径、状态码、耗时
- 日志可通过 `fly logs` 实时查看

**Trigger:** 需要排查后端异常

**Main Success Scenario:**
1. 用户请求触达后端
2. TraceLayer 自动记录请求方法、路径、状态码、耗时
3. 业务代码中的 `tracing` 宏输出结构化日志
4. Developer 通过 `fly logs` 查看实时日志流
5. Developer 结合 `client_errors` 表中的同时间段记录进行前后端关联

**Extensions:**
- 3a. 旧代码仍用 `eprintln!()`：不阻断，渐进迁移
