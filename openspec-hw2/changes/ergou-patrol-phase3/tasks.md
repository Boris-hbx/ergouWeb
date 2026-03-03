## 1. 事件派发

- [x] 1.1 tasks.js: saveProgress() 成功且 completed=true 时 dispatch `patrol:taskComplete` 事件（detail: {itemId, quadrant, cardEl}）
- [x] 1.2 tasks.js: patrol:taskComplete 事件中判断 isLastInQuadrant（同象限同 tab 无其他未完成项）
- [x] 1.3 app.js: activateMobileNav() 调用 jellyMobileNav.moveTo() 前 dispatch `patrol:jellyMove` 事件（detail: {fromX, toX, direction}）
- [x] 1.4 abao.js: 发送消息时 dispatch `patrol:chatStatus` {status:'thinking'}，收到回复完成时 dispatch {status:'idle'}

## 2. CSS 样式

- [x] 2.1 patrol.css: 新增 `.patrol-paw.stamped` 样式（比普通爪印大 20%，opacity 0.7，box-shadow 微光）
- [x] 2.2 patrol.css: 新增 `@keyframes abao-breathing` 呼吸灯动画（opacity 0.5↔1.0，周期 2s）
- [x] 2.3 patrol.css: 新增 `.abao-thinking` class 应用呼吸灯动画到阿宝 logo

## 3. patrol.js 交互逻辑

- [x] 3.1 实现 stampPaw(x, y, color, className, duration)：创建独立爪印元素，duration 后自动淡出移除
- [x] 3.2 实现 handleTaskComplete(detail)：在卡片位置踩已阅爪印
- [x] 3.3 实现 handleCheckmark(quadrantEl)：在象限区域踩 ✓ 图案（3 个爪印，100ms 间隔）
- [x] 3.4 实现 handleJellyFollow(detail)：生成 2-3 步短路径朝 pill 方向快跑
- [x] 3.5 实现 handleChatStatus(detail)：给阿宝 logo 添加/移除 .abao-thinking class

## 4. EventBridge 集成

- [x] 4.1 在 setupEventBridge() 中添加 patrol:taskComplete 监听（门控 canPatrol）
- [x] 4.2 在 setupEventBridge() 中添加 patrol:jellyMove 监听（门控 canPatrol，排除阿宝 tab）
- [x] 4.3 在 setupEventBridge() 中添加 patrol:chatStatus 监听（不门控巡游状态）
- [x] 4.4 在 teardownEventBridge() 中确保所有新监听被清理（通过 _eventCleanup 统一管理）

## 5. index.html 集成

- [x] 5.1 更新缓存版本号

## 6. 验证

- [ ] 6.1 Todo 页：巡游中完成任务，卡片上出现已阅爪印（待测）
- [ ] 6.2 Todo 页：完成象限最后一个任务，出现 ✓ 爪印图案（待测）
- [ ] 6.3 非巡游时完成任务，无任何爪印效果（待测）
- [ ] 6.4 底部 nav 切换时，二狗朝 jelly pill 方向跑动（待测）
- [ ] 6.5 聊天发送消息时阿宝 logo 呼吸灯，回复完成后停止（待测）
- [ ] 6.6 交互不阻塞用户操作（pointer-events:none）（待测）
- [ ] 6.7 交互不中断正常巡游行走（待测）
