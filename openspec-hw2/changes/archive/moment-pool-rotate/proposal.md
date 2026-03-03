## Why

当前每日一言每 15 分钟缓存过期后要实时调 LLM，用户点刷新也会触发 API 调用。一天下来每个活跃用户可能产生 10+ 次 LLM 请求，token 开销高且响应延迟明显。改为每天一次性批量生成候选池，前端本地随机轮换，能把 LLM 调用从"每次展示"降到"每天一次"，同时让刷新/点击天气图标时即时换一条，体验更好。

## What Changes

- 后端新增"每日一言池"生成逻辑：每天首次请求时一次性生成 ~30 条候选（不需要 200 条，30 条足够一天轮换且 prompt 更短、质量更高）
- 后端缓存从"单条 15 分钟"改为"整池 24 小时"，同一天内不再重复调 LLM
- 前端收到整个池子后存本地，点击天气图标或刷新按钮时从池中随机取下一条，无需网络请求
- 前端 Moment 模块增加"点击天气图标换一条"交互
- API 响应结构从 `{ text }` 改为 `{ pool: [...], generated_at }`

## Capabilities

### New Capabilities
- `moment-pool`: 每日一言候选池的批量生成、缓存、分发，以及前端本地轮换逻辑

### Modified Capabilities
（无现有 openspec spec 需要修改）

## Impact

- **后端**: `server/src/routes/moment.rs` — 重写生成逻辑，一次生成 30 条；`server/src/services/context.rs` — prompt 调整为批量输出格式；`server/src/state.rs` — 缓存结构从单条改为池
- **前端**: `frontend/assets/js/app.js` Moment 模块 — 接收池、本地存储、随机轮换；天气图标增加点击事件
- **API**: `GET /api/moment` 响应结构变化（新增 `pool` 字段，保留 `text` 做兼容）
- **Token 成本**: 从 ~10 次/天/用户 降到 1 次/天/用户，单次 token 略多（~800 output tokens）但总量大幅下降
