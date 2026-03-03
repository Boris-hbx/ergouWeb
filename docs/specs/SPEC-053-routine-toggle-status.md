# SPEC-053: 例行任务每日事项点击确认后状态不更新
> 起草日期: 2026-02-27
> 状态: 已完成

## 问题

用户在"例行"面板中点击"每日"事项的复选框后，UI 状态没有更新（不显示已完成样式）。

## 现状分析

### 前端 (`routines.js`)

`toggleRoutine()` 已实现乐观更新（commit `a8fcc9e`）：
1. 立即切换本地 `routines[idx].completed_today`
2. 调用 `renderRoutines()` 重新渲染
3. API 成功后用服务端数据同步
4. 失败时 `loadRoutines()` 回滚

代码逻辑无误。

### 后端 (`routines.rs`)

Owner 路径（行 329-402）：
1. 从 DB 读取 `completed_today` + `last_completed_date`
2. 计算当前是否已完成今天：`completed_int != 0 && last_date == today`
3. 取反 → 写回 DB → 返回新状态

**发现问题：行 387 有一个 `.unwrap()` 未处理**，如果 DB 写入失败会导致 panic（服务崩溃），但这不是 UI 不更新的直接原因。

### 可能的根因

1. **时区不一致**：后端用 `chrono::Local::now()` 计算 `today`，服务器 `TZ=Asia/Shanghai`。如果用户实际在北美（CAD 为默认币种），客户端日期与服务端可能差一天，导致 toggle 逻辑判定异常
2. **缓存旧 JS**：虽然版本号已递增到 `20260227d`，但浏览器可能缓存了 Service Worker 中的旧版本
3. **Service Worker 拦截**：`sw.js` 可能缓存了旧的 JS 文件，需要清理 SW 缓存

## 修复方案

### 1. 修复 `.unwrap()` (安全加固)

```rust
// 现有代码:
db.execute(...).unwrap();

// 改为:
if let Err(e) = db.execute(...) {
    eprintln!("[routines] toggle update failed: {}", e);
    return (StatusCode::INTERNAL_SERVER_ERROR, Json(RoutineResponse {
        success: false, item: None,
        message: Some("更新失败".into()),
    }));
}
```

### 2. 时区统一

将后端 `today` 计算从 `Local::now()` 改为使用固定时区，确保与用户预期一致：

```rust
// 现有:
let today = chrono::Local::now().format("%Y-%m-%d").to_string();

// 改为明确使用上海时区:
use chrono::FixedOffset;
let tz = FixedOffset::east_opt(8 * 3600).unwrap(); // UTC+8
let today = chrono::Utc::now().with_timezone(&tz).format("%Y-%m-%d").to_string();
```

### 3. 前端 Service Worker 版本检查

在 `sw.js` 中确认缓存版本号与 `index.html` 中的 `?v=` 参数一致，或在部署时强制 SW 更新。

### 变更范围

| 文件 | 改动 |
|------|------|
| `server/src/routes/routines.rs` | 修复 `.unwrap()` + 可选的时区修正 |
| `frontend/sw.js` | 确认缓存版本号 |

## 调试步骤

部署修复前，先确认问题根因：
1. 打开浏览器 DevTools → Network → 点击 routine checkbox → 检查 API 请求/响应
2. 确认返回的 `data.item.completed_today` 值是否正确
3. Console 中是否有 JS 报错
4. 检查 Application → Service Worker → 是否有旧版本缓存

## 测试用例

1. 点击未完成的每日事项 → 立即显示 ✓ + 半透明样式
2. 再次点击已完成事项 → 立即恢复为未完成样式
3. 刷新页面后状态保持一致
4. 跨日后（次日）已完成事项自动重置为未完成
