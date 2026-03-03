## ADDED Requirements

### Requirement: 任务完成已阅爪印

当用户完成 Todo 任务且二狗正在巡游时，系统应在完成的卡片上踩一个"已阅"爪印：

- 任务卡片的 `.task-item` 元素位置获取 getBoundingClientRect
- 在卡片中心偏上位置放置一个爪印，使用 `stamped` 样式（比普通爪印大 20%，opacity 0.7，带微光）
- 爪印不经过正常蒸发周期，持续显示 2 秒后淡出
- 爪印颜色使用当前环境色

#### Scenario: 巡游中完成任务
- **WHEN** 二狗在 walk/pause/rest 状态
- **AND** 用户完成一个任务（completed: true）
- **THEN** 在该任务卡片上显示已阅爪印
- **AND** 已阅爪印 2 秒后淡出

#### Scenario: 非巡游时完成任务
- **WHEN** 二狗在 home 状态
- **AND** 用户完成一个任务
- **THEN** 不产生任何爪印效果

---

### Requirement: 象限清空 ✓ 爪印图案

当象限内最后一个任务被完成时，二狗踩出 ✓ 形爪印图案：

- 检测当前象限是否还有未完成任务（`allItems` 中同 quadrant 同 tab 的非 completed 非 deleted 项）
- 如果是最后一个，在象限区域踩出 3 个爪印组成的 ✓ 形状
- ✓ 形路径：左下→中下→右上，爪印间隔 100ms
- 所有 ✓ 爪印 3 秒后同时淡出

#### Scenario: 完成象限最后任务
- **WHEN** 二狗在巡游中
- **AND** 用户完成某象限最后一个任务
- **THEN** 在该象限区域踩出 ✓ 形爪印图案

#### Scenario: 象限仍有任务
- **WHEN** 用户完成任务但象限内仍有其他未完成任务
- **THEN** 仅显示普通已阅爪印（不触发 ✓ 图案）

---

### Requirement: Jelly pill 跟随跑动

当底部 nav 切换导致 jelly pill 滑动时，二狗在 pill 滑动方向跑 2-3 步：

- 监听 `patrol:jellyMove` 自定义事件（由 app.js jelly pill moveTo 时 dispatch）
- 事件 detail 包含 `{fromX, toX, direction}` — pill 的起止 X 坐标和方向
- 二狗从当前位置向 pill 滑动方向快速移动 2-3 步
- 步间隔缩短为正常的 60%（表现为快速小跑）
- 切到阿宝 tab 时不触发（已有收敛逻辑优先）

#### Scenario: 切换到例行审视页
- **WHEN** 二狗在巡游中
- **AND** 用户从 Todo 切到例行审视
- **AND** jelly pill 向右滑动
- **THEN** 二狗向右快跑 2-3 步

#### Scenario: 切到阿宝 tab
- **WHEN** 用户切到阿宝 tab
- **THEN** 收敛动画优先，不触发跟随跑动

---

### Requirement: 聊天状态 logo 联动

阿宝 tab 聊天状态变化时，底部 nav 阿宝 logo 显示对应动效：

- AI 思考中（等待回复）：阿宝 logo 呼吸灯效果（opacity 在 0.5-1.0 之间缓慢脉冲，周期 2 秒）
- AI 回复完成：停止呼吸灯，恢复正常
- 使用 CSS class `.abao-thinking` 控制动画
- 不依赖二狗巡山状态，任何时候都生效

#### Scenario: AI 思考中
- **WHEN** 用户发送消息
- **AND** AI 正在生成回复
- **THEN** 阿宝 logo 显示呼吸灯效果

#### Scenario: AI 回复完成
- **WHEN** AI 回复生成完毕
- **THEN** 阿宝 logo 呼吸灯停止

---

### Requirement: 事件派发契约

交互系统依赖以下自定义事件，由对应模块负责 dispatch：

| 事件名 | dispatch 位置 | detail |
|--------|--------------|--------|
| `patrol:taskComplete` | tasks.js `saveProgress()` | `{itemId, quadrant, isLastInQuadrant, cardEl}` |
| `patrol:jellyMove` | app.js `activateMobileNav()` | `{fromX, toX, direction}` |
| `patrol:chatStatus` | abao.js 发送/接收处理 | `{status: 'thinking'\|'idle'}` |
