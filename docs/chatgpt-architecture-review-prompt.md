# 请你对以下项目做一个系统性的架构和功能评估

我是这个项目的独立开发者。请你从技术架构、产品设计、工程实践、可扩展性等角度给出评估和建议。不用客套，直接说问题和机会。

---

## 一、项目概述

**Next — Focus on the Right Thing**

个人任务管理 Web 应用。核心理念：优先级泳道（艾森豪威尔矩阵）+ 时间维度（今天/本周/本月），帮用户看清"下一步该做什么"。

内嵌 AI 助手"二狗"，基于 Claude API，定位是"毒舌损友"——嘴上不饶人、干活特靠谱。不是客服不是教练，是那个会吐槽你拖延但帮你把事理清楚的朋友。

**目标用户规模**: 1-10 人（个人/小团队），不追求企业级。

**线上地址**: https://next-boris.fly.dev

---

## 二、技术栈

| 层 | 技术 | 说明 |
|----|------|------|
| 后端 | Rust (Axum 0.8) | 单进程，~10MB 二进制 |
| 数据库 | SQLite (WAL mode) | Arc<Mutex<Connection>>，单写多读 |
| 前端 | Vanilla HTML/CSS/ES5 JS | 零构建工具，35 个 JS 模块 |
| AI | Claude Sonnet 4.5 | 对话模式 + 工具调用 |
| 部署 | Docker + Fly.io (东京 nrt) | 256MB RAM，auto-stop |
| PWA | Service Worker | 离线缓存，Web Push 通知 |

**选型原则**: 极简、单二进制、零运维依赖。没有 Redis、没有消息队列、没有构建步骤。

---

## 三、系统架构

```
浏览器 (PWA + Vanilla JS)
    ↓ HTTPS
Fly.io (东京)
    ├── Axum 0.8 (单进程)
    │   ├── 静态文件服务 (ServeDir)
    │   ├── API 路由 (/api/*)
    │   │   ├── 认证路由 (register/login/logout)
    │   │   └── 20 个业务路由模块
    │   └── 安全中间件 (CSP/HSTS)
    │
    ├── 服务层
    │   ├── ClaudeClient (对话 + 工具调用循环)
    │   ├── ContextBuilder (系统提示词 + 任务上下文注入)
    │   ├── ToolExecutor (16 个 AI 可调用工具)
    │   ├── PushService (VAPID + AES-GCM 加密)
    │   ├── ReminderPoller (30s 轮询后台任务)
    │   └── CollaborationEngine (协作确认流)
    │
    ├── SQLite (WAL mode)
    │   └── 22 张表
    │
    └── 后台任务
        ├── 提醒轮询 (30s)
        └── 每日备份 (VACUUM INTO)
    ↓ HTTPS
Anthropic API (Claude)
```

---

## 四、功能模块全景

### 4.1 核心：四象限任务管理

- **3 个时间标签**: 今天 / 本周 / 本月
- **4 个优先级泳道**: 重要紧急 / 重要不紧急 / 不重要紧急 / 待分类
- 支持拖拽跨泳道、跨标签
- 进度追踪 (0-100%)、截止日期、标签、Markdown 描述
- 软删除 + 回收站恢复
- 变更日志 (changelog)

### 4.2 例行习惯 (Routines)

- 每日打卡式习惯追踪
- 支持协作（多人共同维护一个习惯）
- 完成记录按日期存储

### 4.3 定期审视 (Reviews)

- 频率：每天/每周/每月/每年 + 自定义
- 可暂停
- 用于定期反思："这周最重要的三件事做了吗？"

### 4.4 AI 助手"二狗"

**两种工作模式**：

1. **对话模式** (`/api/chat`)
   - 完整对话历史
   - 工具调用循环 (最多 5 轮)
   - 上下文注入：当前任务数据、紧急程度、完成情况、当前页面
   - 16 个可用工具（创建/修改/查询任务、记账、提醒等）

