## Context

设置页当前 11 个 `.settings-section` 卡片纵向堆叠，总高约 1920px。头像选择器独占 400px，两个单按钮卡片各 100px，好友/联系人分两张卡。间距 gap 20px × 12 + padding 20px 累积大量空白。纯前端改动，不涉及后端。

关键代码位置：
- HTML: `index.html` 设置 tab 区域（`#settings` 内）
- CSS: `style.css` 中 `.settings-section`、`.avatar-picker`、`.settings-about` 等
- JS: `settings.js` 中 `initAvatarPicker()`、`initPatrolToggle()`；联系人 section 由 `settings.js` 中 `Contacts.ensureSection()` 动态插入

## Goals / Non-Goals

**Goals:**
- 设置页总高度降至 ~1200px（约 1.5 屏），减少 ~35% 垂直空间
- 合并逻辑相关的 section，减少卡片总数从 11 降至 7-8
- 头像选择器紧凑化，默认折叠预设
- 统一间距规范

**Non-Goals:**
- 不改变任何设置项的功能行为（只调布局）
- 不重构 JS 模块结构
- 不添加新设置项
- 不做分组 tab/折叠面板等重交互改造

## Decisions

### D1: 头像选择器横排布局

**方案**: 预览圆（72px）固定在左侧，预设网格在右侧。默认显示 8 个预设（2 行 × 4 列，每个 40px），点击"更多"展开全部。

**替代方案**: 把头像选择器做成弹窗/底部抽屉 → 增加交互复杂度，改动量大，排除。

**实现**:
- `.avatar-picker` 改为 `display: flex; flex-direction: row; align-items: flex-start; gap: 16px`
- 预览圆 `flex-shrink: 0`
- 网格容器 `flex: 1; overflow: hidden; max-height: 96px`（2 行 × 40px + 16px gap）
- 展开时 `max-height: none`，CSS transition 平滑过渡
- "更多/收起" 用一个小文字链接，位于网格底部

### D2: 修改密码并入账户信息

**方案**: 删除 `#settings-password-section` 独立卡片，将按钮移入账户信息 section 末尾。

**实现**:
- HTML: 在账户信息 section 的最后一个 `.setting-group-inline` 之后添加修改密码按钮
- CSS: 按钮改为 `width: auto; align-self: flex-start`，不再全宽
- JS: 无需改动，按钮 onclick 绑定不变

### D3: 退出登录并入应用信息

**方案**: 删除退出登录独立卡片，按钮移入 `.settings-about` 底部。

**实现**:
- HTML: 在版本号 div 之后添加退出按钮
- CSS: 退出按钮 `margin-top: 16px; width: 100%`，保持红色危险样式
- `.settings-about` 的 padding 从 `30px 20px` 改为 `16px`（统一）

### D4: 好友 + 联系人合并

**方案**: `Contacts.ensureSection()` 不再创建独立 section，改为在 `#settings-friends-section` 内部追加联系人子区域。

**实现**:
- JS: `ensureSection()` 改为在 `#settings-friends-section` 内部查找/创建 `#contacts-list` 容器
- HTML: 在好友 section 内添加一个分隔线 + 联系人子标题 + 列表容器
- 联系人为空时整个子区域 `display: none`

### D5: 全局间距收紧

**方案**: `.settings-container` gap 20px → 12px，`.settings-section` padding 20px → 16px。

**实现**:
- CSS 两行改动
- 需确认深色主题下间距视觉效果

## Risks / Trade-offs

- [头像网格折叠 max-height 动画] → 已知预设数量固定（25 个），可预计算展开高度，不会出现跳动
- [联系人合并改 DOM 结构] → `Contacts.ensureSection()` 是唯一插入点，改动可控
- [间距收紧后卡片内容拥挤] → padding 从 20→16px 仅减少 4px 每侧，视觉影响小
