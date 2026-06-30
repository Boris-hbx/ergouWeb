# TEST-praxis-contact-logs — Praxis 关系人交流记录 (T-287 / SPEC praxis §7)

> 表 `praxis_contact_logs` + `GET/POST /api/praxis/contacts/:id/logs`（全 `AdminUserId` 守卫）。
> 扩展 T-283 关系人：记录每次交流 + 历史时间线 + 回写最近联系时间/质量驱动节点状态。

## 后端 · API

| # | 场景 | 步骤 | 预期 |
|---|------|------|------|
| B1 | 守卫 | 普通 user `GET /api/praxis/contacts/1/logs` | 403 |
| B2 | 列表归属 | admin 查不属于自己的 contact 的 logs | 404「未找到关系人」 |
| B3 | 新建 | admin `POST {at,method,quality,content,note}` | 200，返回 item + `contactUpdated` |
| B4 | 字段命名 | 返回 | 驼峰 `contactId/createdAt`，无下划线 |
| B5 | 回写·更晚 | contact 无/较早 last_contact_at，提交更晚的交流 | `contactUpdated=true`；该 contact `lastContactAt=at`、`lastQuality=quality` |
| B6 | 回写·更早 | 已有较晚 last_contact_at，提交更早的交流 | `contactUpdated=false`；contact 不被覆盖 |
| B7 | 倒序 | 列出 logs | 按 `at` 倒序（最新在前） |
| B8 | 数据隔离 | 别的 admin 访问该 contact logs | 404（contact 不归他） |
| B9 | 校验·at | `POST {at:"  "}` | 400 |
| B10 | 校验·quality | `POST {at:..., quality:"meh"}` | 400 |
| B11 | 双注册 | 同测试在 lib.rs + main.rs 两 binary | 均通过 |

## 前端 · 关系详情交流记录

| # | 场景 | 预期 |
|---|------|------|
| F1 | 入口 | 点关系弧节点 → 详情下方出现「交流记录」区 + 「+ 记录交流」按钮 |
| F2 | 表单 | 点「+ 记录交流」展开：时间(默认今天)/方式(面对面/电话/微信/会议/邮件/其他)/质量(浅/有效/深度)/聊了什么/心得 |
| F3 | 时间线 | 已有交流倒序展示：日期 + 方式 + 质量徽标 + 摘要；心得用 `<details>` 可展开 |
| F4 | 保存驱动状态 | 保存一条更晚的深度交流 → 弧重渲，该节点状态变化（dim→solid 等），最近联系/质量同步 |
| F5 | 空态 | 无交流记录显示「还没有交流记录，点『+ 记录交流』开始」 |
| F6 | 新建关系人 | 新建（未保存）态不显示交流记录区（无 id 不可记录） |

## 边界（v0.3 不做）

- AI 自动摘要/建议话题、关系阶段复盘、隐私分级、从今日经营自动生成交流记录（spec §7 边界）。
- 交流记录暂不支持编辑/删除（v0.2 只追加）。
