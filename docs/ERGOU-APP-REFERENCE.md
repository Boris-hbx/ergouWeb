# Next 功能模块参考文档 -- 二狗 App 对接指南

> 生成日期: 2026-03-05
> 用途: 供二狗 App 工程参考，理解 Next 后端各模块的数据结构、API、业务逻辑

---

## 总体架构

### 后端

- **技术**: Rust (Axum 0.8) + SQLite (WAL mode) + Claude API
- **部署**: Docker 单容器，Fly.io (东京 nrt)
- **认证**: Cookie Session (Argon2 密码哈希, HttpOnly, SameSite=Lax, 30天有效期)
- **API 格式**: `{ "success": true/false, ... }`, 错误: `{ "success": false, "error": "CODE", "message": "..." }`
- **数据隔离**: 所有查询强制 `WHERE user_id = ?`

### 认证

```
POST /api/auth/login   { username, password }  -> Set-Cookie: session=<hex>
POST /api/auth/register { username, password, display_name }
POST /api/auth/logout
GET  /api/auth/me      -> { user: { id, username, display_name, avatar } }
```

二狗 App 对接时需要:
1. 登录后保存 Cookie，后续请求带上
2. 每个受保护的 API 都需要有效 session Cookie
3. session 过期返回 401

---

## 一、Todo (任务管理)

### 核心概念

**两个维度交叉**:
- **纵轴 (象限/Quadrant)**: 艾森豪威尔四象限
  - `important-urgent`: 优先处理
  - `important-not-urgent`: 就等你翻牌子了 (重要不紧急)
  - `not-important-urgent`: 短平快
  - `not-important-not-urgent`: 待分类
- **横轴 (时间段/Tab)**: `today` | `week` | `month`

新任务默认: tab=`today`, quadrant=`not-important-not-urgent` (待分类)

### 数据结构

```typescript
interface Todo {
  id: string;              // UUID 8字符短ID
  text: string;            // 任务标题 (max 500字符)
  content: string;         // 详细描述 Markdown (max 10000字符)
  tab: "today" | "week" | "month";
  quadrant: "important-urgent" | "important-not-urgent" | "not-important-urgent" | "not-important-not-urgent";
  progress: number;        // 0-100
  completed: boolean;
  completed_at: string | null;   // ISO 8601
  due_date: string | null;       // YYYY-MM-DD
  assignee: string;              // 负责人
  tags: string[];                // 标签数组
  sort_order: number;            // 浮点数，拖拽排序用
  is_collaborative: boolean;
  created_at: string;
  updated_at: string;
  deleted: boolean;              // 软删除标记
  deleted_at: string | null;
  changelog: ChangeEntry[];      // 变更历史 (最近50条)
  next_reminder?: { id, text, remind_at, status };  // 关联的下一个提醒
}

interface ChangeEntry {
  time: string;       // ISO 8601
  field: string;      // 变更字段名
  from: string;       // 原值
  to: string;         // 新值
  label: string;      // 显示标签 (如 "进度", "截止日期")
}
```

### API

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/todos?tab=today` | 列表 (按tab过滤，返回自己的+协作的) |
| POST | `/api/todos` | 创建 |
| GET | `/api/todos/:id` | 详情 (含changelog + next_reminder) |
| PUT | `/api/todos/:id` | 更新 (部分字段) |
| DELETE | `/api/todos/:id` | 软删除 |
| POST | `/api/todos/:id/restore` | 恢复 |
| DELETE | `/api/todos/:id/permanent` | 永久删除 |
| PUT | `/api/todos/batch` | 批量更新 (max 200条) |
| GET | `/api/todos/counts?tab=today` | 各象限计数 |

### 关键业务逻辑

1. **完成逻辑**: progress=100 自动标 completed=true + completed_at; 也可以直接设 completed 独立于 progress
2. **软删除**: DELETE 只设 deleted=1, 可恢复; /permanent 才真删
3. **变更记录**: 每次 update 自动记录 field/from/to/label 到 todo_changelog (最多保留50条)
4. **拖拽排序**: sort_order 为浮点数，拖拽时取目标位置前后两个 sort_order 的中间值
5. **Tab 自动计算** (AI创建任务时): due_date=今天->today, 本周->week, 其他->month
6. **协作任务**: 协作者在 todo_collaborators 表有独立的 tab/quadrant/sort_order (视图独立，数据共享)
7. **协作删除**: 协作任务的删除/完成需要确认流 (pending_confirmations)

---

## 二、Routine (例行任务)

### 核心概念

每日习惯打卡。每天重置，只跟踪"今天是否完成"。

### 数据结构

```typescript
interface Routine {
  id: string;
  text: string;                       // 任务内容
  completed_today: boolean;           // 今天是否已完成
  last_completed_date: string | null; // YYYY-MM-DD
  is_collaborative: boolean;
  created_at: string;
  // 协作任务额外字段:
  owner_name?: string;
  owner_id?: string;
}
```

### API

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/routines` | 列表 (自己的 + 协作的，含 owner 信息) |
| POST | `/api/routines` | 创建 `{ text }` |
| DELETE | `/api/routines/:id` | 删除 (硬删) |
| POST | `/api/routines/:id/toggle` | 切换今日完成状态 |

### 关键业务逻辑

1. **每日重置**: 根据 last_completed_date 是否 = 今天 (UTC+8) 来判断 completed_today
2. **协作完成**: 协作者的完成状态存在 routine_completions 表 (routine_id, user_id, completed_date)，owner 完成状态存在 routines 表
3. **简单模型**: 只有文本和完成状态，没有频率、统计等复杂逻辑

---

## 三、Review (例行审视)

### 核心概念

定期反思。支持不同频率，系统自动计算"到期状态"。

### 数据结构

