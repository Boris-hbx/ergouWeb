/**
 * patrol.js — 二狗值班系统主控模块
 * SPEC-057 Phase 1+2: 核心循环 + 地形感知
 *
 * 依赖: patrol-utils.js (ObjectPool, DeviceProfile, CSSAnimator, IdleDetector, PatrolStateMachine, PawPool)
 *       patrol-terrain.js (TerrainScanner, PathPlanner, optional — 无则回退 Phase 1 随机路径)
 *       patrol-debug.js (PatrolDebug, optional)
 */

var Patrol = (function() {
    'use strict';

    // ─── Internal state ───
    var _initialized = false;
    var _layer = null;          // #patrol-layer
    var _pawPool = null;
    var _sm = null;             // PatrolStateMachine
    var _idle = null;           // IdleDetector
    var _tabIcon = null;        // .tab-icon-patrol element
    var _terrainOverlay = null;  // terrain debug overlay element

    // Walk state
    var _walkTimer = null;
    var _walkPath = [];         // [{x, y}]
    var _walkIndex = 0;
    var _isLeftFoot = true;
    var _lastHeading = 90;      // degrees

    // Pause/rest timers
    var _pauseTimer = null;
    var _restTimer = null;

    // EventBridge cleanup
    var _eventCleanup = [];

    // MutationObserver for modals
    var _modalObserver = null;

    // Scroll debounce
    var _scrollTimer = null;

    // ─── Abao button breathing (intermittent, scroll-aware) ───
    var _breatheState = {
        timer: null,
        count: 0,         // breaths done in current burst
        iconEl: null,
        scrollTimer: null,
        paused: false,     // true while user is scrolling
        destroyed: false
    };

    function breatheInit() {
        var el = document.querySelector('.mobile-nav-abao .mobile-nav-icon');
        if (!el) return;
        _breatheState.iconEl = el;
        _breatheState.destroyed = false;

        // Listen for scroll → pause breathing
        var onScroll = function() {
            _breatheState.paused = true;
            // Remove animation immediately
            if (_breatheState.iconEl) _breatheState.iconEl.classList.remove('abao-inhale');
            clearTimeout(_breatheState.scrollTimer);
            clearTimeout(_breatheState.timer);
            // Resume after 6s idle
            _breatheState.scrollTimer = setTimeout(function() {
                _breatheState.paused = false;
                breatheSchedule();
            }, 6000);
        };
        window.addEventListener('scroll', onScroll, true);
        _breatheState._onScroll = onScroll;

        // Start first cycle after a short delay
        _breatheState.timer = setTimeout(function() { breatheSchedule(); }, 3000);
    }

    function breatheSchedule() {
        if (_breatheState.destroyed || _breatheState.paused) return;
        _breatheState.count = 0;
        breatheOnce();
    }

    function breatheOnce() {
        if (_breatheState.destroyed || _breatheState.paused) return;
        var el = _breatheState.iconEl;
        if (!el) return;

        // Don't breathe if chat panel is open
        if (el.closest('.active')) return;

        // Trigger one breath
        el.classList.remove('abao-inhale');
        // Force reflow to restart animation
        void el.offsetWidth;
        el.classList.add('abao-inhale');

        _breatheState.count++;

        // Listen for this breath to finish
        el.addEventListener('animationend', function handler() {
            el.removeEventListener('animationend', handler);
            el.classList.remove('abao-inhale');

            if (_breatheState.destroyed || _breatheState.paused) return;

            if (_breatheState.count < 2) {
                // Second breath after a short gap
                _breatheState.timer = setTimeout(breatheOnce, 800);
            } else {
                // Done breathing — rest 8-12 seconds
                var rest = 8000 + Math.random() * 4000;
                _breatheState.timer = setTimeout(breatheSchedule, rest);
            }
        });
    }

    function breatheDestroy() {
        _breatheState.destroyed = true;
        clearTimeout(_breatheState.timer);
        clearTimeout(_breatheState.scrollTimer);
        if (_breatheState._onScroll) {
            window.removeEventListener('scroll', _breatheState._onScroll, true);
        }
        if (_breatheState.iconEl) {
            _breatheState.iconEl.classList.remove('abao-inhale');
        }
        _breatheState.iconEl = null;
    }

    // Debug params (overridable via patrol:debugParam)
    var _params = {
        opacity: 0.5,
        speed: 30,      // px/s
        size: 20,        // half-size for positioning
        stride: 40,      // px between steps
        lateralOffset: 12,
        toeAngle: 8      // degrees
    };

    // ─── Helpers ───

    function isMobile() {
        return DeviceProfile.isMobile;
    }

    function isEnabled() {
        return localStorage.getItem('patrol-enabled') !== '0';
    }

    // Organic gait rhythm — not a metronome, more like a real dog
    // Pattern: normal, normal, linger, normal, normal, quick, normal, normal...
    var _gaitRhythm = [1.0, 1.0, 1.09, 1.0, 1.0, 0.88, 1.0, 1.0];

    function getStepInterval() {
        var base = (_params.stride / _params.speed) * 1000;
        var rhythm = _gaitRhythm[_walkIndex % _gaitRhythm.length];
        return base * rhythm;
    }

    // Get color from element under point (environment tinting)
    function getEnvColor(x, y) {
        if (DeviceProfile.tier !== 'high') {
            return getComputedStyle(document.documentElement).getPropertyValue('--primary-color').trim() || '#6366f1';
        }
        // Temporarily hide patrol layer to avoid hitting our own elements
        if (_layer) _layer.style.display = 'none';
        var el = document.elementFromPoint(x, y);
        if (_layer) _layer.style.display = '';
        if (!el) return '#6366f1';

        // Walk up to find non-transparent background
        var current = el;
        while (current && current !== document.body) {
            var bg = getComputedStyle(current).backgroundColor;
            if (bg && bg !== 'rgba(0, 0, 0, 0)' && bg !== 'transparent') {
                return bg;
            }
            current = current.parentElement;
        }
        return '#6366f1';
    }

    // ─── Path generation ───

    /** Phase 2: terrain-aware path via PathPlanner, with Phase 1 fallback */
    function generatePath(startX, startY, useScanAhead) {
        // Phase 2: use PathPlanner if available
        if (typeof PathPlanner !== 'undefined' && typeof TerrainScanner !== 'undefined') {
            var platforms;
            if (useScanAhead) {
                // Continuing walk: scan ahead from current position
                platforms = TerrainScanner.scanAhead(startX, startY, _lastHeading);
            } else {
                // Fresh walk: use full cache
                platforms = TerrainScanner.platforms;
            }
            var path = PathPlanner.planPath(startX, startY, platforms, {
                stride: _params.stride,
                lastHeading: _lastHeading
            });
            if (path && path.length > 0) return path;
        }

        // Phase 1 fallback: random path
        return generateRandomPath(startX, startY);
    }

    /** Phase 1 random path (kept as fallback) */
    function generateRandomPath(startX, startY) {
        var vw = window.innerWidth;
        var vh = window.innerHeight;
        var safeTop = 44;
        var safeBottom = vh - 60;
        var safeLeft = 16;
        var safeRight = vw - 16;

        var steps = 5 + Math.floor(Math.random() * 4); // 5-8 steps
        var path = [];

        // Pick a random direction, avoid repeating similar angle
        var angle;
        do {
            angle = Math.random() * 360;
        } while (Math.abs(angle - _lastHeading) < 30 || Math.abs(angle - _lastHeading) > 330);

        var rad = angle * Math.PI / 180;
        var x = startX;
        var y = startY;

        // Add slight curve
        var curveBias = (Math.random() - 0.5) * 0.02; // radians per step

        for (var i = 0; i < steps; i++) {
            rad += curveBias;
            x += Math.cos(rad) * _params.stride;
            y += Math.sin(rad) * _params.stride;

            // Clamp to safe area
            x = Math.max(safeLeft, Math.min(safeRight, x));
            y = Math.max(safeTop, Math.min(safeBottom, y));

            path.push({ x: x, y: y });
        }

        return path;
    }

    function headingBetween(ax, ay, bx, by) {
        // Returns degrees: 0=up, 90=right, 180=down, 270=left
        var dx = bx - ax;
        var dy = by - ay;
        var rad = Math.atan2(dy, dx);
        // atan2 gives 0=right, pi/2=down. Convert to 0=up
        var deg = (rad * 180 / Math.PI + 90);
        return ((deg % 360) + 360) % 360;
    }

    // ─── Phase 3: Interaction handlers ───

    /** Create a standalone stamped paw (not from pool) */
    function stampPaw(x, y, color, className, duration) {
        if (!_layer) return;
        var el = document.createElement('div');
        el.className = 'patrol-paw ' + (className || 'stamped');
        el.innerHTML = '<svg viewBox="0 0 24 24" width="20" height="20">' +
            '<circle cx="7" cy="5" r="2.2" fill="currentColor"/>' +
            '<circle cx="12" cy="3.5" r="2.2" fill="currentColor"/>' +
            '<circle cx="17" cy="5" r="2.2" fill="currentColor"/>' +
            '<circle cx="20" cy="10" r="2" fill="currentColor"/>' +
            '<ellipse cx="12" cy="13" rx="5.5" ry="4.5" fill="currentColor"/>' +
            '</svg>';
        el.style.cssText =
            'position:absolute;left:' + x + 'px;top:' + y + 'px;' +
            'color:' + (color || '#6366f1') + ';' +
            'pointer-events:none;z-index:1;';
        el.style.setProperty('--paw-heading', '0deg');
        el.style.setProperty('--paw-mirror', '1');
        _layer.appendChild(el);

        var dur = duration || 2000;
        setTimeout(function() {
            el.style.transition = 'opacity 0.3s ease-out';
            el.style.opacity = '0';
            setTimeout(function() {
                if (el.parentNode) el.parentNode.removeChild(el);
            }, 300);
        }, dur);
    }

    /** Handle task completion: stamp paw on card */
    function handleTaskComplete(detail) {
        if (!detail || !detail.cardRect) return;
        var r = detail.cardRect;
        // Position relative to viewport, convert to layer-relative
        var x = r.left + r.width * 0.5;
        var y = r.top + r.height * 0.3;
        var color = getEnvColor(x, y);
        stampPaw(x, y, color, 'stamped', 2000);

        // "踩凹" — brief inset shadow on the card
        var card = detail.cardEl;
        if (card) {
            card.classList.add('patrol-dent');
            setTimeout(function() { card.classList.remove('patrol-dent'); }, 400);
        }

        // Check if last in quadrant → checkmark pattern
        if (detail.isLastInQuadrant) {
            var qEl = document.getElementById('quadrant-' + detail.quadrant);
            if (!qEl) qEl = document.querySelector('.quadrant[data-quadrant="' + detail.quadrant + '"]');
            if (qEl) {
                handleCheckmark(qEl);
            }
        }
    }

    /** Stamp ✓ pattern in quadrant area (3 paws) */
    function handleCheckmark(quadrantEl) {
        var r = quadrantEl.getBoundingClientRect();
        var color = getEnvColor(r.left + r.width / 2, r.top + r.height / 2);
        // ✓ shape: left-bottom → center-bottom → right-top
        var points = [
            { x: r.left + r.width * 0.3, y: r.top + r.height * 0.7 },
            { x: r.left + r.width * 0.45, y: r.top + r.height * 0.8 },
            { x: r.left + r.width * 0.7, y: r.top + r.height * 0.35 }
        ];
        for (var i = 0; i < points.length; i++) {
            (function(pt, delay) {
                setTimeout(function() {
                    stampPaw(pt.x, pt.y, color, 'stamped', 3000);
                }, delay);
            })(points[i], i * 100);
        }
    }

    /** Handle jelly pill follow: quick dash in pill direction */
    function handleJellyFollow(detail) {
        if (!_sm || !_sm.canPatrol) return;
        if (!detail || !detail.direction) return;

        // Get current position
        var startX, startY;
        if (_walkPath.length > 0 && _walkIndex > 0) {
            var last = _walkPath[Math.min(_walkIndex, _walkPath.length - 1)];
            startX = last.x;
            startY = last.y;
        } else {
            startX = window.innerWidth / 2;
            startY = window.innerHeight - 100;
        }

        // Generate 2-3 quick steps in pill direction
        var dx = detail.direction === 'right' ? _params.stride * 0.8 : -_params.stride * 0.8;
        var quickPath = [];
        for (var i = 0; i < 3; i++) {
            quickPath.push({
                x: Math.max(16, Math.min(window.innerWidth - 16, startX + dx * (i + 1))),
                y: startY + (Math.random() - 0.5) * 10
            });
        }

        // Temporarily speed up stepping
        clearWalkTimers();
        _walkPath = quickPath;
        _walkIndex = 0;
        var origSpeed = _params.speed;
        _params.speed = origSpeed * 1.6;
        _walkTimer = setTimeout(function() {
            _params.speed = origSpeed;
            stepOnce();
        }, 50);
    }

    /** Handle chat status: toggle abao logo breathing */
    function handleChatStatus(detail) {
        if (!detail) return;
        var abaoIcon = document.querySelector('#mobile-nav-abao .nav-icon, #mobile-nav-abao img');
        if (!abaoIcon) return;
        if (detail.status === 'thinking') {
            abaoIcon.classList.add('abao-thinking');
        } else {
            abaoIcon.classList.remove('abao-thinking');
        }
    }

    // ─── Tab icon sync ───

    function syncTabIcon(fromState, toState) {
        if (!_tabIcon) return;

        // Clear all patrol classes
        _tabIcon.classList.remove('patrol-out', 'patrol-returning', 'patrol-pulse');

        if (toState === 'on_duty' || toState === 'patrol' || toState === 'standby' || toState === 'rest') {
            _tabIcon.classList.add('patrol-out');
        } else if (toState === 'converge') {
            _tabIcon.classList.add('patrol-returning');
        } else if (toState === 'off_duty' && fromState !== 'off_duty') {
            // Coming home — pulse
            _tabIcon.classList.add('patrol-pulse');
            _tabIcon.addEventListener('animationend', function handler() {
                _tabIcon.classList.remove('patrol-pulse');
                _tabIcon.removeEventListener('animationend', handler);
            });
        }
    }

    // ─── State change handler ───

    function onStateChange(from, to, event) {
        // Sync tab icon
        syncTabIcon(from, to);

        // Update debug
        if (typeof PatrolDebug !== 'undefined' && PatrolDebug.enabled) {
            // State is auto-read from sm reference
        }

        // Handle state transitions
        switch (to) {
            case 'on_duty':
                handlePeek();
                break;
            case 'patrol':
                handleWalk(from);
                break;
            case 'standby':
                handlePause();
                break;
            case 'rest':
                handleRest();
                break;
            case 'off_duty':
                handleHome(from, event);
                break;
            case 'converge':
                handleConverge();
                break;
        }
    }

    // ─── State handlers ───

    function handlePeek() {
        if (!_tabIcon) return;

        // Phase 2: full terrain scan on peek (出场时扫描)
        if (typeof TerrainScanner !== 'undefined') {
            TerrainScanner.scan();
        }

        var rect = _tabIcon.getBoundingClientRect();
        var startX = rect.left + rect.width / 2;
        var startY = rect.top - 20;

        // Place a peek paw (half visible)
        var color = getEnvColor(startX, startY);
        _pawPool.step(startX, startY, true, color, 0); // heading up

        // After 300ms, transition to walk
        setTimeout(function() {
            if (_sm && _sm.state === 'on_duty') {
                _sm.transition('peekDone');
            }
        }, 300);
    }

    function handleWalk(fromState) {
        clearWalkTimers();

        var startX, startY;
        if (fromState === 'on_duty' || fromState === 'off_duty') {
            // Start from tab icon area
            if (_tabIcon) {
                var rect = _tabIcon.getBoundingClientRect();
                startX = rect.left + rect.width / 2;
                startY = rect.top - 40;
            } else {
                startX = window.innerWidth / 2;
                startY = window.innerHeight - 100;
            }
        } else if (fromState === 'standby' || fromState === 'rest') {
            // Continue from last position
            if (_walkPath.length > 0) {
                var last = _walkPath[_walkPath.length - 1];
                startX = last.x;
                startY = last.y;
            } else {
                startX = window.innerWidth / 2;
                startY = window.innerHeight / 2;
            }
        } else {
            startX = window.innerWidth / 2;
            startY = window.innerHeight / 2;
        }

        // Phase 2: use scanAhead when continuing from pause/rest
        var useScanAhead = (fromState === 'standby' || fromState === 'rest');
        _walkPath = generatePath(startX, startY, useScanAhead);
        _walkIndex = 0;
        _isLeftFoot = true;

        stepOnce();
    }

    function stepOnce() {
        if (!_sm || _sm.state !== 'patrol') return;
        if (_walkIndex >= _walkPath.length) {
            _sm.transition('walkEnd');
            return;
        }

        var pt = _walkPath[_walkIndex];

        // Phase 2: check platform validity (dirty check)
        if (pt.platformId && typeof TerrainScanner !== 'undefined') {
            if (!TerrainScanner.isPlatformValid(pt.platformId)) {
                // Platform gone — graceful exit without cooldown
                _pawPool.fadeAll(150);
                clearWalkTimers();
                _sm.reset(); // back to home, no cooldown
                return;
            }
        }

        // Calculate heading
        var heading;
        if (_walkIndex < _walkPath.length - 1) {
            var next = _walkPath[_walkIndex + 1];
            heading = headingBetween(pt.x, pt.y, next.x, next.y);
        } else if (_walkIndex > 0) {
            var prev = _walkPath[_walkIndex - 1];
            heading = headingBetween(prev.x, prev.y, pt.x, pt.y);
        } else {
            heading = _lastHeading;
        }
        _lastHeading = heading;

        // Apply lateral offset (perpendicular to heading)
        var perpRad = (heading - 90) * Math.PI / 180;
        var offset = _isLeftFoot ? -_params.lateralOffset : _params.lateralOffset;
        var px = pt.x + Math.cos(perpRad) * offset;
        var py = pt.y + Math.sin(perpRad) * offset;

        // Apply toe angle
        var toeOffset = _isLeftFoot ? -_params.toeAngle : _params.toeAngle;
        var finalHeading = heading + toeOffset;

        // Environment color
        var color = getEnvColor(px, py);

        _pawPool.step(px, py, _isLeftFoot, color, finalHeading);

        // Debug position update
        if (typeof PatrolDebug !== 'undefined') {
            PatrolDebug.updatePosition(px, py);
            PatrolDebug.updatePlatform(pt.platformId || null);
        }

        _isLeftFoot = !_isLeftFoot;
        _walkIndex++;

        // Check if next step would be out of bounds
        if (_walkIndex < _walkPath.length) {
            var nextPt = _walkPath[_walkIndex];
            if (nextPt.x < 8 || nextPt.x > window.innerWidth - 8 ||
                nextPt.y < 36 || nextPt.y > window.innerHeight - 52) {
                // Walk off screen — let overflow handle it
                _sm.transition('walkEnd');
                return;
            }
        }

        _walkTimer = setTimeout(stepOnce, getStepInterval());
    }

    function handlePause() {
        clearWalkTimers();

        // After 5s, continue walking
        _pauseTimer = setTimeout(function() {
            if (_sm && _sm.state === 'standby') {
                _sm.transition('pauseTimeout');
            }
        }, 5000);

        // After 15s total, enter rest
        _restTimer = setTimeout(function() {
            if (_sm && _sm.state === 'standby') {
                _sm.transition('restTimeout');
            }
        }, 15000);
    }

    function handleRest() {
        clearWalkTimers();
        // Rest state: make remaining paws breathe
        // The paws that are still visible will naturally evaporate
        // We don't add breathe class here since paws auto-evaporate via PawPool
    }

    function handleHome(fromState, event) {
        clearWalkTimers();

        if (fromState === 'off_duty') return; // no-op

        // Choose exit animation based on event
        switch (event) {
            case 'click':
                _pawPool.fadeWave(150, 50);
                if (_idle) _idle.startCooldown();
                break;
            case 'modal':
                _pawPool.fadeAll(200);
                if (_idle) _idle.startCooldown();
                break;
            case 'scroll':
                _pawPool.fadeAll(200);
                // No cooldown for scroll — restart idle timer
                break;
            default:
                // Generic exit (tab switch, force, etc.)
                _pawPool.fadeAll(200);
                if (_idle && event !== 'reset' && event !== 'force') {
                    _idle.startCooldown();
                }
                break;
        }
    }

    function handleConverge() {
        clearWalkTimers();
        _pawPool.fadeAll(150);

        // Phase 4: Light arc convergence animation
        var fromX, fromY;
        if (_walkPath.length > 0 && _walkIndex > 0) {
            var lastPt = _walkPath[Math.min(_walkIndex, _walkPath.length - 1)];
            fromX = lastPt.x;
            fromY = lastPt.y;
        } else {
            fromX = window.innerWidth / 2;
            fromY = window.innerHeight / 2;
        }

        // Get abao logo position
        var abaoEl = document.querySelector('#mobile-nav-abao .nav-icon, #mobile-nav-abao img');
        if (abaoEl && _layer) {
            var abaoRect = abaoEl.getBoundingClientRect();
            var toX = abaoRect.left + abaoRect.width / 2;
            var toY = abaoRect.top + abaoRect.height / 2;

            // Create convergence point — straight line, clean, no fuss
            var arc = document.createElement('div');
            arc.className = 'patrol-arc';
            arc.style.setProperty('--arc-from-x', fromX + 'px');
            arc.style.setProperty('--arc-from-y', fromY + 'px');
            arc.style.setProperty('--arc-to-x', toX + 'px');
            arc.style.setProperty('--arc-to-y', toY + 'px');
            _layer.appendChild(arc);

            // Arc arrives → logo pulse + cleanup
            arc.addEventListener('animationend', function() {
                if (arc.parentNode) arc.parentNode.removeChild(arc);
                // Logo pulse
                if (_tabIcon) {
                    _tabIcon.classList.add('patrol-pulse');
                    _tabIcon.addEventListener('animationend', function handler() {
                        _tabIcon.classList.remove('patrol-pulse');
                        _tabIcon.removeEventListener('animationend', handler);
                        if (_sm && _sm.state === 'converge') {
                            _sm.transition('convergeDone');
                        }
                    });
                } else {
                    if (_sm && _sm.state === 'converge') {
                        _sm.transition('convergeDone');
                    }
                }
            });
        } else {
            // Fallback: no abao element, just go home
            setTimeout(function() {
                if (_sm && _sm.state === 'converge') {
                    _sm.transition('convergeDone');
                }
            }, 400);
        }
    }

    function clearWalkTimers() {
        if (_walkTimer) { clearTimeout(_walkTimer); _walkTimer = null; }
        if (_pauseTimer) { clearTimeout(_pauseTimer); _pauseTimer = null; }
        if (_restTimer) { clearTimeout(_restTimer); _restTimer = null; }
    }

    // ─── EventBridge ───

    function setupEventBridge() {
        // 1. Click → exit
        function onDocClick(e) {
            if (!_sm || !_sm.canPatrol) return;
            // Ignore clicks on patrol-debug panel
            if (e.target.closest && e.target.closest('#patrol-debug')) return;
            _sm.transition('click');
        }
        document.addEventListener('click', onDocClick, { capture: true, passive: true });
        _eventCleanup.push(function() {
            document.removeEventListener('click', onDocClick, { capture: true });
        });

        // 2. Scroll → exit (debounced 100ms)
        function onScroll() {
            if (!_sm || !_sm.canPatrol) return;
            if (_scrollTimer) clearTimeout(_scrollTimer);
            _scrollTimer = setTimeout(function() {
                if (_sm && _sm.canPatrol) {
                    _sm.transition('scroll');
                }
            }, 100);
        }
        window.addEventListener('scroll', onScroll, { passive: true });
        _eventCleanup.push(function() {
            window.removeEventListener('scroll', onScroll);
        });

        // Also listen on main scroll containers
        var scrollContainers = document.querySelectorAll('.todo-container, .review-container, .english-container, .life-container');
        scrollContainers.forEach(function(el) {
            el.addEventListener('scroll', onScroll, { passive: true });
            _eventCleanup.push(function() {
                el.removeEventListener('scroll', onScroll);
            });
        });

        // 3. Modal detection via MutationObserver
        _modalObserver = new MutationObserver(function(mutations) {
            if (!_sm || !_sm.canPatrol) return;
            for (var i = 0; i < mutations.length; i++) {
                var m = mutations[i];
                if (m.type === 'attributes' && m.attributeName === 'style') {
                    var target = m.target;
                    if (target.id && (target.id.indexOf('overlay') !== -1 || target.id.indexOf('modal') !== -1)) {
                        var display = target.style.display;
                        if (display && display !== 'none') {
                            _sm.transition('modal');
                            return;
                        }
                    }
                }
                // Check added nodes (new overlays)
                if (m.type === 'childList') {
                    for (var j = 0; j < m.addedNodes.length; j++) {
                        var node = m.addedNodes[j];
                        if (node.nodeType === 1 && node.id &&
                            (node.id.indexOf('overlay') !== -1 || node.id.indexOf('modal') !== -1)) {
                            if (node.style.display !== 'none') {
                                _sm.transition('modal');
                                return;
                            }
                        }
                    }
                }
            }
        });
        _modalObserver.observe(document.body, {
            childList: true,
            attributes: true,
            attributeFilter: ['style'],
            subtree: false
        });
        // Also observe direct children for style changes
        document.querySelectorAll('[id*="overlay"], [id*="modal"]').forEach(function(el) {
            _modalObserver.observe(el, { attributes: true, attributeFilter: ['style'] });
        });

        // 4. Tab switch: listen to Abao toggle
        var abaoNav = document.getElementById('mobile-nav-abao');
        if (abaoNav) {
            function onAbaoClick() {
                if (_sm && _sm.canPatrol) {
                    _sm.transition('abaoTab');
                }
            }
            abaoNav.addEventListener('click', onAbaoClick);
            _eventCleanup.push(function() {
                abaoNav.removeEventListener('click', onAbaoClick);
            });
        }

        // 5. visibilitychange
        function onVisChange() {
            if (document.hidden) {
                if (_sm && _sm.canPatrol) {
                    clearWalkTimers();
                    _pawPool.clear();
                    _sm.reset();
                }
            }
            // Visible again — idle detector handles restart automatically
        }
        document.addEventListener('visibilitychange', onVisChange);
        _eventCleanup.push(function() {
            document.removeEventListener('visibilitychange', onVisChange);
        });

        // 6. Debug events
        function onDebugParam(e) {
            var d = e.detail;
            if (d && d.key && d.value !== undefined) {
                _params[d.key] = d.value;
            }
        }
        document.addEventListener('patrol:debugParam', onDebugParam);
        _eventCleanup.push(function() {
            document.removeEventListener('patrol:debugParam', onDebugParam);
        });

        // 7. Page switch — update TerrainScanner view container
        function onPageSwitch(e) {
            if (typeof TerrainScanner === 'undefined') return;
            var page = e.detail && e.detail.page;
            if (!page) return;
            var viewEl = document.getElementById(page + '-view');
            if (viewEl) {
                TerrainScanner.setViewContainer(viewEl);
                TerrainScanner.invalidate();
            }
            // If walking, graceful exit (terrain changed)
            if (_sm && _sm.canPatrol) {
                _pawPool.fadeAll(150);
                clearWalkTimers();
                _sm.reset();
            }
        }
        document.addEventListener('patrol:pageSwitch', onPageSwitch);
        _eventCleanup.push(function() {
            document.removeEventListener('patrol:pageSwitch', onPageSwitch);
        });

        // 8. Task completion interaction (Phase 3)
        function onTaskComplete(e) {
            if (!_sm || !_sm.canPatrol) return;
            handleTaskComplete(e.detail);
        }
        document.addEventListener('patrol:taskComplete', onTaskComplete);
        _eventCleanup.push(function() {
            document.removeEventListener('patrol:taskComplete', onTaskComplete);
        });

        // 9. Jelly pill follow (Phase 3)
        function onJellyMove(e) {
            if (!_sm || !_sm.canPatrol) return;
            handleJellyFollow(e.detail);
        }
        document.addEventListener('patrol:jellyMove', onJellyMove);
        _eventCleanup.push(function() {
            document.removeEventListener('patrol:jellyMove', onJellyMove);
        });

        // 10. Leave Abao — logo wiggle (Phase 4, not gated by canPatrol)
        function onLeaveAbao() {
            var abaoIcon = document.querySelector('#mobile-nav-abao .nav-icon, #mobile-nav-abao img');
            if (!abaoIcon) return;
            abaoIcon.classList.remove('abao-wiggle');
            // Force reflow to restart animation
            void abaoIcon.offsetWidth;
            abaoIcon.classList.add('abao-wiggle');
            abaoIcon.addEventListener('animationend', function handler() {
                abaoIcon.classList.remove('abao-wiggle');
                abaoIcon.removeEventListener('animationend', handler);
            });
        }
        document.addEventListener('patrol:leaveAbao', onLeaveAbao);
        _eventCleanup.push(function() {
            document.removeEventListener('patrol:leaveAbao', onLeaveAbao);
        });

        // 11. Chat status (Phase 3 — not gated by canPatrol)
        function onChatStatus(e) {
            handleChatStatus(e.detail);
        }
        document.addEventListener('patrol:chatStatus', onChatStatus);
        _eventCleanup.push(function() {
            document.removeEventListener('patrol:chatStatus', onChatStatus);
        });

        function onDebugPause(e) {
            var paused = e.detail && e.detail.paused;
            if (paused) {
                clearWalkTimers();
            } else {
                // Resume: if in walk state, continue
                if (_sm && _sm.state === 'patrol') {
                    stepOnce();
                }
            }
        }
        document.addEventListener('patrol:debugPause', onDebugPause);
        _eventCleanup.push(function() {
            document.removeEventListener('patrol:debugPause', onDebugPause);
        });
    }

    function teardownEventBridge() {
        _eventCleanup.forEach(function(fn) { fn(); });
        _eventCleanup.length = 0;
        if (_modalObserver) {
            _modalObserver.disconnect();
            _modalObserver = null;
        }
        if (_scrollTimer) {
            clearTimeout(_scrollTimer);
            _scrollTimer = null;
        }
    }

    // ─── Public API ───

    return {
        init: function() {
            if (_initialized) return;

            // Gate checks
            if (typeof DeviceProfile === 'undefined') return;
            if (DeviceProfile.reduceMotion) return;
            if (!DeviceProfile.isSupported) return;
            if (!isMobile()) return;
            if (!isEnabled()) return;

            // Create patrol layer
            _layer = document.createElement('div');
            _layer.id = 'patrol-layer';
            if (DeviceProfile.tier === 'high') {
                _layer.classList.add('patrol-enhanced');
            }
            document.body.appendChild(_layer);

            // Create PawPool
            _pawPool = PawPool.create({
                container: _layer,
                size: 8
            });

            // Create StateMachine
            _sm = PatrolStateMachine.create({
                onStateChange: onStateChange
            });

            // Cache tab icon
            _tabIcon = document.querySelector('.tab-icon-patrol');

            // Create IdleDetector
            _idle = IdleDetector.create({
                idleThreshold: 3000,
                cooldown: 180000,
                onIdle: function() {
                    if (_sm) _sm.transition('idle');
                },
                onActive: function() {
                    // If in walk/pause/rest and user becomes active via scroll
                    // the scroll handler takes care of it
                }
            });
            _idle.start();

            // Phase 2: Initialize TerrainScanner
            if (typeof TerrainScanner !== 'undefined') {
                // Find current view container
                var viewIds = ['todo-view', 'review-view', 'english-view', 'life-view', 'settings-view'];
                for (var vi = 0; vi < viewIds.length; vi++) {
                    var viewEl = document.getElementById(viewIds[vi]);
                    if (viewEl && viewEl.style.display !== 'none') {
                        TerrainScanner.setViewContainer(viewEl);
                        break;
                    }
                }
            }

            // Phase 2: Create terrain debug overlay
            _terrainOverlay = document.createElement('div');
            _terrainOverlay.id = 'patrol-terrain-overlay';
            _terrainOverlay.style.cssText = 'position:fixed;top:0;left:0;width:100%;height:100%;pointer-events:none;display:none;z-index:9998;';
            document.body.appendChild(_terrainOverlay);
            if (typeof TerrainScanner !== 'undefined') {
                TerrainScanner.setOverlay(_terrainOverlay);
            }

            // EventBridge
            setupEventBridge();

            // Debug panel integration
            if (typeof PatrolDebug !== 'undefined') {
                PatrolDebug.connect({
                    stateMachine: _sm,
                    idleDetector: _idle,
                    pawPool: _pawPool,
                    terrainOverlay: _terrainOverlay
                });
            }

            // Abao button breathing (intermittent, scroll-aware)
            breatheInit();

            _initialized = true;
        },

        destroy: function() {
            if (!_initialized) return;
            breatheDestroy();

            clearWalkTimers();
            teardownEventBridge();

            // Phase 2: cleanup terrain
            if (typeof TerrainScanner !== 'undefined') {
                TerrainScanner.destroy();
            }
            if (_terrainOverlay && _terrainOverlay.parentNode) {
                _terrainOverlay.parentNode.removeChild(_terrainOverlay);
                _terrainOverlay = null;
            }

            if (_idle) { _idle.destroy(); _idle = null; }
            if (_pawPool) { _pawPool.destroy(); _pawPool = null; }
            if (_sm) { _sm.reset(); _sm = null; }
            if (_layer && _layer.parentNode) {
                _layer.parentNode.removeChild(_layer);
                _layer = null;
            }

            // Reset tab icon
            if (_tabIcon) {
                _tabIcon.classList.remove('patrol-out', 'patrol-returning', 'patrol-pulse');
                _tabIcon = null;
            }

            _initialized = false;
        },

        get enabled() {
            return _initialized;
        },

        /** Expose internal refs for admin PatrolLab panel */
        getDebugRefs: function() {
            if (!_initialized) return null;
            return {
                sm: _sm,
                idle: _idle,
                pawPool: _pawPool,
                terrainOverlay: _terrainOverlay
            };
        }
    };
})();
