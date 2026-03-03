## 1. HTML 结构改造

- [x] 1.1 将 `#settings-password-section` 内的表单（3 个 input + hint + 提交按钮）替换为单个"修改密码"按钮
- [x] 1.2 在 `#settings-view` 内添加 `pwd-modal-overlay` + `pwd-modal` 的 HTML 结构，包含 3 个 password input、警告提示、提交按钮和关闭按钮
- [x] 1.3 保持 input 的 id 不变（`settings-old-password`、`settings-new-password`、`settings-confirm-password`），确保密码管理器兼容

## 2. CSS 样式

- [x] 2.1 添加 `pwd-modal-overlay` 样式（fixed 全屏、半透明背景、flex 居中）
- [x] 2.2 添加 `pwd-modal` 样式（~400px 宽、max-width 90vw、圆角、backdrop-filter、暗色主题适配）
- [x] 2.3 添加 modal 内部表单样式（复用现有 `.setting-group` 样式，添加 modal header 关闭按钮样式）

## 3. JS 逻辑

- [x] 3.1 在 `settings.js` 中添加 `openPwdModal()` 函数：显示 overlay、清空 input、聚焦当前密码字段
- [x] 3.2 在 `settings.js` 中添加 `closePwdModal()` 函数：隐藏 overlay、清空所有 input
- [x] 3.3 添加 Escape 键监听：modal 打开时注册 keydown listener，关闭时移除
- [x] 3.4 修改 `changePassword()` 函数：成功后调用 `closePwdModal()` 自动关闭 modal
- [x] 3.5 保持 `loadSettingsData()` 中 guest 用户隐藏密码 section 的逻辑不变

## 4. 验证

- [ ] 4.1 桌面端验证：按钮点击打开 modal → 填写 → 提交成功 → 自动关闭
- [ ] 4.2 桌面端验证：点击 overlay / X 按钮 / Escape 均可关闭 modal
- [ ] 4.3 桌面端验证：验证失败（空密码、短密码、不匹配）modal 保持打开、字段保留
- [ ] 4.4 手机端验证：modal 在小屏幕上显示正常，键盘弹出时 input 可见
- [ ] 4.5 Guest 模式验证：设置页不显示修改密码按钮
- [ ] 4.6 暗色主题验证：modal 在 dark mode 下样式正确
