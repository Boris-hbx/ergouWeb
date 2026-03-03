## ADDED Requirements

### Requirement: 后端数据库操作不得 panic
所有路由处理器中的数据库操作（`db.prepare`、`db.execute`、`db.query_row`、`stmt.query_map` 等）SHALL NOT 使用 `.unwrap()` 或 `.expect()`。MUST 使用 `?` 操作符或 `match` 进行错误传播。

#### Scenario: 数据库查询返回错误
- **WHEN** `db.prepare("SELECT ...")` 返回 `Err(SqliteFailure)`
- **THEN** 处理器捕获错误，返回 HTTP 500 `{"success": false, "error": "内部错误"}`，服务器进程不中断

#### Scenario: 数据库写入返回错误
- **WHEN** `db.execute("INSERT ...")` 因唯一约束冲突返回 `Err`
- **THEN** 处理器返回适当的 HTTP 错误码（如 409），不发生 panic

#### Scenario: 高并发下 SQLITE_BUSY
- **WHEN** 多个请求并发写入，某个 `db.execute` 返回 `SQLITE_BUSY`
- **THEN** 处理器返回 HTTP 503 或 500，不崩溃，不阻塞其他请求

### Requirement: 后端错误不得静默吞掉
路由处理器中的关键操作（数据库写入、状态变更）SHALL NOT 使用 `.ok()` 忽略错误。MUST 记录错误日志并返回错误响应，或在非关键路径使用 `if let Err(e) = ... { eprintln!(...) }` 记录。

#### Scenario: 协作操作中的数据库错误
- **WHEN** `collaborate.rs` 中 `db.execute("UPDATE ...")` 返回错误，当前使用 `.ok()` 忽略
- **THEN** 系统记录 `eprintln!("[collaborate] update failed: {}", e)` 并返回错误响应

#### Scenario: 后台 session 清理失败
- **WHEN** session 清理任务的 `db.execute("DELETE ...")` 返回错误
- **THEN** 系统记录 `eprintln!("[cleanup] session cleanup failed: {}", e)`，下次周期重试，不影响请求处理

### Requirement: 前端错误不得静默吞掉
前端 JavaScript 中所有 `catch` 块 SHALL NOT 为空。MUST 至少记录 `console.error` 并在用户可见操作中显示 `showToast` 提示。

#### Scenario: API 请求 catch 块
- **WHEN** 前端 `fetch` 调用在 `.catch(function(e) {})` 中捕获网络错误
- **THEN** catch 块执行 `console.error('[模块名]:', e)` 并显示 `showToast("操作失败", "error")`

#### Scenario: JSON 解析失败
- **WHEN** API 返回非 JSON 响应，`resp.json()` 抛出异常
- **THEN** catch 块记录错误详情，显示"服务器响应异常"提示

### Requirement: API 错误响应格式统一
所有后端 API 端点 SHALL 返回统一的错误 JSON 格式：`{"success": false, "error": "<错误代码或描述>"}`。SHALL NOT 返回裸 StatusCode（无 body）或不含 `success` 字段的 JSON。

#### Scenario: 鉴权失败
- **WHEN** 未登录用户访问需要认证的端点
- **THEN** 返回 HTTP 401 `{"success": false, "error": "未登录"}`

#### Scenario: 服务端内部错误
- **WHEN** 路由处理器遇到未预期的数据库错误
- **THEN** 返回 HTTP 500 `{"success": false, "error": "内部错误"}`，不暴露堆栈或 SQL 细节

#### Scenario: 请求参数校验失败
- **WHEN** 用户提交的 JSON body 缺少必填字段
- **THEN** 返回 HTTP 400 `{"success": false, "error": "缺少必填字段: xxx"}`