2. **此刻模式** (`/api/moment`)
   - 手机顶栏一句话状态
   - 时间感知（早/午/晚/深夜不同语气）
   - 15 分钟内存缓存
   - API 失败时回退到本地 fallback

**人设核心**：
- 毒舌损友，话少管用，冷幽默
- 执行优先——用户说"加个任务"就直接调工具，不反问确认
- 有严格的职责边界——只管 Next 里的事，闲聊按"浪费主人狗粮"拒绝
- 有完整的行为边界规范（16 条规则 + 决策原则），核心原则：**存在的意义不是增加互动，而是减少犹豫**

**上下文注入机制** (`context.rs`)：
- 每次对话重建系统提示词
- 注入：任务统计、今日待办详情、当前页面、用户正在查看的具体数据
- 注入：用户的人物档案（二狗认识用户生活中的人，知道怎么称呼）
- 注入：用户记忆（习惯、事实、性格偏好、未完成意图）
- 主人（管理员）在线时切换为特殊忠犬模式

### 4.5 记账 (Expenses)

- 手动记账 + AI 拍照识别
- 支持多币种 (CAD/CNY)
- 标签分类、按月汇总
- AI 分析：支持纯文字/纯图片/图文混合

### 4.6 差旅 (Trips)

- 行程创建（出发/目的地/日期）
- 行程内消费条目（机票/酒店/餐饮等）
- 报销状态追踪
- 支持协作

### 4.7 学习笔记 (English Scenarios)

- 场景化学习内容管理
- AI 生成对话/内容
- 支持多分类（不限于英语）

### 4.8 提醒与推送

- 自然语言时间解析（"3点提醒我开会"→ 今天15:00）
- 后台轮询 (30s) → 触发 → Web Push + 应用内通知
- 支持暂停 (snooze) 和确认 (acknowledge)
- VAPID + AES-GCM 加密的 Web Push

### 4.9 社交与协作

- 好友系统（请求/接受/拒绝）
- 任务分享（发送到对方收件箱）
- 协作任务（多人各自管理自己的象限和排序）
- 协作确认流（A 标记完成 → B 确认）

### 4.10 巡逻系统 (Patrol) — 二狗的视觉存在

- 二狗在页面上留下半透明爪印，像真的有只狗在巡逻
- 状态机：off_duty → on_duty → patrol → standby → converge
- 有机步态节奏（非匀速），冷却 ≥3 分钟
- 滚动时暂停，用户操作时退场
- `prefers-reduced-motion` 完全关闭
- 狗窝按钮有间歇呼吸动画（2 次呼吸 → 8-12s 休息）
- pointer-events: none，绝不阻挡用户交互

---

## 五、数据模型

**22 张表**，关键设计：

- `users` — 账号、头像、角色 (admin/user/guest)
- `todos` — 四象限任务，含 tab/quadrant/progress/completed/deleted
- `todo_changelog` — 变更历史
- `todo_collaborators` — 协作关系（每人独立 tab/quadrant/sort_order）
- `routines` + `routine_completions` — 习惯 + 按日打卡记录
- `reviews` — 定期审视
- `sessions` — HttpOnly cookie 会话（每用户最多 5 个）
- `conversations` + `chat_messages` + `chat_usage_log` — 对话管理 + token 追踪
- `expense_entries` — 记账条目
- `trips` + `trip_items` + `trip_collaborators` — 差旅
- `english_scenarios` — 学习笔记
- `friendships` + `shared_items` — 社交
- `reminders` + `push_subscriptions` + `notifications` — 提醒系统
- `ergou_people` — 二狗的人物档案（知道用户身边的人）
- `ergou_memories` — 二狗的记忆（用户习惯、事实、偏好）
- `pending_confirmations` + `confirmation_responses` — 协作确认流

**隔离策略**: 所有查询包含 `WHERE user_id = ?`，外键 CASCADE DELETE。

---

## 六、安全架构

