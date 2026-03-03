## ADDED Requirements

### Requirement: 后端按天批量生成候选池
系统 SHALL 在每用户每天首次请求 `GET /api/moment` 时，调用 LLM 一次性生成约 30 条每日一言候选，以 JSON 数组形式返回给前端。同一自然日内的后续请求 SHALL 直接返回缓存的池，不再调用 LLM。

#### Scenario: 当天首次请求生成池
- **WHEN** 用户当天首次请求 `GET /api/moment`（内存缓存无该用户当天数据）
- **THEN** 后端构建用户上下文（任务数/完成数/紧急/逾期），调用 LLM 生成 ~30 条候选
- **THEN** 返回 `{ "success": true, "pool": ["...", "..."], "generated_at": "2026-03-02" }`

#### Scenario: 当天重复请求命中缓存
- **WHEN** 用户当天已生成过池，再次请求 `GET /api/moment`
- **THEN** 后端直接返回缓存的池，不调用 LLM
- **THEN** 响应中包含 `"cached": true`

#### Scenario: 跨天缓存失效
- **WHEN** 用户请求时，缓存中的池 `generated_at` 日期与当前日期（用户时区）不一致
- **THEN** 后端视为缓存未命中，重新生成新池

### Requirement: LLM 批量生成 prompt 格式
系统 SHALL 使用专门的批量生成 prompt，要求 LLM 以 JSON 数组格式一次性输出所有候选。每条候选 SHALL 不超过 10 个汉字，风格保持"毒舌损友"二狗人设。

#### Scenario: prompt 包含用户上下文
- **WHEN** 构建批量生成 prompt
- **THEN** prompt 中 SHALL 包含：用户名、当前时段、今日任务数/完成数/紧急数/逾期数
- **THEN** 要求 LLM 输出纯 JSON 数组 `["条目1", "条目2", ...]`，不含多余文字

#### Scenario: 输出多样性要求
- **WHEN** LLM 生成候选池
- **THEN** 候选内容 SHALL 涵盖多种类型：鼓励、调侃、催促、吐槽、关心等，避免重复表达

### Requirement: 生成失败时使用 fallback 池
当 LLM 调用失败（超时、网络错误、响应格式异常）时，系统 SHALL 返回预定义的 fallback 问候语池，确保前端始终能拿到可用数据。

#### Scenario: LLM 超时或网络错误
- **WHEN** LLM 调用在 15 秒内未返回或网络异常
- **THEN** 后端返回预定义的 fallback 池（~10 条基于时段的通用问候语）
- **THEN** 响应中 `"fallback": true`

#### Scenario: LLM 返回格式无法解析
- **WHEN** LLM 返回的内容无法解析为字符串数组
- **THEN** 后端尝试从返回文本中逐行提取有效条目
- **THEN** 若提取结果少于 5 条，使用 fallback 池

#### Scenario: 访客用户 AI 配额不足
- **WHEN** 访客用户请求且 AI 配额已耗尽
- **THEN** 直接返回 fallback 池，不消耗配额

### Requirement: 前端本地缓存候选池
前端 SHALL 将后端返回的候选池缓存到 `localStorage`，附带生成日期。后续展示和轮换 SHALL 从本地池读取，无需网络请求。

#### Scenario: 首次加载写入本地缓存
- **WHEN** 前端从 `GET /api/moment` 获取到池数据
- **THEN** 存入 `localStorage`，key 为 `momentPool`，值包含 `pool` 数组和 `date` 字段

#### Scenario: 页面加载时检查本地缓存
- **WHEN** 页面加载，前端初始化 Moment 模块
- **THEN** 检查 `localStorage` 中 `momentPool.date` 是否为今天
- **THEN** 若匹配，直接从本地池取一条显示；若不匹配，请求后端获取新池

### Requirement: 点击天气图标换一条
用户 SHALL 可以通过点击顶栏天气图标从本地池中切换到另一条每日一言，实现零延迟轮换。

#### Scenario: 点击天气图标触发轮换
- **WHEN** 用户点击顶栏天气图标（`moment-icon`）
- **THEN** 前端从本地池中随机选取一条（不与当前显示的重复）
- **THEN** 文字以淡出-淡入过渡动画切换

#### Scenario: 池中仅一条时点击无效
- **WHEN** 本地池中只有一条候选，用户点击天气图标
- **THEN** 保持当前显示不变，不触发动画

### Requirement: 刷新按钮同时换一条
用户点击顶栏刷新按钮时，除执行原有的页面数据刷新外，SHALL 同时从本地池中切换一条每日一言。

#### Scenario: 刷新按钮触发双重行为
- **WHEN** 用户点击刷新按钮
- **THEN** 执行原有页面数据刷新逻辑（loadItems/loadReviews 等）
- **THEN** 同时从本地池中随机取另一条每日一言显示

### Requirement: API 响应格式兼容
`GET /api/moment` SHALL 同时返回 `pool` 数组和 `text` 字段（池中第一条），兼容可能存在的旧客户端。

#### Scenario: 正常响应包含两种格式
- **WHEN** 后端成功生成或返回缓存池
- **THEN** 响应 SHALL 包含 `{ "success": true, "pool": [...], "text": "池中第一条", "generated_at": "YYYY-MM-DD" }`
