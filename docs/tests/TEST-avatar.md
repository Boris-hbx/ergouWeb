# TEST-avatar: 头像系统排查分析与测试用例

> 日期: 2026-02-25
> 触发: 打开网页后右上角头像始终显示默认 "B"，而非用户预设的自定义头像

---

## 一、问题现象

用户已在设置页选择/上传过自定义头像，但每次刷新页面或重新登录后，右上角 header 头像始终显示蓝紫渐变 + 首字母 "B"（默认态），而非已设置的头像。

---

## 二、根因分析

### BUG-1（主因）：脚本加载顺序竞态 — `applyAvatar()` 未就绪时被调用

**相关文件:**
- `frontend/assets/js/app.js:4-31` — `checkAuth()` IIFE
- `frontend/assets/js/settings.js:130-183` — `applyAvatar()` 定义
- `frontend/index.html:1178-1204` — script 加载顺序

**脚本加载顺序:**
```
#3  app.js        ← checkAuth() IIFE 立即执行，发起 fetch('/api/auth/me')
#4  tasks.js
#5  modal.js
... (中间 20+ 个脚本)
#26 settings.js   ← applyAvatar() 在这里定义
#27 admin.js
```

**时序分析:**
```
时间线:
  t0   app.js 执行 → checkAuth() IIFE 启动 → fetch() 发出
  t1   tasks.js 加载执行
  t2   modal.js 加载执行
  ...
  t?   ⚡ fetch 响应到达 → async 函数恢复执行
       → typeof applyAvatar === 'function' ?
       → 如果 settings.js 还没加载 → FALSE → 头像不会被应用！
  ...
  t25  settings.js 加载执行 → applyAvatar() 才被定义
```

`checkAuth()` 是 async IIFE，在 `await fetch()` 恢复执行时，如果 `settings.js`（第 26 个脚本）尚未加载，`applyAvatar` 就是 `undefined`。

**关键代码 (app.js:26):**
```javascript
if (typeof applyAvatar === 'function') applyAvatar();
```

这个 typeof 检查本意是防御性编程，但实际上掩盖了竞态条件——当条件为 false 时静默跳过，不会重试。

**为什么没有兜底？**
`applyAvatar()` 的所有调用点：
| 调用位置 | 触发时机 | 页面加载时会执行？ |
|---------|---------|------------------|
| `app.js:26` checkAuth() | 页面加载（异步） | ⚠ 可能失败（竞态） |
| `settings.js:21` loadSettingsData() | 用户打开设置页 | ❌ 不会自动执行 |
| `settings.js:87` selectPresetAvatar() | 用户点击预设头像 | ❌ 用户交互触发 |
| `settings.js:117` handleAvatarUpload() | 用户上传头像 | ❌ 用户交互触发 |

**结论：** 页面加载后，没有任何兜底机制保证 `applyAvatar()` 一定会执行。如果 `checkAuth()` 那一次调用失败，头像就永远是 "B"，直到用户手动进入设置页。

---

### BUG-2（关联）：登录页不保存头像到 localStorage

**相关文件:**
- `frontend/login.html:235-241`
- `server/src/auth.rs:468-488` — 登录响应包含 avatar 字段

**后端登录响应确实返回了 avatar:**
```rust
// auth.rs:472-482
user: Some(UserInfo {
    id: user_id,
    username,
    display_name,
    avatar: if avatar.is_empty() { None } else { Some(avatar) },
}),
```

**但前端登录页只保存了 username 和 displayName，完全忽略了 avatar:**
```javascript
// login.html:235-241
localStorage.setItem('loggedIn', 'true');
localStorage.setItem('username', data.user.username);
if (data.user.display_name) {
    localStorage.setItem('displayName', data.user.display_name);
}
// ❌ 缺少: localStorage.setItem('userAvatar', data.user.avatar);
window.location.href = '/';
```

**影响：** 用户登录后跳转到首页，此时 localStorage 没有 `userAvatar`。虽然 `checkAuth()` 会重新从服务器同步，但又受 BUG-1 的竞态影响。

---

### BUG-3（潜在）：头像 API 保存失败被静默吞掉

**相关文件:**
- `frontend/assets/js/settings.js:89,119`

```javascript
// 预设头像选择
API.updateAvatar(value).catch(function() {});   // ← 错误被静默吞掉

// 自定义上传
API.updateAvatar(dataURL).catch(function() {}); // ← 同上
```

**影响：** 如果 `PUT /api/auth/avatar` 请求失败（网络抖动、session 过期、服务器错误），用户在当前页面看到头像已更新（因为 localStorage 已写入），但服务器没有保存。下次刷新页面或换设备后，头像丢失。用户完全不知道保存失败了。

