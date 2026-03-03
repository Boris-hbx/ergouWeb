# SPEC-056: Today 红色拖尾效果重构
> 起草日期: 2026-03-02
> 状态: 草稿

## 背景

Today 按钮和例行按钮（`.btn-routine`）共用一套"旋转红色拖尾 + 紫色背景填充"的 CSS 动效。当前实现用两个 pseudo-element：

- `::before` — 旋转的红色光弧边框（conic-gradient + mask-composite）
- `::after` — 紫色渐变背景填充

问题：Today 按钮的 `::after` 紫色背景与 jelly pill（matrix tab 的滑动选中指示器）视觉冲突 —— 非激活状态下 today 仍显示满紫色背景，容易误认为是选中状态。

## 目标

- **保留**：`::before` 红色旋转拖尾边框
- **保留**：速度调整逻辑（`speed-fast` / `speed-slow`，基于完成比例）
- **去掉**：Today 按钮的 `::after` 紫色背景填充，让 today 非激活时不再有满紫色背景
- Today 激活时的紫色背景由 jelly pill 或 `.matrix-tab.active` 样式提供，不需要 `::after` 重复

## 改动范围

### 1. CSS (`components.css`)

#### 1.1 修改 `.has-pending` 基础样式

当前：
```css
.btn-routine.has-pending,
.matrix-tab[data-tab="today"].has-pending {
    position: relative;
    overflow: visible;
    isolation: isolate;
    background: transparent !important;
}
```

改为：将 today 的 `background: transparent !important` 去掉（不再需要强制透明来让 `::after` 显示），today 的背景交由 `.matrix-tab.active` / jelly pill 控制：

```css
.btn-routine.has-pending {
    position: relative;
    overflow: visible;
    isolation: isolate;
    background: transparent !important;
}
.matrix-tab[data-tab="today"].has-pending {
    position: relative;
    overflow: visible;
}
```

> 注：btn-routine 保留 `isolation: isolate` 和 `background: transparent !important` 不变。

#### 1.2 删除 Today 的 `::after` 规则

删除整个 block：
```css
.matrix-tab[data-tab="today"].has-pending::after {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: var(--primary-gradient);
    z-index: -1;
}
```

btn-routine 的 `::after` 保留不变。

#### 1.3 `::before` 红色拖尾保留不变

```css
.btn-routine.has-pending::before,
.matrix-tab[data-tab="today"].has-pending::before {
    /* 旋转红色光弧 — 完全保留 */
}
```

#### 1.4 速度变体保留不变

```css
.btn-routine.speed-fast::before,
.matrix-tab.speed-fast::before {
    --rotate-speed: 2s;
}
.btn-routine.speed-slow::before,
.matrix-tab.speed-slow::before {
    --rotate-speed: 5s;
}
```

### 2. JS (`routines.js`) — 无改动

`updateButtonAnimations()` 逻辑完全保留：
- 有未完成任务 → 添加 `has-pending`
- 完成率 < 50% → `speed-fast`（2s 旋转一圈）
- 完成率 >= 50% → `speed-slow`（5s 旋转一圈）
- 全部完成 → 移除 `has-pending`、`speed-fast`、`speed-slow`

### 3. 视觉预期

| 状态 | Today 按钮外观 |
|------|---------------|
| 激活 + 有待办 | jelly pill 紫色背景 + 红色旋转边框 + 白色文字 |
| 激活 + 无待办 | jelly pill 紫色背景 + 白色文字（无红框） |
| 非激活 + 有待办 | 普通灰底 + 红色旋转边框 + 默认文字色 |
| 非激活 + 无待办 | 普通灰底 + 默认文字色 |

## 不影响的部分

- `btn-routine`（例行按钮）的拖尾 + 紫色背景保持原样
- `@property --btn-angle`、`@keyframes btn-border-rotate` 保持原样
- Jelly pill indicator 系统不受影响