```typescript
interface ReviewItem {
  id: string;
  text: string;                    // 审视内容
  frequency: "daily" | "weekly" | "monthly" | "quarterly" | "yearly" | "custom";
  frequency_config: object;        // JSON, 如 { day_of_week: 1 }
  notes: string;
  category: string;
  last_completed: string | null;   // ISO 8601
  paused: boolean;
  created_at: string;
  updated_at: string;
  // 计算字段:
  due_status: "overdue" | "due_today" | "due_soon" | "upcoming" | "completed" | "paused";
  days_until_due: number;
  due_label: string;               // 如 "已逾期3天", "今天到期"
}
```

### API

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/reviews` | 列表 (含计算的 due_status，按优先级排序) |
| POST | `/api/reviews` | 创建 `{ text, frequency, frequency_config?, notes?, category? }` |
| PUT | `/api/reviews/:id` | 更新 (部分字段) |
| DELETE | `/api/reviews/:id` | 删除 (硬删) |
| POST | `/api/reviews/:id/complete` | 标记完成 (设 last_completed = now) |
| POST | `/api/reviews/:id/uncomplete` | 取消完成 (清空 last_completed) |

### 关键业务逻辑

1. **到期计算**: last_completed + frequency 间隔 = 下次到期时间，与 today 比较得到 due_status
2. **排序**: overdue(0) > due_today(1) > due_soon(2) > upcoming(3) > completed(4) > paused(5)，同优先级按 days_until_due 排
3. **暂停**: paused=true 跳过到期计算，不显示逾期
4. **完成**: 只记录最后一次完成时间，不是 toggle

---

## 四、Expense (记账)

### 核心概念

- 不用预定义分类，用 **AI 自动打标签**
- 支持手动记一笔 和 拍照识别收据
- 三表设计: 条目(entry) + 明细行(items) + 照片(photos)

### 数据结构

```typescript
// 记账条目
interface ExpenseEntry {
  id: string;
  amount: number;              // 总金额
  date: string;                // YYYY-MM-DD
  notes: string;               // 备注 (商家名等)
  tags: string[];              // AI 打的标签 JSON 数组
  ai_processed: boolean;       // AI 是否已解析
  currency: string;            // 默认 "CAD"
  created_at: string;
  updated_at: string;
  photo_count: number;         // 照片数
  item_count: number;          // 明细行数
}

// AI 解析的明细行
interface ExpenseItem {
  id: string;
  entry_id: string;
  name: string;                // 商品名 (中文)
  quantity: number;
  unit_price: number;
  amount: number;              // 小计
  specs: string;               // 规格 (英文名等)
  sort_order: number;
}

// 照片
interface ExpensePhoto {
  id: string;
  entry_id: string;
  filename: string;
  storage_path: string;        // /data/uploads/{user_id}/{photo_id}.{ext}
  file_size: number;
  mime_type: string;
  created_at: string;
}

// AI 解析预览结果
interface ParsePreview {
  merchant: string;
  date: string;
  currency: string;
  tags: string[];
  items: { name, quantity, unit_price, amount, specs }[];
  subtotal: number;
  tax: number;
  tip: number;
  total_amount: number;
}

// 聚合摘要
interface ExpenseSummary {
  total_amount: number;
  entry_count: number;
  period: string;
  from: string;
  to: string;
  tag_totals: { tag: string; amount: number }[];
}
```

### API

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/expenses?from=&to=&tags=` | 按日期范围+标签筛选 |
| POST | `/api/expenses` | 创建条目 (含可选 items 数组) |
| GET | `/api/expenses/:id` | 详情 (条目+明细+照片) |
| PUT | `/api/expenses/:id` | 更新 |
| DELETE | `/api/expenses/:id` | 删除 (硬删，级联删明细+照片+文件) |
| GET | `/api/expenses/summary?period=day\|week\|month&date=` | 聚合摘要 |
| GET | `/api/expenses/tags` | 所有已用标签去重 |
| POST | `/api/expenses/:id/photos` | Multipart 上传照片 (10MB limit) |
| DELETE | `/api/expenses/photos/:photo_id` | 删除单张照片 |
| POST | `/api/expenses/parse-preview` | 上传照片 -> AI 解析 -> 返回预览 (不保存) |

### 关键业务逻辑

1. **两条路径**:
   - 手动: 填金额+日期+备注 -> 保存 -> 后台异步根据备注AI打标签
   - 拍照: 选照片 -> AI识别 -> 预览结果 -> 用户确认/修改 -> 保存
2. **parse-preview**: 发 base64 图片给后端，后端调 Claude vision 解析，返回结构化数据但不写库
3. **自动标签**: 无照片有备注时，通过 simple_generate 根据金额+备注生成标签
4. **AI解析规则**: name 用中文, amount 直接抄收据, total = 最终刷卡金额(含税含小费)
5. **照片存储**: `/data/uploads/{user_id}/{uuid}.{ext}`
6. **无协作**: 记账模块只有单用户

---

## 五、Trip (差旅管理)

### 核心概念

- 一次差旅 = 一个 Trip (标题、目的地、起止日期)
- 每条明细 = TripItem (类型、日期、金额、报销状态)
- 按天组织显示
- 支持协作者 (viewer/editor)

### 数据结构

```typescript
// 差旅
interface Trip {
  id: string;
  title: string;                // 如 "回国述职"
  destination: string;          // 目的地
  date_from: string;            // YYYY-MM-DD
  date_to: string;
  purpose: string;              // 事由
  notes: string;
  currency: string;             // 默认 "CAD"
  created_at: string;
  updated_at: string;
}

// 条目类型
type TripItemType = "flight" | "train" | "hotel" | "taxi" | "meal" | "meeting" | "telecom" | "misc";

// 差旅条目
interface TripItem {
  id: string;
  trip_id: string;
  type: TripItemType;           // flight=机票, train=火车, hotel=酒店, taxi=交通, meal=餐饮, meeting=会议, telecom=通讯, misc=杂费
  date: string;                 // YYYY-MM-DD
  description: string;          // 如 "CA1234 温哥华->北京"
  amount: number;               // 金额 (0=无费用，如会议)
  currency: string;
  reimburse_status: "pending" | "submitted" | "approved" | "rejected" | "na";
  notes: string;
  sort_order: number;
  photo_count: number;
  created_at: string;
  updated_at: string;
}

// 报销状态含义:
// pending=待提交(黄), submitted=已提交(蓝), approved=已批准(绿), rejected=已拒绝(红), na=无需报销(灰)

// 差旅协作者
interface TripCollaborator {
  user_id: string;
  display_name: string;
  role: "owner" | "editor" | "viewer";
  created_at: string;
}

// 报销汇总
interface ReimburseSummary {
  total_amount: number;
  pending_count: number;
  submitted_count: number;
  approved_count: number;
  rejected_count: number;
}

// 照片 (与 ExpensePhoto 结构相似)
interface TripPhoto {
  id: string;
  item_id: string;
  filename: string;
  storage_path: string;
  file_size: number;
  mime_type: string;
  created_at: string;
}
```