---

### BUG-4（潜在）：服务器无头像时不清理本地缓存

**相关文件:**
- `frontend/assets/js/app.js:22-25`

```javascript
// 只在服务器有头像时同步，不清理
if (data.user.avatar) {
    localStorage.setItem('userAvatar', data.user.avatar);
}
// ❌ 缺少 else { localStorage.removeItem('userAvatar'); }
```

**影响：** 如果用户在设备 A 重置了头像，设备 B 的 localStorage 中仍保留着旧头像，不会被清除。这种双向同步的不一致会导致多设备使用时的困惑。

---

## 三、举一反三 — 类似模式检查

### 3.1 同类竞态风险

项目中存在多处 "早期脚本异步调用晚期脚本函数" 的模式：

| 调用点 | 被调函数 | 风险 |
|-------|---------|------|
| `app.js:26` | `applyAvatar()` (settings.js) | **高** — 已确认 BUG |
| `app.js:125` switchPage() | `loadItems()` (tasks.js) | 低 — 用户交互触发，scripts 已加载 |
| `app.js:137` switchPage() | `loadSettingsData()` (settings.js) | 低 — 同上 |

`checkAuth()` 是唯一在脚本加载期间异步执行的 IIFE，所以目前只有 `applyAvatar()` 受影响。但这种模式本身是脆弱的。

### 3.2 阿宝聊天头像硬编码 preset 映射

**相关文件:**
- `frontend/assets/js/abao.js:537-550`

```javascript
function createUserAvatarContent() {
    var avatarValue = localStorage.getItem('userAvatar') || '';
    if (avatarValue && avatarValue.startsWith('data:image/')) {
        return '<img src="' + avatarValue + '">';
    }
    // ⚠ 硬编码的 preset 映射，和 settings.js 中的 AVATAR_PRESETS 重复
    var presets = {
        'preset:cat': 'assets/images/preset-cat.png',
        'preset:panda': 'assets/images/preset-panda.png'
    };
    if (presets[avatarValue]) {
        return '<img src="' + presets[avatarValue] + '">';
    }
    return window._userInitial || 'B';
}
```

**问题：** 如果后续新增预设头像（如 `preset:dog`），需要同时改 `settings.js` 和 `abao.js` 两个地方，容易遗漏。且 `abao.js` 不识别渐变色类型（`color:blue` 等），渐变色头像在阿宝聊天中只会显示首字母。

### 3.3 好友列表 / 分享弹窗不显示自定义头像

**相关文件:**
- `frontend/assets/js/friends.js:63-74`
- `frontend/assets/js/share-modal.js:58-65,152-164,182-190`

好友列表和分享弹窗中，所有用户只显示首字母 + 固定渐变色，不读取对方的自定义头像。虽然这需要后端提供其他用户的 avatar 字段，但目前连当前用户自己在好友列表中的头像也是固定渐变色，与 header 头像不一致。

---

## 四、修复建议（已全部修复 2026-03-02）

### ~~修复 BUG-1~~（已修复）：在 settings.js 末尾添加自启动

在 `settings.js` 文件末尾（`applyAvatar()` 定义之后），立即执行一次头像应用：

```javascript
// settings.js 末尾追加
// 页面加载完成后立即应用头像（兜底 checkAuth 中的竞态）
applyAvatar();
```

这样无论 `checkAuth()` 是否赶上时机，`settings.js` 加载后都会立即应用头像。

### ~~修复 BUG-2~~（已修复）：登录页保存 avatar

```javascript
// login.html 中补充
if (data.user.avatar) {
    localStorage.setItem('userAvatar', data.user.avatar);
}
```

### ~~修复 BUG-3~~（已修复）：API 保存失败给用户反馈

```javascript
API.updateAvatar(value).catch(function() {
    showToast('头像同步到服务器失败，请稍后重试', 'error');
});
```

### ~~修复 BUG-4~~（已修复）：补充双向同步

```javascript
if (data.user.avatar) {
    localStorage.setItem('userAvatar', data.user.avatar);
} else {
    localStorage.removeItem('userAvatar');
}
```

---

## 五、测试用例

### TC-A01: 基本头像显示（页面加载）

| # | 操作 | 预期结果 |
|---|------|---------|
| 1 | 在设置页选择预设头像（如猫咪），确认 header 头像已更新 | header 显示猫咪图片 |
| 2 | 刷新页面（F5） | header 头像仍显示猫咪图片，不应闪回 "B" |
| 3 | 强制刷新（Ctrl+Shift+R 清除缓存） | 头像仍显示猫咪图片 |
| 4 | 关闭浏览器标签，重新打开页面 | 头像仍显示猫咪图片 |

