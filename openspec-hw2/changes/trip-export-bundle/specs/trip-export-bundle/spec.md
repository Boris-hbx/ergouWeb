## ADDED Requirements

### Requirement: Bundle export endpoint

系统 SHALL 提供 `GET /api/trips/:id/export/bundle` 端点，返回一个 zip 文件包含：

1. **根目录**: `{行程名}-报销清单.xlsx`（与现有 `export_xlsx` 生成逻辑一致）
2. **照片文件夹**: 每个有照片的 trip_item 一个文件夹

zip 文件名: `{行程名}-报销材料.zip`

Content-Type: `application/zip`
Content-Disposition: `attachment; filename*=UTF-8''{encoded_filename}`

#### Scenario: 打包下载含照片的行程
- **WHEN** 用户请求 `GET /api/trips/:id/export/bundle`，行程有 3 个条目，其中 2 个有照片
- **THEN** 返回 zip 包含 1 个 xlsx 文件 + 2 个照片文件夹

#### Scenario: 打包下载无照片的行程
- **WHEN** 用户请求 `GET /api/trips/:id/export/bundle`，行程有条目但无照片
- **THEN** 返回 zip 仅包含 1 个 xlsx 文件，无照片文件夹

#### Scenario: 无权限访问
- **WHEN** 用户请求不属于自己的行程
- **THEN** 返回 404

### Requirement: Photos organized by item folder

照片 SHALL 按 trip_item 分组，每个事项一个文件夹。

文件夹命名规则：
- 格式: `{date} - {description}`
- 示例: `2026-3-1 - Uber Waterloo→Toronto Pearson机场`
- 描述为空时: `{date} - 未命名`
- 文件系统非法字符处理: `/` → `-`，`\` → `-`，`:` → `-`，`*?"<>|` → 移除
- 同名文件夹冲突: 追加序号 `(2)`、`(3)`...

文件夹内照片命名：
- 格式: `{序号}.{原始扩展名}`（如 `01.jpg`、`02.png`）
- 序号按 `trip_photos.created_at` 排序

#### Scenario: 多事项不同日期
- **WHEN** 行程有: 3/1 酒店（2张照片）、3/2 打车（1张照片）
- **THEN** zip 中有两个文件夹: `2026-3-1 - 酒店/01.jpg, 02.jpg` 和 `2026-3-2 - 打车/01.jpg`

#### Scenario: 同日期同描述的多个事项
- **WHEN** 行程有两个条目都是 3/1 午餐
- **THEN** 文件夹名为 `2026-3-1 - 午餐` 和 `2026-3-1 - 午餐 (2)`

#### Scenario: 描述含特殊字符
- **WHEN** 事项描述为 `Uber: Waterloo/Airport`
- **THEN** 文件夹名为 `2026-3-1 - Uber- Waterloo-Airport`（`:` 和 `/` 替换为 `-`）

#### Scenario: 描述为空
- **WHEN** 事项没有描述
- **THEN** 文件夹名为 `2026-3-1 - 未命名`

### Requirement: Frontend bundle download option

前端导出菜单 SHALL 新增"打包下载"选项，位于现有选项之前（最优先位置）。

按钮文案: "打包下载（Excel + 照片）"

#### Scenario: 有照片时显示打包选项
- **WHEN** 行程有照片，用户点击导出按钮
- **THEN** 导出菜单显示三个选项: 打包下载、仅 Excel、仅照片

#### Scenario: 无照片时显示打包选项
- **WHEN** 行程无照片，用户点击导出按钮
- **THEN** 导出菜单显示两个选项: 打包下载（Excel）、仅 Excel（打包下载仍可用，zip 内只有 Excel）
