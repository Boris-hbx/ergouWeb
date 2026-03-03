## Context

Phase 1-2 完成了二狗巡游的核心循环和地形感知。Phase 3 要让二狗对页面操作做出反应。核心挑战：交互动画要叠加在正常巡游之上，不中断行走节奏，不阻塞用户操作。

当前关键代码结构：
- `tasks.js saveProgress()` — 任务完成的核心函数，成功后显示 toast
- `app.js activateMobileNav(el)` — 底部 nav 切换，调用 `_jellyMobileNav.moveTo(el)`
- `abao.js` — 聊天消息发送和接收处理
- `patrol.js setupEventBridge()` — 所有事件监听的集中点

## Goals / Non-Goals

**Goals:**
- 任务完成已阅爪印 + 象限清空 ✓ 图案
- Jelly pill 跟随跑动
- 聊天状态 logo 呼吸灯
- 所有交互不干扰用户正常操作

**Non-Goals:**
- 复杂的二狗动画（身体、尾巴等 — Phase 5）
- 交互音效
- 桌面端适配

## Decisions

### D1: 已阅爪印用独立 DOM 元素

**决定**: 已阅爪印不使用 PawPool，而是创建独立的临时 DOM 元素。

**理由**: PawPool 最大 8 个，正常行走已在使用。已阅爪印是叠加效果，不应占用行走配额。独立元素有独立生命周期（2s），创建后自我管理销毁。

### D2: 事件派发采用自定义事件解耦

**决定**: tasks.js / app.js / abao.js 通过 `document.dispatchEvent(new CustomEvent(...))` 通知 patrol 系统，patrol.js 在 EventBridge 中监听。

**备选方案**:
- A) 直接在 tasks.js 中调用 Patrol.onTaskComplete() → 耦合太紧
- B) 全局回调注册 → 需要额外注册机制

**理由**: 自定义事件完全解耦。patrol.js 可选加载，不影响原有功能。已有 `patrol:pageSwitch` 的成功先例。

### D3: Jelly pill 跟随用方向脉冲

**决定**: 检测到 jelly pill 移动时，生成 2-3 步短路径朝同方向快跑，然后自动回到正常巡游。

**备选方案**:
- A) 实时跟踪 pill 位置 → 需要 RAF 轮询 pill 坐标，性能不好
- B) 直接移动到 pill 目标位置 → 太机械

**理由**: 方向脉冲最自然——二狗感受到了 pill 的"风"，跑了几步。短路径生成后按正常 stepOnce 执行，不需要新机制。

### D4: 聊天呼吸灯用纯 CSS 动画

**决定**: `.abao-thinking` class 控制呼吸灯，纯 CSS `@keyframes` 实现。

**理由**: 不涉及 JS 定时器，性能零开销。abao.js 在发送消息时添加 class，收到回复时移除。

### D5: ✓ 图案用 3 个固定偏移爪印

**决定**: ✓ 图案由 3 个爪印组成，位置相对于象限容器固定偏移：
- 左下角 (30%, 70%)
- 底部中间 (50%, 80%)
- 右上角 (70%, 30%)

用 setTimeout 100ms 间隔依次放置。

**理由**: 简单可靠。3 个点足以形成辨识度高的 ✓ 形状。不需要路径规划。

### D6: 文件结构

| 文件 | 职责 |
|------|------|
| `assets/js/patrol.js` | **修改** — EventBridge 增加交互事件监听 + 响应逻辑 |
| `assets/js/tasks.js` | **修改** — saveProgress 成功后 dispatch patrol:taskComplete |
| `assets/js/app.js` | **修改** — activateMobileNav 调用前 dispatch patrol:jellyMove |
| `assets/js/abao.js` | **修改** — 发送/接收处理中 dispatch patrol:chatStatus |
| `assets/css/patrol.css` | **修改** — 新增 .stamped 和 .abao-thinking 样式 |

## Risks / Trade-offs

- **[已阅爪印位置可能不精确]** → 卡片完成后 DOM 会变化（移到已完成区），需在 DOM 变化前获取位置。在 dispatchEvent 前调用 getBoundingClientRect。
- **[✓ 图案可能与正在行走的爪印重叠]** → 可接受，✓ 爪印有 stamped 样式区分，视觉上不冲突。
- **[Jelly pill 跟随可能与退场冲突]** → 切到阿宝 tab 时 abaoTab 事件先于 jellyMove 处理，收敛逻辑优先。

## Migration Plan

1. tasks.js / app.js / abao.js 的 dispatchEvent 向后兼容 — 无 listener 时事件被忽略
2. patrol.js 的交互监听检查模块是否存在 — 不影响 Phase 1-2 功能
3. 回滚方案：移除 EventBridge 中的交互事件监听即可
