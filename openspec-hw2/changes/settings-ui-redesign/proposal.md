## Why

设置页当前采用单列全宽卡片堆叠布局，11 个独立 section 总高度约 1920px，手机上需要滚动两屏以上。多个 section 只包含一个按钮却独占整张卡片（修改密码、退出登录），头像选择器占页面 20% 空间，好友与联系人主题相同却分成两张卡。需要通过合并、折叠、紧凑化来大幅减少垂直空间占用，让用户更快找到目标设置项。

## What Changes

- **合并单按钮卡片**：修改密码按钮并入"账户信息"section，退出登录按钮并入"应用信息"section，消除两张只有单按钮的独立卡片
- **头像选择器紧凑化**：预览圆与预设网格改为横向排列（预览居左、网格居右），预设默认折叠只显示 8 个，点击展开查看全部
- **好友 + 联系人合并**：两个主题相同的 section 合并为一张卡片，用 tab 或分隔线区分
- **section 间距收紧**：卡片间 gap 从 20px 减至 12px，section 内 padding 从 20px 统一为 16px
- **冗余 desc 文字清理**：移除与 h4 标题重复的 `setting-desc` 说明文字

## Capabilities

### New Capabilities
- `settings-layout`: 设置页布局重构 — section 合并策略、间距规范、头像选择器紧凑布局、折叠/展开交互

### Modified Capabilities

（无 spec 级行为变更，仅 UI 布局调整）

## Impact

- `frontend/index.html` — 设置页 HTML 结构重组（合并 section、调整头像选择器标记）
- `frontend/assets/css/style.css` — settings 相关样式（间距、布局、折叠动画）
- `frontend/assets/js/settings.js` — 头像折叠展开逻辑、联系人 section 合并逻辑
- 不涉及后端、API、数据库变更
