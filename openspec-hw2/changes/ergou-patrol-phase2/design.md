## Context

Phase 1 已完成核心循环（patrol.js），二狗可以出场、行走、退场，但路径是视口内的随机直线/缓弧。Phase 2 引入地形感知，让行走路径有 UI 语义。

当前 patrol.js 的路径生成在 `generatePath(startX, startY)` 函数中，返回 `[{x, y}]` 数组。Phase 2 需要替换这个函数为地形感知版本。

Next 的页面结构：
- Todo 页：4 个 `.quadrant` 容器，每个包含 `.quadrant-header` 和多个任务卡片
- 例行审视页：`.review-card` 列表
- 记账页：`.expense-card` 列表
- 学习页：学习卡片
- 生活页：各类功能卡片

## Goals / Non-Goals

**Goals:**
- 扫描视口内 DOM 构建平台缓存
- 沿卡片顶部边缘行走
- 绕开按钮/可点击区域
- MutationObserver 脏位标记 + 按需重算
- 地形叠加层调试可视化
- 卡片消失时容错退场

**Non-Goals:**
- 全量 DOM 扫描（只扫描视口内 + 前方局部）
- 复杂物理引擎（跳跃只是简单抛物线弧）
- 不同页面的定制行走策略（统一的平台识别规则）
- 页面交互响应（Phase 3）

## Decisions

### D1: patrol-terrain.js 独立文件

**决定**: 新建 `patrol-terrain.js` 包含 TerrainScanner 和 PathPlanner 两个模块。

**理由**: 地形扫描和路径规划都是独立能力，但彼此紧密关联。放在同一文件减少 HTTP 请求，同时与 patrol.js（主控）和 patrol-utils.js（通用工具）保持分离。

### D2: 平台识别用 CSS 选择器白名单

**决定**: 用一组 CSS 选择器白名单匹配 DOM 元素到地形类型，而非通用启发式（如"所有 block 元素"）。

**备选方案**:
- A) 通用启发式（检测所有 block 元素的尺寸/位置）→ 误判太多，性能差
- B) 自定义 data-terrain 属性 → 侵入式，需要修改所有模板

**理由**: Next 的 UI 组件有限且可枚举。白名单精确可控，新页面元素只需加一行选择器。

### D3: 扫描时机——出场一次 + 路径切换时局部

**决定**:
- 出场（home→peek）时执行一次全视口 `scan()`
- 每次需要新路径（walkEnd→pause→walk）时执行 `scanAhead()` 更新前方平台
- 不在 RAF 中扫描

**理由**: SPEC-057 明确要求"不做全量扫描"、"不在动画帧内查询"。出场扫描 + 按需局部更新满足需求。

### D4: MutationObserver 只监听视图容器

**决定**: 对当前活跃的视图容器（如 `#todo-view`、`#review-view`）添加 MutationObserver，不监听整个 document.body。

**理由**: 减少 mutation 回调次数。切换页面时更新 observer 的目标。

### D5: 跳跃路径用简单抛物线

**决定**: 平台间跳跃用 3-4 个路径点模拟抛物线弧：起点→最高点（两平台中间上方）→终点。

**备选方案**:
- A) CSS offset-path 贝塞尔曲线 → 需要额外 DOM 元素和 getComputedStyle
- B) 更多路径点的精确抛物线 → 过度精确，不值得

**理由**: 3-4 个点的弧线在实际视觉中已经足够自然。步频在跳跃期间加快即可模拟跳跃感。

### D6: 文件结构

| 文件 | 职责 |
|------|------|
| `assets/js/patrol-terrain.js` | **新增** — TerrainScanner + PathPlanner |
| `assets/js/patrol.js` | **修改** — 路径生成替换、地形容错、调试叠加层 |
| `index.html` | **修改** — 引入 patrol-terrain.js |

## Risks / Trade-offs

- **[CSS 选择器白名单可能遗漏新组件]** → 可接受，新增组件时加一行选择器即可。调试面板的地形可视化能快速发现遗漏。
- **[getBoundingClientRect 性能]** → 每次扫描最多查询 20-30 个元素，<1ms。不在 RAF 中调用。
- **[MutationObserver 回调频率]** → 只标记脏位不重算，单次回调 <0.1ms。
- **[跳跃弧线不够物理真实]** → Phase 2 目标是"走得有道理"，不是物理模拟。简单弧线足够。

## Migration Plan

1. patrol-terrain.js 是全新文件，不影响现有功能
2. patrol.js 的修改向后兼容——如果 TerrainScanner 未加载，回退到 Phase 1 随机路径
3. 回滚方案：从 index.html 移除 patrol-terrain.js 引用即可
