# TEST-stakeholder-frontend

覆盖 T-223：工作 Hub 干系人前端入口、独立页、表格/看板/分布三视图、详情抽屉和新建表单。

## T-223 前端

### S1 Hub 入口
- 操作：进入工作 Hub，点击「干系人」卡片。
- 预期：隐藏工作 Hub 和任务表，显示干系人独立页；刷新后可恢复干系人页。

### S2 数据加载
- 操作：打开干系人页。
- 预期：并行请求 `/api/work/stakeholder-columns` 和 `/api/work/stakeholders`；加载失败时显示 toast 并记录 `console.error('[stakeholder]', err)`。

### S3 表格视图
- 操作：查看表格视图，搜索姓名/部门/地域/事项。
- 预期：列头来自 stakeholder column 配置；multi 字段显示 chip；搜索胶囊显示「N 人命中 / 共 M 人」；行点击打开详情抽屉。

### S4 看板视图
- 操作：切到看板，选择按部门/地域等 select 字段分列，拖卡到另一列。
- 预期：卡片显示姓名、职务、地域、管理方式摘要；拖卡 patch 对应 select 字段；不套用任务 status/完成链。

### S5 分布视图
- 操作：切到分布，切换任意 select/multi 维度。
- 预期：气泡大小按人数缩放，颜色为中性；multi 字段一个人可落多个卡，显示 `+N`；未填分组置末。

### S6 详情与新建
- 操作：点击行/卡打开详情，修改字段保存；点击「+ 新建干系人」创建。
- 预期：姓名必填；multi 字段支持逗号/分号输入；保存走 PATCH，新建走 POST；删除走软删除接口并从前端列表移除。

### S7 列设置
- 操作：点击「⚙ 列设置」，改列名/类型/宽度，给 select/multi 增删改选项，拖动调整顺序，新增并删除自定义列。
- 预期：批量变更走 `PUT /api/work/stakeholder-columns`；新增走 `POST /api/work/stakeholder-columns`；删除自定义列走 `DELETE /api/work/stakeholder-columns/{key}`；内置列显示锁定不可删；保存后表格/看板/分布同步刷新。
