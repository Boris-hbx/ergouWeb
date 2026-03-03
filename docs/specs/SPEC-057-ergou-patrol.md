# SPEC-057: 二狗巡游系统
> 起草日期: 2026-03-02
> 状态: 实施中
> 红蓝军演练: 2026-03-02 完成，裁决已合并

## 概述

二狗（阿宝）是 Next 的 AI 助手，具备多模态交互能力（文字、图片、语音、拍照识别），能通过对话创建、编辑、管理整个应用，并做统计分析。

本 spec 定义二狗在非聊天页面的**巡游态**——AI 助手在 UI 空间中的具象化存在。

---

## 设计理念

### 1. 二狗是这个空间的主人

Next 的所有功能——任务、记账、例行、学习——都是二狗能力的不同面。屏幕上走动的爪印是 AI 助手在 UI 空间中的具象化：聊天框里它说话，页面上它巡视。是同一只二狗。

### 2. 两种形态，一个连续体

| | 对话态 | 巡游态 |
|---|---|---|
| 在哪 | 阿宝聊天面板 | 其他所有页面 |
| 形态 | 聊天界面、logo | 爪印轨迹、极简轮廓 |
| 能力 | 多模态交互（文字、图片、语音、拍照识别）、执行指令、统计分析 | 视觉提示、环境感知 |
| 感知 | "我在跟二狗协作" | "二狗在帮我看着" |

收敛动画连接两态——爪印飞回 logo 是巡游转对话，logo 释放爪印是对话转巡游。

### 3. 巡游是履职，不是表演

行为有语义：沿卡片走是在检查、逾期任务旁停留是发现问题、空白区域趴下是一切正常。看不看得懂不重要，看懂了会觉得合理。

### 4. 被发现，而非被展示

用户第三次用才注意到的存在感才是对的。尺寸小、透明度低、速度慢于阅读速度。住在注意力的边缘。

### 5. 你动它让（分级响应）

`pointer-events: none` 永远不挡操作。用户操作时分级退让：
- **滚动浏览**：二狗不消失，快速 fade out 退场，滚动停止后重新 idle 计时
- **点击元素 / 打开 modal**：立即消失
- **输入态**：完全退场

> 演练修正：原方案"touch 即隐"导致二狗几乎永远不可见。分级响应让二狗在浏览态有存在感，操作态仍零干扰。

### 6. 视觉必须一流

每一帧流畅、优美、精确。宁可砍功能也不要掉帧。做不到 60fps 就不做。线条粗糙、动画生硬、抖动卡顿，任何一项出现都不如不做。

### 7. 性能分级

核心动画只用 `transform` + `opacity`（纯 GPU 合成）。高级视觉效果按设备能力分级：

| 能力 | 低端设备 | 高端设备 |
|------|---------|---------|
| 爪印移动 | transform + opacity | transform + opacity |
| 内容避让 | 固定半透明色 | `mix-blend-mode: soft-light` |
| 涟漪 | CSS animation | CSS animation |
| 环境染色 | 固定主题色 | computed style 采样 |

设备判定：`navigator.deviceMemory >= 4` 或 `navigator.hardwareConcurrency >= 4` 视为高端。

行走动画优先用 CSS animation / `offset-path` 驱动，JS 只在状态切换节点介入。目标 <1ms/帧。

### 8. 最小化表达

Phase 1-3 严格只有爪印，没有身体轮廓。用户脑中补全的那只狗，比画出来的任何一只都好。

需要身体才能表达的动作（哈欠、伸懒腰、摇尾巴、挂住边缘摇晃）全部归入 Phase 5+，作为可选增强。

---

## 二狗状态机

```
  ┌──────┐  idle 8s   ┌──────┐   探出完成   ┌──────┐
  │ 隐藏  │ ────────→ │ 探头  │ ──────────→ │ 行走  │
  │(home) │  冷却期后  │(peek) │             │(walk) │
  └──┬────┘           └──┬────┘             └──┬────┘
     ↑                   │                     │
     │   点击/modal      │  点击/modal         │  点击/modal/滚动
     │←──────────────────┘                     │
     │←────────────────────────────────────────┘
     │                                    点击阿宝tab
     │                                         │
     │        收敛完成                          ↓
     │←─────────────────────────────────  ┌──────────┐
                                          │ 收敛回家  │
                                          │(converge)│
                                          └──────────┘
```

行走中可进入子状态：停留(pause)、趴下(rest)。

