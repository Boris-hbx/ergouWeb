## Context

当前记账和差旅模块各自实现了照片选择、缩略图预览、删除、Base64 转换等逻辑，代码高度重复但参数不一致。记账的 `fileToBase64` 用 2048px/0.82 quality，差旅用 utils 的 `imageFileToBase64` 用 1024px/0.85 quality。记账有 10MB 硬拒绝，差旅无校验。记账新建时添加照片后"保存"按钮被"识别账单"替换。

涉及文件：
- `frontend/assets/js/expense.js` — `handlePhotoSelect`、`renderPhotoGrid`、`fileToBase64`、`updateFooterButtons`
- `frontend/assets/js/trip.js` — `handlePhotoSelect`（内联渲染逻辑）
- `frontend/assets/js/utils.js` — `imageFileToBase64`
- `frontend/index.html` — 记账照片区域 HTML（lines 751-757）
- 照片缩略图样式散落在 `<style>` 或内联中，CSS 文件中无对应类名

## Goals / Non-Goals

**Goals:**
- 抽取 PhotoManager 到 utils.js，统一照片选择、压缩、预览、删除、放大查看
- 记账和差旅模块改为调用 PhotoManager，消除重复代码
- 记账模块：添加照片后"保存"和"识别账单"并存
- 缩略图支持点击放大预览
- 文档保障：CLAUDE.md 约定 + FRONTEND.md 组件 API

**Non-Goals:**
- 不改变 AI 识别/分析的核心流程（startParse、analyzeText 内部逻辑不动）
- 不改变照片上传到服务端的 API 接口（FormData POST 保持不变）
- 不改变已上传照片的展示和删除（编辑模式下已有照片由各模块自行管理，因为涉及不同的 API 端点）
- 不引入新的前端框架或构建工具

## Decisions

### D1: PhotoManager 作为 utils.js 中的构造函数

**选择**: 在 utils.js 中新增 `PhotoManager` 构造函数，每个模块创建自己的实例。

**替代方案**:
- 全局单例 → 不行，记账和差旅需要各自独立的照片列表
- 独立文件 photo-manager.js → 增加一个 script 标签和请求，当前项目 utils.js 已是公共工具的归集点

**API 设计**:
```javascript
var pm = new PhotoManager({
    container: '#expense-photo-grid',  // 缩略图渲染容器
    onChange: function(files) { ... }   // 照片列表变化回调
});

pm.addFiles(fileList);    // 添加文件（自动压缩、渲染）
pm.remove(index);         // 删除指定位置
pm.clear();               // 清空全部
pm.getFiles();            // 获取当前 File 列表（用于上传）
pm.getBase64();           // 获取压缩后的 Base64 列表（用于 AI 识别）
```

**理由**: 构造函数模式与项目现有风格一致（Vanilla JS，无 class 语法），实例化时传配置，每个模块独立管理自己的照片状态。

### D2: 统一压缩参数为 1600px / 0.85 quality

**选择**: maxPx=1600, quality=0.85

**替代方案**:
- 保持记账 2048px → 文件仍较大，上传慢
- 用差旅 1024px → 账单细节可能模糊，AI 识别准确度下降

**理由**: 1600px 在清晰度和文件大小之间取平衡。账单文字在 1600px 下仍清晰可读。0.85 quality 是 JPEG 的甜点。统一后两个模块表现一致。

### D3: 移除 10MB 硬拒绝，改为静默压缩

**选择**: 不做文件大小检查，所有图片统一走 Canvas 压缩流程。

**理由**: Canvas drawImage + toDataURL 本身就会将任意大小的图片压缩到合理范围。10MB 原图经过 1600px 重采样 + 0.85 JPEG 后通常不超过 500KB。拒绝用户是糟糕的体验。

### D4: 记账 footer 按钮改为"保存 + 识别账单"并存

**选择**: 当有待上传照片时，footer 显示两个按钮：左侧"保存"，右侧"识别账单 ✨"。

**实现**: 修改 `updateFooterButtons()` 中 `_pendingPhotos.length > 0` 分支，从只显示"识别账单"改为同时显示"保存"和"识别账单"。编辑模式下补充新照片时也显示双按钮。

**布局**: 两个按钮等宽排列。"保存"用 `btn-secondary`（次要操作风格），"识别账单 ✨"用 `btn-primary` 渐变紫色（保持当前 AI 特色样式）。这样视觉引导用户优先 AI 识别，但保存始终可用。

### D5: 缩略图点击放大使用全屏遮罩层

**选择**: 点击缩略图时创建全屏遮罩层（dark overlay）+ 居中大图，点击遮罩或关闭按钮关闭。

**实现**: PhotoManager 内部处理，缩略图 img 绑定 click 事件，删除按钮 click 阻止冒泡（避免同时触发放大）。遮罩层动态创建/销毁，不需要预置 HTML。

**替代方案**:
- 使用第三方 lightbox 库 → 项目不引入额外依赖
- 在模态框内放大 → 空间不够

### D6: 照片样式统一为 `.pm-` 前缀的 CSS 类

**选择**: 新增 `.pm-grid`、`.pm-thumb`、`.pm-remove`、`.pm-lightbox` 等统一 CSS 类到 style.css。

**理由**: 当前记账用 `expense-photo-thumb`，差旅用 `trip-photo-thumb-wrap`，样式分散。统一为 `pm-` 前缀后，两个模块共享相同样式，未来新模块也自动继承。

### D7: expense.js 中删除 fileToBase64，改用 PhotoManager.getBase64()

**选择**: 删除 expense.js 中自写的 `fileToBase64` 函数，`startParse` 改为调用 `pm.getBase64()` 获取压缩后的 Base64 数据。

**理由**: 消除重复实现，确保压缩参数一致。trip.js 的 `analyzeText` 中同理改用 PhotoManager。

## Risks / Trade-offs

**[风险] 重构范围较大，可能引入回归** → 逐模块迁移：先改记账模块验证，再改差旅模块。每步都对照 usecases 验证。

**[风险] Canvas 压缩在低端设备上对超大图片可能卡顿** → 1600px 上限已经控制了 Canvas 尺寸。如果未来发现问题，可加 Web Worker 异步处理，但当前不需要过度设计。

**[权衡] getBase64() 是异步操作（需要 Canvas 绘制）** → 每次调用 getBase64 时重新压缩。不缓存是因为用户可能在两次调用之间删除了照片。性能可接受（几张照片的压缩在百毫秒级）。

**[权衡] 已上传照片不纳入 PhotoManager 管理** → 编辑模式下已有照片的展示和删除涉及不同的 API 端点（记账 vs 差旅），抽象收益低、复杂度高。保持各模块自行处理。
