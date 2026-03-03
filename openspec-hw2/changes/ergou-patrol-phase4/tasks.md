## 1. CSS 样式

- [x] 1.1 patrol.css: 新增 `.patrol-arc` 光点基础样式（8px 圆，发光，position:absolute）
- [x] 1.2 patrol.css: 新增 `@keyframes patrol-arc-fly` 光弧飞行动画（起点→弧顶→终点，0.4s）
- [x] 1.3 patrol.css: 新增 `@keyframes abao-wiggle` 微晃动画（±3°旋转，0.3s）
- [x] 1.4 patrol.css: 新增 `.abao-wiggle` class 应用微晃动画

## 2. patrol.js 收敛动画

- [x] 2.1 重写 handleConverge()：爪印 fadeAll + 光弧飞行 + logo 脉冲 + convergeDone
- [x] 2.2 实现 createArc(fromX, fromY, toX, toY)：创建光点元素并执行飞行动画（内联在 handleConverge 中）
- [x] 2.3 光弧到达后触发 logo 脉冲（复用已有 patrol-pulse class）
- [x] 2.4 脉冲完成后 transition convergeDone

## 3. 微晃集成

- [x] 3.1 abao.js: close() 中 dispatch `patrol:leaveAbao`（阿宝是 overlay 面板非 page，改为在关闭时触发）
- [x] 3.2 patrol.js: EventBridge 监听 patrol:leaveAbao，给 logo 添加 .abao-wiggle
- [x] 3.3 animationend 后移除 .abao-wiggle class

## 4. 集成

- [x] 4.1 更新缓存版本号

## 5. 验证

- [ ] 5.1 巡游中切到阿宝 tab：看到光弧飞回 + logo 脉冲（待测）
- [ ] 5.2 从阿宝 tab 切走：logo 微晃（待测）
- [ ] 5.3 非巡游时切到阿宝：无收敛动画（待测）
- [ ] 5.4 收敛动画不阻塞 tab 切换（待测）
