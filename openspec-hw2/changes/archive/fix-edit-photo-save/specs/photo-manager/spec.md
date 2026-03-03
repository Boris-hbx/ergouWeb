## ADDED Requirements

### Requirement: PhotoManager 提供统一的照片选择能力
系统 SHALL 提供 PhotoManager 公共组件，所有需要照片上传的模块 MUST 通过 PhotoManager 处理照片选择，不得各自实现。

#### Scenario: 用户选择单张照片
- **WHEN** 用户通过文件选择器选择一张照片
- **THEN** PhotoManager 将照片加入待上传列表并触发变更回调

#### Scenario: 用户选择多张照片
- **WHEN** 用户一次选择多张照片
- **THEN** PhotoManager 将所有照片加入待上传列表，不覆盖已有照片

#### Scenario: 用户分多次添加照片
- **WHEN** 用户先选择了照片 A，之后再次选择照片 B
- **THEN** 待上传列表包含 A 和 B，累加而非替换

### Requirement: PhotoManager 自动压缩照片
系统 SHALL 在添加照片时自动压缩，用户无需关心文件大小。压缩参数 MUST 统一（maxPx、quality），由 PhotoManager 内部调用已有的 `imageFileToBase64` 能力。

#### Scenario: 大文件照片自动压缩
- **WHEN** 用户选择一张 15MB 的高分辨率照片
- **THEN** PhotoManager 自动压缩后加入列表，用户无感知，不显示任何错误或警告

#### Scenario: 小文件照片同样经过压缩流程
- **WHEN** 用户选择一张 500KB 的照片
- **THEN** PhotoManager 同样执行压缩流程（确保一致性），加入列表

### Requirement: PhotoManager 渲染缩略图预览网格
系统 SHALL 在指定容器内渲染照片缩略图网格，每张缩略图可点击放大预览，并附带删除按钮。

#### Scenario: 添加照片后显示预览
- **WHEN** 用户添加照片
- **THEN** 预览容器内显示该照片的缩略图和删除按钮

#### Scenario: 点击缩略图放大预览
- **WHEN** 用户点击某张缩略图
- **THEN** 系统全屏展示该照片的大图，用户可点击关闭或点击遮罩层关闭

#### Scenario: 删除一张照片
- **WHEN** 用户点击某张缩略图的删除按钮
- **THEN** 仅该缩略图从网格中移除，其余缩略图及删除按钮保持原位不变，页面不刷新、表单不关闭，用户可连续删除多张

#### Scenario: 删除所有照片
- **WHEN** 用户逐一删除所有照片
- **THEN** 列表清空，预览容器为空，变更回调通知列表为空

### Requirement: PhotoManager 通过回调通知模块状态变化
系统 SHALL 在照片列表发生变化时（添加、删除）调用模块注册的回调函数，传入当前照片列表。模块据此更新自身 UI（如按钮状态）。

#### Scenario: 添加照片触发回调
- **WHEN** 用户添加照片导致列表从 0 张变为 1 张
- **THEN** 回调被调用，模块可据此显示 AI 分析按钮

#### Scenario: 删除最后一张照片触发回调
- **WHEN** 用户删除最后一张照片导致列表清空
- **THEN** 回调被调用，模块可据此隐藏 AI 分析按钮

### Requirement: PhotoManager 处理非图片文件
系统 SHALL 在用户选择非图片文件或无法解码的文件时，提示并跳过该文件，不影响其余正常文件的处理。

#### Scenario: 选择非图片文件
- **WHEN** 用户选择了一个 PDF 文件和一张 JPG 照片
- **THEN** PDF 被跳过并提示，JPG 正常压缩并加入列表

#### Scenario: 图片文件损坏无法解码
- **WHEN** 用户选择了一张损坏的图片文件
- **THEN** 该文件被跳过并提示，不阻塞流程

### Requirement: 保存按钮始终可用
所有带照片的模块（记账、差旅等），无论是否已添加照片，"保存"按钮 MUST 始终可见可用。AI 分析/识别按钮 MUST 作为独立的可选操作，不得替换保存按钮。

#### Scenario: 记账新建添加照片后的按钮状态
- **WHEN** 用户在新建记账表单中添加了照片
- **THEN** 同时显示"保存"和"识别账单 ✨"两个按钮

#### Scenario: 记账新建未添加照片时的按钮状态
- **WHEN** 用户在新建记账表单中未添加照片
- **THEN** 仅显示"保存"按钮，不显示 AI 相关按钮

#### Scenario: 记账编辑模式补充新照片
- **WHEN** 用户在编辑记账条目时补充了新照片
- **THEN** 同时显示"保存"和"识别账单 ✨"两个按钮

#### Scenario: 差旅模式按钮不受影响
- **WHEN** 用户在差旅条目表单中操作
- **THEN** "保存"始终可见，"阿宝分析 ✨"作为独立按钮存在（现有行为保持）

### Requirement: 文档记录公共组件 API
CLAUDE.md MUST 新增公共 UI 组件复用约定。docs/ref/FRONTEND.md MUST 新增 PhotoManager API 文档章节，包含初始化参数、方法列表和使用示例。

#### Scenario: 开发新模块时查阅文档
- **WHEN** 开发者（人或 AI）需要在新模块中添加照片上传功能
- **THEN** CLAUDE.md 约定引导其查阅 FRONTEND.md，FRONTEND.md 提供 PhotoManager 的完整 API 和使用示例
