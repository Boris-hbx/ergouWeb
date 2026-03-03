## Use Cases

### Use Case: View paw prints during idle browsing

**Primary Actor:** Mobile user
**Scope:** Next 前端巡游系统
**Level:** User goal

**Stakeholders and Interests:**
- User — 浏览任务/记账/例行页面时，不被 AI 助手干扰，但能自然感知到"二狗在帮我看着"
- Product — 在非对话场景建立 AI 助手的存在感

**Preconditions:**
- 用户在非阿宝 tab 的页面
- 设备支持（非 `prefers-reduced-motion: reduce`）
- 巡游功能已开启（设置中未关闭）
- 冷却时间已结束

**Success Guarantee (Postconditions):**
- 爪印按行进方向出现并自然蒸发
- 页面操作不受任何影响（pointer-events: none）
- 主线程占用 < 1ms/帧

**Trigger:** 用户停止操作 8 秒（idle 阈值到达）

**Main Success Scenario:**
1. IdleDetector 检测到用户 8 秒无操作，且冷却时间已过。
2. 状态机从 home 转为 peek，tab icon 开始变淡。
3. 爪印从 tab 栏上方探出，弹性落地 + 涟漪扩散。
4. 状态机转为 walk，爪印左右交替沿行进方向踩下，趾头指向前进方向。
5. 爪印在存活 800ms 后蒸发淡出（opacity→0, scale→0.95）。
6. 屏幕上始终最多 8 个爪印（对象池 FIFO 复用）。
7. 行走路径结束，状态机转为 pause，最后爪印保持不动。

**Extensions:**
- 4a. 用户点击页面元素：爪印从远到近波浪式淡出（50ms 间隔），状态机→home，启动冷却。
- 4b. 用户滚动页面：爪印快速 fade out（0.2s），状态机→home，重新 idle 计时。
- 4c. Modal 打开：爪印向反方向微移 8px 后 fade out，状态机→home。
- 4d. 用户切到阿宝 tab：爪印就地消散 + 光弧飞回 tab icon，icon 脉冲恢复实心，状态机→converge→home。
- 1a. 仍在冷却中：不出场，等待冷却结束后下一次 idle 再触发。

---

### Use Case: Verify animation quality in showcase

**Primary Actor:** Developer
**Scope:** 动画展台（patrol-showcase.html）
**Level:** User goal

**Stakeholders and Interests:**
- Developer — 在独立页面调参、验证动画效果和性能，不依赖主应用
- Product — 确保视觉质量达标后再集成

**Preconditions:**
- patrol-utils.js 和 patrol.css 已加载

**Success Guarantee (Postconditions):**
- 所有动画效果可独立预览和重播
- 性能基准通过（p99 < 2ms, over-budget < 5%）
- 状态机单元测试全绿

**Trigger:** Developer 在浏览器打开 patrol-showcase.html

**Main Success Scenario:**
1. Developer 打开展台页面，看到 12 个展区。
2. Developer 点击"放置爪印"，看到爪印在白底和彩色背景上的渲染效果。
3. Developer 在路径测试区选择不同路径（直线/转弯/弧线/绕圈/S弯/折返/螺旋），爪印趾头始终指向行进方向。
4. Developer 调节参数滑块（步幅/偏移/外旋/蒸发时长），动画实时更新。
5. Developer 切换设备模拟（high/low），查看降级效果。
6. Developer 运行性能基准，确认通过。
7. Developer 运行状态机单元测试，确认全绿。

**Extensions:**
- 6a. 性能基准 FAIL：检查 over-budget 帧占比，优化动画实现后重跑。
- 7a. 状态机测试 FAIL：检查失败的转换路径，修复 PatrolStateMachine 逻辑。

---

### Use Case: Debug patrol behavior in main app

**Primary Actor:** Developer
**Scope:** 调试面板（patrol-debug.js）
**Level:** Subfunction

**Stakeholders and Interests:**
- Developer — 在主应用中实时观察巡游状态、性能指标、参数调整

**Preconditions:**
- `localStorage.setItem('patrol-debug', '1')` 已设置

**Success Guarantee (Postconditions):**
- 调试面板显示实时状态和帧耗时
- 参数滑块修改实时生效（无需重新部署）

**Trigger:** 页面加载时检测到 localStorage 标记

**Main Success Scenario:**
1. 页面加载，调试面板出现在右下角。
2. 面板显示当前状态（home/peek/walk/pause/rest）和冷却倒计时。
3. Developer 点击 "Force Out" 跳过 idle 等待直接出场。
4. 面板实时更新爪印池占用数和帧耗时（avg/peak/over-budget%）。
5. Developer 拖动 opacity 滑块，爪印透明度实时变化。
6. Developer 点击 "Reset CD" 清零冷却，反复测试出场动画。

**Extensions:**
- 3a. 巡游功能被设置关闭：Force Out 无效，面板显示 disabled 状态。

---

### Use Case: Adapt to device capabilities

**Primary Actor:** System (DeviceProfile)
**Scope:** 设备能力检测
**Level:** Subfunction

**Stakeholders and Interests:**
- User — 低端设备不掉帧，高端设备视觉更丰富
- Product — 性能红线不可妥协

**Preconditions:**
- 浏览器支持 requestAnimationFrame、classList、CSS transform

**Success Guarantee (Postconditions):**
- 高端设备启用 mix-blend-mode: soft-light + 环境色采样
- 低端设备用固定半透明度 + 固定主题色
- prefers-reduced-motion: reduce 时整个巡游系统不初始化

**Trigger:** patrol-utils.js 加载时自动检测

**Main Success Scenario:**
1. DeviceProfile 读取 navigator.deviceMemory 和 navigator.hardwareConcurrency。
2. 若 deviceMemory >= 4 或 hardwareConcurrency >= 4，判定为 high tier。
3. 若 prefers-reduced-motion: reduce，标记 reduceMotion = true。
4. 上层模块根据 tier 决定是否给 patrol-layer 加 `.patrol-enhanced` class。

**Extensions:**
- 2a. 两个 API 都不可用（返回 0/undefined）：降级为 low tier。
- 3a. 用户在系统设置中切换 reduced-motion：mediaQuery change 事件触发回调，巡游系统热关闭。
