/**
 * patrol-utils.js — 二狗巡山系统通用工具模块
 * SPEC-057 Phase 0b
 *
 * 包含: ObjectPool, DeviceProfile, CSSAnimator, IdleDetector
 * 接口契约见 patrol-interfaces.js
 */

// ============================================================
// 1. ObjectPool — 通用 DOM 对象池
// ============================================================

var ObjectPool = {
    create: function(opts) {
        var pool = [];
        var active = new Set();
        var container = opts.container || null;

        // Pre-create elements
        for (var i = 0; i < opts.size; i++) {
            var el = opts.factory();
            el.style.display = 'none';
            if (container) container.appendChild(el);
            pool.push(el);
        }

        return {
            acquire: function() {
                for (var j = 0; j < pool.length; j++) {
                    if (!active.has(pool[j])) {
                        active.add(pool[j]);
                        pool[j].style.display = '';
                        return pool[j];
                    }
                }
                return null; // Pool exhausted
            },

            release: function(el) {
                if (!active.has(el)) return;
                active.delete(el);
                if (opts.reset) opts.reset(el);
                el.style.display = 'none';
            },

            releaseAll: function() {
                active.forEach(function(el) {
                    if (opts.reset) opts.reset(el);
                    el.style.display = 'none';
                });
                active.clear();
            },

            get activeCount() {
                return active.size;
            },

            destroy: function() {
                active.clear();
                pool.forEach(function(el) {
                    if (el.parentNode) el.parentNode.removeChild(el);
                });
                pool.length = 0;
            }
        };
    }
};


// ============================================================
// 2. DeviceProfile — 设备能力检测
// ============================================================

var DeviceProfile = (function() {
    var mem = navigator.deviceMemory || 0;
    var cores = navigator.hardwareConcurrency || 0;
    var isHigh = mem >= 4 || cores >= 4;

    var mqReduce = window.matchMedia
        ? window.matchMedia('(prefers-reduced-motion: reduce)')
        : { matches: false };

    var supported = !!(
        window.requestAnimationFrame &&
        document.documentElement.classList &&
        ('transform' in document.documentElement.style ||
         'webkitTransform' in document.documentElement.style)
    );

    var mobile = (window.innerWidth <= 768) &&
        ('ontouchstart' in window || navigator.maxTouchPoints > 0);

    return {
        tier: isHigh ? 'high' : 'low',
        canBlend: isHigh,
        reduceMotion: mqReduce.matches,
        isSupported: supported,
        isMobile: mobile,

        // Listen for reduced-motion changes
        onReduceMotionChange: function(cb) {
            if (mqReduce.addEventListener) {
                mqReduce.addEventListener('change', function(e) {
                    cb(e.matches);
                });
            }
        }
    };
})();


// ============================================================
// 3. CSSAnimator — 运行时 CSS 动画管理
// ============================================================

var CSSAnimator = (function() {
    var styleEl = null;
    var sheet = null;
    var ruleMap = {}; // name → index in sheet

    function ensureSheet() {
        if (styleEl) return;
        styleEl = document.createElement('style');
        styleEl.id = 'patrol-dynamic-animations';
        document.head.appendChild(styleEl);
        sheet = styleEl.sheet;
    }

    return {
        inject: function(name, keyframesCSS) {
            ensureSheet();
            // Remove existing rule with same name
            if (name in ruleMap) {
                this.remove(name);
            }
            var rule = '@keyframes ' + name + ' { ' + keyframesCSS + ' }';
            var idx = sheet.insertRule(rule, sheet.cssRules.length);
            ruleMap[name] = idx;
        },

        remove: function(name) {
            if (!sheet || !(name in ruleMap)) return;
            // Find the rule by name (indices shift on delete)
            for (var i = 0; i < sheet.cssRules.length; i++) {
                if (sheet.cssRules[i].name === name) {
                    sheet.deleteRule(i);
                    break;
                }
            }
            delete ruleMap[name];
        },

        clear: function() {
            if (!sheet) return;
            while (sheet.cssRules.length > 0) {
                sheet.deleteRule(0);
            }
            ruleMap = {};
        },

        generateWalkKeyframes: function(points) {
            if (!points || points.length < 2) return '';
            var parts = [];
            for (var i = 0; i < points.length; i++) {
                var pct = Math.round((i / (points.length - 1)) * 100);
                var p = points[i];
                var rotate = p.rotate || 0;
                parts.push(
                    pct + '% { transform: translate(' +
                    p.x + 'px, ' + p.y + 'px) rotate(' +
                    rotate + 'deg); }'
                );
            }
            return parts.join(' ');
        }
    };
})();


// ============================================================
// 4. IdleDetector — 用户活动监听 + 冷却管理
// ============================================================

