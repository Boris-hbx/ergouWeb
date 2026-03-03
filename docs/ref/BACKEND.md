# Backend (Rust / Axum)

> 后端模块职责、添加新功能指南、关键依赖
> 最后更新: 2026-02-21

## 目录结构

```
server/
├── Cargo.toml              # 依赖管理
├── src/
│   ├── main.rs             # 入口：路由注册、中间件、ServeDir、备份调度、提醒轮询
│   ├── auth.rs             # 认证：注册/登录/登出/改密码/头像、UserId Extractor
│   ├── db.rs               # SQLite 初始化、CREATE TABLE、迁移、每日备份
│   ├── state.rs            # AppState { db, moment_cache }
│   ├── models/
│   │   ├── mod.rs          # 模块导出
│   │   ├── todo.rs         # Todo 序列化/反序列化结构体
│   │   ├── routine.rs      # Routine 结构体
│   │   ├── review.rs       # Review 结构体
│   │   └── conversation.rs # Conversation/ChatMessage 结构体
│   ├── routes/
│   │   ├── mod.rs          # 路由导出 (15 个模块)
│   │   ├── todos.rs        # Todo CRUD: list/create/get/update/delete/restore/batch/counts
│   │   ├── routines.rs     # Routine: list/create/delete/toggle
│   │   ├── reviews.rs      # Review: list/create/update/delete/complete/uncomplete
│   │   ├── quotes.rs       # 随机名言（读 data/quotes.txt）
│   │   ├── chat.rs         # 阿宝聊天入口 → ClaudeClient
│   │   ├── conversations.rs# 对话列表/消息/删除/重命名/使用量
│   │   ├── english.rs      # 英语场景 CRUD + AI 生成
│   │   ├── friends.rs      # 好友 + 请求 + 搜索 + 分享收件箱
│   │   ├── reminders.rs    # 提醒 CRUD + acknowledge/snooze/pending-count
│   │   ├── push.rs         # VAPID 公钥 + Push 订阅/取消
│   │   ├── notifications.rs# 应用内通知 unread/read/read-all
│   │   ├── contacts.rs     # 联系人 CRUD
│   │   ├── collaborate.rs  # Todo 协作 + 确认流
│   │   ├── routine_collab.rs # Routine 协作
│   │   └── moment.rs       # 此刻文案 (AI 生成 + 缓存)
│   └── services/
│       ├── mod.rs          # 服务导出 (6 个模块)
│       ├── claude.rs       # Claude API 客户端 (chat + simple_generate)
│       ├── context.rs      # 系统 Prompt 构建 + 任务上下文注入 + Moment 上下文
│       ├── tool_executor.rs# AI Tool 实现 (16 个 tools)
│       ├── push.rs         # Web Push: VAPID 签名、内容加密 (AES-GCM + ECDH)
│       ├── reminder_poller.rs # 后台提醒轮询 (每 30s)
│       └── collaboration.rs# 协作逻辑、确认流程
└── data/                   # 本地开发数据（.gitignore）
```

## 添加新路由步骤

1. **定义 Handler** — 在 `routes/` 下新建或追加函数：
```rust
pub async fn my_handler(
    State(state): State<AppState>,  // 数据库访问
    user_id: UserId,                // 自动认证（声明即启用）
    Json(req): Json<MyRequest>,     // 请求体
) -> impl IntoResponse {
    let db = state.db.lock().unwrap();
    // 业务逻辑...
    Json(json!({ "success": true }))
}
```

2. **注册路由** — `main.rs` 中添加到对应的 Router：
```rust
let my_routes = Router::new()
    .route("/", get(routes::my_module::list).post(routes::my_module::create))
    .route("/{id}", put(routes::my_module::update).delete(routes::my_module::delete));

// 嵌套到 api_routes
let api_routes = Router::new()
    .nest("/my-resource", my_routes)
    // ...existing nests
```

3. **导出模块** — `routes/mod.rs` 加 `pub mod my_module;`

4. **建表**（如需）— `db.rs` 的 `create_tables()` 追加 CREATE TABLE

## AppState

```rust
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    /// Cache for moment text: user_id -> (text, timestamp)
    pub moment_cache: Arc<Mutex<HashMap<String, (String, chrono::DateTime<chrono::Utc>)>>>,
}
```

所有 Handler 通过 `State(state): State<AppState>` 注入。数据库操作需先 `state.db.lock().unwrap()` 获取连接。

**注意**: `Mutex` 意味着同一时刻只有一个请求能写库。对于当前用户规模足够。

## UserId Extractor

在 Handler 参数中声明 `user_id: UserId` 即自动启用认证。未登录请求会被拦截返回 401。

Auth 相关路由（register/login）不需要 UserId，直接处理。

## 后台任务

| 任务 | 启动方式 | 间隔 | 职责 |
|------|---------|------|------|
| ReminderPoller | `tokio::spawn` in `main.rs` | 30s | 检查到期提醒 → 触发 + Push + 通知 |
| DailyBackup | `tokio::spawn` in `main.rs` | 1h (检查) | 每天首次 VACUUM INTO 备份 |

## 关键依赖

| Crate | 用途 |
|-------|------|
| `axum 0.8` | Web 框架 |
| `tokio 1` (full) | 异步运行时 |
| `rusqlite 0.32` (bundled) | SQLite，自带编译 |
| `argon2 0.5` | 密码哈希 |
| `reqwest 0.12` (json) | HTTP 客户端（Claude API + Push） |
| `serde / serde_json` | 序列化 |
| `chrono 0.4` (serde) | 日期时间 |
| `uuid 1` (v4) | ID 生成 |
| `tower-http 0.6` | ServeDir 静态文件、CORS、Header |
| `axum-extra 0.10` | Cookie 处理 |
| `hex 0.4` | Session Token 编码 |
| `rand 0.9` | 随机数（Salt、Token） |
| `p256 0.13` (ecdsa, ecdh) | VAPID 签名 + ECDH 密钥交换 |
| `aes-gcm 0.10` | Push 内容加密 |
| `hkdf 0.12` + `sha2 0.10` | Push 密钥派生 |
| `base64 0.22` | Base64 编解码 |
| `url 2` | URL 解析 |

## Release 优化

```toml
[profile.release]
panic = "abort"       # 不生成 unwind 代码
codegen-units = 1     # 全程序优化
lto = true            # 链接时优化
opt-level = "s"       # 优化体积
strip = true          # 去除符号
```

产出: ~10MB 二进制，内存占用 <50MB。

## 日志规范

使用 `tracing` + `tracing-subscriber`，由 `TraceLayer` 中间件自动记录每个 HTTP 请求。

### 日志级别

| 级别 | 用途 | 示例 |
|------|------|------|
| `error!` | 需要排查的故障 | DB 错误、外部 API 失败 |
| `warn!` | 可恢复的异常 | 限流触发、token 过期 |
| `info!` | 关键业务事件 | 用户注册、AI 调用完成 |
| `debug!` | 开发调试 | SQL 参数、请求体详情 |

### 规则

- **新代码**必须用 `tracing` 宏（`info!`, `warn!`, `error!`），禁止 `eprintln!()`
- **旧代码**的 `eprintln!()` 不专门迁移，修改该文件时顺手替换
- 环境变量 `RUST_LOG` 控制日志级别，默认 `info`
- 生产环境使用紧凑格式，保持 `fly logs` 人眼可读
