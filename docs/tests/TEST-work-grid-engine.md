# TEST-work-grid-engine

覆盖 T-222：抽 `work-table.js` / `work-board.js` / `work-distribution.js` 为通用「列配置 + 行数据」渲染引擎后，工作任务表现有行为必须不变。

## 回归对账

### S1 表格视图骨架不变
- 操作：进入工作表表格视图。
- 预期：序号列仍为 54px；表格总宽仍等于序号列 + 所有列宽；表头不可编辑；拖宽条只出现在相邻列之间，最右列右侧没有拖宽条。

### S2 表格编辑链不变
- 操作：分别点击文本、日期、数字、进度、select、multi、longtext 单元格。
- 预期：仍走原 WorkTable 编辑器 / WorkPick / datepicker / progress dialog；更新仍通过 `Work.updateRow` 或 `Work.createRow`。

### S3 表头筛选与搜索不变
- 操作：使用全局搜索，再打开表头漏斗筛选 select/multi/status 列。
- 预期：总数胶囊显示「N 条命中 / 共 M 项」；清除全部筛选链接仅在列筛选生效时出现；0 命中显示原空状态。

### S4 看板行为不变
- 操作：切到看板视图，拖卡到待办/进行中/阻塞列。
- 预期：看板仍只显示三列，不显示已完成列；拖放只 patch `status`；卡片空白处打开详情抽屉，简介图标仍只打开简介。

### S5 分布视图行为不变
- 操作：切到分布视图，切换任一 select/multi/status 维度。
- 预期：multi 字段同一任务出现在多个卡片；`+N` 标签仍显示；未标记永远在末尾；气泡颜色仍按逾期/P0/正常/低优；点气泡 toggle 展开并高亮对应卡片。

### S6 加载顺序
- 操作：打开主页面。
- 预期：`work-grid-engine.js` 在 `work-table.js`、`work-board.js`、`work-distribution.js` 之前加载；四个文件 `node --check` 均通过。
