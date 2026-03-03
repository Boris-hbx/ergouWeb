## MODIFIED Requirements

### Requirement: PawPool.fadeWave 实现波浪式淡出

PawPool.fadeWave(duration, interval) 应实现真正的波浪淡出效果（Phase 0 为 TODO 占位）：

- 按放置顺序从最近到最远依次触发 fade
- 每个爪印间隔 `interval` ms（默认 50ms）
- 每个爪印 fade 时长 `duration` ms（默认 150ms）
- fade 完成后自动 release 回池

#### Scenario: 波浪式淡出
- **WHEN** 调用 fadeWave(150, 50) 且池中有 5 个活跃爪印
- **THEN** 第 1 个爪印立即 fade，第 2 个 50ms 后 fade，...第 5 个 200ms 后 fade
- **AND** 所有爪印 fade 完成后 release 回池

---

### Requirement: PawPool 追踪放置顺序

PawPool 应维护爪印的放置顺序（FIFO），用于：

- fadeWave 按顺序淡出
- 满池时可知道最老的爪印

实现方式：维护一个有序数组记录活跃爪印的 acquire 顺序。

#### Scenario: 满池时获取最老爪印
- **WHEN** 池满（8/8），新 step() 调用
- **THEN** 返回 null（由上层决定是否强制释放最老的）
