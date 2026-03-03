## Why

Phase 1-2 让二狗具备了出场、地形感知行走、退场的完整循环，但二狗目前只是"路过"，与页面内容没有任何互动。Phase 3 增加页面交互，让二狗对用户操作做出反应，从一个"会走的装饰"变成一个"有感知的伙伴"。

## What Changes

- Todo 页面：用户在二狗巡山期间完成任务时，二狗在完成的卡片上踩一个"已阅"爪印
- Todo 页面：当象限内最后一个任务被完成时，二狗踩出特殊 ✓ 爪印图案
- 记账/例行审视页：底部 nav 切换时 jelly pill 滑动，二狗同向跑动响应
- 阿宝 tab：聊天对话进行时，阿宝 logo 脉冲/呼吸灯联动

## Capabilities

### New Capabilities
- `patrol-interact`: 二狗与页面内容的交互反应系统——任务完成踩爪印、jelly pill 跟随、聊天状态联动

### Modified Capabilities
- `patrol-core`: 增加交互事件监听（任务完成、jelly pill 移动、聊天状态变化）和对应的状态/行为

## Impact

- `frontend/assets/js/patrol.js` — 新增交互事件监听和响应逻辑
- `frontend/assets/js/tasks.js` — 任务完成时 dispatch 自定义事件
- `frontend/assets/js/app.js` — jelly pill moveTo 时 dispatch 自定义事件
- `frontend/assets/js/abao.js` — 聊天状态变化时 dispatch 自定义事件
- `frontend/assets/css/patrol.css` — 新增已阅爪印和 ✓ 图案样式
