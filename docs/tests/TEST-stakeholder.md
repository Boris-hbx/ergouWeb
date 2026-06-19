# TEST-stakeholder

覆盖工作 Hub 干系人管理模块。T-221 仅覆盖后端 schema/API，前端视图由后续 T-223 覆盖。

## T-221 后端

### S1 列配置首访种子
- 前置：新用户无 `stakeholder_columns` 行。
- 操作：`GET /api/work/stakeholder-columns`。
- 预期：返回 11 个内置列，按 position 排序；`method` 为 multi 且含「定期汇报/定期吃饭/定期拜访/电话沟通」；`cadence` 为 select 且含「每周/每月/每季/不定期」。

### S2 列配置自定义 CRUD
- 操作：先 GET 种子，再 POST 新列 `{name:"影响力", type:"text"}`。
- 预期：返回 `createdKey:"c1"`；新列 builtin=false。
- 操作：DELETE `/api/work/stakeholder-columns/name`。
- 预期：400，内置列不能删除。
- 操作：DELETE `/api/work/stakeholder-columns/c1`。
- 预期：删除成功，并从所有未删除干系人的 `customFields` 移除 `c1`。

### S3 干系人创建
- 操作：POST `/api/work/stakeholders`，传 `name/team/region/title/duty/liaison/method/customFields`。
- 预期：`name` 必填；成功响应 `{success:true,item}`；multi 字段以数组返回；JSON 字段为 `customFields/sortOrder/createdAt/updatedAt`。

### S4 合并更新
- 前置：已有干系人 `customFields:{c1:"A"}`。
- 操作：PATCH `/api/work/stakeholders/{id}`，传 `region/liaison/customFields:{c2:"B"}`。
- 预期：仅更新传入字段；`customFields` 合并后保留 c1 并新增 c2。

### S5 查询过滤
- 操作：GET `/api/work/stakeholders?q=采购&region=上海`。
- 预期：按 user_id + deleted=0 + q/team/region AND 过滤；`count` 与 `items.length` 一致。

### S6 软删除与隔离
- 操作：DELETE 某干系人后再次 list。
- 预期：deleted=1 记录不返回。
- 操作：另一个用户 list。
- 预期：看不到当前用户干系人和列配置。