> 演练修正：
> - idle 阈值从 5s 调为 8s（可调参数）
> - 触发机制从"70% 概率"改为"冷却时间"：退场后至少 3 分钟冷却，冷却后下一次 idle 必定出场
> - 去掉 nudge（扒拉）子状态——需要身体轮廓，移到 Phase 5+

---

## 前置工作：SVG 资产

Phase 1 开始前，需要设计以下 SVG 资产：

### 单爪 SVG（爪印用）
- viewBox: 16×16
- 结构：4 个趾垫椭圆 + 1 个掌垫椭圆
- 微妙不对称（让左右可辨）
- 右脚 = 左脚 `scaleX(-1)`，不需要两套
- 文件 <500 bytes

### Tab icon SVG（替换 emoji 🐾）
- 双爪组合图案（与现有 logo 一致）
- 支持 `stroke` / `fill` 独立控制
- 支持 `opacity` 过渡（实心 ↔ 空心轮廓）
- 替换 index.html 中 `mobile-nav-abao`、`abao-avatar`、`abao-mini-avatar` 三处 emoji

---

## 爪印设计

### 单爪结构

每一步是**一个**爪印（不是 logo 的双爪）。左脚和右脚互为镜像：

```
  左脚          右脚
  · ·            · ·
 ·   ·          ·   ·
  (__) ↗外旋    (__) ↖外旋
```

### 步态

交替单脚落地，左右错开中心线：

```
行走方向 →

  L         L         L
      R         R         R
```

| 状态 | 步幅间距 | 左右偏移 | 外旋角度 |
|------|---------|---------|---------|
| 慢走 | 小（密） | 大（明显左右摇摆） | ±10° |
| 快走 | 中 | 中 | ±8° |
| 小跑 | 大（疏） | 小（趋近直线） | ±5° |

转弯时内侧脚步密、外侧脚步疏。

### 爪印视觉属性

```css
/* 基础样式（所有设备） */
.patrol-paw {
    opacity: 0.5;
    mask-image: radial-gradient(circle, rgba(0,0,0,0.6) 30%, rgba(0,0,0,0.2) 100%);
    pointer-events: none;
}

/* 高端设备增强 */
.patrol-enhanced .patrol-paw {
    mix-blend-mode: soft-light;
}
```

- **环境染色**：高端设备取脚下元素 computed style 主色调（放置瞬间查询一次，缓存到元素 style.color）；低端设备用固定主题色
- **内容避让**：高端设备 `mix-blend-mode: soft-light` 自动避让；低端设备靠固定半透明度

### 爪印生命周期

```
踩下:   scale 0 → 1.1 → 1 (弹性落地, 0.15s)
        opacity 0 → 0.5
存活:   opacity 0.5 保持, 颜色 = 环境色 × 30%
消亡:   opacity 0.5 → 0 (1.5s ease-in, 像水渍蒸发)
        scale 1 → 0.95 (微缩)
```

屏幕上最多同时 8 个爪印（对象池 FIFO 复用）。

### 踩水纹涟漪

每个爪印落地时扩散涟漪（每步一个，交替左右）：

```css
.patrol-paw.landing::before {
    animation: ripple 0.6s ease-out forwards;
}
.patrol-paw.landing::after {
    animation: ripple 0.6s ease-out 0.1s forwards;
}
@keyframes ripple {
    0%   { transform: scale(0.3); opacity: 0.4; }
    100% { transform: scale(2.5); opacity: 0; }
}
```

涟漪颜色继承环境色（`currentColor`）。

---

## Tab 栏联动

二狗出去巡游时，底部 tab 栏的阿宝 icon 同步变化：

| 二狗状态 | tab icon 表现 |
|---------|--------------|
| 在家（隐藏/对话态） | 实心双爪，opacity 100% |
| 探头中 | 开始变淡（100% → 50%，同步探头动画） |
| 巡游中 | 淡空心轮廓（opacity 30%，stroke-only） |
| 收敛回家中 | 轮廓逐渐填充（30% → 100%，同步飞回动画） |
| 回家落定 | 脉冲恢复实心（scale 1→1.15→1, 0.3s） |

---

## 存在过渡

### 出场：从 tab icon "生长"出来

```
tab icon 微泛光晕 (0.5s)
  → icon 边缘探出半个爪印 (0.3s)
  → 第一个完整爪印落在 tab 栏上方 (弹性落地 + 涟漪)
  → 第二个爪印 (另一只脚)
  → 开始行走
```