### API

| 方法 | 路径 | 权限 | 说明 |
|------|------|------|------|
| GET | `/api/trips` | 登录 | 列表 (自己的+被共享的) |
| POST | `/api/trips` | 登录 | 创建差旅 |
| GET | `/api/trips/:id` | owner/collaborator | 完整详情 (items按天分组+协作者+报销汇总) |
| PUT | `/api/trips/:id` | owner | 更新元信息 |
| DELETE | `/api/trips/:id` | owner | 删除 (级联) |
| POST | `/api/trips/:id/items` | owner/editor | 添加条目 |
| PUT | `/api/trips/items/:item_id` | owner全字段, editor仅reimburse_status | 更新 |
| DELETE | `/api/trips/items/:item_id` | owner | 删除条目 |
| POST | `/api/trips/items/:item_id/photos` | owner/editor | 上传票据 (multipart) |
| DELETE | `/api/trips/photos/:photo_id` | owner | 删除照片 |
| POST | `/api/trips/:id/collaborators` | owner | 添加协作者 (须是好友) |
| DELETE | `/api/trips/:id/collaborators/:uid` | owner | 移除协作者 |
| GET | `/api/trips/:id/export/csv` | owner/collaborator | 下载报销清单 CSV |
| GET | `/api/trips/:id/export/photos` | owner/collaborator | 下载全部票据 ZIP |
| POST | `/api/trips/items/:item_id/analyze` | owner/editor | AI 分析条目 |

### 关键业务逻辑

1. **权限模型**:
   - Owner: 全部操作
   - Editor: 查看 + 添加条目 + 更新报销状态 + 上传照片
   - Viewer: 只读
   - 添加协作者需要先是好友 (friendships 表 status='accepted')
2. **按天组织**: 详情接口返回 items 按 date 分组
3. **报销追踪**: 每个条目独立的 reimburse_status, 汇总在差旅级别统计
4. **金额=0的条目**: 如会议，不显示金额，reimburse_status 默认 na
5. **AI 分析**: 支持文字或照片，有照片走 vision_generate，纯文字走 simple_generate
6. **导出**: CSV (UTF-8 BOM, Excel 兼容) + ZIP (按日期文件夹组织)
7. **照片存储**: 存在 trip owner 的 user_id 目录下

---

## 六、English/Learning (学习笔记)

### 核心概念

按场景学习英语 (扩展为多分类学习笔记)。AI 生成双语对话/学习内容。

### 数据结构

```typescript
interface EnglishScenario {
  id: string;
  title: string;               // 中文标题
  title_en: string;            // 英文标题
  description: string;
  icon: string;                // 默认 "book" (书本图标)
  content: string;             // AI 生成的内容 (Markdown)
  status: "draft" | "ready" | "generating" | "error";
  archived: boolean;
  category: string;            // "英语" | "编程" | "职场" | "生活" | 其他
  notes: string;
  created_at: string;
  updated_at: string;
}
```

### API

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/english/scenarios?archived=0&category=` | 列表 |
| POST | `/api/english/scenarios` | 创建 `{ title, title_en?, description?, icon?, content?, category?, notes? }` |
| GET | `/api/english/scenarios/:id` | 详情 |
| PUT | `/api/english/scenarios/:id` | 更新 (部分字段) |
| DELETE | `/api/english/scenarios/:id` | 删除 (硬删) |
| POST | `/api/english/scenarios/:id/generate` | AI 生成内容 |
| POST | `/api/english/scenarios/:id/archive` | 归档 |

### 关键业务逻辑

1. **分类 Prompt**: 不同 category 使用不同的 AI 生成 prompt:
   - 英语: 对话格式 + 词汇 + 表达
   - 编程: 概念 + 代码示例 + 要点
   - 职场: 情境分析 + 技巧 + 案例
   - 生活: 知识 + 步骤 + 注意事项
2. **Status 流转**: draft -> generating -> ready/error
3. **频率限制**: 每用户每 30 秒最多 1 次 AI 生成
4. **归档**: 软标记，不删除，默认列表不显示归档的

---

## 七、AI 助手 (二狗/阿宝) 集成

### 对话接口

```
POST /api/chat
{
  "message": "帮我加个任务",
  "conversation_id": "uuid | null",
  "page_context": { "page": "expense", "detail_id": "abc123" }  // 可选
}

Response:
{
  "success": true,
  "reply": "AI 回复文本",
  "conversation_id": "uuid",
  "tool_calls": [["create_todo", {input}, {result}]],
  "usage": { "input_tokens": 500, "output_tokens": 200 }
}
```

### 对话管理

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/chat` | 发消息 |
| GET | `/api/chat/usage` | 使用量统计 |
| GET | `/api/conversations` | 对话列表 |
| GET | `/api/conversations/:id/messages` | 对话消息 |
| DELETE | `/api/conversations/:id` | 删除对话 |
| POST | `/api/conversations/:id/rename` | 重命名 |

### Tool Use 循环

1. 前端发 message -> 后端构建 system prompt (含用户数据摘要)
2. 调 Claude API，如果返回 tool_use -> 执行 tool -> 结果回传 Claude
3. 最多 5 轮，直到 end_turn
4. 返回最终文本 + 所有 tool_calls 记录

