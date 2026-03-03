## Context

Phase 1 的 handleConverge() 是临时占位：fadeAll + 400ms 后 convergeDone。Phase 4 替换为完整光弧收敛动画。

关键约束：
- 光弧飞行 0.4s，贝塞尔曲线
- Logo 脉冲 0.3s
- 总时长 <1s
- 不阻塞 tab 切换（tab 内容已经在切换，收敛是视觉糖果）

## Decisions

### D1: 光弧用 CSS animation + 绝对定位

**决定**: 光点 `.patrol-arc` 创建在 patrol-layer 中，通过 CSS `@keyframes` 从起点飞到终点。

**理由**: 纯 CSS 动画不阻塞主线程。起点和终点通过 CSS 自定义属性 `--arc-from-x/y` `--arc-to-x/y` 动态设置。

### D2: 贝塞尔曲线用 3 个路径点

**决定**: 光弧路径用 3 个 CSS keyframe 帧：起点 → 中间高点（上弧）→ 终点。不用真正的贝塞尔曲线 API。

**理由**: CSS animation 的 keyframe 百分比足以模拟弧线。中间点在起点和终点的中间偏上 40px，产生自然弧度。

### D3: 微晃用 patrol:leaveAbao 自定义事件

**决定**: app.js 在 switchPage 检测"从阿宝切走"时 dispatch `patrol:leaveAbao`，patrol.js 监听并添加 `.abao-wiggle` class。

**理由**: 与 Phase 3 的事件派发模式一致，解耦。

## Files

| 文件 | 职责 |
|------|------|
| `patrol.js` | **修改** — handleConverge 重写 + 微晃监听 |
| `patrol.css` | **修改** — .patrol-arc 光弧样式 + .abao-wiggle 微晃 |
| `app.js` | **修改** — switchPage 中 dispatch patrol:leaveAbao |
| `index.html` | **修改** — 版本号更新 |