### 退场 A：用户点击元素

```
二狗本体: opacity → 0 + scale 1 → 0.8 (0.15s ease-out)
爪印轨迹: 从最远到最近依次 fade (间隔 50ms, 波浪式消失)
总时长: ~0.3s
```

### 退场 B：打开 Modal

```
二狗向 modal 反方向微移 8px (躲闪)
然后 opacity fade out (0.2s)
爪印同时 fade (modal 遮住了, 不需要波浪细节)
```

### 退场 C：切换到阿宝 tab（回家收敛）

```
所有爪印就地 fade out (0.15s)
同时从二狗当前位置发射一个光点
  → 光点沿单条贝塞尔曲线飞向 tab 栏阿宝 logo
  → 到达时 scale → 0, opacity → 0
  → logo 脉冲 scale 1 → 1.15 → 1 (0.3s)
```

> 演练修正：原方案 9 条独立曲线同步飞行，复杂度过高。简化为爪印就地消散 + 单条光弧飞回，更干净也更易实现。

### 退场 D：走出屏幕边缘

```
身体被边缘自然裁切 (overflow hidden)
不需要 fade, 物理裁切即是最自然的消失
```

### 退场 E：用户滚动

```
二狗快速 fade out (0.2s)
爪印同时 fade (0.2s)
滚动停止后重新进入 idle 计时
```

> 演练修正：原方案滚动时二狗跟随移动，需要每帧读 scrollTop 违反性能原则。改为直接退场。

### 状态间过渡

| 从 → 到 | 过渡 |
|---------|------|
| 行走 → 停下 | 减速 ease-out，最后两步间距缩短 |
| 停下 → 趴下 | 爪印 opacity 从 0.5 降到 0.3（暗示静止），scale 微缩 0.95 |
| 趴下 → 呼吸 | opacity 0.3 → 0.35 → 0.3 循环 (3s, ease-in-out) |

> 演练修正：去掉了受惊弹跳、滑行等需要身体轮廓的过渡。纯爪印的表达：速度变化、密度变化、透明度变化。

---

## 地形感知系统

二狗把 UI 当作物理空间：

| UI 元素 | 地形含义 |
|--------|---------|
| 卡片顶部边缘 | 可行走平台（墙头） |
| 分割线 | 地面 / 休息点 |
| 屏幕底部 | 地板 |
| 空白区域 | 可穿越空间 |
| 按钮 / 可点击区域 | 障碍物（绕行） |

### 地形扫描

不做全量扫描。只维护二狗**当前所在平台**和**前方 2-3 个平台**的 rect：
- 出场时扫描一次初始路径
- 用 MutationObserver 监听可见区域 DOM 变化，标记脏位
- 二狗需要下一步路径时才重算局部，不在动画帧内查询
- 卡片消失时：爪印直接 fade out，回到 home 状态，冷却后重新出场

> 演练修正：原方案全量扫描 + 实时重建过重。局部按需扫描大幅降低复杂度，且卡片消失时的容错更简单可靠。

---

## 全操作 × 行为映射

### A. 导航

| 操作 | 二狗隐藏时 | 二狗巡游时 |
|------|----------|----------|
| 切换普通 tab | 无反应 | 爪印轨迹向下坠落淡出，回 home |
| 切换到阿宝 tab | 无反应 | **收敛回家动画** |
| 从阿宝 tab 切走 | logo 微晃（被叫醒） | — |

### B. 滚动

| 操作 | 行为 |
|------|------|
| 任何滚动 | 退场 E（快速 fade out），滚动停止后重新 idle 计时 |

> 演练修正：原方案有慢速跟随/快速挂住/回弹翻身等复杂行为，全部需要身体轮廓且违反性能原则。统一简化为滚动即退场。

### C. 弹窗 / Modal

| 操作 | 行为 |
|------|------|
| 打开任何 modal | 退场 B |
| 关闭 modal | 不立刻出现，重新 idle 计时 |
| Modal 内操作 | 完全不出现 |
| Toast 弹出 | 不反应（Phase 1-3 只有爪印，无法表达"耳朵微抖"） |

### D. Todo 页面

| 操作 | 行为 |
|------|------|
| 勾选完成任务 | 巡游中且附近：在卡片上踩一个爪印（已阅标记） |
| 完成今日最后一个任务 | 巡游中：踩出一圈爪印组成 ✓ 渐隐。不在场则不触发 |
| 删除/拖拽/折叠 | 巡游中则 fade out 回 home |

