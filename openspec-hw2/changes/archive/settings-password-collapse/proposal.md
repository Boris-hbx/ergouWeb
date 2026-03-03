## Why

设置页的「修改密码」区块占据了大量垂直空间（3 个 input + 警告提示 + 按钮），但修改密码是极低频操作。这个区块挤压了更常用设置项（好友、AI 模型、通知）的可见性，尤其在手机端需要多滚一屏才能看到下面的内容。

## What Changes

- 将密码修改区块从「始终展开的表单」改为「一个按钮触发弹窗」
- 设置页中密码区只保留一个「修改密码」按钮，点击后弹出 Modal 完成操作
- Modal 内包含原有的 3 个密码输入框 + 警告提示 + 提交按钮
- 现有验证逻辑和 API 调用保持不变

## Capabilities

### New Capabilities

- `settings-password-modal`: 将密码修改从内联表单改为按钮触发的 Modal 弹窗交互

### Modified Capabilities

（无现有 spec 需要修改）

## Impact

- **前端 HTML**: `frontend/index.html` — 密码区块 HTML 结构重写（移除内联表单，添加按钮 + Modal）
- **前端 JS**: `frontend/assets/js/settings.js` — 添加 Modal 打开/关闭逻辑，`changePassword()` 函数改为读取 Modal 内的 input
- **前端 CSS**: `frontend/assets/css/style.css` — 密码 Modal 样式（复用项目现有 Modal 模式或新增）
- **后端**: 无变更，API 接口不变
- **Guest 模式**: 维持现有行为（guest 用户不显示修改密码按钮）