var IdleDetector = {
    create: function(opts) {
        var idleThreshold = opts.idleThreshold || 8000;
        var cooldown = opts.cooldown || 180000;
        var onIdle = opts.onIdle || function() {};
        var onActive = opts.onActive || function() {};
        var activeEvents = opts.activeEvents || ['click', 'scroll', 'keydown', 'touchstart'];

        var idleTimer = null;
        var cooldownEnd = 0;
        var _isIdle = false;
        var running = false;
        var handlers = {};

        function resetTimer() {
            if (!running) return;
            if (_isIdle) {
                _isIdle = false;
                onActive('interaction');
            }
            clearTimeout(idleTimer);
            idleTimer = setTimeout(function() {
                if (!running) return;
                var now = Date.now();
                if (now < cooldownEnd) {
                    // Still in cooldown — reschedule
                    idleTimer = setTimeout(arguments.callee, cooldownEnd - now);
                    return;
                }
                _isIdle = true;
                onIdle();
            }, idleThreshold);
        }

        function onVisibilityChange() {
            if (document.hidden) {
                clearTimeout(idleTimer);
                if (_isIdle) {
                    _isIdle = false;
                    onActive('hidden');
                }
            } else {
                resetTimer();
            }
        }

        return {
            start: function() {
                if (running) return;
                running = true;
                activeEvents.forEach(function(evt) {
                    var handler = function() { resetTimer(); };
                    handlers[evt] = handler;
                    document.addEventListener(evt, handler, { passive: true });
                });
                document.addEventListener('visibilitychange', onVisibilityChange);
                resetTimer();
            },

            stop: function() {
                running = false;
                clearTimeout(idleTimer);
                activeEvents.forEach(function(evt) {
                    if (handlers[evt]) {
                        document.removeEventListener(evt, handlers[evt]);
                    }
                });
                document.removeEventListener('visibilitychange', onVisibilityChange);
                _isIdle = false;
            },

            destroy: function() {
                this.stop();
                handlers = {};
                onIdle = null;
                onActive = null;
            },

            get isIdle() {
                return _isIdle;
            },

            get cooldownRemaining() {
                var r = cooldownEnd - Date.now();
                return r > 0 ? r : 0;
            },

            startCooldown: function() {
                cooldownEnd = Date.now() + cooldown;
                _isIdle = false;
                clearTimeout(idleTimer);
                // Schedule next idle check after cooldown
                if (running) {
                    idleTimer = setTimeout(function() {
                        if (!running) return;
                        _isIdle = true;
                        onIdle();
                    }, cooldown + idleThreshold);
                }
            },

            resetCooldown: function() {
                cooldownEnd = 0;
                if (running) resetTimer();
            },

            // Allow runtime adjustment of thresholds
            setIdleThreshold: function(ms) {
                idleThreshold = ms;
                if (running) resetTimer();
            },

            setCooldown: function(ms) {
                cooldown = ms;
            }
        };
    }
};


// ============================================================
// 5. StateMachine — 二狗状态机（纯逻辑）
// ============================================================

var PatrolStateMachine = {
    create: function(opts) {
        var onStateChange = opts.onStateChange || function() {};
        var _state = 'home';

        // Transition table: [currentState][event] → nextState
        var transitions = {
            home: {
                idle: 'peek'
            },
            peek: {
                peekDone: 'walk',
                click: 'home',
                scroll: 'home',
                modal: 'home'
            },
            walk: {
                click: 'home',
                scroll: 'home',
                modal: 'home',
                abaoTab: 'converge',
                walkEnd: 'pause'
            },
            pause: {
                click: 'home',
                scroll: 'home',
                modal: 'home',
                abaoTab: 'converge',
                pauseTimeout: 'walk',
                restTimeout: 'rest'
            },
            rest: {
                click: 'home',
                scroll: 'home',
                modal: 'home',
                abaoTab: 'converge',
                idle: 'walk'
            },
            converge: {
                convergeDone: 'home'
            }
        };

        return {
            get state() {
                return _state;
            },

            get canPatrol() {
                return _state !== 'home' && _state !== 'converge';
            },

            transition: function(event) {
                var table = transitions[_state];
                if (!table || !table[event]) return false;

                var from = _state;
                var to = table[event];
                _state = to;
                onStateChange(from, to, event);
                return true;
            },

            // Force state (for debug)
            forceState: function(state) {
                var from = _state;
                _state = state;
                onStateChange(from, state, 'force');
            },

            reset: function() {
                var from = _state;
                _state = 'home';
                if (from !== 'home') {
                    onStateChange(from, 'home', 'reset');
                }
            }
        };
    }
};


