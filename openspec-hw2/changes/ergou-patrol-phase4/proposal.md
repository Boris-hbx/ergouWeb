## Why

Phase 1-3 实现了巡游→退场→交互，但切到阿宝 tab 时的收敛动画只是简单 fadeAll + 400ms 后回 home。Phase 4 补上收敛光弧动画，建立"爪印飞回 logo = 二狗回家"的视觉认知。同时增加从阿宝 tab 切走时 logo 微晃（"被叫醒"）。

## What Changes

- 切到阿宝 tab：爪印就地 fade + 单条光弧从二狗当前位置飞向 logo + logo 脉冲
- 从阿宝 tab 切走：logo 微晃动画
- 替换 handleConverge() 的临时实现为完整收敛动画

## Capabilities

### New Capabilities
- `patrol-converge`: 收敛光弧动画——爪印消散 + 光点飞回 logo + logo 脉冲 + 离开微晃

### Modified Capabilities
- `patrol-core`: 替换 handleConverge() 临时实现，增加离开阿宝 tab 的微晃事件

## Impact

- `frontend/assets/js/patrol.js` — handleConverge() 重写，新增光弧动画和微晃逻辑
- `frontend/assets/css/patrol.css` — 新增光弧飞行和 logo 微晃 keyframes
- `frontend/assets/js/app.js` — 从阿宝 tab 切走时 dispatch 事件
