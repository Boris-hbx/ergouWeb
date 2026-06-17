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

## P1 专属记忆层与面板

- 工厂记忆面板支持按 type 筛选、新增、编辑、启用/禁用和删除，且只调用 `/api/insight-factory/memories*`。
- 手动新增 `report_preference` 后，下次 worker 上下文包含该 enabled 记忆。
- 禁用某条 factory memory 后，下次 worker 上下文不再包含该记忆。
- worker 注入顺序固定为 `project_fact -> boris_profile -> report_preference -> insight_summary`，同类内部按 importance DESC、updated_at DESC。
- factory memories 不同写、不污染 `/api/memories` 通用记忆。

## T-217 生产 worker 运行时与可观测性

- 运行时镜像内必须存在 codex 可执行文件（`/usr/local/bin/codex`），`codex --version` 可读。
- worker 默认沙箱为 `bypass`（`--dangerously-bypass-approvals-and-sandbox`）；`INSIGHT_FACTORY_CODEX_SANDBOX=read-only` 可回退到 `-s read-only`，无需改代码。
- job 失败时 `error_message` 带分类前缀：`[codex_missing]`（os error 2）、`[auth_expired]`（401/未登录）、`[sandbox_failed]`（landlock/seccomp）、`[quota_blocked]`（OPENAI_API_KEY）、`[timeout]`、`[other]`。
- `worker/health` 区分：CLI 缺失 → `cliAvailable=false`/`gate=cli_unavailable`；CLI 在但无 auth.json → `status=blocked`/`gate=auth_missing`/`authPresent=false`；CLI+auth 齐 → `status=ready`/`gate=chatgpt_subscription`/`authPresent=true`，并带 `lastRefresh`。
- worker 处理每个 job 输出结构化日志：成功 `info`、失败/阻塞 `warn`，字段含 `job_id/task_id/mode/provider/status/error`，error 经 sanitize 不含 token/prompt。
- 前端：最新 job `failed`/`blocked` 且无报告时，「最新报告」区展示失败摘要 + 重试入口，不再停留在「等待 worker 写回」。
- 前端：详情存在 active job（pending/running）时自动轮询（~6s），job 终态后页面自动从「等待」翻到报告或失败态；离开详情停止轮询。
- 前端：列表页 health 徽标在未就绪时显示原因（`auth_missing`/`cli_unavailable` 等）。
