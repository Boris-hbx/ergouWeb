## Why

Phase 1 实现了核心循环但二狗走的是随机路径——不感知页面结构，看起来在"空中游荡"而非"巡视空间"。Phase 2 引入地形感知，让二狗沿卡片边缘行走、绕开按钮，走出来的路径与 UI 布局有语义关联。这是实现 SPEC-057 "二狗把 UI 当作物理空间" 设计理念的关键一步。

## What Changes

- 新增 TerrainScanner 模块：局部扫描可见 DOM 构建平台缓存（卡片顶部 = 可行走平台，分割线 = 地面，按钮 = 障碍物）
- 新增 PathPlanner 模块：基于平台缓存生成沿边缘行走的路径（替代 Phase 1 的随机路径）
- 用 MutationObserver 监听可见区域 DOM 变化，标记脏位，按需重算
- 修改 patrol.js 的 `generatePath` 替换为地形感知路径
- 调试面板增加地形可视化叠加层（Terrain 按钮已预留）
- 卡片消失时容错：爪印 fade out，回到 home 状态

## Capabilities

### New Capabilities
- `patrol-terrain`: 地形扫描与平台缓存——DOM 元素到地形类型的映射、局部按需扫描、脏位标记与重算
- `patrol-pathplan`: 地形感知路径规划——沿平台边缘生成行走路径、障碍物绕行、跳跃与下落

### Modified Capabilities
- `patrol-core`: 替换随机路径为地形感知路径、增加地形相关容错（卡片消失时退场）

## Impact

- **新增文件**: `frontend/assets/js/patrol-terrain.js`（TerrainScanner + PathPlanner）
- **修改文件**: `frontend/assets/js/patrol.js`（路径生成替换、地形容错）、`frontend/assets/js/patrol-debug.js`（地形可视化）、`frontend/index.html`（引入新 JS）
- **无后端变更**
- **无数据库变更**
