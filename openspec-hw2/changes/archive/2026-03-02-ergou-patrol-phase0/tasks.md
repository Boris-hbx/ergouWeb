## 1. SVG 资产

- [x] 1.1 设计单爪 SVG（16×16, 4 趾垫 + 1 掌垫, <500 bytes）
- [x] 1.2 设计双爪 tab icon SVG（stroke/fill 双层，支持实心↔空心过渡）
- [x] 1.3 替换 index.html 中 4 处 🐾 emoji 为 inline SVG
- [x] 1.4 真机验证 SVG 在不同背景上的渲染效果（白底/彩色/深色）

## 2. CSS 视觉层

- [x] 2.1 创建 patrol.css：爪印基础样式 + heading 自定义属性系统
- [x] 2.2 实现落地弹性动画（patrol-land keyframes，含 --paw-mirror/--paw-heading）
- [x] 2.3 实现涟漪扩散（::before/::after 双环 patrol-ripple）
- [x] 2.4 实现蒸发动画（patrol-evaporate keyframes）
- [x] 2.5 实现呼吸动画（patrol-breathe keyframes）
- [x] 2.6 实现 tab icon 状态过渡（patrol-out/returning/pulse/breathing/bounce）
- [x] 2.7 实现 reduced-motion 媒体查询禁用规则
- [x] 2.8 在 index.html 中引入 patrol.css

## 3. 通用工具模块

- [x] 3.1 实现 ObjectPool（create/acquire/release/releaseAll/destroy/activeCount）
- [x] 3.2 实现 DeviceProfile（tier/canBlend/reduceMotion/isSupported/onReduceMotionChange）
- [x] 3.3 实现 CSSAnimator（inject/remove/clear/generateWalkKeyframes）
- [x] 3.4 实现 IdleDetector（start/stop/destroy/isIdle/cooldownRemaining/startCooldown/resetCooldown/setIdleThreshold/setCooldown）
- [x] 3.5 实现 PatrolStateMachine（transition/state/canPatrol/forceState/reset）
- [x] 3.6 实现 PawPool（step 含 heading 参数/fadeAll/fadeWave/clear/activeCount/destroy）

## 4. 动画展台

- [x] 4.1 创建 patrol-showcase.html 基础结构（mobile-first 布局，桌面端提示条）
- [x] 4.2 实现爪印渲染展区（白底 + 彩色背景）
- [x] 4.3 实现落地动画展区（可调时长参数）
- [x] 4.4 实现蒸发动画展区（可调时长参数）
- [x] 4.5 实现步态预览展区（慢走/快走/小跑，可调步幅/偏移/外旋）
- [x] 4.6 实现路径测试展区（直线/转弯/弧线/绕圈/S弯/折返/螺旋，含参考路径线）
- [x] 4.7 实现退场动画展区（波浪淡出/Modal 躲闪/光弧收敛）
- [x] 4.8 实现 blend mode 对比展区（固定透明度 vs soft-light）
- [x] 4.9 实现 tab icon 过渡展区（实心/空心/回家中/脉冲）
- [x] 4.10 实现设备模拟展区（high/low 切换）
- [x] 4.11 实现状态机可视化展区（状态节点 + 事件按钮）
- [x] 4.12 实现性能基准展区（10 秒自动巡游，输出 avg/p95/p99/over-budget）
- [x] 4.13 实现状态机单元测试展区（覆盖所有转换路径）
- [x] 4.14 底部 FPS 计数器

## 5. 调试面板

- [x] 5.1 创建 patrol-debug.js 面板结构（localStorage 激活）
- [x] 5.2 实现状态/位置/平台/爪印池/冷却/设备 tier 显示
- [x] 5.3 实现帧耗时监控（FPS/avg/peak/over-budget%）
- [x] 5.4 实现操作按钮（Force Out/Force Home/Pause/Reset CD/Terrain）
- [x] 5.5 实现参数滑块（idle 阈值/冷却/opacity/速度/尺寸）
- [x] 5.6 实现 CustomEvent 通信（patrol:debugParam/patrol:debugPause）

## 6. 验证

- [x] 6.1 在展台中运行状态机单元测试，确认全绿
- [x] 6.2 在展台中运行性能基准（high + low 模式），确认通过
- [x] 6.3 在手机浏览器中打开展台，验证所有展区正常显示和交互
- [x] 6.4 验证路径测试：爪印趾头始终指向行进方向
- [x] 6.5 验证 index.html 中 SVG 替换后的视觉效果（tab icon、聊天头像、桌面菜单）