### TC-A02: 预设头像选择

| # | 操作 | 预期结果 |
|---|------|---------|
| 1 | 进入设置页，点击"蓝色渐变"预设 | header + 设置预览均显示蓝紫渐变 + 首字母 |
| 2 | 点击"猫咪"预设 | header + 设置预览均显示猫咪图片 |
| 3 | 点击"熊猫"预设 | header + 设置预览均显示熊猫图片 |
| 4 | 点击"绿色渐变"预设 | header + 设置预览均显示绿色渐变 + 首字母 |
| 5 | 点击"橙色渐变"预设 | header + 设置预览均显示橙色渐变 + 首字母 |
| 6 | 点击"粉色渐变"预设 | header + 设置预览均显示粉色渐变 + 首字母 |
| 7 | 选中的预设应有高亮边框 | 当前选中的预设有 `.selected` 样式 |

### TC-A03: 自定义上传头像

| # | 操作 | 预期结果 |
|---|------|---------|
| 1 | 点击"上传头像"，选择一张 JPG 图片 | header + 设置预览显示压缩后的圆形头像 |
| 2 | 上传一张 PNG 透明背景图片 | 正常显示（转为 JPEG） |
| 3 | 上传一张非正方形图片（如 1920x1080） | 居中裁切为正方形，无变形 |
| 4 | 上传一张极小图片（如 16x16） | 放大到 128x128 后显示 |
| 5 | 上传后刷新页面 | 头像仍为上传的图片 |
| 6 | 选择同一张图片再次上传 | 正常触发（input 已 reset） |

### TC-A04: 头像跨会话持久化

| # | 操作 | 预期结果 |
|---|------|---------|
| 1 | 设置头像为"熊猫" → 退出登录 → 重新登录 | 登录后头像为熊猫（从服务器同步） |
| 2 | 设备 A 设置头像为"猫咪" → 设备 B 刷新页面 | 设备 B 显示猫咪头像 |
| 3 | 清除浏览器 localStorage → 刷新页面 | 从服务器重新同步头像并显示 |
| 4 | 使用隐私/无痕模式打开页面并登录 | 头像从服务器加载并正确显示 |

### TC-A05: 头像在各组件中的一致性

| # | 检查位置 | 预期结果 |
|---|---------|---------|
| 1 | 右上角 header 头像 | 显示用户设置的头像 |
| 2 | 设置页头像预览 | 与 header 一致 |
| 3 | 阿宝聊天（手机端）用户消息头像 | 与 header 一致 |
| 4 | 切换页面（Todo→英语→生活→设置）后回到 Todo | header 头像不变 |

### TC-A06: 头像 API 错误处理

| # | 操作 | 预期结果 |
|---|------|---------|
| 1 | 在弱网环境下选择预设头像 | 本地立即更新；如果 API 失败，应提示用户 |
| 2 | Session 过期时修改头像 | 跳转到登录页（API 401 处理） |
| 3 | 上传超大图片（>5MB 原图） | canvas 压缩为 128x128 JPEG，应正常 |
| 4 | localStorage 已满时上传头像 | 提示"图片太大，保存失败" |

### TC-A07: 头像默认状态

| # | 操作 | 预期结果 |
|---|------|---------|
| 1 | 新注册用户首次登录 | 显示蓝紫渐变 + 用户名首字母 |
| 2 | 用户 display_name 为 "Boris" | 默认首字母为 "B" |
| 3 | 用户 display_name 为中文 "小明" | 默认首字母为 "小" |
| 4 | 用户无 display_name，username 为 "alice" | 默认首字母为 "A" |

### TC-A08: 页面加载时序（回归重点）

| # | 场景 | 预期结果 |
|---|------|---------|
| 1 | 正常网络下刷新页面 | 头像在页面可见后 1 秒内显示正确 |
| 2 | 使用 Chrome DevTools Throttling 设为 "Slow 3G" | 头像仍能正确显示（可接受短暂闪烁） |
| 3 | 使用 DevTools 禁用缓存后刷新 | 头像正确显示 |
| 4 | ServiceWorker 缓存命中（离线状态） | 如果 localStorage 有值，头像正确显示 |

---

## 六、影响范围

| 影响项 | 严重程度 | 说明 |
|-------|---------|------|
| Header 头像显示 | **高** | 每次打开页面都可能看到错误的默认头像 |
| 设置页头像预览 | 低 | 打开设置页时会调用 applyAvatar()，能正确显示 |
| 阿宝聊天头像 | 中 | 依赖 localStorage，受 BUG-1 间接影响 |
| 多设备同步 | 中 | BUG-4 导致头像不一致 |
| 数据安全 | 低 | 头像数据（base64）不涉及敏感信息 |
