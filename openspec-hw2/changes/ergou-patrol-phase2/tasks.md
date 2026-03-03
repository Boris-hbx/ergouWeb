## 1. TerrainScanner 模块

- [x] 1.1 创建 patrol-terrain.js 文件骨架（IIFE，暴露 TerrainScanner 和 PathPlanner）
- [x] 1.2 定义平台类型常量和 CSS 选择器白名单映射表
- [x] 1.3 实现 scan()：遍历白名单选择器，获取视口内元素的 getBoundingClientRect，构建平台缓存数组
- [x] 1.4 实现 scanAhead(x, y, direction)：从指定位置向指定方向查找前方 2-3 个最近平台
- [x] 1.5 实现 getPlatformAt(x, y)：查找指定坐标所在的平台（用于脏位检测）
- [x] 1.6 实现 MutationObserver 脏位标记（监听当前视图容器的 childList + attributes 变化）
- [x] 1.7 实现 setViewContainer(el)：切换监听目标（页面切换时调用）
- [x] 1.8 实现 invalidate()：标记所有缓存为脏
- [x] 1.9 实现 destroy()：清理 observer 和缓存

## 2. PathPlanner 模块

- [x] 2.1 实现 planPath(startX, startY, platforms)：基于平台缓存生成路径点数组
- [x] 2.2 实现沿平台顶部边缘行走路径（从一端到另一端，5-10 步）
- [x] 2.3 实现障碍物检测：检查路径上是否有 obstacle 类型平台
- [x] 2.4 实现障碍物绕行：在障碍物上方/下方生成绕行弧线
- [x] 2.5 实现平台间跳跃路径（3-4 步抛物线弧）
- [x] 2.6 实现下一平台选择策略（优先水平接近、其次下方、距离 >200px 视为不可达）
- [x] 2.7 实现回退到随机路径（无可用平台时使用 Phase 1 逻辑）

## 3. patrol.js 集成

- [x] 3.1 在 Patrol.init() 中初始化 TerrainScanner（设置视图容器、首次扫描）
- [x] 3.2 在 handlePeek() 中调用 TerrainScanner.scan() 获取初始平台
- [x] 3.3 替换 generatePath() 为调用 PathPlanner.planPath()（保留原函数作为回退）
- [x] 3.4 在需要新路径时（pause→walk）调用 scanAhead() 更新前方平台
- [x] 3.5 在 stepOnce() 中增加脏位检查：当前路径点的平台无效则退场
- [x] 3.6 在 Patrol.destroy() 中调用 TerrainScanner.destroy()
- [x] 3.7 监听页面切换（switchPage），调用 TerrainScanner.setViewContainer() 和 invalidate()

## 4. 地形可视化调试

- [x] 4.1 在 patrol.js init() 中创建地形叠加层 DOM（position:fixed, display:none, pointer-events:none）
- [x] 4.2 传入 PatrolDebug.connect({ terrainOverlay: overlayEl })
- [x] 4.3 TerrainScanner 扫描完成后更新叠加层内容（绿色平台、红色障碍、蓝色地面）
- [x] 4.4 验证调试面板 Terrain 按钮切换叠加层显示/隐藏

## 5. index.html 集成

- [x] 5.1 引入 patrol-terrain.js（在 patrol-utils.js 之后、patrol.js 之前）
- [x] 5.2 更新缓存版本号

## 6. 验证

- [x] 6.1 Todo 页验证：爪印沿 quadrant 顶部边缘行走
- [x] 6.2 例行页验证：爪印沿 review-card 边缘行走
- [x] 6.3 记账页验证：爪印沿 expense-card 边缘行走
- [x] 6.4 空白页回退：无平台时使用随机路径
- [ ] 6.5 卡片删除容错：删除卡片时二狗优雅退场（待测）
- [ ] 6.6 地形可视化：Terrain 按钮正确显示/隐藏叠加层（待测）
- [x] 6.7 性能验证：扫描不在 RAF 中，不导致帧率下降
