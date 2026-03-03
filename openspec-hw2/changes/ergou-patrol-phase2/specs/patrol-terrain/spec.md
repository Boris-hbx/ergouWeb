## ADDED Requirements

### Requirement: TerrainScanner 扫描视口内 DOM 构建平台缓存

系统应提供 TerrainScanner 模块，扫描当前视口内可见 DOM 元素并分类为地形类型：

| CSS 选择器 | 地形类型 | 行为 |
|-----------|---------|------|
| `.quadrant`, `.task-card`, `.expense-card`, `.review-card`, `.routine-item` | platform（可行走平台） | 沿顶部边缘行走 |
| `hr`, `.divider`, `.separator`, `.quadrant-header` | ground（地面） | 沿线行走 |
| `button`, `a`, `[onclick]`, `.btn`, `input`, `select` | obstacle（障碍物） | 绕行 |
| 屏幕底部 60px | floor（地板） | 兜底行走面 |

扫描结果缓存为 `Platform` 对象数组：`{ el, type, rect, dirty }`

#### Scenario: 扫描 Todo 页面
- **WHEN** TerrainScanner.scan() 在 Todo 页面执行
- **THEN** 应识别出 `.quadrant` 元素为 platform
- **AND** 返回每个 quadrant 的 bounding rect

#### Scenario: 元素不在视口内
- **WHEN** 扫描发现元素的 bounding rect 完全在视口外
- **THEN** 不将其加入缓存

---

### Requirement: 局部按需扫描

TerrainScanner 不做全量扫描。应支持局部扫描策略：

- `scan()`: 扫描整个视口（出场时调用一次）
- `scanAhead(x, y, direction)`: 从指定位置向指定方向扫描前方 2-3 个平台
- 扫描结果缓存，避免重复查询

#### Scenario: 前方扫描
- **WHEN** 二狗在 x=100, y=300 向右行走
- **THEN** scanAhead 应返回 x>100 范围内最近的 2-3 个平台

---

### Requirement: MutationObserver 脏位标记

系统应用 MutationObserver 监听当前页面视图容器的 DOM 变化：

- 监听 childList（卡片增删）和 attributes（class/style 变化导致的尺寸变化）
- DOM 变化时标记受影响平台为 `dirty: true`
- 不在 mutation 回调中重算——只标记脏位
- 下次 PathPlanner 需要路径时检查脏位，按需重算

#### Scenario: 卡片删除
- **WHEN** 用户删除一个任务卡片
- **THEN** MutationObserver 检测到变化，对应平台标记为 dirty

#### Scenario: 当前平台消失
- **WHEN** 二狗当前行走的平台被标记为 dirty
- **AND** 重新查询后该元素已不存在或尺寸为零
- **THEN** 触发退场（fade out → home）

---

### Requirement: 地形可视化叠加层

调试模式下（patrol-debug 开启），点击 Terrain 按钮应显示/隐藏地形叠加层：

- 叠加层为 position:fixed 的 div，z-index 高于 patrol-layer
- 每个已扫描平台用半透明色块覆盖：
  - platform → 绿色 rgba(0,255,0,0.15)
  - ground → 蓝色 rgba(0,0,255,0.15)
  - obstacle → 红色 rgba(255,0,0,0.15)
- 叠加层 pointer-events:none

#### Scenario: 切换地形可视化
- **WHEN** 开发者点击 Terrain 按钮
- **THEN** 叠加层显示当前缓存的所有平台
- **WHEN** 再次点击
- **THEN** 叠加层隐藏
