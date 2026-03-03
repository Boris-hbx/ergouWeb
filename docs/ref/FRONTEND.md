# Frontend

> JS 模块、CSS 组织、快捷键、PWA
> 最后更新: 2026-02-21

## 目录结构

```
frontend/
├── index.html              # 主页面（任务管理、侧边栏、弹窗、阿宝面板）
├── login.html              # 登录/注册页面
├── sw.js                   # Service Worker (PWA 离线缓存 + 推送通知)
└── assets/
    ├── css/
    │   ├── base.css        # CSS 变量、主题（深色/浅色）、Reset、排版
    │   ├── style.css       # 主布局：顶栏、侧边栏、内容区、泳道、任务卡片、此刻
    │   ├── components.css  # 弹窗、日期选择器、Toast、搜索框、进度条
    │   ├── mobile.css      # 移动端：底部 Tab、单列布局、触摸优化
    │   ├── abao.css        # 阿宝对话面板样式
    │   └── english.css     # 英语场景页面样式
    ├── js/                 # 见下方模块详解
    ├── icons/              # favicon, PWA icons (各尺寸)
    ├── images/             # 头像预设图片
    └── manifest.json       # PWA manifest (name, icons, theme_color)
```

## JS 模块加载顺序

**必须按此顺序在 HTML 中引入**（后面的模块依赖前面的全局变量）：

```
api.js → utils.js → app.js → tasks.js → modal.js → datepicker.js → drag.js → review.js → routines.js → features.js → particles.js → living-line.js → abao.js → settings.js → notifications.js
```

## JS 模块职责

| 文件 | 职责 | 关键导出/全局 |
|------|------|-------------|
| `api.js` | REST API 封装，统一 fetch + 401 拦截跳转 | `window.API` 对象 |
| `utils.js` | 工具函数 | `escapeHtml()`, `showToast()`, `formatDate()` |
| `app.js` | 全局状态、Tab/Page 切换、侧边栏、头像菜单、Moment 模块 | `currentTab`, `switchTab()`, `switchPage()`, `Moment` |
| `tasks.js` | 任务列表渲染、CRUD、象限折叠、排序 | `renderItems()`, `loadItems()` |
| `modal.js` | 任务详情/编辑/创建弹窗 | `openTaskModal()`, `openCreateModal()` |
| `datepicker.js` | 自然语言日期选择器 | 弹窗内使用 |
| `drag.js` | 鼠标 + 触屏统一拖拽（跨泳道 + 跨 Tab） | `DragManager`, `startCustomDrag()` |
| `review.js` | 例行审视 CRUD、频率配置 | `loadReviews()` |
| `routines.js` | 例行任务面板、About 弹窗 | `loadRoutines()` |
| `features.js` | 快捷键、每日回顾、工具提示、搜索 | 自动初始化 |
| `particles.js` | 顶栏彗星粒子 Canvas 动画 | 自动初始化 |
| `living-line.js` | 底部呼吸线 Canvas 动画 | 自动初始化 |
| `abao.js` | 阿宝 AI 对话面板、消息渲染、对话管理 | `toggleAbao()` |
| `settings.js` | 设置页面：账户信息、改密码、注销、好友管理 | 自动初始化 |
| `notifications.js` | 推送通知订阅、应用内通知、提醒弹窗 | 自动初始化 |

## CSS 文件分工

| 文件 | 内容 |
|------|------|
| `base.css` | `:root` CSS 变量、`[data-theme="dark"]` 主题、全局 Reset |
| `style.css` | 桌面端主布局（顶栏 + 侧边栏 + 内容区 + 泳道 + 任务卡片 + 此刻文案） |
| `components.css` | 弹窗、日期选择器、Toast 通知、搜索框、进度条、按钮 |
| `mobile.css` | `@media (max-width: 768px)` 移动端覆盖样式 |
| `abao.css` | 阿宝对话面板（独立，避免影响主样式） |
| `english.css` | 英语场景页面样式 |

**重要**: `mobile.css` 中的样式必须包裹在 `@media (max-width: 768px)` 内，否则会影响桌面端。

**CSS 变量用法**: `var(--primary-color)`, `var(--bg-color)`, `var(--text-color)` 等。主题切换通过 `document.documentElement.dataset.theme = 'dark'` 触发。

## 全局状态

```javascript
window.currentTab = 'today';     // 当前时间维度 tab
window.currentPage = 'todo';     // 当前页面 (todo/review/english/inbox/settings)
window.allItems = [];            // 当前 tab 的全部任务
window.showCompleted = true;     // 是否显示已完成任务
```

## 快捷键

| 按键 | 功能 | 实现位置 |
|------|------|---------|
| `N` | 新建任务 | features.js |
| `S` | 搜索 | features.js |
| `B` | 打开/关闭阿宝 | features.js |
| `1` / `2` / `3` | 切换 Today / Week / Month | features.js |
| `D` | 切换深色/浅色主题 | features.js |
| `R` | 每日回顾 | features.js |
| `?` | 快捷键帮助 | features.js |