### 全部 AI Tools (33个)

**任务 (7)**:
- `create_todo(text, tab?, quadrant?, due_date?, assignee?, tags?, collaborator?)`
- `update_todo(id, text?, tab?, quadrant?, progress?, due_date?, completed?)`
- `delete_todo(id)` / `restore_todo(id)`
- `query_todos(tab?, quadrant?, completed?, keyword?, assignee?, tag?)`
- `batch_update_todos(updates[])`
- `get_statistics(period?)`

**例行 (4)**:
- `create_routine(text)` / `query_routines(keyword?)` / `update_routine(id, text)` / `delete_routine(id)`

**审视 (4)**:
- `create_review(text, frequency, frequency_config?)` / `query_reviews(keyword?, frequency?)` / `update_review(id, ...)` / `delete_review(id)`

**学习 (4)**:
- `create_english_scenario(title, description?, category?)` / `query_english_scenarios(keyword?, include_content?)` / `update_english_scenario(id, ...)` / `delete_english_scenario(id)`

**记账 (5)**:
- `create_expense(amount, date?, notes?, tags?, currency?)` / `query_expenses(date_from?, date_to?, tag?, keyword?, limit?)` / `update_expense(id, ...)` / `delete_expense(id)` / `get_expense_summary(period)`

**差旅 (8)**:
- `query_trips()` / `get_trip_detail(trip_id)` / `create_trip(title, date_from, date_to, ...)` / `update_trip(id, ...)` / `delete_trip(id)`
- `create_trip_item(trip_id, type, date, description, ...)` / `update_trip_item(id, ...)` / `delete_trip_item(id)`
- `get_trip_summary(trip_id?)`

**提醒 (4)**:
- `create_reminder(text, remind_at, related_todo_id?, repeat?)` / `query_reminders(status?)` / `cancel_reminder(id)` / `snooze_reminder(id, minutes)`

**工具 (2)**: `get_current_datetime()` / `report_security_event(...)`

**人物记忆 (4)**: `save_person(...)` / `update_person(...)` / `delete_person(id)` / `save_memory(content, category)` / `delete_memory(id)`

### 上下文注入策略 (重要)

**"轻上下文 + 重工具"** 原则:
- System prompt 只注入数字摘要 (~600 tokens):
  ```
  - 待办: 今天 5 个(3 已完成)，本周 12 个，3 个即将到期
  - 例行: 今天 8 个(6 已完成)
  - 审视: 12 个事项(2 个逾期)
  - 学习: 23 条笔记
  - 记账: 本月已花 CA$1,203.50(28 笔)
  - 差旅: 2 次行程
  - 提醒: 3 个待触发
  ```
- 只有**今日待办**保留详情 (最多10条, 含 quadrant/progress/due_date)
- 其他模块需要详情时，AI 自己调 query 工具按需查询
- 页面感知: 前端传 page_context，后端注入当前查看的条目详情

### Moment (此刻一句话)

```
GET /api/moment -> { text: "有两件急的", cached: true/false }
```

- 15分钟内存缓存
- AI 基于任务状态生成，最多10个汉字
- 失败兜底: 时段问候 ("上午好"/"晚上好")

---

## 八、提醒 & 通知

### 提醒

```typescript
interface Reminder {
  id: string;
  text: string;
  remind_at: string;           // ISO 8601 带时区 (如 "2026-02-21T15:00:00+08:00")
  status: "pending" | "triggered" | "acknowledged" | "snoozed" | "cancelled";
  related_todo_id: string | null;
  repeat: null | "daily" | "weekly";
  created_at: string;
  triggered_at: string | null;
  acknowledged_at: string | null;
}
```

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/reminders?status=pending` | 列表 |
| POST | `/api/reminders` | 创建 |
| PUT | `/api/reminders/:id` | 更新 |
| DELETE | `/api/reminders/:id` | 取消 |
| POST | `/api/reminders/:id/acknowledge` | 确认 |
| POST | `/api/reminders/:id/snooze` | 延后 `{ minutes: 5 }` |
| GET | `/api/reminders/pending-count` | 待触发数量 |

### 通知

```typescript
interface Notification {
  id: string;
  type: "reminder" | "friend_request" | "share" | "collaboration";
  title: string;
  body: string;
  reminder_id: string | null;
  todo_id: string | null;
  read: boolean;
  created_at: string;
}
```

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/notifications/unread` | 未读通知 |
| POST | `/api/notifications/:id/read` | 标记已读 |
| POST | `/api/notifications/read-all` | 全部已读 |

### 后台轮询

后端 ReminderPoller 每 30s 检查到期提醒:
1. 更新 status -> triggered
2. 创建 in-app notification
3. 发 Web Push 推送 (VAPID + AES-128-GCM)

---

## 九、好友 & 协作

### 好友

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/friends` | 好友列表 |
| GET | `/api/friends/requests` | 好友请求 |
| POST | `/api/friends/request` | 发送请求 `{ username }` |
| GET | `/api/friends/search?q=keyword` | 搜索用户 |
| POST | `/api/friends/:id/accept` | 接受 |
| POST | `/api/friends/:id/decline` | 拒绝 |
| DELETE | `/api/friends/:id` | 删除好友 |

### 任务分享

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/share` | 分享 `{ friend_id, item_type, item_id, message? }` |
| GET | `/api/share/inbox` | 收件箱 |
| GET | `/api/share/inbox/count` | 未读数 |
| POST | `/api/share/:id/accept` | 接受 |
| POST | `/api/share/:id/dismiss` | 忽略 |

### 协作确认流

当协作任务的 owner 或 collaborator 完成/删除任务:
1. 创建 pending_confirmation
2. 对方收到通知
3. 对方 confirm/reject
4. 全部确认后执行操作

---

## 十、数据库 Schema 汇总

### 核心业务表

