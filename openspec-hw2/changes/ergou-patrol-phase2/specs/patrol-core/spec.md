## MODIFIED Requirements

### Requirement: 路径生成替换为地形感知

patrol.js 的 `generatePath` 应替换为调用 PathPlanner：

1. 出场时调用 `TerrainScanner.scan()` 获取初始平台缓存
2. handleWalk 中调用 `PathPlanner.planPath(startX, startY, platforms)` 替代 Phase 1 的随机路径
3. 路径走完需要新路径时，先调用 `TerrainScanner.scanAhead()` 更新前方平台
4. 保留 Phase 1 随机路径作为回退方案

#### Scenario: 地形感知行走
- **WHEN** 二狗出场
- **THEN** TerrainScanner.scan() 获取平台
- **AND** PathPlanner 生成沿平台边缘的路径

---

### Requirement: 地形变化容错

当二狗当前行走的平台被删除或尺寸变化时：

1. MutationObserver 标记脏位
2. 下一步 stepOnce() 前检查当前路径点对应的平台是否有效
3. 平台无效 → 立即 fadeAll + 回 home + 不冷却（允许快速重新出场）

#### Scenario: 卡片被删除
- **WHEN** 二狗正沿某卡片行走，该卡片被用户删除
- **THEN** 爪印立即 fade out
- **AND** 状态回到 home，不触发冷却

---

### Requirement: 调试面板地形叠加层集成

patrol.js 初始化时应创建地形叠加层 DOM 并传给 PatrolDebug.connect()：

- 创建一个 div 作为地形叠加层容器（初始 display:none）
- PatrolDebug.connect({ terrainOverlay: overlayEl }) 替代原来的 null
- TerrainScanner 扫描后更新叠加层内容
