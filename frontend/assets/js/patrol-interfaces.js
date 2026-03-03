/**
 * patrol-interfaces.js — 二狗巡山系统接口契约
 * SPEC-057 Phase 0
 *
 * 本文件定义所有模块间的接口，不包含实现。
 * 三条并行开发流（视觉/逻辑/测试）都基于这些接口。
 */

// ============================================================
// 1. ObjectPool — 通用 DOM 对象池
// ============================================================
//
// var pool = ObjectPool.create({
//     size: 8,                          // 池大小
//     factory: function() {             // 创建元素
//         var el = document.createElement('div');
//         el.className = 'patrol-paw';
//         return el;
//     },
//     reset: function(el) {             // 回收时清理
//         el.className = 'patrol-paw';
//         el.style.cssText = '';
//     },
//     container: document.body           // 挂载容器
// });
//
// var el = pool.acquire();              // 获取空闲元素（null 如果满了）
// pool.release(el);                     // 回收
// pool.activeCount;                     // 当前占用数
// pool.releaseAll();                    // 全部回收
// pool.destroy();                       // 销毁池，移除所有 DOM

// ============================================================
// 2. DeviceProfile — 设备能力检测
// ============================================================
//
// DeviceProfile.tier           → 'low' | 'high'
// DeviceProfile.canBlend       → boolean   (是否开 mix-blend-mode)
// DeviceProfile.reduceMotion   → boolean   (prefers-reduced-motion: reduce)
// DeviceProfile.isSupported    → boolean   (浏览器是否支持必要 API)
//
// 判定规则:
//   high: deviceMemory >= 4 || hardwareConcurrency >= 4
//   low:  其余
//   reduceMotion: matchMedia('(prefers-reduced-motion: reduce)').matches
//   isSupported: 支持 requestAnimationFrame + classList + CSS transform

// ============================================================
// 3. IdleDetector — 用户活动监听 + 冷却管理
// ============================================================
//
// var idle = IdleDetector.create({
//     idleThreshold: 8000,              // ms: 无操作多久算 idle
//     cooldown: 180000,                 // ms: 退场后冷却时间
//     onIdle: function() {},            // idle 触发
//     onActive: function(event) {},     // 用户恢复活动 (event = 'click'|'scroll'|'modal'|'touchstart')
//     activeEvents: ['click', 'scroll', 'keydown', 'touchstart']
// });
//
// idle.start();                         // 开始监听
// idle.stop();                          // 停止监听
// idle.destroy();                       // 清理所有 listener
// idle.isIdle;                          // boolean
// idle.cooldownRemaining;               // ms, 0 = 冷却结束
// idle.startCooldown();                 // 手动触发冷却（退场时调用）
// idle.resetCooldown();                 // 调试用：清零冷却

// ============================================================
// 4. CSSAnimator — 运行时 CSS 动画管理
// ============================================================
//
// CSSAnimator.inject(name, keyframesCSS)
//   注入 @keyframes 到 <style> 标签
//   例: CSSAnimator.inject('walk-path-1', '0%{transform:translate(10px,200px)} 100%{...}')
//
// CSSAnimator.remove(name)
//   移除指定 @keyframes
//
// CSSAnimator.clear()
//   移除所有运行时注入的 @keyframes
//
// CSSAnimator.generateWalkKeyframes(points)
//   输入: [{x, y, rotate}...] 路径点数组
//   输出: keyframes CSS 字符串
//   用途: 根据地形平台坐标生成行走动画

// ============================================================
// 5. StateMachine — 二狗状态机（纯逻辑，不操作 DOM）
// ============================================================
//
// 状态: 'home' | 'peek' | 'walk' | 'pause' | 'rest' | 'converge'
//
// 事件:
//   'idle'          — IdleDetector 触发 idle
//   'peekDone'      — 探头动画完成
//   'click'         — 用户点击元素
//   'scroll'        — 用户滚动
//   'modal'         — modal 打开
//   'abaoTab'       — 点击阿宝 tab
//   'convergeDone'  — 收敛动画完成
//   'walkEnd'       — 行走路径走完
//   'pauseTimeout'  — 停留超时
//
// 转换表:
//   home    + idle         → peek      (前提: 冷却结束)
//   home    + idle         → home      (冷却中, 不出场)
//   peek    + peekDone     → walk
//   peek    + click/scroll/modal → home
//   walk    + click        → home      (退场A: 波浪淡出)
//   walk    + scroll       → home      (退场E: 快速fade)
//   walk    + modal        → home      (退场B: 躲闪fade)
//   walk    + abaoTab      → converge  (退场C: 光弧收敛)
//   walk    + walkEnd      → pause
//   pause   + pauseTimeout → walk      (继续下一段路径)
//   pause   + (长时间)     → rest
//   rest    + idle(重新)   → walk
//   rest    + click/scroll/modal → home
//   converge + convergeDone → home
//   任何状态 + reduceMotion → home     (无障碍强制关闭)
//
// var sm = StateMachine.create({
//     onStateChange: function(from, to, event) {}
// });
//
// sm.transition('idle');               // 触发事件
// sm.state;                            // 当前状态
// sm.canPatrol;                        // boolean (非 home 且非 converge)

