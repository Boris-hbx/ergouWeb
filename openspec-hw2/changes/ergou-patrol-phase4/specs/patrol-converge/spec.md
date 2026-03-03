## ADDED Requirements

### Requirement: 收敛光弧动画

切到阿宝 tab 触发收敛时，系统应播放光弧飞回动画：

1. 所有爪印就地 fade out (0.15s)
2. 创建光点元素 `.patrol-arc`（8px 圆，主题色，发光效果）
3. 光点从二狗最后位置沿贝塞尔曲线飞向阿宝 logo
4. 飞行时长 0.4s，曲线 cubic-bezier(0.2, 0, 0, 1)
5. 到达后光点 scale→0 + opacity→0
6. 同时 logo 脉冲 scale 1→1.15→1 (0.3s)
7. 脉冲完成后 transition convergeDone

#### Scenario: 巡游中切到阿宝 tab
- **WHEN** _sm.state 为 walk/pause/rest
- **AND** 用户点击阿宝 tab
- **THEN** 播放完整收敛动画序列

---

### Requirement: Logo 微晃

从阿宝 tab 切走时，阿宝 logo 微晃：

- CSS class `.abao-wiggle` 控制
- 动画: rotate(0) → rotate(-3deg) → rotate(3deg) → rotate(0)，0.3s
- animationend 后移除 class

#### Scenario: 离开阿宝 tab
- **WHEN** 用户从阿宝 tab 切到其他 tab
- **THEN** 阿宝 logo 播放微晃动画
