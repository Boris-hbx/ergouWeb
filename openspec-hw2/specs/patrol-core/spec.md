## ADDED Requirements

### Requirement: Patrol 主控模块完成初始化检测与资源创建

系统应提供 `Patrol.init()` 方法，按顺序完成以下初始化：

1. 检测 `DeviceProfile.reduceMotion` → true 则直接 return
2. 检测 `DeviceProfile.isSupported` → false 则直接 return
3. 检测移动端（viewport width <= 768px 或 `ontouchstart in window`）→ 桌面端直接 return
4. 读取 localStorage `patrol-enabled` → `'0'` 则 return（默认开启）
5. 创建 `#patrol-layer`（position:fixed, inset:0, pointer-events:none, z-index:999）
6. 若 `DeviceProfile.tier === 'high'`，给 patrol-layer 加 `.patrol-enhanced`
7. 创建 PawPool（container: patrol-layer, size: 8）
8. 创建 PatrolStateMachine（绑定 onStateChange 回调）
9. 创建 IdleDetector（idleThreshold: 8000, cooldown: 180000）→ start()
10. 绑定 EventBridge 事件

提供 `Patrol.destroy()` 方法清理一切。提供 `Patrol.enabled` 只读属性。

#### Scenario: 正常初始化（移动端高端设备）
- **WHEN** 页面加载，设备为 high tier 移动端，巡游开关开启
- **THEN** patrol-layer 应创建并带 `.patrol-enhanced` class
- **AND** IdleDetector 应开始监听

#### Scenario: 桌面端跳过
- **WHEN** 页面加载，viewport > 768px 且无 touch
- **THEN** Patrol.init() 直接 return，不创建任何 DOM

#### Scenario: reduced-motion 跳过
- **WHEN** 用户设置了 prefers-reduced-motion: reduce
- **THEN** Patrol.init() 直接 return

---

### Requirement: EventBridge 桥接用户事件到状态机

系统应监听以下事件并映射到状态机转换：

| DOM 事件 | 状态机事件 | 条件 |
|---------|-----------|------|
| click (document, capture) | 'click' | 仅 canPatrol 时 |
| scroll (scroll container) | 'scroll' | 仅 canPatrol 时 |
| 自定义 modal.open / `showModal()` 检测 | 'modal' | 仅 canPatrol 时 |
| tab 切换到阿宝 | 'abaoTab' | 仅 canPatrol 时 |
| tab 切换到非阿宝 | 立即退场 | 仅 canPatrol 时 |

EventBridge 应使用 passive listener，不阻塞用户操作。

#### Scenario: 用户点击退场
- **WHEN** 二狗正在行走，用户点击任何元素
- **THEN** 状态机收到 'click' 事件，转换到 home

#### Scenario: 滚动退场
- **WHEN** 二狗正在行走，用户滚动页面
- **THEN** 状态机收到 'scroll' 事件，转换到 home

---

### Requirement: 出场流程从 tab icon 生长

当状态机从 home 转换到 peek 时：

1. tab icon 添加 `.patrol-out` class 开始变淡
2. 在 tab icon 上方位置放置第一个半隐藏的探头爪印（peek 动画）
3. peek 动画完成（0.3s）后触发 `peekDone` 事件
4. 进入 walk 状态，从 tab icon 上方开始行走

#### Scenario: 出场序列
- **WHEN** IdleDetector 触发 idle 且冷却已结束
- **THEN** 状态机 home → peek → walk 依次转换
- **AND** 第一个爪印出现在 tab 栏上方

---

### Requirement: 行走循环产生爪印轨迹

walk 状态下，系统应按以下规则产生爪印：

- 使用 CSS animation 驱动行走路径（Phase 1 为简单直线/曲线路径）
- JS 在路径关键点调用 PawPool.step() 放置爪印
- 步态参数：步幅 40px、左右偏移 12px、外旋 ±8°
- 左右脚交替
- 爪印趾头始终指向行进方向（通过 heading 参数）
- 行走速度约 30px/s

环境染色规则：
- 高端设备：放置瞬间用 `document.elementFromPoint(x, y)` 获取脚下元素，读取其 `computed backgroundColor` 作为爪印颜色
- 低端设备：使用 CSS 变量 `var(--primary-color)` 的值

