# TEST-praxis-journal — Praxis 今日经营记录 (T-285 / SPEC praxis §5)

> 覆盖：`praxis_journal` 表 + `/api/praxis/journal` CRUD + `analyze` AI 结构化 + 今日经营页（三栏）。
> 后端鉴权：全部端点 `AdminUserId` 守卫（owner/admin 放行，user/guest 403）。

## 后端 · API

| # | 场景 | 步骤 | 预期 |
|---|------|------|------|
| B1 | 守卫 | 普通 user token 调 `GET /api/praxis/journal` | 403 |
| B2 | 新建 | admin `POST {entryDate,rawText,tags}` | 200，返回 item，`structured=null`、`analyzedAt=null` |
| B3 | 默认日期 | `POST` 不带 entryDate | 200，entryDate=今天(UTC) |
| B4 | 字段命名 | 任意返回 | JSON 驼峰 `entryDate/rawText/analyzedAt`，无下划线 |
| B5 | 数据隔离 | admin A 建记录后 admin B `GET` | B 看不到（count=0） |
| B6 | 日期校验 | `POST {entryDate:"06/30/2026"}` | 400 |
| B7 | 列表筛选 | `GET ?date=YYYY-MM-DD` / `?from=&to=` | 仅返回命中日期；默认近 60 条，按 entry_date desc |
| B8 | 编辑 | `PATCH {rawText, structured}` | 200，原文更新；用户可修正 AI 归类（structured 覆盖） |
| B9 | 软删除 | `DELETE /:id` | 200；之后列表不含该条；再删 404 |
| B10 | 分析·空原文 | 对 rawText 为空的记录 `POST /:id/analyze` | 400「原文为空，无法分析」 |
| B11 | 分析·无 key | 有原文但环境无 ANTHROPIC_API_KEY | 503「AI 服务未配置」 |
| B12 | 分析·成功 | 有原文 + 有 key | 200，structured 为对象(summary/events/boards/value/risks/tomorrow)，analyzedAt 写入 |
| B13 | 分析·解析失败 | LLM 返回非 JSON | 422，retryable=true，**原记录 structured 不被改写**（可重试，不阻塞原文） |
| B14 | fences 容错 | LLM 返回 ```json{...}``` 包裹 | 正确解析出对象 |

## 前端 · 今日经营页

| # | 场景 | 预期 |
|---|------|------|
| F1 | 入口 | Praxis 驾驶舱顶部「今日经营」主行动，状态徽标=未记录/已记录/已分析 |
| F2 | 进入 | 点入口 → 三栏页（左：今日状态 / 中：主记录区 / 右：AI 观察）；驾驶舱隐藏 |
| F3 | 主输入 | 大 textarea + 占位引导 + 5 个引导小按钮（点击把问题插入输入框） |
| F4 | 快速标记 | 6 组 chip（精力/情绪/类型/时间质量/关系事件/风险）；风险多选其余单选；可不选直接存 |
| F5 | 按钮规范 | 「仅保存」始终可用；「保存并分析」为可选独立操作，不替换保存按钮（CLAUDE.md 按钮模式） |
| F6 | 保存 | 保存后状态→已记录；左栏「今日状态」刷新 |
| F7 | 分析 | 点保存并分析 → 右栏出「今日经营卡」（类型 + 摘要 + 涉及板块 + 信号 ≤3 + 明日调整），淡入 |
| F8 | 失败不阻塞 | 分析失败 toast「可重试」，原文已保存不丢 |
| F9 | 补记 | 改日期选过去某天，状态栏标「补记」，加载该天记录 |
| F10 | 返回 | 「← 驾驶舱」回八板块，顶部入口徽标同步最新状态 |

## 边界 / 异常

- 原文超长（>20000 字）→ 400「rawText too long」。
- tags 传非对象 → 后端归一为 `{}`，不报错。
- 同一天多次保存 → 复用同一条记录（PATCH），不重复建。
- 重新分析 → 覆盖上次 structured + analyzedAt。