| 表 | 说明 | 关键字段 |
|---|------|---------|
| `users` | 用户 | id, username, password_hash, display_name, avatar |
| `todos` | 任务 | user_id, text, content, tab, quadrant, progress, completed, due_date, sort_order, deleted |
| `todo_changelog` | 变更历史 | todo_id, field, from_val, to_val, label |
| `routines` | 例行 | user_id, text, completed_today, last_completed_date |
| `reviews` | 审视 | user_id, text, frequency, frequency_config, last_completed, paused |
| `expense_entries` | 记账 | user_id, amount, date, notes, tags(JSON), currency |
| `expense_items` | 记账明细 | entry_id, name, quantity, unit_price, amount, specs |
| `expense_photos` | 记账照片 | entry_id, filename, storage_path |
| `trips` | 差旅 | user_id, title, destination, date_from, date_to, purpose, currency |
| `trip_items` | 差旅条目 | trip_id, type, date, description, amount, reimburse_status |
| `trip_photos` | 差旅照片 | item_id, filename, storage_path |
| `english_scenarios` | 学习 | user_id, title, content, status, category, archived |
| `reminders` | 提醒 | user_id, text, remind_at, status, related_todo_id, repeat |

### 协作表

| 表 | 说明 |
|---|------|
| `todo_collaborators` | 任务协作者 (独立 tab/quadrant/sort_order) |
| `routine_collaborators` | 例行协作者 |
| `routine_completions` | 协作例行完成记录 (per user per date) |
| `trip_collaborators` | 差旅协作者 (viewer/editor) |
| `pending_confirmations` | 协作确认请求 |
| `confirmation_responses` | 确认回复 |

### 社交表

| 表 | 说明 |
|---|------|
| `friendships` | 好友关系 (pending/accepted/declined) |
| `shared_items` | 分享记录 |
| `contacts` | 联系人 |

### 系统表

| 表 | 说明 |
|---|------|
| `sessions` | 登录会话 (每用户最多5个) |
| `conversations` | AI 对话 |
| `chat_messages` | 对话消息 |
| `chat_usage_log` | AI 使用量 |
| `notifications` | 应用内通知 |
| `push_subscriptions` | Web Push 订阅 |
| `user_settings` | 用户设置 (push, 免打扰, AI模型偏好) |

---

## 十一、跨模块设计模式

### 1. 删除策略
| 模块 | 策略 |
|------|------|
| Todo | 软删除 (deleted flag), 可恢复, /permanent 硬删 |
| 其他所有 | 硬删除 |

### 2. AI 集成模式
| 模式 | 方法 | 场景 | 超时 |
|------|------|------|------|
| 对话式 | `chat()` | 二狗聊天，多轮 tool use | 30s |
| 单次文本 | `simple_generate()` | Moment 一句话, 标签生成 | 10s |
| 图片理解 | `vision_generate()` | 收据解析, 差旅票据分析 | 120s |

