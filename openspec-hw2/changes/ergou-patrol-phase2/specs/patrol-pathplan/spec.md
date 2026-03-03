## ADDED Requirements

### Requirement: PathPlanner 基于平台缓存生成行走路径

系统应提供 PathPlanner 模块，根据 TerrainScanner 的平台缓存生成有语义的行走路径：

- `planPath(startX, startY, platforms)`: 输入起点和平台列表，输出路径点数组 `[{x, y}]`
- 路径优先沿平台顶部边缘行走
- 每段路径 5-10 步
- 无可用平台时回退到随机路径

#### Scenario: 沿卡片顶部行走
- **WHEN** 起点在一个 platform 类型平台的上方
- **THEN** 生成沿该平台顶部边缘从一端走到另一端的路径

#### Scenario: 无平台回退
- **WHEN** platforms 为空数组
- **THEN** 回退到随机路径（Phase 1 逻辑）

---

### Requirement: 障碍物绕行

PathPlanner 生成路径时应避开 obstacle 类型元素：

- 检测路径上是否有 obstacle
- 如有障碍物，在其上方或下方生成绕行弧线
- 绕行距离为障碍物高度 + 8px margin

#### Scenario: 绕开按钮
- **WHEN** 路径上有一个 button（obstacle）
- **THEN** 路径点应绕过该 button
- **AND** 绕行后回到原路线

---

### Requirement: 平台间连接

当二狗走到当前平台末端，PathPlanner 应选择下一个平台：

- 从 TerrainScanner 获取前方 2-3 个可达平台
- 选择策略：优先选择 y 坐标接近的平台（水平移动），其次选择下方平台（向下走）
- 平台间生成跳跃路径：3-4 步的抛物线弧
- 两个平台距离 > 200px 时视为不可达，回退到随机路径

#### Scenario: 跳到下一个平台
- **WHEN** 二狗到达平台右端
- **AND** 右侧 200px 内有另一个平台
- **THEN** 生成跳跃弧线路径连接两个平台

#### Scenario: 无可达平台
- **WHEN** 二狗到达平台末端
- **AND** 200px 内没有其他平台
- **THEN** 回退到随机路径或触发 walkEnd

---

### Requirement: 路径点包含地形元数据

PathPlanner 输出的路径点应包含元数据：

```
{ x, y, platformId?, onEdge? }
```

- `platformId`: 当前路径点所在平台的标识（用于脏位检测）
- `onEdge`: 是否在平台边缘（用于步态调整：边缘步态更小心）
