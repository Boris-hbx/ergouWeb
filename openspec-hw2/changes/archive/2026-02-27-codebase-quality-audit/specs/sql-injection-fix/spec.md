## ADDED Requirements

### Requirement: 工具调用查询参数白名单化
系统 SHALL 对 `tool_get_statistics` 的 `period` 参数使用白名单映射，将允许的值（如 `"today"`、`"week"`、`"month"`、`"all"`）映射为预定义的参数化 WHERE 子句。不在白名单中的值 SHALL 被拒绝或回退到默认值，不得拼接进 SQL。

#### Scenario: period 值在白名单中
- **WHEN** AI 工具调用 `tool_get_statistics`，`period` = `"week"`
- **THEN** 系统使用预定义的参数化查询 `WHERE created_at >= ?`，绑定计算后的日期值

#### Scenario: period 值不在白名单中
- **WHEN** AI 工具调用 `tool_get_statistics`，`period` = `"1; DROP TABLE todos--"`
- **THEN** 系统拒绝该值，返回工具错误信息"无效的时间范围"，不执行任何 SQL

#### Scenario: period 值为空
- **WHEN** AI 工具调用 `tool_get_statistics`，`period` 为空或未提供
- **THEN** 系统默认查询全部时间范围，使用无 WHERE 条件的参数化查询

### Requirement: 工具调用查询字段名硬编码
系统 SHALL 在 `tool_query_todos` 中仅允许硬编码的列名（`tab`、`quadrant`、`completed` 等）出现在 WHERE 子句中。系统 SHALL NOT 将用户提供的值用作列名或表名。

#### Scenario: 使用合法字段名过滤
- **WHEN** AI 工具调用 `tool_query_todos`，过滤条件为 `tab = "life"`
- **THEN** 系统使用 `WHERE tab = ?` 参数化查询，绑定 `"life"`

#### Scenario: 尝试注入非法字段名
- **WHEN** AI 工具调用 `tool_query_todos`，包含未知字段 `"1=1 OR"`
- **THEN** 系统忽略该字段，不将其纳入 WHERE 子句

### Requirement: admin 查询参数化
系统 SHALL 在 `admin.rs` 的 `query_ai_period` 中使用参数化日期查询，不得将日期表达式通过 `format!()` 拼接到 SQL 字符串中。

#### Scenario: 查询最近 7 天统计
- **WHEN** admin 调用 AI 用量统计，周期为最近 7 天
- **THEN** 系统使用 `WHERE created_at >= ?` 参数化查询，绑定计算后的日期字符串

### Requirement: 备份路径规范化验证
系统 SHALL 在执行 `VACUUM INTO` 前，对备份路径进行规范化（canonicalize），并验证结果路径在允许的备份目录内。包含 `..`、绝对路径覆盖或符号链接逃逸的路径 SHALL 被拒绝。

#### Scenario: 正常备份路径
- **WHEN** 备份目录为 `/data/backups`，文件名为 `backup-20260227.db`
- **THEN** 系统验证 `/data/backups/backup-20260227.db` 在允许目录内，执行备份

#### Scenario: 路径穿越尝试
- **WHEN** 备份路径包含 `../../etc/passwd`
- **THEN** 系统检测到路径逃出备份目录，记录警告日志，中止备份，不写入任何文件

### Requirement: IP 限流使用可信 header
系统 SHALL 使用 `Fly-Client-IP` header（由 Fly.io 代理设置，不可被客户端伪造）作为限流的客户端 IP 来源。仅在 `Fly-Client-IP` 不存在时回退到 `x-forwarded-for`。

#### Scenario: 请求经过 Fly.io 代理
- **WHEN** 请求包含 `Fly-Client-IP: 1.2.3.4` 和 `x-forwarded-for: 5.6.7.8`
- **THEN** 系统使用 `1.2.3.4` 作为限流 IP，忽略 `x-forwarded-for`

#### Scenario: 本地开发无 Fly-Client-IP
- **WHEN** 请求不包含 `Fly-Client-IP`，包含 `x-forwarded-for: 127.0.0.1`
- **THEN** 系统回退到 `x-forwarded-for` 的 `127.0.0.1`
