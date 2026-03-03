## MODIFIED Requirements

### Requirement: handleConverge 替换为完整收敛动画

patrol.js 的 handleConverge() 应从临时实现替换为完整收敛动画序列。

当前临时实现：fadeAll + 400ms setTimeout + convergeDone
替换为：fadeAll + 光弧飞行 + logo 脉冲 + convergeDone

### Requirement: 离开阿宝 tab 微晃事件

EventBridge 应监听 `patrol:leaveAbao` 事件，触发 logo 微晃。
app.js 的 switchPage() 在从阿宝相关页面切走时 dispatch 此事件。