- **密码**: Argon2 哈希 (GPU 抗性)
- **会话**: 32 字节随机 token，HttpOnly + SameSite=Lax + Secure
- **HTTP 头**: CSP (default-src 'self'), HSTS (2 年), X-Content-Type-Options: nosniff
- **AI 安全**: 系统提示词不可泄露，工具调用绑定 user_id，任务内容不作为 AI 指令执行
- **安全巡逻**: 用户反复刺探他人数据 → 递进式警告 → 上报事件
- **记忆隐私**: 每用户独立记忆，绝不跨用户透露

---

## 七、部署与运维

- **Fly.io**: 东京机房，shared-cpu-1x, 256MB RAM
- **Docker 多阶段构建**: rust:1.92 编译 → debian:bookworm-slim 运行，最终 ~80MB
- **数据持久化**: Fly Volume (`/data/next.db`)
- **备份**: 每日 VACUUM INTO，保留 30 天
- **缓存控制**: `?v=YYYYMMDD[a-z]` 版本号手动递增
- **Auto-stop**: 空闲时自动停机，请求时自动启动
- **内存占用**: 30-50MB

---

## 八、前端架构

- **零构建**: 35 个 JS 模块按依赖顺序加载（`<script>` 标签）
- **全局状态**: `window.currentTab`, `window.currentPage`, `window.allItems`
- **API 封装**: `API.xxx()` → fetch → 自动 401 跳转
- **拖拽**: 统一 mouse + touch，桌面 5px / 移动端 300ms 长按 + 10px 阈值
- **PWA**: Network-First + Cache-Fallback，`?v=` 缓存击穿
- **响应式**: `@media (max-width: 768px)` 移动端适配
- **Canvas 动画**: 彗星粒子 (header) + 呼吸线 (footer)

---

## 九、工程实践

- **后端日志**: `tracing` 宏，禁止 `eprintln!()`
- **前端错误**: catch 块必须 `console.error('[模块名]', error)`
- **前端可观测性**: `observability.js` 错误上报 + 面包屑追踪
- **测试文档**: `docs/tests/TEST-*.md` 手动测试用例
- **自动化测试**: `cargo test` + `cargo clippy -- -D warnings` + `cargo fmt`
- **Spec 驱动**: 57 个功能 Spec 文档，每个功能先写 Spec 再写代码
- **行为边界文档**: 二狗 16 条行为规范 + 工程对照表

---

## 十、性能数据

| 指标 | 目标 | 实际 |
|------|------|------|
| 首屏加载 | <2s (3G) | ~1.5s 缓存后 <0.5s |
| API 响应 | <100ms | SQLite <10ms |
| AI 回复 | <5s | 1-3s + 网络 |
| 内存占用 | <100MB | 30-50MB |
| 二进制大小 | <20MB | ~10MB |

---

## 十一、代码量

| 类别 | 行数 |
|------|------|
| Rust 后端 | ~5,000 |
| JavaScript 前端 | ~15,000 |
| HTML/CSS | ~3,000 |
| SQL (schema) | ~400 |
| 文档 (Spec + Ref) | 大量 |

---

## 请你评估以下方面：

1. **架构合理性**: 技术选型是否匹配目标场景？有没有过度设计或欠缺考虑的地方？
2. **可扩展性**: 如果未来用户从 10 人增长到 100 人、1000 人，哪些地方会先出问题？
3. **前端架构**: 35 个 Vanilla JS 模块 + 全局状态的方式，优劣在哪？有没有更好的组织方式？
4. **AI 集成**: 上下文注入、工具调用循环、人设系统的设计有什么可以改进的？
5. **安全性**: 有没有明显的安全薄弱点？
6. **运维风险**: 单实例 SQLite + Fly.io 的运维风险在哪？
7. **产品设计**: 功能模块是否过于分散？哪些模块的价值最高、哪些可能是累赘？
8. **二狗人设系统**: 行为边界规范、上下文注入、主人模式等设计，从 AI 产品角度怎么看？
9. **你觉得这个项目最大的优势和最大的风险分别是什么？**
10. **如果你来接手这个项目，第一件事会做什么？**

请直接给出你的判断，不需要照顾我的感受。
