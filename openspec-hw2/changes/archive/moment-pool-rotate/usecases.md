## Use Cases

### Use Case: 打开应用看到每日一言

**Primary Actor:** 用户
**Scope:** Next 任务管理应用
**Level:** User goal

**Stakeholders and Interests:**
- 用户 — 打开应用时看到二狗风格的一句话，有陪伴感
- 系统运营 — 控制 LLM token 成本

**Preconditions:**
- 用户已登录

**Success Guarantee (Postconditions):**
- 顶栏显示一条二狗风格的每日一言
- 前端本地缓存了当天的候选池，后续轮换无需网络请求

**Trigger:** 用户打开应用 / 页面加载完成

**Main Success Scenario:**
1. 前端检查本地是否有当天的候选池（日期匹配）
2. 本地池存在且未过期，前端从池中随机取一条显示
3. 顶栏天气图标和文字淡入呈现

**Extensions:**
- 1a. 本地无池或池已过期（跨天）：前端请求 `GET /api/moment`，后端生成 ~30 条候选池返回，前端缓存到本地，取一条显示
- 1a-i. 后端当天已为该用户生成过池（内存缓存命中）：直接返回缓存的池，不调 LLM
- 1a-ii. 后端首次生成：调用 LLM 批量生成，缓存结果，返回给前端
- 1a-iii. LLM 调用失败：后端返回 fallback 池（基于时段的固定问候语列表），前端正常使用

---

### Use Case: 点击换一条

**Primary Actor:** 用户
**Scope:** Next 任务管理应用
**Level:** User goal

**Stakeholders and Interests:**
- 用户 — 觉得当前那条看腻了或想看看别的，点一下换新的
- 系统运营 — 换一条不产生任何 API 调用

**Preconditions:**
- 顶栏已显示每日一言
- 本地候选池已加载

**Success Guarantee (Postconditions):**
- 显示池中另一条（不重复上一条）
- 无网络请求发生

**Trigger:** 用户点击天气图标 或 点击刷新按钮

**Main Success Scenario:**
1. 用户点击顶栏天气图标或刷新按钮
2. 前端从本地池中随机取一条（避免与当前显示的重复）
3. 文字以淡入过渡切换显示
4. 如果触发来源是刷新按钮，同时执行原有的页面数据刷新逻辑

**Extensions:**
- 2a. 池中只剩一条（或池为空）：保持当前显示不变
- 2b. 本地池不存在（异常情况）：静默请求一次 `GET /api/moment` 重新获取池

---

### Use Case: 后端生成每日候选池

**Primary Actor:** 系统（由用户请求触发）
**Scope:** Next 后端服务
**Level:** Subfunction

**Stakeholders and Interests:**
- 系统运营 — 每用户每天最多一次 LLM 调用

**Preconditions:**
- 该用户当天尚未生成过池（缓存未命中）

**Success Guarantee (Postconditions):**
- 内存缓存中存有该用户当天的候选池（~30 条）
- 池带有生成日期标记，跨天自动失效

**Trigger:** 前端请求 `GET /api/moment` 且缓存未命中

**Main Success Scenario:**
1. 后端构建用户上下文（任务数、完成数、紧急/逾期等）
2. 后端组装批量生成 prompt，要求 LLM 一次输出 30 条候选
3. LLM 返回结果，后端解析为数组
4. 后端将池存入内存缓存（key=用户ID，TTL=当天结束）
5. 返回池给前端

**Extensions:**
- 3a. LLM 返回格式异常（无法解析为数组）：使用 fallback 问候语池
- 3b. LLM 超时或网络错误：使用 fallback 问候语池
- 3c. 访客用户 AI 配额不足：使用 fallback 问候语池，不消耗配额

**Open Questions:**
- 缓存 TTL 策略：按自然日（0 点过期）还是按生成后 24 小时？建议按自然日，用户体验更一致
