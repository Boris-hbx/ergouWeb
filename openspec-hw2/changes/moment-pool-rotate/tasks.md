## 1. 后端缓存结构

- [x] 1.1 修改 `state.rs` 中 `MomentCache` 类型：从 `HashMap<String, (String, DateTime<Utc>)>` 改为 `HashMap<String, (Vec<String>, NaiveDate)>`

## 2. 后端批量生成

- [x] 2.1 修改 `context.rs` 中 `build_moment_system_prompt()`：调整为批量生成 prompt，要求输出 30 条 JSON 数组格式
- [x] 2.2 重写 `moment.rs` 中 `get_moment()` handler：缓存判断改为按 NaiveDate 比较（用户时区），首次请求调 LLM 生成池
- [x] 2.3 添加 JSON 数组解析逻辑：正则提取 `[...]`，serde 解析，逐行 fallback，三层容错
- [x] 2.4 扩展 `fallback_greeting()` 为 `fallback_pool(hour)`：返回 ~10 条基于时段的固定问候语 Vec
- [x] 2.5 修改 API 响应结构：返回 `{ success, pool, text, generated_at, cached, fallback? }`

## 3. 前端 Moment 模块改造

- [x] 3.1 重写 `app.js` 中 Moment.load()：先检查 localStorage `momentPool` 日期，命中则用本地池，否则请求后端
- [x] 3.2 实现 `Moment.rotate()`：shuffle + index 指针轮换，淡出-淡入过渡动画
- [x] 3.3 实现 localStorage 读写：存取 `{ pool, date }` 结构
- [x] 3.4 移除 15 分钟自动刷新定时器（不再需要定期请求后端）

## 4. 前端交互绑定

- [x] 4.1 给 `#moment-icon` 添加 click 事件，调用 `Moment.rotate()`，添加 `cursor: pointer` 样式
- [x] 4.2 在 `refreshCurrentPage()` 中调用 `Moment.rotate()`

## 5. 样式与动画

- [x] 5.1 为 `moment-text` 添加淡出-淡入切换动画（opacity transition 或 class toggle）

## 6. 验证

- [x] 6.1 测试首次加载：无本地缓存 → 请求后端 → 显示一条 → localStorage 写入池
- [x] 6.2 测试刷新页面：本地池日期匹配 → 直接从池取一条，无网络请求
- [x] 6.3 测试点击天气图标：换一条，不重复上一条，有过渡动画
- [x] 6.4 测试点击刷新按钮：页面数据刷新 + 换一条一言
- [x] 6.5 测试跨天：日期不匹配 → 重新请求后端获取新池
- [x] 6.6 测试 LLM 失败：返回 fallback 池，前端正常轮换
- [x] 6.7 测试访客配额不足：返回 fallback 池，不消耗配额