// ============================================================
// 6. PawPool — 爪印专用池（基于 ObjectPool）
// ============================================================

var PawPool = {
    create: function(opts) {
        var container = opts.container;
        var size = opts.size || 8;
        var pawSvg = opts.pawSvgLeft ||
            '<svg viewBox="0 0 16 16" fill="currentColor" width="100%" height="100%">' +
            '<ellipse cx="4.2" cy="3.4" rx="1.6" ry="2"/>' +
            '<ellipse cx="8" cy="2.2" rx="1.5" ry="1.9"/>' +
            '<ellipse cx="11.6" cy="3.8" rx="1.4" ry="1.9"/>' +
            '<ellipse cx="14" cy="6.8" rx="1.2" ry="1.7" transform="rotate(20 14 6.8)"/>' +
            '<ellipse cx="8.2" cy="9" rx="4" ry="3.5" transform="rotate(-5 8.2 9)"/>' +
            '</svg>';

        var evapTimers = new Map();
        var ordered = []; // Track placement order for fadeWave

        var pool = ObjectPool.create({
            size: size,
            container: container,
            factory: function() {
                var el = document.createElement('div');
                el.className = 'patrol-paw';
                el.innerHTML = pawSvg;
                return el;
            },
            reset: function(el) {
                el.className = 'patrol-paw';
                el.style.cssText = 'display:none';
                el.style.color = '';
                el.style.removeProperty('--paw-heading');
                el.style.removeProperty('--paw-mirror');
                // Remove from ordered tracking
                var idx = ordered.indexOf(el);
                if (idx !== -1) ordered.splice(idx, 1);
                // Clear evaporate timer
                if (evapTimers.has(el)) {
                    clearTimeout(evapTimers.get(el));
                    evapTimers.delete(el);
                }
            }
        });

        function scheduleEvaporate(el, delay) {
            var timer = setTimeout(function() {
                el.classList.remove('landing');
                el.classList.add('evaporate');
                // Release after evaporate animation completes
                var releaseTimer = setTimeout(function() {
                    pool.release(el);
                }, 1500); // evaporate duration
                evapTimers.set(el, releaseTimer);
            }, delay || 500);
            evapTimers.set(el, timer);
        }

        return {
            /**
             * Place a paw print.
             * @param {number} x - center x
             * @param {number} y - center y
             * @param {boolean} isLeft - left foot?
             * @param {string} color - CSS color
             * @param {number} heading - direction of travel in degrees (0=up, 90=right, 180=down, 270=left)
             */
            step: function(x, y, isLeft, color, heading) {
                var el = pool.acquire();
                if (!el) {
                    return null;
                }
                ordered.push(el);

                // CSS custom props drive the keyframes (see patrol.css)
                // --paw-mirror: 1 = left foot, -1 = right foot (horizontal flip)
                // --paw-heading: rotation to match travel direction
                // scaleX(-1) reverses rotation direction, so negate heading for right foot
                var rot = heading || 0;
                el.style.setProperty('--paw-mirror', isLeft ? '1' : '-1');
                el.style.setProperty('--paw-heading', (isLeft ? rot : -rot) + 'deg');

                el.className = 'patrol-paw ' + (isLeft ? 'left' : 'right') + ' landing';
                el.style.left = (x - 16) + 'px';
                el.style.top = (y - 16) + 'px';
                if (color) el.style.color = color;

                // Schedule evaporation
                scheduleEvaporate(el, 800);

                return el;
            },

            fadeAll: function(duration) {
                var dur = duration || 200;
                pool.releaseAll();
            },

            fadeWave: function(duration, interval) {
                var dur = duration || 150;
                var gap = interval || 50;
                var snapshot = ordered.slice(); // copy current order
                for (var i = 0; i < snapshot.length; i++) {
                    (function(el, delay) {
                        // Cancel evaporate timer
                        if (evapTimers.has(el)) {
                            clearTimeout(evapTimers.get(el));
                            evapTimers.delete(el);
                        }
                        setTimeout(function() {
                            el.classList.remove('landing', 'evaporate', 'breathe');
                            el.style.transition = 'opacity ' + dur + 'ms ease-out';
                            el.style.opacity = '0';
                            setTimeout(function() {
                                el.style.transition = '';
                                pool.release(el);
                            }, dur);
                        }, delay);
                    })(snapshot[i], i * gap);
                }
                ordered.length = 0;
            },

            clear: function() {
                evapTimers.forEach(function(timer) {
                    clearTimeout(timer);
                });
                evapTimers.clear();
                ordered.length = 0;
                pool.releaseAll();
            },

            get activeCount() {
                return pool.activeCount;
            },

            destroy: function() {
                this.clear();
                pool.destroy();
            }
        };
    }
};
