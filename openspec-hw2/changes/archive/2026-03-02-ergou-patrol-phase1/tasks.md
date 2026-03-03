## 1. 基础设施补全

- [x] 1.1 PawPool 增加放置顺序追踪（维护有序数组记录活跃爪印的 acquire 顺序）
- [x] 1.2 PawPool.fadeWave 实现真正的波浪淡出（从近到远，间隔 50ms，逐个 fade）
- [x] 1.3 DeviceProfile 增加 isMobile 属性（innerWidth <= 768 && touch 能力检测）

## 2. 主控模块骨架

- [x] 2.1 创建 patrol.js：IIFE 结构，暴露 Patrol 全局变量（init/destroy/enabled）
- [x] 2.2 实现 Patrol.init() 初始化流程（reduceMotion/isSupported/isMobile/localStorage 检测链）
- [x] 2.3 创建 patrol-layer（fixed, inset:0, pointer-events:none, z-index:999），高端设备加 .patrol-enhanced
- [x] 2.4 实例化 PawPool、PatrolStateMachine、IdleDetector，绑定 onStateChange 回调
- [x] 2.5 实现 Patrol.destroy() 清理所有资源（pool/sm/idle/事件/DOM）
- [x] 2.6 实现 visibilitychange 处理（页面不可见时清除爪印、停止 idle；恢复可见时重新 idle 计时）

## 3. EventBridge 事件桥接

- [x] 3.1 绑定 click（document, capture, passive）→ sm.transition('click')，仅 canPatrol 时
- [x] 3.2 绑定 scroll（passive）→ debounce 100ms → sm.transition('scroll')，仅 canPatrol 时
- [x] 3.3 实现 MutationObserver 检测 modal 打开（监听 body 直接子节点 style.display 变化）→ sm.transition('modal')
- [x] 3.4 监听 tab 切换事件：切到阿宝 tab → sm.transition('abaoTab')，切到其他 tab → 退场

## 4. 出场流程

- [x] 4.1 onStateChange(home→peek)：获取 tab icon 位置，在其上方放置探头爪印（半隐藏），300ms 后触发 peekDone
- [x] 4.2 onStateChange(peek→walk)：从 tab icon 上方开始行走，踩出前两步
- [x] 4.3 IdleDetector.onIdle 回调：检查冷却 → sm.transition('idle')

## 5. 行走循环

- [x] 5.1 实现路径生成（随机方向直线/缓弧，5-8 步，安全区域约束）
- [x] 5.2 实现步行定时器：按步频（~1.3s/步）调用 PawPool.step()，左右交替
- [x] 5.3 实现 heading 计算：根据当前步和下一步位置计算行进方向角度
- [x] 5.4 实现步态参数：步幅 40px、左右偏移 12px、外旋 ±8°
- [x] 5.5 实现环境染色：高端设备 elementFromPoint → getComputedStyle，低端设备用固定主题色
- [x] 5.6 路径走完触发 walkEnd → pause

## 6. 退场动画

- [x] 6.1 onStateChange(walk/pause/rest→home, event='click')：调用 fadeWave + startCooldown
- [x] 6.2 onStateChange(walk/pause/rest→home, event='scroll')：调用 fadeAll，不冷却，重新 idle 计时
- [x] 6.3 onStateChange(walk/pause/rest→home, event='modal')：调用 fadeAll + startCooldown
- [x] 6.4 走出屏幕检测：每步检查位置是否超出安全区域边界，超出则 overflow 自然裁切
- [x] 6.5 统一退场清理：停止步行定时器、清除路径状态

## 7. Pause / Rest 子状态

- [x] 7.1 onStateChange(walk→pause)：停止步行定时器，设置 pauseTimeout（5s）
- [x] 7.2 pauseTimeout 到期：生成新路径，sm.transition('pauseTimeout') → walk
- [x] 7.3 restTimeout（15s 无新路径）：sm.transition('restTimeout') → rest
- [x] 7.4 rest 状态处理：最后两对爪印降 opacity 至 0.3、scale 至 0.95，加 breathe class

## 8. Tab icon 联动

- [x] 8.1 定位 .tab-icon-patrol 元素，缓存引用
- [x] 8.2 onStateChange 中同步 tab icon class（home=清除, peek/walk=patrol-out, converge=patrol-returning, home回来=patrol-pulse）
- [x] 8.3 patrol-pulse 动画结束后移除 class（animationend 监听）

## 9. 设置页开关

- [x] 9.1 settings.js 中添加"二狗巡游"开关（localStorage key: patrol-enabled，默认 '1'）
- [x] 9.2 开关变更时：关闭 → Patrol.destroy()，开启 → Patrol.init()

## 10. 集成与调试

- [x] 10.1 index.html 引入 patrol.js（在 patrol-utils.js 和 patrol-debug.js 之后）
- [x] 10.2 初始化入口：页面加载后调用 Patrol.init()
- [x] 10.3 Patrol.init() 内调用 PatrolDebug.connect() 传入内部引用
- [x] 10.4 行走过程中调用 PatrolDebug.updatePosition(x, y)
- [x] 10.5 监听 patrol:debugParam 事件，实时应用调试参数（opacity/speed/size）
- [x] 10.6 监听 patrol:debugPause 事件，暂停/恢复行走

## 11. 验证

- [x] 11.1 手机端验证：idle → 出场 → 行走 → 爪印轨迹 → 蒸发消失
- [x] 11.2 手机端验证：点击退场（波浪淡出）、滚动退场（快速 fade）、Modal 退场
- [x] 11.3 手机端验证：Tab icon 联动（出场变淡、回家脉冲）
- [x] 11.4 手机端验证：Pause → Rest 子状态（停留 → 趴下 → 呼吸）
- [x] 11.5 桌面端验证：无任何巡游 DOM 或事件
- [x] 11.6 验证设置开关即时生效
- [x] 11.7 验证冷却机制（点击退场后 3 分钟内不出场，滚动退场后可快速重现）
- [x] 11.8 验证 visibilitychange（切后台停止，切回重新计时）
