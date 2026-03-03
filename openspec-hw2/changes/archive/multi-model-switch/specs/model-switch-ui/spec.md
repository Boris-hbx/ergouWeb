## ADDED Requirements

### Requirement: Model selection buttons

设置页 AI 模型区域 SHALL 显示三个可交互的模型选择按钮：

| data-model | 显示名称 | 描述 |
|------------|---------|------|
| `auto` | 自动 | 智能选择最优模型 |
| `claude` | Claude | Anthropic 旗舰模型 |
| `doubao` | 豆包 | 火山引擎大模型 |

当前选中的模型按钮 SHALL 有 `active` class 高亮。按钮移除 `disabled` 属性和 `ai-model-disabled` class。

#### Scenario: 设置页加载时高亮当前模型
- **WHEN** 用户打开设置页
- **THEN** 系统调用 `GET /api/settings/ai-model` 获取当前偏好，对应按钮显示 `active` 高亮状态

#### Scenario: 首次用户看到默认选择
- **WHEN** 用户从未切换过模型（默认 "auto"）
- **THEN** "自动" 按钮显示为 active 状态

### Requirement: Model switch interaction

用户点击模型按钮时，系统 SHALL 执行乐观更新：
1. 立即高亮新按钮、取消旧按钮高亮
2. 调用 `PUT /api/settings/ai-model` 保存
3. 成功后显示 toast "已切换到 {模型名}"
4. 失败时回退高亮到之前的选择，显示 toast "保存失败"

#### Scenario: 成功切换模型
- **WHEN** 用户点击 "豆包" 按钮（当前选择为 "自动"）
- **THEN** "豆包" 按钮立即高亮，"自动" 按钮取消高亮，API 保存成功后显示 toast "已切换到 豆包"

#### Scenario: 切换失败回退
- **WHEN** 用户点击模型按钮但 API 请求失败（网络错误或 400）
- **THEN** 按钮高亮回退到之前的选择，显示 toast "保存失败"

#### Scenario: 点击已选中的按钮
- **WHEN** 用户点击已有 `active` class 的按钮
- **THEN** 不发送 API 请求，无变化

### Requirement: Guest mode restriction

Guest 用户 SHALL 不能切换模型。

- 模型按钮显示为禁用状态（`disabled` + `ai-model-disabled` class）
- 区域描述文字改为 "注册后可切换模型"
- 不调用 `GET /api/settings/ai-model`

#### Scenario: Guest 用户看到禁用状态
- **WHEN** guest 用户打开设置页
- **THEN** AI 模型按钮全部禁用，显示提示 "注册后可切换模型"

#### Scenario: Guest 点击模型按钮
- **WHEN** guest 用户点击任何模型按钮
- **THEN** 无反应（按钮为 disabled 状态）
