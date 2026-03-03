## Context

Next 的 AI 助手"二狗"目前仅存在于聊天面板。SPEC-057 定义了巡游系统——让二狗以爪印轨迹在非聊天页面具象化。Phase 0 是基础设施阶段，为后续 Phase 1-4 提供可复用的通用模块和开发工具。

当前状态：
- 前端为 Vanilla JS（无框架、无构建工具），通过 `<script>` 标签加载
- 仅移动端（Phase 1 明确限定）
- 已有 `patrol-interfaces.js` 定义了接口契约

## Goals / Non-Goals

**Goals:**
- 提供通用工具模块（ObjectPool、DeviceProfile、CSSAnimator、IdleDetector、StateMachine、PawPool），独立于二狗业务逻辑
- 提供动画展台，可在独立页面调参、验证所有动画效果
- 提供调试面板，集成进主应用用于运行时检查
- SVG 资产替换 emoji，支持后续 tab icon 状态联动
- 确保核心动画 <1ms/帧的性能预算

**Non-Goals:**
- 不实现巡游核心循环（Phase 1）
- 不实现地形感知（Phase 2）
- 不实现页面交互响应（Phase 3）
- 不实现收敛动画（Phase 4）
- 不适配桌面端
- 不引入构建工具或框架

## Decisions

### D1: CSS 自定义属性驱动方向旋转

**决定**: 用 `--paw-heading` + `--paw-mirror` CSS 自定义属性让 heading 旋转和 scale 动画共存。

**备选方案**:
- A) 嵌套 DOM（外层旋转、内层动画）——增加 DOM 复杂度和内存
- B) Web Animations API——需要额外 polyfill，与 CSS animation 不一致
- C) 独立 `rotate` CSS 属性——Safari 14.1+ 支持，但仍可能与 keyframes 冲突

**理由**: CSS 自定义属性在 keyframes 中可读取元素上的值，零额外 DOM，与现有 CSS animation 体系一致。所有现代移动浏览器支持。

### D2: 对象池 FIFO 复用而非 LRU

**决定**: ObjectPool 用简单遍历找第一个空闲元素，PawPool 最多 8 个。

**备选方案**:
- A) LRU 淘汰——需要额外数据结构跟踪使用顺序
- B) 动态扩容——违反"不创建/销毁 DOM"红线

**理由**: 8 个元素遍历成本可忽略（<0.01ms），简单可靠。满池时返回 null，由上层决定是否强制释放最老的。

### D3: 展台独立于主应用

**决定**: `patrol-showcase.html` 只引用 `patrol-utils.js` 和 `patrol.css`，不依赖 app.js 或其他模块。

**理由**: 开发阶段需要快速迭代动画参数，不想因主应用状态影响调试。展台同时作为"活的动画文档"，新 Phase 的动画都会加入。

### D4: 调试面板通过 CustomEvent 通信

**决定**: 调试面板的参数变更通过 `patrol:debugParam` CustomEvent 广播，主模块监听并应用。

**备选方案**:
- A) 直接引用主模块对象——耦合过强
- B) 全局变量——不可控

**理由**: CustomEvent 解耦调试面板和主模块，面板不需要知道巡游系统的内部结构。

### D5: 文件结构

| 文件 | 职责 |
|------|------|
| `assets/icons/paw-single.svg` | 单爪 SVG 资产 |
| `assets/icons/paw-tab.svg` | 双爪 tab icon SVG 资产 |
| `assets/css/patrol.css` | 巡游视觉层（爪印/涟漪/蒸发/tab 状态/reduced-motion） |
| `assets/js/patrol-interfaces.js` | 接口契约文档（已有） |
| `assets/js/patrol-utils.js` | ObjectPool + DeviceProfile + CSSAnimator + IdleDetector + StateMachine + PawPool |
| `assets/js/patrol-debug.js` | 调试面板 |
| `patrol-showcase.html` | 动画展台 |
| `index.html` | 引入 CSS + emoji→SVG 替换 |

### D6: 移动端优先，桌面端预留

**决定**: Phase 0 的代码不限制平台（工具模块本身平台无关），但展台 UI 按移动端设计。桌面端加提示条说明仅移动端生效。

**预留**: DeviceProfile 可扩展 `isMobile` 检测，Phase 1 集成时用于跳过桌面端初始化。

## Risks / Trade-offs

- **[CSS 自定义属性在 keyframes 中的兼容性]** → 所有目标浏览器（iOS Safari 13+, Chrome 80+）均支持，实测验证。
- **[对象池 8 个可能不够]** → SPEC-057 明确限定最多 8 个爪印，如需调整只改 pool size 参数。
- **[展台页面体积]** → 单文件含 HTML+CSS+JS，不做拆分。体积可控（<30KB），不影响主应用。
- **[调试面板残留在生产环境]** → 受 localStorage 标记控制，不设标记则零 DOM 开销。

## Migration Plan

1. 新增文件无破坏性，可直接部署
2. emoji→SVG 替换是视觉变更，回滚只需 git revert index.html 改动
3. patrol.css 引入不影响现有样式（所有选择器以 .patrol- 或 #patrol- 为前缀）
4. 无后端变更，无数据库变更

## Open Questions

- 爪印尺寸 32px 是否合适？需要在展台和真机上验证后确定，可通过调试面板滑块实时调整
- 环境染色（高端设备取 computed style 主色调）的具体采样策略放到 Phase 1 实现时决定