> 演练修正：
> - "最后任务完成"不再主动出场，仅在恰好巡游中才响应
> - 删除/拖拽/折叠统一为退场，不做坠落/挤出等复杂动作

### E. 记账页面

| 操作 | 行为 |
|------|------|
| 切换日/周/月 tab | 巡游中且附近：跟 jelly pill 同方向移动一段 |
| 翻页 | 退场，新内容渲染后重新 idle 计时 |
| 保存记账 | 巡游中：新卡片旁踩一个已阅爪印，2s 后淡出 |

### F. 例行审视页面

| 操作 | 行为 |
|------|------|
| 切换频率筛选 | 同记账——跟 pill 移动 |
| 完成一项例行 | 巡游中：踩已阅爪印 |

### G. 阿宝聊天页

| 操作 | 行为 |
|------|------|
| 在聊天页 | 不巡游（已在对话态） |
| 发送消息 | tab icon 爪印脉冲一次 |
| 等待回复 | tab icon 呼吸灯 |
| 收到回复 | tab icon 微跳 |

### H. 系统事件（Phase 5+）

以下行为全部归入 Phase 5+，Phase 1-3 不实现：

| 操作 | 行为（远期） |
|------|------------|
| 下拉刷新 | 爪印从顶部交替踩下替代 spinner |
| 网络断开/恢复 | 爪印状态变化 |
| 长时间未用 | 出场速度更慢 |
| 深夜使用 | 步频降低 |
| 摇晃手机 | 爪印偏移 |

---

## 绝对红线

| 规则 | 原因 |
|------|------|
| `pointer-events: none` | 永远不挡可点击元素 |
| 操作态即退 | 点击/modal/输入时零干扰 |
| 冷却时间机制 | 退场后 3 分钟冷却，避免反复出没 |
| Modal / 输入框激活时不出现 | 工作态不打扰 |
| 无声音 | 纯视觉 |
| 核心动画只用 transform + opacity | 不触发 layout/paint |
| blend mode 按设备分级 | 低端设备不开 mix-blend-mode |
| 行走用 CSS animation 驱动 | JS 只管状态切换，不常驻 RAF |
| 爪印对象池（最多 8 个） | 不创建/销毁 DOM |
| tab 不可见时完全停止 | visibilitychange 一刀切 |
| 路径按需计算 | 只算当前 + 前方 2-3 个平台 |
| `prefers-reduced-motion: reduce` 时不初始化 | 无障碍 |
| 可在设置中关闭 | 不喜欢就永久关掉 |
| Phase 1-3 严格只有爪印 | 身体轮廓/拟人动作归 Phase 5+ |
| 仅移动端（Phase 1） | 桌面端暂不启用 |

---

## 时间参数表

| 动作 | 时长 | 曲线 |
|------|------|------|
| idle 阈值 | 8s（可调） | — |
| 冷却时间 | 3min | — |
| 出场呼吸光晕 | 0.5s | ease-in-out |
| 探头 → 第一步 | 0.3s | ease-out |
| 点击退场（本体） | 0.15s | ease-out |
| 点击退场（爪印波浪） | 0.3s | 各 50ms 间隔 |
| 滚动退场 | 0.2s | ease-out |
| modal 退场 | 0.2s | ease-out |
| 收敛光弧飞行 | 0.4s | cubic-bezier(0.2, 0, 0, 1) |
| logo 脉冲 | 0.3s | ease-out |
| 爪印落地弹性 | 0.15s | spring-like |
| 涟漪扩散 | 0.6s | ease-out |
| 爪印蒸发 | 1.5s | ease-in |
| 趴下（爪印变淡） | 0.5s | ease-in-out |
| 呼吸循环 | 3s | ease-in-out |

---

## 技术架构

```
ergou-patrol.js (新文件)
├── DeviceProfile      — 检测设备能力，决定增强等级
├── TerrainScanner     — 局部扫描 DOM 构建平台缓存（当前 + 前方 2-3 个）
├── PawPool            — 8 个爪印 DOM 元素对象池
├── PathPlanner        — 基于平台缓存生成 CSS offset-path / keyframes
├── StateMachine       — home/peek/walk/pause/rest/converge 状态管理
└── EventBridge        — 监听 click/scroll/modal/page 事件触发状态切换

ergou-patrol.css (新文件)
├── .patrol-paw             — 爪印基础样式 + mask
├── .patrol-enhanced .patrol-paw — 高端设备 blend mode
├── .patrol-paw.landing     — 涟漪 @keyframes
├── .patrol-paw.left/.right — 左右脚 scaleX 镜像
├── .patrol-paw.evaporate   — 蒸发 @keyframes
├── @keyframes ripple
├── @keyframes breathe
└── @keyframes evaporate

改动现有文件:
├── index.html    — 引入新 JS/CSS，阿宝 emoji 换 SVG
├── mobile.css    — 阿宝 tab icon stroke/fill 过渡
├── abao.css      — logo SVG 脉冲/呼吸灯样式
└── settings.js   — 添加"二狗巡游"开关
```

