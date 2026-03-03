## Why

Next 的 AI 助手"二狗"目前只存在于聊天面板中。用户在其他页面（任务、记账、例行）操作时，感知不到 AI 的存在。巡游系统让二狗以爪印轨迹的形式在 UI 空间中具象化——聊天框里它说话，页面上它巡视，是同一只二狗。

Phase 0 是巡游系统的基础设施阶段：SVG 资产、通用工具模块、动画展台、调试面板、性能基准。这些模块独立于业务逻辑，为后续 Phase 1-4 的核心循环、地形感知、页面交互、收敛动画提供地基。

## What Changes

- 新增单爪 SVG 资产（16×16, <500 bytes），用于爪印渲染
- 新增双爪 tab icon SVG，替换现有 🐾 emoji（3 处），支持 stroke/fill 独立控制实现实心↔空心过渡
- 新增通用工具模块 `patrol-utils.js`：ObjectPool（DOM 对象池）、DeviceProfile（设备能力检测）、CSSAnimator（运行时 CSS 动画生成）、IdleDetector（空闲检测+冷却管理）、PatrolStateMachine（状态机）、PawPool（爪印专用池）
- 新增 `patrol.css`：爪印视觉层样式（落地弹性、涟漪、蒸发、呼吸、tab icon 状态过渡）
- 新增动画展台 `patrol-showcase.html`：12 个展区（爪印渲染、落地、蒸发、步态、路径测试、退场、blend mode、tab icon、设备模拟、状态机、性能基准、状态机单元测试）
- 新增调试面板 `patrol-debug.js`：开发时实时监控状态/帧耗时/爪印池，参数滑块实时调参
- index.html 引入 patrol.css，emoji 替换为 inline SVG

## Capabilities

### New Capabilities
- `patrol-infra`: 巡游系统基础设施——SVG 资产、对象池、设备检测、CSS 动画生成器、空闲检测、状态机、爪印池
- `patrol-visual`: 巡游视觉层——爪印 CSS 动画（落地/涟漪/蒸发/呼吸）、tab icon 状态过渡、heading 方向系统
- `patrol-devtools`: 开发工具——动画展台（路径测试/参数调节/性能基准/状态机测试）、调试面板

### Modified Capabilities
（无已有 capability 的需求变更）

## Impact

- **前端文件新增**: `patrol-utils.js`, `patrol.css`, `patrol-debug.js`, `patrol-showcase.html`, `paw-single.svg`, `paw-tab.svg`
- **前端文件修改**: `index.html`（引入 CSS、emoji→SVG）
- **后端**: 无变更
- **API**: 无变更
- **性能**: 核心动画仅使用 transform + opacity（GPU 合成），`prefers-reduced-motion: reduce` 时不初始化
- **兼容**: 低端设备降级（无 mix-blend-mode），高端设备增强