### 3. 照片处理
- 存储: `/data/uploads/{user_id}/{uuid}.{ext}`
- 上传: Multipart, 10MB limit, image/* only
- 删除: 同时删 DB 记录 + 磁盘文件
- 访问: `GET /api/uploads/{user_id}/{filename}` 带鉴权

### 4. 时区
- 统一 UTC+8 (Asia/Shanghai)
- 日期字段: YYYY-MM-DD (纯日期)
- 时间字段: ISO 8601 带时区

### 5. AI Provider
- 主: Claude (claude-opus-4-6)
- 备: Kimi (Moonshot), Doubao (ByteDance)
- 用户可在设置中选择 AI 模型
- Auto 模式: Doubao -> Kimi -> Claude 降级链

---

## 十二、健康模块 -- 经络图 & 养生功法

### 整体架构

```
健康模块由 4 个文件组成：
├── health-data.js      # 数据层：人体轮廓坐标、13关节骨骼、14经络+穴位、八段锦/站桩/易筋经动作数据
├── health-renderer.js  # 渲染层：Canvas2D 引擎，绘制人体、经络、穴位、气流粒子、姿态变形
├── health.js           # 控制层：UI 交互逻辑，分类切换、动作选择、动画控制
└── health.css          # 样式层：Hub 网格、横向滚动卡片、Canvas 容器、详情面板
```

模块关系：
```
Health (控制层)
  ├── 读取 HealthData (数据层) -- 经络、功法、穴位数据
  ├── 创建 HealthRenderer.MeridianRenderer (渲染层) -- Canvas 绑定到 DOM
  ├── 创建 HealthRenderer.PoseInterpolator (渲染层) -- 关键帧插值
  └── 操作 DOM -- 卡片列表、详情面板、正/背面切换
```

### 12.1 坐标系统

**所有坐标都是 0-1 归一化值**，渲染时乘以画布宽高得到像素坐标。

```
(0,0) ---------- (1,0)
  |                |
  |   人体正面/背面  |
  |                |
(0,1) ---------- (1,1)
```

优势：与分辨率无关，手机/平板/桌面统一适配。

### 12.2 人体轮廓 (BODY_OUTLINE)

分为 front（正面）和 back（背面）两套轮廓，每套包含 13 条轮廓线：

```
head                        -- 头部（闭合路径，约 20 个控制点）
neckLeft / neckRight        -- 颈部左右（4 个点）
torsoLeft / torsoRight      -- 躯干左右（8-10 个点）
armLeftOuter / armLeftInner -- 左臂外侧/内侧（10 个点）
armRightOuter / armRightInner -- 右臂（左臂 x 坐标 1-x 镜像）
legLeftOuter / legLeftInner -- 左腿外侧/内侧（13 个点）
legRightOuter / legRightInner -- 右腿（左腿镜像）
```

**镜像技巧**：右侧 = 左侧的 `x` 坐标做 `1 - x` 翻转：
```javascript
function mirrorPoints(pts) {
    return pts.map(p => ({ x: 1 - p.x, y: p.y }));
}
```

### 12.3 骨骼系统 (13 关节)

标准站立姿态定义 13 个关节点：

```javascript
STANDING_POSE = {
    head:      {x:0.50, y:0.05},   // 头顶
    neck:      {x:0.50, y:0.12},   // 颈部
    shoulderL: {x:0.36, y:0.15},   // 左肩
    shoulderR: {x:0.64, y:0.15},   // 右肩
    elbowL:    {x:0.30, y:0.28},   // 左肘
    elbowR:    {x:0.70, y:0.28},   // 右肘
    wristL:    {x:0.25, y:0.40},   // 左腕
    wristR:    {x:0.75, y:0.40},   // 右腕
    hip:       {x:0.50, y:0.42},   // 髋部（躯干/腿分界）
    kneeL:     {x:0.44, y:0.65},   // 左膝
    kneeR:     {x:0.56, y:0.65},   // 右膝
    ankleL:    {x:0.43, y:0.87},   // 左踝
    ankleR:    {x:0.57, y:0.87}    // 右踝
}
```

### 12.4 经络数据 (14 条)

每条经络包含：

```javascript
{
    id: 'LU',                           // 国际穴位编码前缀
    name: '手太阴肺经',                   // 中文全名
    shortName: '肺经',                    // 短名（Canvas 标签用）
    englishName: 'Lung Meridian',
    organ: '肺',                          // 对应脏腑
    element: 'metal',                     // 五行: metal/wood/water/fire/earth
    yinYang: 'yin',                       // 阴阳
    limbType: 'hand' | 'foot' | 'trunk',  // 经络走行类型
    direction: 'centrifugal' | 'centripetal', // 气流方向：离心/向心
    color: '#94a3b8',                     // 经络颜色
    pathFront: [{x, y}, ...],             // 正面走行路径（归一化坐标）
    pathBack: [{x, y}, ...],              // 背面走行路径
    acupoints: [...],                     // 穴位列表
    description: '...'                    // 经络描述
}
```

**穴位数据结构**：
```javascript
{
    id: 'LU-1',                    // 国际穴位编码
    name: '中府',
    pinyin: 'Zhong Fu',
    positionFront: {x, y},         // 正面坐标（可选）
    positionBack: {x, y},          // 背面坐标（可选）
    isKey: true,                   // 是否为重要穴位（影响圆点大小）
    functions: ['宣肺理气', ...],   // 功效列表
    indication: '咳嗽、气喘...'     // 主治
}
```

**limbType 的作用**：`hand`/`foot` 类型的经络自动**镜像绘制**到身体另一侧；`trunk`（任脉、督脉）只画一条。

**完整 14 条经络列表**：

| ID | 名称 | 颜色 | 类型 | 五行 | 阴阳 | 穴位数 |
|----|------|------|------|------|------|--------|
| LU | 手太阴肺经 | #94a3b8 | hand | 金 | 阴 | 5 |
| LI | 手阳明大肠经 | #cbd5e1 | hand | 金 | 阳 | 3 |
| ST | 足阳明胃经 | #fbbf24 | foot | 土 | 阳 | 3 |
| SP | 足太阴脾经 | #f59e0b | foot | 土 | 阴 | 3 |
| HT | 手少阴心经 | #ef4444 | hand | 火 | 阴 | 3 |
| SI | 手太阳小肠经 | #f87171 | hand | 火 | 阳 | 2 |
| BL | 足太阳膀胱经 | #1e3a5f | foot | 水 | 阳 | 5 |
| KI | 足少阴肾经 | #334155 | foot | 水 | 阴 | 3 |
| PC | 手厥阴心包经 | #dc2626 | hand | 火 | 阴 | 3 |
| SJ | 手少阳三焦经 | #fb923c | hand | 火 | 阳 | 3 |
| GB | 足少阳胆经 | #4ade80 | foot | 木 | 阳 | 4 |
| LR | 足厥阴肝经 | #22c55e | foot | 木 | 阴 | 3 |
| RN | 任脉 | #a855f7 | trunk | 水 | 阴 | 4 |
| DU | 督脉 | #3b82f6 | trunk | 火 | 阳 | 4 |

### 12.5 功法动作数据

三套功法共 30 个动作：

| 功法 | 数量 | ID 格式 | 特点 |
|------|------|---------|------|
| 八段锦 | 10 | bdj-00~09 | 预备式 + 8式 + 收势，每式 4-6 个关键帧，动态往复 |
| 站桩 | 8 | zz-00~07 | 含基准对照，2 个关键帧（站立->桩姿），静态保持 |
| 易筋经 | 12 | yjj-01~12 | 每式 4-5 个关键帧，动态动作 |

**每个动作的数据结构**：

```javascript
{
    id: 'bdj-01',
    name: '双手托天理三焦',
    category: '八段锦',
    description: '...',                    // 动作描述
    benefits: ['疏通三焦经气', ...],        // 养生功效
    stimulatedMeridians: [                  // 刺激的经络
        { meridianId: 'SJ', intensity: 'primary', note: '双手上托直接拉伸三焦经' },
        { meridianId: 'DU', intensity: 'secondary', note: '脊柱伸展间接刺激督脉' }
    ],
    keyAcupoints: ['SJ-5', 'LU-1', ...],  // 重点穴位 ID
    videoUrl: '/assets/videos/...',         // 教学视频（可选）
    keyframes: [                            // 关键帧动画
        { time: 0,    pose: {...13个关节坐标}, label: '预备式' },
        { time: 0.35, pose: {...},              label: '双手经胸前上举' },
        { time: 0.55, pose: {...},              label: '双手托天' },
        { time: 1,    pose: {...},              label: '还原' }
    ],
    duration: 12                            // 完整动作时长（秒）
}
```

**站桩特有的 insights 字段**（练功要领）：
```javascript
insights: [
    { label: '意念', content: '意守丹田...' },
    { label: '骨骼排列', content: '百会上领...' },
    { label: '松沉', content: '系统性放松...' },
    { label: '呼吸', content: '腹式深呼吸...' }
]
```

**八段锦完整动作列表**：
| ID | 名称 | 主经络 | 时长 |
|----|------|--------|------|
| bdj-00 | 预备式 | - | 8s |
| bdj-01 | 双手托天理三焦 | 三焦经、肺经、心包经 | 12s |
| bdj-02 | 左右开弓似射雕 | 肺经、大肠经、心经 | 12s |
| bdj-03 | 调理脾胃须单举 | 脾经、胃经 | 10s |
| bdj-04 | 五劳七伤往后瞧 | 膀胱经、小肠经、督脉 | 10s |
| bdj-05 | 摇头摆尾去心火 | 心经、肾经、督脉 | 15s |
| bdj-06 | 两手攀足固肾腰 | 肾经、膀胱经 | 10s |
| bdj-07 | 攒拳怒目增气力 | 肝经、胆经 | 10s |
| bdj-08 | 背后七颠百病消 | 肾经、膀胱经、胃经 | 8s |
| bdj-09 | 收势 | - | 8s |

**站桩完整动作列表**：
| ID | 名称 | 主经络 | 特点 |
|----|------|--------|------|
| zz-00 | 普通站立（基准） | - | 无练功意念的对照组 |
| zz-01 | 混元桩 | 任脉、督脉 | 双臂环抱胸前如抱球 |
| zz-02 | 抱球桩 | 任脉、脾经 | 双臂腹前环抱，掌心对脐 |
| zz-03 | 降气桩 | 肾经、胃经 | 掌心向下如按浮球 |
| zz-04 | 扶按桩 | 大肠经、三焦经 | 双手体侧下按 |
| zz-05 | 提抱桩 | 脾经、肺经 | 掌心向上如托物 |
| zz-06 | 马步桩 | 胃经、脾经、胆经 | 宽步深蹲 |
| zz-07 | 无极桩 | 督脉、任脉 | 最简朴，自然下垂 |

**易筋经完整动作列表**：
| ID | 名称 | 主经络 | 时长 |
|----|------|--------|------|
| yjj-01 | 韦驮献杵第一势 | 心包经、心经 | 10s |
| yjj-02 | 韦驮献杵第二势 | 肺经、大肠经 | 10s |
| yjj-03 | 韦驮献杵第三势 | 三焦经、督脉 | 10s |
| yjj-04 | 摘星换斗势 | 胆经、肾经 | 12s |
| yjj-05 | 倒拽九牛尾势 | 大肠经、小肠经 | 12s |
| yjj-06 | 出爪亮翅势 | 大肠经、三焦经 | 10s |
| yjj-07 | 九鬼拔马刀势 | 小肠经、督脉 | 12s |
| yjj-08 | 三盘落地势 | 脾经、胃经、肾经 | 10s |
| yjj-09 | 青龙探爪势 | 肝经、胆经 | 12s |
| yjj-10 | 卧虎扑食势 | 督脉、膀胱经 | 12s |
| yjj-11 | 打躬势 | 膀胱经、督脉 | 10s |
| yjj-12 | 掉尾势 | 膀胱经、肾经 | 10s |

### 12.6 渲染引擎详解

#### 每帧渲染顺序

```
renderFrame()
  1. drawBackground()      -- 背景网格（深浅主题自适应）
  2. drawBodyOutline()      -- 人体轮廓线（二次贝塞尔平滑曲线）
  3. drawMeridianPaths()    -- 经络路径（发光线条，主经络带 shadowBlur）
  4. drawQiParticles()      -- 气流粒子动画（7个光点沿路径流动）
  5. drawAcupoints()        -- 穴位圆点（普通4px/重要6px/选中10px）
  6. drawLabels()           -- 经络名称标签
  7. drawAcupointTooltip()  -- 穴位点击弹窗（名称+编码+功效）
```

#### 平滑曲线算法

所有路径使用 **二次贝塞尔曲线 (quadraticCurveTo)** 实现平滑：

```javascript
// 控制点 = 当前点，终点 = 当前点与下一点的中点
for (var i = 0; i < points.length - 2; i++) {
    var cpX = points[i+1].x * width;
    var cpY = points[i+1].y * height;
    var endX = (cpX + points[i+2].x * width) / 2;
    var endY = (cpY + points[i+2].y * height) / 2;
    ctx.quadraticCurveTo(cpX, cpY, endX, endY);
}
```

#### 经络路径渲染

```javascript
// 主要经络加发光效果
if (isPrimary) {
    ctx.shadowColor = color;
    ctx.shadowBlur = 10;
}
ctx.strokeStyle = color;
ctx.lineWidth = isPrimary ? 4 : 3;
// hand/foot 类型经络自动镜像绘制（x 坐标 1-x）
```

#### 气流粒子动画

每条经络上 7 个光点沿路径流动：

```javascript
QI_PARTICLE_COUNT = 7       // 粒子数量
QI_SPEED = 50               // 流动速度（像素/秒）
QI_CORE_RADIUS = 3          // 核心圆半径
QI_GLOW_RADIUS = 8          // 发光圈半径
QI_GLOW_OPACITY = 0.3       // 发光透明度

// 均匀分布 + 持续偏移实现流动
t = ((i / 7) + offset / totalLength) % 1;
if (direction === 'centripetal') t = 1 - t;  // 向心方向取反
pos = pointAlongPath(points, t, w, h);       // 沿路径插值

// 双层绘制：外层发光 + 内层实心
ctx.globalAlpha = 0.3; ctx.arc(pos.x, pos.y, 8, ...);
ctx.globalAlpha = 1;   ctx.arc(pos.x, pos.y, 3, ...);
```

#### 穴位交互

```javascript
ACUPOINT_RADIUS = 4          // 普通穴位圆点
ACUPOINT_KEY_RADIUS = 6      // 重要穴位
ACUPOINT_HOVER_RADIUS = 10   // 选中高亮
ACUPOINT_HITBOX_RADIUS = 30  // 触摸命中区域（比视觉大，便于手机操作）
```

点击穴位后显示 Tooltip：标题行"合谷 (LI-4)" + 功效行"疏风解表"，自动避免超出画布。

#### 骨骼变形系统（核心亮点）

**目标**：功法改变姿态时，人体轮廓和经络路径跟着变形。

**核心算法 -- 线段插值变形**：

```javascript
function deformPoints(points, refChain, curChain) {
    // refChain: 参考姿态的关节链（如 [shoulder, elbow, wrist]）
    // curChain: 当前姿态的关节链
    // 每个点根据 Y 坐标在关节链中的位置，计算位移偏移

    t = (point.y - yMin) / (yMax - yMin);  // 相对位置 0-1
    segIdx = floor(t * segments);            // 所在线段
    // 线性插值两端关节的位移
    dx = dxA + (dxB - dxA) * localT;
    dy = dyA + (dyB - dyA) * localT;
    return { x: point.x + dx, y: point.y + dy };
}
```

**变形关节链映射**：

| 身体部位 | 参考关节链 |
|---------|-----------|
| 头部 | [head] |
| 颈部 | [head, shoulder] |
| 躯干 | [shoulder, hip] |
| 手臂 | [shoulder, elbow, wrist] |
| 腿部 | [hip, knee, ankle] |

经络路径也使用同样的变形，根据 `limbType` 选择对应关节链。

#### 关键帧插值动画

```javascript
PoseInterpolator(keyframes)
  .interpolate(t)  // t: 0-1，返回插值后的 13 关节坐标
  .getLabel(t)     // 返回当前阶段的文字标签

// 线性插值
lerpPose(a, b, t) {
    result[joint] = {
        x: a[joint].x + (b[joint].x - a[joint].x) * t,
        y: a[joint].y + (b[joint].y - a[joint].y) * t
    };
}
```

**动画控制**：
- `requestAnimationFrame` 驱动
- 时间 t 在 0-1 之间**来回弹跳**（到 1 反向，到 0 再正向）
- 速度由 `duration`（秒）控制

### 12.7 UI 结构

#### 页面层级

```
健康 Hub（2x2 网格卡片）
  |- 八段锦 -> 分类详情页（功法模式）
  |- 经络   -> 分类详情页（经络浏览模式）
  |- 易筋经 -> 分类详情页（功法模式）
  '- 站桩   -> 分类详情页（功法模式）
```

#### 功法模式布局

```
[<- 健康] [标题]
[横向滚动动作卡片: 预备式 | 第1式 | 第2式 | ...]
[Canvas 经络人体图]
[正面/背面 切换按钮]
[教学视频播放器]（可选）
[详情面板: 名称 + 描述 + 养生功效 + 涉及经络(主/次) + 重点穴位(可点击)]
```

#### 经络浏览模式布局

```
[<- 健康] [经络图]
[横向滚动经络列表: 肺经 | 大肠经 | 胃经 | ...]（带颜色圆点+脏腑名）
[Canvas 经络人体图]
[正面/背面 切换按钮]
[详情面板: 经络名 + 描述 + 基本属性(英文名/五行/阴阳/脏腑) + 穴位列表(可点击高亮)]
```

### 12.8 深/浅色主题适配

| 元素 | 浅色 | 深色 |
|------|------|------|
| 背景 | #fafafa | #1a1a2e |
| 网格线 | #f0f0f0 | #2a2a3e |
| 人体轮廓 | #d1d5db | #4a4a5e |
| Tooltip 背景 | #ffffff | #2a2a3e |
| Tooltip 文字 | #1f2937 | #e5e5e5 |

### 12.9 关键 CSS 要点

```css
/* Canvas 容器：3:4 比例，居中，圆角 */
.health-canvas-wrap {
    position: relative;
    width: 100%;
    aspect-ratio: 3 / 4;
    max-width: 400px;
    margin: 0 auto;
    border-radius: 12px;
    overflow: hidden;
}

