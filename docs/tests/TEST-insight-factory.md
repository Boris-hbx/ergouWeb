# TEST-insight-factory

覆盖洞察工厂 P0 独立后端数据表与 `/api/insight-factory/*` API。

## P0 后端契约

- 创建 task 可自动识别 `inputType`，并可通过 `createGenerateJob` 同时创建 `generate` job；task 状态变为 `pending`。
- 同一 task 同时只允许一个 active job（`pending` / `running`）；重复 generate / feedback / retry 返回 409。
- feedback API 必须保存 `feedbackNote` 并创建 `revise` job，`parentReportId` 指向当前报告。
- 写入 report 时 version 自增，task `currentReportId` 更新，task 状态变为 `done`，job 状态变为 `done`。
- job retry 只允许基于 `failed` / `blocked` job 创建新 job，且原 job 不复用。
- task、job、report、memory 查询必须按 `user_id` 隔离。
- memory 支持列表筛选、新增、修改、启用/禁用和删除。
- `/api/insight-tasks/*` 与旧 `insight_tasks` / `insight_reports` 不受影响。

## P0 Web 页面骨架

- 工作 Hub 显示「洞察工厂」入口，点击后进入独立子视图，不影响现行「洞察」入口。
- `/insight-factory` 直接访问返回主页面并自动打开洞察工厂列表；`/insight-factory/{id}` 直接访问返回主页面并自动打开对应详情。
- 列表页顶部支持录入 URL/topic/prompt/note，前端自动识别 `inputType`，可手动改类型和 template；提交后调用 `/api/insight-factory/tasks` 并创建 generate job。
- 列表页展示 title、status、latestVersion、updatedAt 和 worker health placeholder；状态筛选不影响旧 `/insights` 页面。
- 详情页展示 task 信息、active job、最新 report、反馈框和版本历史；报告正文复用 `.ins-report-body`/`InsightMd` 渲染。
- `idle` 状态可手动创建 v1 generate job；有 active job 时不显示重复生成/反馈入口。
- `done` 且无 active job 时可提交反馈，调用 `/api/insight-factory/tasks/{id}/feedback` 创建 revise job。
- `failed`/`blocked` job 显示错误摘要和 retry 入口，调用 `/api/insight-factory/jobs/{id}/retry`。
- 失败 toast 和 `console.error('[InsightFactory]', error)` 均存在；错误信息不展示 token/env。

## P1 CodexProvider worker 闭环

- worker dispatcher 只领取 `factory_jobs.status='pending'` 的 job，并先切到 `running`，同时把 task 切到 `running`。
- 主生成路径只允许 `provider='codex'`；`OPENAI_API_KEY` 存在时 job 必须进入 `blocked` 或 `failed`，不得执行生成，不得静默 fallback 到 API provider。
- CodexProvider health check 需要报告 CLI 可用性、版本摘要、API key gate 状态和 provider 名称；不得把 token/env 原文返回给前端。
- generate job 的上下文必须包含报告契约、template、input、source_snapshot、enabled factory memories；成功后写入 `factory_reports.version=1`，task 变为 `done`。
- revise job 的上下文必须额外包含 `previous_report_md` 和 `feedback_note`；成功后写入下一版 report，`parent_report_id` 指向上一版，`revision_note` 保存反馈原文。
- worker 输出必须是完整 Markdown；空输出或明显非报告输出要 fail closed，并在 job `error_message` 写入简短原因。
- running 期间重复点击生成/反馈仍受 active job 唯一约束保护，返回 409，不创建并发 job。
- failed/blocked job retry 会创建新的 pending job；dispatcher 处理 retry 时沿用原 job 的语义上下文，成功后写入新版本。
- dispatcher 单轮处理结果可测试：无 pending 返回 `processed=0`；处理成功返回 `processed>=1`，失败不会 panic。
