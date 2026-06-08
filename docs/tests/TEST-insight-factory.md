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
