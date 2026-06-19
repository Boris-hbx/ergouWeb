# TEST-ui-design-system: 统一按钮设计语言

> 覆盖任务: T-238, T-239, T-240, T-241
> 范围: Next Web 一期按钮层、基础视觉 token、弹窗/抽屉/浮层、工具栏/视图切换与首批迁移页面

## S1 公共按钮层

- `.eg-btn` 具备默认、hover、focus-visible、active、disabled、loading 状态。
- `.eg-btn--primary|secondary|ghost|danger|danger-fill` 均可单独使用。
- `.eg-btn--sm|md|lg|icon` 均保持稳定高度，不因文字变化挤压布局。
- `.eg-actionbar--modal` 在桌面端右对齐，在窄屏端纵向排列且按钮占满宽度。

## S1.1 清爽管理型基础 token

- `base.css` 提供可复用的 `--eg-surface-*`、`--eg-border-*`、`--eg-radius-*`、`--eg-space-*`、`--eg-text-*`、`--eg-shadow-*` token。
- 浅色、深色、系统自动深色主题下 token 都有可用值。
- 管理界面默认可用边框和背景层级表达结构，overlay/popover 才使用更明显阴影。
- T-238 的 `.eg-btn` 按钮层消费 `--eg-*` token，不继续扩散私有按钮色值。
- `.eg-surface` / `.eg-surface--muted|raised|overlay`、`.eg-divider`、`.eg-text-*` 可作为后续 T-240/T-241/T-242 的基础类。

## S1.2 弹窗/抽屉/浮层结构

- `.eg-modal-overlay`、`.eg-modal`、`.eg-drawer`、`.eg-popover` 可复用 T-239 token。
- 工作创建弹窗、长文本/列设置弹窗、任务详情抽屉、picker 使用 restrained overlay/popover shadow。
- Admin 人物弹窗有明确 header/body/footer/action bar，不再依赖内联宽度。
- Insight 创建/反馈区域使用 token 化边框、浅阴影和 footer 分隔线。
- Review/Task 编辑 footer 使用 `eg-actionbar--modal`，取消为 secondary，保存为 primary。

## S2 工作任务与干系人创建弹窗

- 工作任务创建弹窗底部按钮使用统一按钮层，取消在前、创建在后。
- 创建按钮在标题为空时保持 disabled，标题填写后可点击。
- 干系人创建弹窗底部按钮使用统一按钮层，取消在前、创建在后。
- 弹窗窄屏展示不横向溢出。

## S3 工作弹窗与选择器

- 长文本弹窗取消/保存按钮使用统一按钮层，保存为主按钮。
- 列设置弹窗完成按钮使用统一按钮层。
- 单选/多选 picker 的取消/确认、清除该列/确认按钮使用统一按钮层。

## S4 Admin 密集操作按钮

- 用户审核、封禁、恢复、权限操作按钮使用统一按钮层。
- 风险、审计、分析、人物管理、巡逻控制等密集表格动作不换行溢出。
- 危险操作使用 danger 语义，主操作使用 primary，次操作使用 secondary。

## S5 Insight 创建与提交

- 洞察创建、反馈提交、工厂创建并生成、生成 v1、提交修订使用统一按钮层。
- 失败重试按钮使用 danger 语义的小尺寸按钮。

## S6 回归边界

- 不修改任何接口字段、请求参数和保存流程。
- 不新增前端框架或构建链。
- 修改过的 JS 文件通过 `node --check`。
- `git diff --check` 无空白错误。

## S7 工具栏与主次动作层级

- Work Hub 工具栏保持左侧上下文/搜索/视图切换、右侧配置/创建动作的结构。
- 干系人页工具栏与 Work Hub 使用同一套工具栏、视图切换和动作区视觉语言。
- 视图切换使用 segmented control 语义，active 状态不使用大面积渐变，不与普通按钮混淆。
- 每个页面只保留一个最突出的 primary 创建动作，列设置、筛选、分组选项保持 secondary/低干扰层级。
- Admin 过滤栏、分析工具栏和 Insight 创建 footer 使用 token 化边框、浅背景、统一控件高度和 focus ring。
- 窄屏下搜索、视图切换、动作区可换行，不互相挤压或横向溢出。