#### Scenario: 行走产生爪印
- **WHEN** 二狗进入 walk 状态
- **THEN** 爪印应左右交替出现，间距约 40px
- **AND** 每个爪印播放落地弹性动画 + 涟漪

#### Scenario: 高端设备环境染色
- **WHEN** 高端设备上爪印放置
- **THEN** 爪印颜色应取自脚下元素的 computed backgroundColor

---

### Requirement: 退场动画按类型分级

系统应根据退场原因执行不同的退场动画：

| 退场类型 | 触发 | 动画 | 冷却 |
|---------|------|------|------|
| A: 点击退场 | click 事件 | 爪印波浪淡出（从近到远，间隔 50ms），总 ~0.3s | 触发 3 分钟冷却 |
| B: Modal 退场 | modal 打开 | 所有爪印同时 fade out（0.2s） | 触发 3 分钟冷却 |
| D: 走出屏幕 | 爪印到达屏幕边缘 | overflow hidden 自然裁切 | 不冷却 |
| E: 滚动退场 | scroll 事件 | 所有爪印同时 fade out（0.2s） | 不冷却，滚动停止后重新 idle 计时 |

#### Scenario: 点击退场波浪淡出
- **WHEN** 用户点击，状态机转到 home
- **THEN** 爪印从最近到最远依次 fade（间隔 50ms）
- **AND** 触发 IdleDetector.startCooldown()

#### Scenario: 滚动退场不冷却
- **WHEN** 用户滚动，状态机转到 home
- **THEN** 爪印同时 fade out
- **AND** 不触发冷却，滚动停止后重新 idle 计时

---

### Requirement: Pause 和 Rest 子状态

行走路径走完后系统应进入子状态：

1. walk + walkEnd → pause：停止产生新爪印，现有爪印正常蒸发
2. pause + pauseTimeout（5s）→ walk：生成新路径继续行走
3. pause + restTimeout（15s）→ rest：最后两对爪印降 opacity 至 0.3、scale 至 0.95
4. rest 状态下爪印进入呼吸动画（breathe class）
5. rest + idle → walk：重新开始行走

#### Scenario: 自然停留
- **WHEN** 行走路径走完
- **THEN** 进入 pause，5s 后继续行走

#### Scenario: 趴下休息
- **WHEN** pause 持续 15s
- **THEN** 进入 rest，爪印变淡并呼吸

---

### Requirement: 路径生成（Phase 1 简化版）

Phase 1 使用简化路径（非地形感知）：

- 路径为视口内的随机直线/缓弧段
- 每段路径 5-10 步
- 路径不穿越屏幕底部 60px（tab 栏区域）
- 路径不穿越屏幕顶部 44px（状态栏/导航栏区域）
- 到达路径终点触发 walkEnd 事件
- 使用 CSSAnimator.generateWalkKeyframes() 生成路径动画

#### Scenario: 路径在安全区域内
- **WHEN** 生成新行走路径
- **THEN** 所有路径点 y 坐标在 44px ~ (viewHeight - 60px) 之间

---

### Requirement: 设置页巡游开关

settings.js 应添加"二狗巡游"开关：

- 存储 key: `patrol-enabled`
- 默认值: `'1'`（开启）
- 关闭时调用 `Patrol.destroy()`，开启时调用 `Patrol.init()`
- 开关位于设置页"个性化"分区

#### Scenario: 关闭巡游
- **WHEN** 用户关闭巡游开关
- **THEN** 二狗立即退场，后续不再出场
- **AND** localStorage 设为 `'0'`

---

### Requirement: 调试面板集成

Patrol 主控模块初始化后，应调用 `PatrolDebug.connect()` 传入内部引用：

- stateMachine
- idleDetector
- pawPool
- terrainOverlay（Phase 1 为 null）

行走过程中持续调用 `PatrolDebug.updatePosition(x, y)` 更新位置。

#### Scenario: 调试面板可查看实时状态
- **WHEN** admin 用户开启 patrol-debug
- **THEN** 面板显示实时的状态、位置、爪印池占用

---

### Requirement: visibilitychange 时完全停止

当页面不可见时（`document.hidden === true`）：

- 取消所有进行中的 CSS animation
- 停止 IdleDetector
- 清除所有爪印
- 页面恢复可见时重新 idle 计时

#### Scenario: 切到后台
- **WHEN** 用户切换到其他 app
- **THEN** 所有巡游活动停止，零 CPU 开销