/* 横向滚动卡片 */
.health-action-cards {
    display: flex;
    gap: 8px;
    overflow-x: auto;
    scroll-snap-type: x mandatory;
    scrollbar-width: none;
}

/* 手机适配 */
@media (max-width: 767px) {
    .health-canvas-wrap { aspect-ratio: 1/1; max-height: 280px; }
}
```

### 12.10 移植到手机 App 的建议

**技术映射**：

| Web 技术 | App 替代方案 |
|---------|-------------|
| Canvas2D | iOS: Core Graphics / SwiftUI Canvas; Android: Canvas; Flutter: CustomPainter |
| requestAnimationFrame | DisplayLink (iOS) / Choreographer (Android) / Ticker (Flutter) |
| touch/click 事件 | GestureDetector / onTapGesture |
| CSS scroll-snap | 原生 ScrollView snap |

**数据可直接复用**：所有坐标（轮廓、骨骼、经络路径、穴位、关键帧）都是 0-1 归一化 JSON，可直接导出为 JSON 文件。

**可优化方向**：
1. **3D 人体模型** -- SceneKit/ARKit 或 three.js 实现 360 度旋转
2. **动作追踪** -- 摄像头 + 姿态检测（MediaPipe/Vision），实时对比练习姿势
3. **音频引导** -- 每个动作添加语音引导和呼吸提示音
4. **练习计时器** -- 站桩计时、八段锦完整套路计时
5. **练习记录** -- 记录每日练习时长，生成统计图表
6. **穴位 AR** -- 放大穴位区域，结合 AR 在摄像头画面上叠加穴位位置

### 12.11 文件清单

| 文件 | 行数 | 职责 |
|------|------|------|
| `frontend/assets/js/health-data.js` | ~1593 | 全部静态数据（轮廓、骨骼、14经络、30动作） |
| `frontend/assets/js/health-renderer.js` | ~692 | Canvas2D 渲染引擎 |
| `frontend/assets/js/health.js` | ~487 | UI 控制逻辑 |
| `frontend/assets/css/health.css` | ~448 | 样式 |
| `frontend/index.html` (984-1035行) | ~52 | HTML 结构 |

总计约 3270 行代码，其中数据层占一半以上。
