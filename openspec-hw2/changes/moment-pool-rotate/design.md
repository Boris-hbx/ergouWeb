## Context

当前 `GET /api/moment` 每次调用 LLM 生成单条一言（15 分钟缓存），活跃用户一天 ~10 次 LLM 调用。缓存结构为 `HashMap<String, (String, DateTime)>`（用户 ID → 单条文本 + 时间戳）。前端 Moment 模块每 15 分钟或页面可见时请求后端。

目标：改为每天一次批量生成 ~30 条候选池，前端本地轮换，点击天气图标/刷新按钮即时换一条。

## Goals / Non-Goals

**Goals:**
- 将 LLM 调用从 ~10 次/天/用户 降至 1 次/天/用户
- 点击天气图标和刷新按钮时零延迟换一条
- 保持 fallback 机制和访客配额逻辑

**Non-Goals:**
- 不做持久化（数据库存池）——内存缓存足够，服务重启后下次请求自动重新生成
- 不做用户间共享池——每用户上下文不同，池内容应个性化
- 不做增量更新——不需要一天中间追加新条目

## Decisions

### D1: 缓存结构改为 `(Vec<String>, NaiveDate)`

**选择**: `MomentCache` 从 `HashMap<String, (String, DateTime<Utc>)>` 改为 `HashMap<String, (Vec<String>, NaiveDate)>`。key 不变（用户 ID），value 改为（候选池数组，生成日期）。

**理由**: 用日期而非时间戳判断过期更符合"每天一次"语义。`NaiveDate` 使用用户本地日期，跨时区准确。

**替代方案**: 用 `DateTime + 24h TTL` —— 但会导致用户在午夜前后看到昨天的内容，体验不一致。

### D2: LLM prompt 要求输出纯 JSON 数组

**选择**: system prompt 明确要求输出格式为 `["条目1", "条目2", ...]`，不含其他文字。`max_tokens` 设为 1500（30 条 × ~50 token/条）。

**理由**: JSON 数组最容易解析。如果 LLM 输出了多余前缀文字，后端用正则提取 `[...]` 部分做容错。

**替代方案**: 换行分隔 —— 解析简单但无法处理条目内含换行的边界情况。

### D3: 前端 localStorage 存池

**选择**: 前端收到池后存入 `localStorage`，key `momentPool`，值为 `{ pool: [...], date: "YYYY-MM-DD" }`。页面加载时先检查本地池日期，匹配则直接使用。

**理由**: 避免页面刷新/切换时重复请求后端。localStorage 持久化跨页面存活。

**替代方案**: sessionStorage —— 关闭标签页就丢失，用户重开应用必须重新请求。

### D4: 前端轮换逻辑 — shuffle + index 指针

**选择**: 收到池后做一次 Fisher-Yates shuffle 打乱顺序，维护一个 `_poolIndex` 指针。每次"换一条"时 index++，循环到末尾则重新 shuffle。

**理由**: 比纯随机更好——保证不重复直到池耗尽，且不需要记录"已展示"列表。

**替代方案**: 纯随机 —— 可能连续出现同一条。用 Set 去重 —— 实现更复杂。

### D5: 天气图标增加点击手势

**选择**: 给 `#moment-icon` 添加 `click` 事件，调用 `Moment.rotate()` 换一条。添加 `cursor: pointer` 样式。

**理由**: 天气图标在视觉上是顶栏最自然的可交互元素，点击换一条符合直觉。

### D6: 刷新按钮同时换一条

**选择**: 在 `refreshCurrentPage()` 中调用 `Moment.rotate()`，与页面数据刷新并行执行。

**理由**: 用户点刷新时期望"一切都刷新"，一言也应该换。

### D7: API 响应兼容

**选择**: 响应同时包含 `pool` 和 `text`（pool[0]），旧前端只读 `text` 不受影响。

**理由**: 零成本兼容，避免前后端部署时序问题。

## Risks / Trade-offs

- **[内存占用]** 每用户 ~30 条短文本 ≈ 1KB，1000 用户 ≈ 1MB → 可忽略。服务重启丢失缓存也无影响，下次请求重新生成。

- **[LLM 单次调用 token 增加]** 单次 ~800 output tokens（vs 原来 ~30 tokens），但调用频次从 ~10 次/天降到 1 次，总 token 大幅减少。

- **[JSON 解析失败]** LLM 可能不输出标准 JSON → 用正则 `\[[\s\S]*\]` 提取，逐行 fallback 解析，最终 fallback 到固定池。三层容错。

- **[首次请求延迟]** 批量生成比单条慢（~3s vs ~1s）→ 前端先显示时段问候语，池加载完成后替换。用户感知为"先看到一条简单的，然后换成更有趣的"。