// ============================================================
// 6. PawPool — 爪印专用池（基于 ObjectPool）
// ============================================================
//
// var paws = PawPool.create({
//     container: document.getElementById('patrol-layer'),
//     size: 8,
//     pawSvgLeft: '<svg>...</svg>',     // 左脚 SVG markup
// });
//
// paws.step(x, y, isLeft, color)       // 在 (x,y) 放一个爪印
//   - isLeft: true=左脚, false=右脚(自动 scaleX(-1))
//   - color: CSS 颜色值 (环境色)
//   - 自动播放落地弹性 + 涟漪
//   - 自动在 1.5s 后蒸发回收
//
// paws.fadeAll(duration)                // 所有爪印同时 fade out
// paws.fadeWave(duration, interval)     // 从远到近依次 fade out
// paws.clear()                         // 立即清除所有爪印（无动画）

// ============================================================
// 7. Patrol — 主控模块（集成以上所有模块）
// ============================================================
//
// Patrol.init()                        // 初始化（检测设备、创建池、绑定事件）
// Patrol.destroy()                     // 销毁一切
// Patrol.enabled                       // boolean (设置开关 + 设备支持 + 非 reduceMotion)
//
// 初始化流程:
//   1. DeviceProfile 检测 → reduceMotion 则 return
//   2. 读取 localStorage 设置开关 → 关闭则 return
//   3. 创建 patrol-layer (div, position:fixed, inset:0, pointer-events:none, z-index:999)
//   4. PawPool.create()
//   5. StateMachine.create()
//   6. IdleDetector.create() → start()
//   7. EventBridge 绑定 click/scroll/modal/page 事件
//   8. 如果 DeviceProfile.tier === 'high', 给 patrol-layer 加 .patrol-enhanced

// ============================================================
// 8. CSS Class 命名规范（流A 视觉流使用）
// ============================================================
//
// 容器层:
//   #patrol-layer                      — fixed 全屏容器, pointer-events:none, z-index:999
//   #patrol-layer.patrol-enhanced      — 高端设备增强样式
//
// 爪印:
//   .patrol-paw                        — 爪印基础样式
//   .patrol-paw.left                   — 左脚
//   .patrol-paw.right                  — 右脚 (scaleX(-1))
//   .patrol-paw.landing                — 落地态 (触发涟漪 ::before/::after)
//   .patrol-paw.evaporate              — 蒸发态
//
// Tab icon:
//   .tab-icon-patrol                   — SVG tab icon 容器
//   .tab-icon-patrol.patrol-out        — 二狗出去了 (opacity 30%, stroke-only)
//   .tab-icon-patrol.patrol-returning  — 二狗回来中 (过渡动画)
//   .tab-icon-patrol.patrol-pulse      — 回家脉冲
//
// 收敛光弧:
//   .patrol-arc                        — 飞行光点
//
// 调试:
//   #patrol-debug                      — 调试面板容器
//   .patrol-terrain-overlay            — 地形可视化叠加层

// ============================================================
// 9. 事件命名规范（EventBridge 使用）
// ============================================================
//
// patrol:stateChange     — { from, to, event }
// patrol:pawStep         — { x, y, isLeft, color }
// patrol:fadeAll          — { duration }
// patrol:fadeWave         — { duration, interval }
// patrol:convergeTo      — { targetX, targetY }
//
// 外部事件映射:
//   click (document)         → sm.transition('click')
//   scroll (scroll container) → sm.transition('scroll')
//   modal.open (自定义事件)   → sm.transition('modal')
//   switchPage('abao')       → sm.transition('abaoTab')
