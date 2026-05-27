## 1. 全局间距收紧

- [x] 1.1 CSS: `.settings-container` gap 从 20px 改为 12px
- [x] 1.2 CSS: `.settings-section` padding 从 20px 改为 16px
- [x] 1.3 CSS: `.settings-about` padding 从 `30px 20px` 改为 16px

## 2. 合并修改密码到账户信息

- [x] 2.1 HTML: 将修改密码按钮移入账户信息 section 末尾
- [x] 2.2 HTML: 删除 `#settings-password-section` 独立卡片
- [x] 2.3 CSS: 修改密码按钮样式调整（不再全宽，`margin-top: 12px`）

## 3. 合并退出登录到应用信息

- [x] 3.1 HTML: 将退出登录按钮移入 `.settings-about` section 版本号之后
- [x] 3.2 HTML: 删除退出登录独立卡片
- [x] 3.3 CSS: 退出按钮在 `.settings-about` 内的间距样式（`margin-top: 16px; width: 100%`）

## 4. 头像选择器紧凑化

- [x] 4.1 CSS: `.avatar-picker` 改为 flex row 布局（预览圆左，网格右）
- [x] 4.2 CSS: 预设网格默认 `max-height` 折叠为 2 行，展开时 `max-height: none` + transition
- [x] 4.3 HTML: 添加"更多/收起"文字链接
- [x] 4.4 JS: `settings.js` 添加展开/收起切换逻辑
- [x] 4.5 预设头像尺寸从 48px 调整为 40px 以适配横排布局

## 5. 好友与联系人合并

- [x] 5.1 JS: `Contacts.ensureSection()` 改为在 `#settings-friends-section` 内部创建联系人子区域
- [x] 5.2 HTML: 好友 section 内添加分隔线 + 联系人子标题 + 列表容器
- [x] 5.3 联系人为空时子区域 `display: none`

## 6. 冗余描述清理

- [x] 6.1 HTML: 移除 AI 模型区 "选择二狗使用的 AI 模型" 描述文字
- [x] 6.2 确认二狗设置区时区说明文字保留
