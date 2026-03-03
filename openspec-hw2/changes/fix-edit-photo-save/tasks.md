## 1. PhotoManager 公共组件

- [x] 1.1 在 `utils.js` 中实现 `PhotoManager` 构造函数，支持 `container`、`onChange` 配置项
- [x] 1.2 实现 `addFiles(fileList)` — 遍历文件，跳过非图片文件（提示），将有效文件加入内部列表，触发渲染和回调
- [x] 1.3 实现自动压缩 — 在 `addFiles` 内部调用 `imageFileToBase64`（统一 maxPx=1600, quality=0.85），不做文件大小拒绝
- [x] 1.4 实现缩略图预览网格渲染 — 在 `container` 中生成 `.pm-thumb` 元素，每个含缩略图 img + `.pm-remove` 删除按钮
- [x] 1.5 实现缩略图点击放大 — img 绑定 click 弹出全屏遮罩层 `.pm-lightbox`，点击遮罩或关闭按钮关闭
- [x] 1.6 实现 `remove(index)` — 删除指定照片，局部更新网格（不刷新页面），触发 onChange 回调
- [x] 1.7 实现 `clear()` 和 `getFiles()` 方法
- [x] 1.8 实现 `getBase64()` — 返回 Promise，对所有待上传文件调用 `imageFileToBase64` 返回 Base64 数组

## 2. CSS 样式

- [x] 2.1 在 `style.css` 中新增 `.pm-grid`、`.pm-thumb`、`.pm-remove`、`.pm-lightbox` 等统一样式
- [x] 2.2 新增记账 footer 双按钮并排布局样式（保存 btn-secondary + 识别 btn-primary 等宽排列）

## 3. 记账模块迁移

- [x] 3.1 在 `expense.js` 中创建 PhotoManager 实例替换 `_pendingPhotos` 数组和 `renderPhotoGrid()`
- [x] 3.2 删除 `expense.js` 中的 `handlePhotoSelect` 函数，改为调用 `pm.addFiles()`
- [x] 3.3 删除 `expense.js` 中的 `fileToBase64` 函数，`startParse` 改为调用 `pm.getBase64()`
- [x] 3.4 修改 `updateFooterButtons()` — 有照片时同时显示"保存"(secondary) 和"识别账单 ✨"(primary)
- [x] 3.5 修改 `updateFooterButtons()` — 编辑模式补充新照片时也显示双按钮
- [x] 3.6 更新 `submitEntry()` 和 `submitEdit()` 中的照片获取逻辑，改用 `pm.getFiles()`
- [x] 3.7 更新 `index.html` 中记账照片区域，file input 的 onchange 改为调用 PhotoManager

## 4. 差旅模块迁移

- [x] 4.1 在 `trip.js` 的 `openItemModal` 中创建 PhotoManager 实例替换 `_pendingPhotos` 和内联渲染逻辑
- [x] 4.2 删除 `trip.js` 中的 `handlePhotoSelect` 函数，改为调用 `pm.addFiles()`
- [x] 4.3 修改 `analyzeText()` 中的 Base64 转换，改为调用 `pm.getBase64()`
- [x] 4.4 更新 `submitItem()` 中的照片获取逻辑，改用 `pm.getFiles()`
- [x] 4.5 更新 `openItemModal` 中动态生成的 HTML，file input 的 onchange 改为调用 PhotoManager

## 5. 验证

- [ ] 5.1 验证记账新建：无照片 → 仅保存按钮；有照片 → 保存 + 识别并存；点保存直接保存；点识别走 AI 流程
- [ ] 5.2 验证记账编辑：加载已有照片；补充新照片 → 显示双按钮；删除照片局部更新
- [ ] 5.3 验证差旅新建/编辑：照片选择、预览、删除、AI 分析均正常
- [ ] 5.4 验证缩略图点击放大预览，删除按钮不触发放大
- [ ] 5.5 验证大文件照片自动压缩（选择 >10MB 照片不报错）
- [ ] 5.6 验证非图片文件被跳过并提示

> 注：验证任务需要部署到 staging 后手动测试

## 6. 文档

- [x] 6.1 在 `CLAUDE.md` 新增公共 UI 组件复用约定，要求新模块使用 PhotoManager
- [x] 6.2 在 `docs/ref/FRONTEND.md` 新增 PhotoManager 章节：初始化参数、方法列表、使用示例、按钮模式规范
