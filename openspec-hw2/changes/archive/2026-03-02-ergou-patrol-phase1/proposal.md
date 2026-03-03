## Why

Phase 0 已完成基础设施（SVG 资产、通用工具模块、动画展台、调试面板），但二狗的爪印还无法在 Next 主应用中出现。Phase 1 将这些基础设施集成为完整的巡游核心循环——让二狗真正在页面上行走、退场、联动 tab icon。这是整个巡游系统从"开发工具"到"用户可感知功能"的关键一步。

## What Changes

- 新增 `patrol.js` 主控模块：初始化检测、patrol-layer 创建、状态机接入 IdleDetector、EventBridge 事件绑定
- 实现出场流程：tab icon 泛光 → 探头 → 第一步落地 → 行走
- 实现行走循环：左右交替踩爪印、环境染色（高端 computed style / 低端固定色）、步态参数
- 实现 5 种退场：点击（波浪淡出）、滚动（快速 fade）、Modal（躲闪 fade）、走出屏幕（overflow 裁切）、切换普通 tab（坠落淡出）
- 实现 Tab icon 联动：SVG stroke/fill 状态过渡（实心 → 变淡 → 空心 → 填回 → 脉冲）
- 实现 pause/rest 子状态（行走路径走完后停留、长时间停留转趴下）
- 在设置页添加"二狗巡游"开关
- 调试面板集成（connect 主模块引用）
- 仅移动端生效（桌面端跳过初始化）

## Capabilities

### New Capabilities
- `patrol-core`: 巡游核心循环——主控模块初始化、状态机驱动、出场/行走/退场完整流程、事件桥接
- `patrol-tab-sync`: Tab icon 联动——根据二狗状态实时切换 tab icon 的 fill/stroke/pulse/breathe 样式

### Modified Capabilities
- `patrol-infra`: 补充 PawPool.fadeWave 的真实实现（Phase 0 为 TODO）、增加 patrol-layer 容器自动创建逻辑

## Impact

- **新增文件**: `frontend/assets/js/patrol.js`（主控模块）
- **修改文件**: `frontend/index.html`（引入 patrol.js、初始化调用）、`frontend/assets/js/patrol-utils.js`（补全 fadeWave）、`frontend/assets/js/settings.js`（添加巡游开关）、`frontend/assets/css/patrol.css`（可能新增退场/出场相关样式）
- **无后端变更**: 纯前端功能
- **无数据库变更**: 开关状态存 localStorage
