## ADDED Requirements

### Requirement: Section 合并 — 账户信息包含修改密码

账户信息 section SHALL 包含用户名、昵称和修改密码按钮，不再为修改密码设置独立卡片。

#### Scenario: 账户信息区展示修改密码按钮
- **WHEN** 用户进入设置页
- **THEN** 账户信息 section 内依次显示用户名、昵称、修改密码按钮
- **AND** 不存在独立的修改密码 section

### Requirement: Section 合并 — 应用信息包含退出登录

应用信息 section SHALL 在版本号下方包含退出登录按钮，不再为退出登录设置独立卡片。

#### Scenario: 应用信息区展示退出按钮
- **WHEN** 用户滚动到设置页底部
- **THEN** 应用信息 section 内依次显示 App 名称、标语、版本号、退出登录按钮
- **AND** 退出按钮保持红色危险样式
- **AND** 不存在独立的退出登录 section

### Requirement: 头像选择器紧凑布局

头像选择器 SHALL 采用横向布局：预览圆居左，预设网格居右。预设网格默认折叠显示前 8 个，可展开查看全部。

#### Scenario: 默认折叠状态
- **WHEN** 用户进入设置页查看头像区
- **THEN** 左侧显示当前头像预览圆（72px）
- **AND** 右侧显示 8 个预设头像 + 上传按钮
- **AND** 显示"展开"入口

#### Scenario: 展开预设网格
- **WHEN** 用户点击"展开"入口
- **THEN** 网格展开显示全部预设头像（含颜色渐变选项）
- **AND** "展开"变为"收起"

#### Scenario: 收起预设网格
- **WHEN** 用户在展开状态点击"收起"
- **THEN** 网格收回到 8 个预设
- **AND** 折叠动画平滑过渡

### Requirement: 好友与联系人合并

好友列表和联系人列表 SHALL 合并在同一个 settings-section 卡片内，用分隔线区分两个列表。

#### Scenario: 两个列表同卡展示
- **WHEN** 用户在设置页查看好友区
- **THEN** 好友列表和联系人列表在同一张卡片内
- **AND** 两个列表之间用分隔线分隔
- **AND** 联系人部分有自己的子标题

#### Scenario: 好友为空但联系人有数据
- **WHEN** 好友列表为空
- **THEN** 好友区显示空状态提示
- **AND** 联系人列表正常显示在下方

#### Scenario: 联系人为空
- **WHEN** 联系人列表为空
- **THEN** 联系人区域不显示（不占空间）

### Requirement: 全局间距规范

设置页 SHALL 统一使用紧凑间距：section 间 gap 12px，section 内 padding 16px。

#### Scenario: 间距一致性
- **WHEN** 设置页渲染完成
- **THEN** 所有 `.settings-section` 之间的间距为 12px
- **AND** 所有 `.settings-section` 内部 padding 为 16px
- **AND** `.settings-about`（应用信息）padding 与其他 section 一致（不再使用 30px）

### Requirement: 冗余描述文字清理

与 section 标题重复的 `setting-desc` 说明文字 SHALL 被移除。

#### Scenario: AI 模型区无冗余描述
- **WHEN** 用户查看 AI 模型设置区
- **THEN** 不显示"选择二狗使用的 AI 模型"描述文字
- **AND** h4 标题后直接展示模型选择按钮

#### Scenario: 二狗设置区保留有用描述
- **WHEN** 用户查看二狗设置区
- **THEN** 时区选择前的说明文字保留（因其补充了非显而易见的信息）
