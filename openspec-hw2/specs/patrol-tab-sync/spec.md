## ADDED Requirements

### Requirement: Tab icon 根据二狗状态实时切换样式

系统应监听状态机变化，同步更新 tab icon 的 CSS class：

| 二狗状态 | Tab icon class | 视觉效果 |
|---------|---------------|---------|
| home | （无额外 class） | 实心双爪，opacity 100% |
| peek | `.patrol-out` (过渡中) | 开始变淡（100% → 50%） |
| walk | `.patrol-out` | 淡空心轮廓（opacity 30%, stroke-only） |
| pause / rest | `.patrol-out` | 保持空心轮廓 |
| converge | `.patrol-returning` | 轮廓逐渐填充（30% → 100%） |
| home（从 converge 回来） | `.patrol-pulse` | 脉冲 scale 1→1.15→1（0.3s），然后移除 class |

#### Scenario: 出场时 tab icon 变淡
- **WHEN** 状态从 home 变为 peek
- **THEN** tab icon 添加 `.patrol-out`，fill 层 opacity 渐变至 0，stroke 层 opacity 渐变至 0.3

#### Scenario: 回家时 tab icon 恢复
- **WHEN** 状态从任意巡游状态变为 home
- **THEN** tab icon 移除 `.patrol-out`，添加 `.patrol-pulse`
- **AND** 脉冲动画完成后移除 `.patrol-pulse`

---

### Requirement: Tab icon 元素定位

系统应通过 `.tab-icon-patrol` class 定位 tab icon SVG 元素。该元素已在 Phase 0 中替换 emoji 时添加。

- 元素位置通过 `getBoundingClientRect()` 获取
- 用于计算出场起点坐标

#### Scenario: 获取 tab icon 位置
- **WHEN** 巡游系统需要出场起点
- **THEN** 读取 `.tab-icon-patrol` 的 bounding rect 中心点作为起点

---

### Requirement: 聊天页 tab icon 状态（预留）

Phase 1 预留聊天态 tab icon 联动接口，但不实现具体逻辑：

- `.patrol-breathing`：等待回复时呼吸灯（预留，Phase 3 实现）
- `.patrol-bounce`：收到回复时微跳（预留，Phase 3 实现）

CSS 样式已在 Phase 0 的 patrol.css 中定义。
