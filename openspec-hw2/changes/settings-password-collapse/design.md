## Context

设置页的密码修改区块（`#settings-password-section`）当前是内联表单，包含 3 个 password input、红色警告提示和一个全宽按钮，约占 280px 垂直空间。项目中已有多种 modal 模式（task-modal、trip-modal、share-modal 等），均使用 `overlay.style.display` 切换 + 点击 overlay 关闭的模式。

## Goals / Non-Goals

**Goals:**
- 将密码区块从 ~280px 缩减到单行按钮高度（~60px）
- 复用项目现有的 modal 交互模式，保持一致性
- 保持现有 `changePassword()` 验证逻辑和 API 调用不变

**Non-Goals:**
- 不重新设计密码强度校验或添加密码可见性切换
- 不修改后端 API
- 不改动其他设置区块的布局

## Decisions

### 1. Modal 实现方式：HTML 内联 + display 切换

**选择**：在 `index.html` 中静态声明 modal HTML，通过 `style.display` 切换显隐。

**替代方案**：JS 动态创建 DOM。

**理由**：项目所有现有 modal（task-modal、trip-modal、english-modal、share-modal、review-modal）都是 HTML 内联 + display 切换的模式。遵循现有约定，零学习成本。动态创建无明显优势，反而不一致。

### 2. Modal 放置位置：settings-view 内部

**选择**：将 modal overlay 放在 `#settings-view` 内部，紧跟 `settings-container` 之后。

**替代方案**：放在 `<body>` 末尾（与 task-modal 等平级）。

**理由**：密码 modal 只在设置页使用，作用域限定在 settings-view 内更清晰。使用 `position: fixed` 不受父容器影响，视觉效果与全局放置一致。

### 3. 设置页按钮样式：复用 settings-section 卡片

**选择**：保留 `settings-section` 卡片容器，内部只放一个按钮，不再有 `<h4>` 标题。按钮文案"修改密码"自身已表达含义，不需要额外标题。

**替代方案 A**：保留 `<h4>修改密码</h4>` + 按钮。

**替代方案 B**：不用卡片，直接放一个独立按钮。

**理由**：去掉 `<h4>` 避免"修改密码"文字重复（标题说一遍、按钮说一遍）。保留卡片容器保持与其他 section 的视觉一致性。

### 4. Modal 样式：轻量化，不复用 task-modal

**选择**：新建 `pwd-modal` 样式类，参考 task-modal 的 overlay 模式但简化——不需要 header/body/footer 三段式，只需要一个紧凑的表单卡片。

**理由**：task-modal 是 720px 宽的复杂编辑器，密码 modal 只需要 ~400px 宽的简单表单。复用 task-modal 的类名会导致不必要的耦合。样式独立但风格一致（同样的圆角、backdrop-filter、暗色主题适配）。

### 5. 关闭行为：复用现有模式

- 点击 overlay 关闭（`onclick` 在 overlay 元素上）
- modal 内容区 `event.stopPropagation()` 阻止冒泡
- Escape 键关闭（在 `openPwdModal` 时注册 keydown listener，关闭时移除）
- 关闭时清空所有 input 值

### 6. JS 组织：在 settings.js 中添加

**选择**：`openPwdModal()` / `closePwdModal()` 和修改后的 `changePassword()` 全部放在 `settings.js`。

**理由**：密码逻辑已经在 settings.js 中，保持集中。不值得为一个小 modal 新建文件。

## Risks / Trade-offs

- **[风险] 手机端 modal 内键盘遮挡输入框** → Modal 使用 `overflow-y: auto`，input 获得焦点时浏览器会自动滚动到可见区域。如果仍有问题，后续可加 `scrollIntoView()`。
- **[风险] 密码管理器自动填充被 modal 影响** → input 的 `id` 和 `type="password"` 保持不变（只是从 settings section 移到 modal），密码管理器应能正常识别。如果不行，可添加 `autocomplete` 属性辅助。
- **[取舍] 去掉 h4 标题** → 节省空间但降低了一点可发现性。按钮文案"修改密码"足够明确，且修改密码本身是低频操作，可接受。