---

## 验收标准

### 性能验收
- 在 3 台设备（iPhone、中端安卓、低端安卓）运行 10 分钟
- Performance 面板无可见帧率下降
- 二狗巡游期间主线程占用 <1ms/帧

### 感知验收
- 3 名测试用户，不告知有此功能，正常使用 5 分钟
- 至少 1 人自然发现 → 存在感合格
- 3 人都没发现 → 存在感过弱，调大尺寸/透明度/idle 阈值
- 有人主动问"怎么关" → 太烦了，调小或下线

---

## 实施分期

每个 Phase 独立可交付，Phase 之间必须有 go/no-go 评审。

### Phase 0: 基础设施

在写任何巡游逻辑之前，先把地基打好。Phase 0 产出的模块独立于二狗业务逻辑，其他功能也能复用。

#### 0a. SVG 资产设计

| 资产 | 规格 | 用途 |
|------|------|------|
| 单爪 SVG（左脚） | viewBox 16×16，4 趾垫椭圆 + 1 掌垫椭圆，<500 bytes | 爪印主体，右脚 `scaleX(-1)` |
| 双爪 SVG（tab icon） | 与现有 logo 一致的双爪组合，支持 stroke/fill 独立控制 | 替换 emoji 🐾，支持实心↔空心过渡 |

#### 0b. 通用工具模块（`patrol-utils.js`）

**ObjectPool** — 通用 DOM 对象池
```
ObjectPool.create({ size: 8, factory: fn, reset: fn })
  .acquire()   → 返回一个空闲元素
  .release(el) → 回收元素，调用 reset 清理状态
  .activeCount → 当前占用数
```
爪印池、涟漪池、未来粒子效果都用同一个 Pool。

**DeviceProfile** — 设备能力检测
```
DeviceProfile.tier   → 'low' | 'high'
DeviceProfile.canBlend → boolean (是否开 mix-blend-mode)
DeviceProfile.reduceMotion → boolean (prefers-reduced-motion)
```
判定规则：`deviceMemory >= 4` 或 `hardwareConcurrency >= 4` → high。`prefers-reduced-motion: reduce` → 整个巡游系统不初始化。

**CSSAnimator** — 运行时 CSS 动画生成器
```
CSSAnimator.inject(name, keyframes) → 注入 @keyframes 到 stylesheet
CSSAnimator.remove(name)            → 用完后清理
CSSAnimator.offsetPath(points)      → 生成 offset-path 字符串
```
行走路径在运行时根据地形生成，不能硬编码。这个工具让 JS 生成路径、CSS 执行动画。

**IdleDetector** — 用户活动监听器
```
IdleDetector.create({
    idleThreshold: 8000,     // ms
    cooldown: 180000,        // ms (3 分钟)
    onIdle: fn,              // idle 触发回调
    onActive: fn,            // 用户恢复活动回调
    activeEvents: ['click', 'scroll', 'keydown', 'touchstart']
})
  .start() / .stop() / .destroy()
  .isIdle → boolean
  .cooldownRemaining → ms
```

#### 0c. 动画展台（`patrol-showcase.html`）

独立 HTML 页面，不依赖 Next 主应用。所有动画在这里开发、调参、验证后，再集成进主应用。

包含以下展区：

| 展区 | 内容 |
|------|------|
| **爪印渲染** | 单爪 SVG 在白底/文字/彩色卡片上的效果，左脚/右脚对比 |
| **落地动画** | 弹性 scale + 涟漪扩散，可重播，可调参数 |
| **蒸发动画** | opacity + scale 衰减，可调时长 |
| **步态预览** | 慢走/快走/小跑三种步态的爪印序列，可调间距/偏移/旋转 |
| **退场动画** | 波浪淡出 / modal 躲闪 / 光弧收敛，可重播 |
| **blend mode 对比** | 同一个爪印在不同背景上的 soft-light 效果 vs 固定透明度 |
| **Tab icon 过渡** | 实心 → 变淡 → 空心 → 填回，可手动步进 |
| **设备模拟** | 切换 low/high 模式，查看降级效果 |

