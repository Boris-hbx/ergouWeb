## MODIFIED Requirements

### Requirement: EventBridge 增加交互事件监听

patrol.js 的 EventBridge 应增加以下事件监听：

1. `patrol:taskComplete` — 任务完成时触发已阅/✓ 爪印
2. `patrol:jellyMove` — jelly pill 滑动时触发跟随跑动
3. `patrol:chatStatus` — 聊天状态变化时控制 logo 动效

所有交互事件的处理函数应检查 `_sm.canPatrol` 门控（chatStatus 例外，不依赖巡游状态）。

#### Scenario: 交互事件处理
- **WHEN** patrol:taskComplete 事件触发
- **AND** _sm.canPatrol 为 true
- **THEN** 调用已阅爪印逻辑

---

### Requirement: 交互不中断正常巡游

交互动画（已阅爪印、跟随跑动）应叠加在正常巡游之上，不中断当前行走状态：

- 已阅爪印使用独立的 DOM 元素，不占用 PawPool
- 跟随跑动通过临时修改行走方向实现，不重置状态机
- 交互完成后自动恢复正常巡游

#### Scenario: 行走中触发交互
- **WHEN** 二狗在 walk 状态
- **AND** 用户完成任务
- **THEN** 已阅爪印出现，同时行走不中断
