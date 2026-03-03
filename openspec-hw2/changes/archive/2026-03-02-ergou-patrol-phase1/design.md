## Context

Phase 0 已完成基础设施：ObjectPool、DeviceProfile、CSSAnimator、IdleDetector、PatrolStateMachine、PawPool、patrol.css、调试面板。Phase 1 将这些模块组装为完整的巡游核心循环，在 Next 主应用的手机端运行。

当前状态：
- Vanilla JS 无框架，`<script>` 标签加载
- patrol-utils.js 已包含所有工具模块
- patrol.css 已包含爪印/涟漪/蒸发/Tab icon/收敛光弧/reduced-motion 样式
- patrol-debug.js 已包含调试面板（admin-only）
- index.html 已引入上述 CSS/JS，SVG 已替换 emoji
- 页面通过 `switchPage(page)` 切换视图，modal 通过 `overlay.style.display = 'flex'` 打开
- Tab 切换通过底部 `mobile-nav-item` 元素
- 阿宝 tab 的 SVG 元素带 `.tab-icon-patrol` class

## Goals / Non-Goals

**Goals:**
- 实现完整的 home → peek → walk → pause → rest → home 状态循环
- 实现 5 种退场动画（点击/Modal/走出屏幕/滚动/tab切换）
- Tab icon 实时联动
- 设置页开关
- 仅移动端

**Non-Goals:**
- 地形感知（Phase 2）
- 页面交互响应（Phase 3）
- 收敛回家动画（Phase 4）
- 身体轮廓/拟人动作（Phase 5+）
- 桌面端适配

## Decisions

### D1: patrol.js 作为独立主控文件

**决定**: 新建 `frontend/assets/js/patrol.js` 作为主控模块，不修改 patrol-utils.js 的模块结构。

**理由**: patrol-utils.js 是通用工具，patrol.js 是业务逻辑。分离让展台继续独立运行，也方便未来 Phase 逐步增强主控逻辑。

### D2: 行走路径用 JS 定时器驱动步点，非整段 CSS animation

**决定**: 行走循环用 `setInterval` / `setTimeout` 按固定步频调用 `PawPool.step()`，而非生成整段 CSS offset-path。

**备选方案**:
- A) 整段 CSS offset-path + animation → 需要一个不可见的"引导元素"沿路径移动，再用 `getComputedStyle` 读取位置 → 复杂且每帧查询违反性能原则
- B) Web Animations API → 额外 API，Safari 兼容性需考虑

**理由**: Phase 1 路径简单（随机直线/缓弧），用 JS 计算下一步位置足够轻量（每步一次计算，不在 RAF 中）。步频约 1 步/1.3s，JS 仅在步点时刻触发一次 PawPool.step()，其余时间全靠 CSS animation（落地弹性、蒸发）。满足 <1ms/帧预算。

### D3: Modal 检测用 MutationObserver

**决定**: 用 MutationObserver 监听 `document.body` 的 `childList` 和 `attributes` 变化，检测 `*-overlay` / `*-modal` 元素的 `display` 从 `none` 变为可见。

**备选方案**:
- A) 在每个 modal 打开函数中手动 dispatch 自定义事件 → 侵入式修改大量现有代码
- B) 轮询检测 → 性能差

**理由**: Next 的 modal 统一用 `overlay.style.display = 'flex'/'none'` 控制。MutationObserver 一次配置，自动覆盖所有 modal，零侵入。

### D4: 滚动退场不触发冷却

**决定**: 滚动退场后不调用 `startCooldown()`，仅重新开始 idle 计时。点击退场和 Modal 退场触发 3 分钟冷却。

**理由**: SPEC-057 演练修正——滚动是浏览行为，不是"驱赶"行为。滚动停止后用户回到 idle，二狗应该可以快速重新出场。如果滚动也冷却，二狗在实际使用中几乎不可见。

### D5: 移动端检测策略

**决定**: 用 `window.innerWidth <= 768 && ('ontouchstart' in window || navigator.maxTouchPoints > 0)` 判断移动端。

**理由**: 纯 width 判断会误判桌面端缩窄窗口。结合 touch 能力更准确。Phase 0 的 DeviceProfile 可扩展 `isMobile` 属性。

### D6: 环境染色实现

**决定**: 高端设备在 `PawPool.step()` 调用前，用 `document.elementFromPoint(x, y)` 获取脚下元素，读取 `getComputedStyle(el).backgroundColor`。如果是透明（rgba 0,0,0,0）则向上遍历 parentElement 直到找到非透明色或到达 body。

**理由**: 放置瞬间查询一次，不在动画帧中。遍历深度有限（通常 2-3 层），性能可接受。

### D7: 路径生成（Phase 1 简化版）

**决定**: Phase 1 不做地形感知，路径为视口安全区域内的随机路径段：
1. 起点：上次终点（或 tab icon 位置）
2. 方向：随机角度（避免 ±30° 内重复前一段方向）
3. 步数：每段 5-8 步
4. 约束：不超出安全区域（top:44px, bottom:viewHeight-60px, left:8px, right:viewWidth-8px）
5. 到达终点触发 walkEnd → pause

**理由**: Phase 2 才引入地形感知。Phase 1 先让二狗能走起来，验证核心循环。

### D8: 文件结构

| 文件 | 职责 |
|------|------|
| `assets/js/patrol.js` | **新增** — 主控模块（init/destroy/EventBridge/路径生成/行走循环/退场/Tab联动） |
| `assets/js/patrol-utils.js` | **修改** — 补全 fadeWave、PawPool 追踪放置顺序 |
| `assets/css/patrol.css` | **修改** — 新增退场相关样式（如果需要） |
| `assets/js/settings.js` | **修改** — 添加巡游开关 |
| `index.html` | **修改** — 引入 patrol.js、初始化调用 |

## Risks / Trade-offs

- **[路径随机性导致不自然]** → Phase 1 可接受，Phase 2 地形感知后大幅改善。调试面板可实时调参。
- **[MutationObserver 性能]** → 仅监听 body 直接子节点的 style/display 变化，不监听 subtree，开销极低。
- **[elementFromPoint 在动画中的准确性]** → 仅在步点瞬间调用一次（非 RAF 中），此时 DOM 稳定。
- **[滚动检测的灵敏度]** → 使用 passive scroll listener，100ms debounce 后触发退场（避免轻微触碰误退）。

## Migration Plan

1. patrol-utils.js 的修改向后兼容（fadeWave 从 TODO 变为真实实现）
2. patrol.js 是全新文件，不影响现有功能
3. settings.js 添加新开关，不影响现有设置
4. 所有巡游逻辑受 `patrol-enabled` localStorage 控制，默认开启但可随时关闭
5. 回滚方案：从 index.html 移除 patrol.js 引用即可