页面底部显示实时 FPS。后续每个 Phase 的新动画都加入展台，形成活的动画文档。

#### 0d. 调试面板（`patrol-debug.js`）

集成进 Next 主应用的开发工具，通过 `localStorage.setItem('patrol-debug', '1')` 开启。

显示内容：
- 实时状态（home/peek/walk/pause/rest）
- 二狗位置坐标 + 当前平台标识
- 爪印池占用（active/total）
- 冷却倒计时
- 帧耗时（avg / peak / over-budget count）
- 设备 tier

操作按钮：
- **强制出场**：跳过 idle 等待，直接从 home → peek → walk
- **强制回家**：任何状态 → home
- **暂停/恢复**：冻结当前状态，方便截图检查
- **重置冷却**：清零冷却时间，方便反复测试出场

参数滑块（实时生效，不需要重新部署）：
- idle 阈值（1s - 30s）
- 冷却时间（0s - 10min）
- 爪印透明度（0.1 - 1.0）
- 步速（10px/s - 100px/s）
- 爪印尺寸（8px - 32px）

地形可视化：按钮切换显示/隐藏，叠加半透明色块标识已扫描的可行走平台。

#### 0e. 性能基准 + 状态机测试

**性能基准脚本**（在展台页面运行）：
- 自动触发 10 秒巡游：出场 → 行走 20 步 → 退场
- 记录每帧耗时，输出：avg、p95、p99、over-budget（>1ms）帧数
- 在 low/high 两种模式下各跑一次
- 通过标准：p99 < 2ms，over-budget < 5%

**状态机单元测试**（纯逻辑，不需要 DOM）：
- 覆盖所有状态转换路径：
  - home → peek → walk → home（正常循环）
  - walk + click → home（点击退场）
  - walk + scroll → home（滚动退场）
  - walk + modal → home（modal 退场）
  - walk + 阿宝 tab → converge → home（收敛回家）
  - home + idle（冷却中）→ home（不出场）
  - home + idle（冷却完）→ peek（出场）
- 验证冷却时间逻辑
- 验证 `prefers-reduced-motion` 时不初始化

**评审点**：展台中所有动画视觉达标 + 性能基准通过 + 状态机测试全绿

---

### Phase 1: 核心循环（MVP）

在 Phase 0 的展台中打磨好所有动画后，集成进 Next 主应用。

- 状态机：home → peek → walk → home（接入 IdleDetector）
- 爪印：单爪 SVG、左右交替、ObjectPool
- 环境染色：高端 computed style 采样（放置瞬间查一次）/ 低端固定主题色
- 内容避让：高端 mix-blend-mode / 低端固定半透明
- 涟漪：落地水纹 CSS animation
- 退场：点击退场（波浪淡出）、滚动退场、modal 退场
- Tab icon 联动（SVG stroke/fill 过渡）
- 设置开关
- 调试面板集成
- 仅移动端

**评审点**：性能验收 + 感知验收通过后才进 Phase 2

### Phase 2: 地形感知
- 局部 DOM 扫描构建平台缓存
- 沿卡片边缘行走（CSSAnimator 生成 offset-path）
- 绕开按钮/可点击区域
- MutationObserver 脏位标记 + 按需重算

**评审点**：地形行走是否自然、性能是否稳定

### Phase 3: 页面交互
- Todo：完成任务踩已阅爪印、最后任务 ✓ 爪印（仅巡游中触发）
- 记账/例行：跟 jelly pill 同向移动
- 阿宝 tab 聊天状态 logo 联动（脉冲/呼吸灯/微跳）

**评审点**：交互是否自然、是否干扰正常操作

### Phase 4: 收敛动画
- 切到阿宝 tab 时：爪印就地消散 + 单条光弧飞回 logo
- Logo 脉冲
- 从阿宝 tab 切走时 logo 微晃

**评审点**：动画是否流畅、是否建立"同一只二狗"的认知

### Phase 5+: 高级行为（可选增强）
- 二狗身体轮廓（极简 SVG，几条线）
- 拟人动作：摇尾巴、打哈欠、伸懒腰、扒拉、挂住边缘
- 下拉刷新爪印替代 spinner
- 系统事件响应（网络、深夜、摇晃）
- 桌面端适配
