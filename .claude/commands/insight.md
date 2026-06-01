洞察任务工人:按 ID 直达生产 API 看资源池 / 生成 / 按反馈优化洞察报告。CC 是"工人",Web 是"收件箱+存储"。

> 命令参数:`$ARGUMENTS`
> - 无参 / `pool` / `资源池` → 列整个资源池(全状态,= Web 列表)
> - `ready` / `待认领` / `候选` → 只列待认领(status=ready,可被 claim 处理的)
> - 一个数字 `<id>`(如 `42`)→ 处理/优化该洞察任务
> - `done` → 列已完成任务
>
> 术语(PM 口径,T-125/PROP § 十一):**「资源池」= 整列表(全状态收集箱)**;单状态 `ready` 叫**「待认领/待处理」**,不再叫"资源池"。

## 铁律(不许偷懒)

1. **只走生产 HTTP API,绝不碰数据库**:生产镜像无 sqlite3、库是 WAL 模式,直接读文件会漏最新写入。也不要重新 grep schema——存取契约就是下面这些,记牢直达。
2. **鉴权走 PAT**:每个请求带 `Authorization: Bearer <NEXT_PAT>`,token 从环境变量 `NEXT_PAT` 读。
3. **一切 by ID**,后端已按 `user_id` 强隔离,不用自己拼 user 条件。
4. **报告结构以 ergouPM 权威文件为准,生成前必读**——不许自由发挥:
   - SOP:`C:\Project\ergouPM\llm\insight-sop.md`(首版/修订模式、各 input_type 处理)
   - 模板:`C:\Project\ergouPM\llm\insight-templates\{template}.md`(survey / decision / watch),**严格遵守其章节顺序与章节名**。
5. **每份报告必含「思辨」章节(硬约束,绝不省略)**:作为倒数第二章(Sources 之前),对前面各节做**对抗性质疑**(核心假设脆弱性 / 反方证据 / 来源可信度盲区 / "我最可能错在哪"),占全文约 15-25%;章节名就叫「思辨」,不用别名。漏了「思辨」= 报告不合格,必须重做。
6. **读响应里的中文用 UTF-8**:PS 5.1 的 `Invoke-RestMethod` 会按非 UTF-8 解码 → 中文(如 `feedbackNote`)变乱码。读中文字段改用 `curl.exe -o file` 取原始字节 + `Get-Content -Encoding UTF8 | ConvertFrom-Json`;POST 中文 body 用 `[Text.Encoding]::UTF8.GetBytes()` 发字节。

## 配置

- Base URL:生产 `https://next-boris.fly.dev`(如需 staging,改用 `https://next-boris-staging.fly.dev`)
- PAT:环境变量 `NEXT_PAT`。**开工前先确认它存在**:
  - PowerShell:`if (-not $env:NEXT_PAT) { "缺 NEXT_PAT" }`
  - 为空 → 停下,告诉 Boris:去 Web 设置页生成 PAT(T-116),然后 `setx NEXT_PAT "<token>"`(新开终端生效)。
- 调用示例(任选其一,本机是 Windows,优先 PowerShell):
  - PowerShell:`Invoke-RestMethod -Uri "https://next-boris.fly.dev/api/insight-tasks?status=ready" -Headers @{ Authorization = "Bearer $env:NEXT_PAT" }`
  - curl:`curl -s -H "Authorization: Bearer $env:NEXT_PAT" "https://next-boris.fly.dev/api/insight-tasks?status=ready"`

## 数据位置速查(背景,别再去重新发现)

- 资源池 = `insight_tasks` 整表(全状态收集箱);每条创建即有唯一 `id`。**待认领** = `status='ready'` 的行(可 claim 处理)。
- 你给的反馈 = `insight_tasks.feedback_note`(同一行的字段,最新一条;非空=待修订)。
- 报告 = `insight_reports` 表(`version` 递增,`content_md` 正文,`revision_note` 改动说明),最新版挂在 `insight_tasks.current_report_id`。

## 流程 A:列资源池 / 待认领

- **无参 / `pool` / `资源池`** → `GET /api/insight-tasks`(全状态,= Web 列表)。
- **`ready` / `待认领`** → `GET /api/insight-tasks?status=ready`(只列可被 claim 的)。

表格列出:`id` · 标题 · `inputType`(url/topic/prompt/note) · `status` · `updatedAt` · 是否待修订(`feedbackNote` 非空打标)。空则说"资源池空"。末尾提示:`/insight <id>` 处理某条。

## 流程 B:处理 / 优化某条(`/insight <id>`)

1. `GET /api/insight-tasks/{id}` → 拿到 `inputType`、`inputContent`、`template`、`status`、`feedbackNote`、以及内嵌的最新报告 `report`(含 `report.id`、`report.version`、`report.contentMd`)。url 类型还带 `sourceSnapshot`(已抓取的正文,直接用,别重抓)。
2. 若 `status == 'ready'` → `POST /api/insight-tasks/{id}/claim`(ready→processing,占坑)。返回 409 = 已被占/状态不对,**停下**报告当前状态,不要硬上。
3. **先读模板**:`template` 为空时按 SOP 自动判定(survey/decision/watch),然后读 `C:\Project\ergouPM\llm\insight-templates\{template}.md` + `insight-sop.md`,**严格按其章节顺序生成,必含「思辨」章节(铁律 5)**。判断首版还是修订:
   - **首版**(`report` 为 null / 无 `currentReportId`):按模板用 `inputContent` 生成实质 Markdown 报告(topic/url 涉及外部信息时做 web 检索核实;note/prompt 按内容办)。
   - **修订**(已有 `report` 且 `feedbackNote` 非空):读 `report.contentMd` + `feedbackNote`,**有针对性**改(别全量重写),保留未被反馈的章节,**「思辨」章节重写不照抄**;`parentReportId` = 当前 `report.id`;`revisionNote` = 一句话说明按反馈改了什么。
4. `POST /api/insight-tasks/{id}/reports`,body:
   ```json
   { "template": "survey|decision|watch", "contentMd": "...", "parentReportId": <可选>, "revisionNote": "<可选>" }
   ```
   成功后后端自动:`version+1`、`status→done`、清空 `feedback_note`、`current_report_id` 指向新报告。
5. 向 Boris 汇报:任务 #id、新版本号、一句话摘要 + 让他去 Web 看/给反馈。
6. **出错兜底**:若已 claim 但生成失败,`POST /api/insight-tasks/{id}/release`(processing→ready,保留反馈),别把任务卡在 processing。

## 模板(`template`)取值

- `survey` 调研综述 · `decision` 决策建议 · `watch` 持续观察
- 任务 `template` 为空 = 让你选;你定的值会随报告写回固定。

## 已完成(`/insight done`)

`GET /api/insight-tasks?status=done`,列出 id · 标题 · 最新版本 · 更新时间。