## Moment（此刻）模块

`app.js` 中的 `Moment` IIFE 模块，管理顶栏一句话：

```javascript
Moment.load();           // 设置时段 icon + 调 API + 淡入文本
Moment.startAutoRefresh(); // 15 分钟定时 + visibilitychange 监听
Moment.refreshIfStale();  // 任务加载后调用，>5分钟才真正刷新
```

**前端兜底**: API 失败时显示时段问候（"上午好"/"晚上好"）

## 拖拽系统

`drag.js` 中的 `DragManager` 统一处理鼠标和触屏拖拽：

| 输入 | 触发条件 | 阈值 |
|------|---------|------|
| 鼠标 | mousedown → mousemove | 5px |
| 触屏 | touchstart → 长按 300ms | 10px (取消长按) |

**交互流程**: 按住/长按 → 出现半透明克隆体 → 拖到目标象限/Tab → 释放 → 执行移动

**排除元素**: button、.task-checkbox、.task-delete、.progress-ring、input 不触发拖拽。

## API 调用模式

```javascript
// API 对象封装了 fetch + 错误处理
const todos = await API.getTodos('today');
await API.createTodo({ text: '新任务', tab: 'today' });
await API.updateTodo(id, { progress: 50 });
await API.deleteTodo(id);
await API.getMoment();  // 此刻文案

// 401 自动跳转 login.html
```

## 用户反馈

```javascript
showToast('操作成功', 'success');  // 绿色
showToast('操作失败', 'error');    // 红色
showToast('提示信息', 'info');     // 蓝色
```

## PWA / Service Worker

**缓存策略**: Network First, Cache Fallback

- **Install**: 预缓存所有静态资源（HTML/CSS/JS），`skipWaiting()`
- **Activate**: 清理旧版本缓存，`clients.claim()`
- **Fetch**:
  - `/api/*` 请求：直接走网络，不缓存
  - 静态资源：先尝试网络，成功则更新缓存；失败则回退缓存
- **Push**: 接收推送消息 → `showNotification()` + `setAppBadge()`
- **NotificationClick**: 知道了 → 聚焦窗口 + clearBadge；5分钟后 → snooze API
- **版本管理**: `CACHE_NAME = 'next-v13'`，更新时改版本号

**注意**: `sw.js` 和 `index.html` 由服务器设置 `Cache-Control: no-cache` 头，避免 SW 缓存死循环。

## localStorage 存储

| Key | 用途 |
|-----|------|
| `userAvatar` | 头像选择（预设头像名） |
| `theme` | 主题偏好 (`dark` / `light` / `system`) |
| `quadrantStates` | 各象限折叠/展开状态 |
| `sidebarCollapsed` | 侧边栏是否收起 |

## 页面结构

| Page ID | 页面 | 切换函数 |
|---------|------|---------|
| `todo` | 任务管理（四象限 + 例行任务） | `switchPage('todo')` |
| `review` | 例行审视 | `switchPage('review')` |
| `english` | 英语场景学习 | `switchPage('english')` |
| `inbox` | 收件箱（协作确认 + 分享） | `switchPage('inbox')` |
| `settings` | 设置（账户 + 好友 + 联系人） | `switchPage('settings')` |

## 公共组件

### PhotoManager（照片处理）

位置: `frontend/assets/js/utils.js`

统一处理照片选择、自动压缩、缩略图预览、删除、放大查看、Base64 转换。所有涉及照片上传的模块（记账、差旅等）必须使用此组件。

**初始化:**
```javascript
var pm = new PhotoManager({
    container: '#my-photo-grid',        // 缩略图渲染容器（CSS 选择器）
    onChange: function(files) {          // 照片列表变化回调
        // files 是当前照片 File 数组的副本
        updateMyButtons(files.length);
    }
});
```

**方法:**

| 方法 | 说明 |
|------|------|
| `pm.addFiles(fileList)` | 添加文件，自动过滤非图片并提示，触发渲染和回调 |
| `pm.remove(index)` | 删除指定位置的照片，局部更新网格 |
| `pm.clear()` | 清空所有照片 |
| `pm.getFiles()` | 获取当前 File 数组（用于 FormData 上传） |
| `pm.getBase64()` | 返回 Promise\<Array\>，压缩后的 Base64 数据（用于 AI 识别） |

**HTML 结构:**
```html
<div class="pm-grid" id="my-photo-grid"></div>
<label>
    <span>+ 添加照片</span>
    <input type="file" multiple accept="image/*" style="display:none"
           onchange="MyModule.handlePhotos(this.files)">
</label>
```

**CSS 类名:** `pm-grid`、`pm-thumb`、`pm-remove`、`pm-lightbox`、`pm-lightbox-close`

**按钮模式规范:**
- 保存按钮始终可见可用
- AI 分析/识别按钮作为独立的可选操作，不替换保存按钮
- 有照片时：保存(secondary) + AI识别(primary) 并排显示
