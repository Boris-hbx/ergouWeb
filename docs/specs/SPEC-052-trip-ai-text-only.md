# SPEC-052: 差旅 AI 分析支持纯文字输入
> 起草日期: 2026-02-27
> 状态: 已完成

## 问题

用户在差旅模块添加条目时，如果只输入文字（无票据照片），点击"阿宝分析"按钮后 AI 不进行分析。

## 根因分析

后端 `trips.rs` 的 `analyze_item()` 无论是否有图片，统一调用 `vision_generate()`。当 `images` 为空时：

1. **Claude 路径**: `content` 数组只包含一个 `{"type":"text"}` 块，语义上可行但走了 vision 通道
2. **Kimi 路径**: `content` 为 `[{"type":"text","text":"..."}]` 数组格式。部分 OpenAI 兼容 API 对纯文本请求期望 `content` 为字符串而非数组，可能返回错误

核心问题：**纯文字请求不应走 vision 通道**，应使用 `simple_generate()`。

## 修复方案

### 后端 `server/src/routes/trips.rs`

在 `analyze_item()` 中，根据是否有图片选择不同的 LLM 调用方法：

```rust
// 现有代码（统一调用 vision_generate）:
match client.vision_generate(system, images, &user_message, 4096).await

// 改为：
let result = if has_images {
    client.vision_generate(system, images, &user_message, 4096).await
} else {
    client.simple_generate(system, &user_message, 4096).await
};
match result
```

### 变更范围

| 文件 | 改动 |
|------|------|
| `server/src/routes/trips.rs` | `analyze_item()` 中按 `has_images` 分支调用 |

### 不需要改动

- 前端 `trip.js`：`analyzeText()` 已正确支持纯文字提交
- `AnalyzeItemRequest` 结构体：`images` 已有 `#[serde(default)]`
- `simple_generate` / `vision_generate`：已有完整实现

## 测试用例

1. 只输入文字（如"北京飞上海 CA1234 2月28日 ¥800"），点击阿宝分析 → 应返回解析结果
2. 只上传票据照片 → 应正常分析（回归验证）
3. 同时输入文字 + 照片 → 应正常分析（回归验证）
4. 文字和照片都为空 → 应提示"请选择票据照片或粘贴行程信息"
