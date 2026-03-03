/**
 * patrol-debug.js — 二狗巡山调试面板
 * SPEC-057 Phase 0d
 *
 * 仅 admin 用户可用。
 * 开启: localStorage.setItem('patrol-debug', '1'); location.reload();
 * 关闭: localStorage.removeItem('patrol-debug'); location.reload();
 */

var PatrolDebug = (function() {
    var panel = null;
    var rafId = null;
    var frameTimes = [];
    var overBudgetCount = 0;
    var totalFrames = 0;
    var _isAdmin = false;

    // References set by Patrol main module
    var _stateMachine = null;
    var _idleDetector = null;
    var _pawPool = null;
    var _terrainOverlay = null;

    function isEnabled() {
        return localStorage.getItem('patrol-debug') === '1';
    }

    // 验证当前用户是否为 admin（调 admin API，成功即有权限）
    async function checkAdmin() {
        if (typeof API === 'undefined' || !API.getAdminDashboard) return false;
        try {
            var data = await API.getAdminDashboard();
            return data && data.success;
        } catch (e) {
            return false;
        }
    }

    function createPanel() {
        panel = document.createElement('div');
        panel.id = 'patrol-debug';
        panel.innerHTML =
            '<style>' +
            '#patrol-debug {' +
            '  position:fixed; bottom:60px; right:8px; width:240px;' +
            '  background:rgba(0,0,0,0.9); color:#0f0; font:11px/1.4 monospace;' +
            '  border-radius:8px; padding:10px; z-index:10000;' +
            '  pointer-events:auto; user-select:none;' +
            '  max-height:70vh; overflow-y:auto;' +
            '}' +
            '#patrol-debug .pd-title { color:#fff; font-weight:700; margin-bottom:6px; font-size:12px; }' +
            '#patrol-debug .pd-row { display:flex; justify-content:space-between; margin-bottom:2px; }' +
            '#patrol-debug .pd-label { color:#888; }' +
            '#patrol-debug .pd-val { color:#0f0; }' +
            '#patrol-debug .pd-sep { border-top:1px solid #333; margin:6px 0; }' +
            '#patrol-debug .pd-btns { display:flex; flex-wrap:wrap; gap:4px; margin-top:6px; }' +
            '#patrol-debug .pd-btn {' +
            '  padding:3px 8px; border:1px solid #555; border-radius:4px;' +
            '  background:transparent; color:#ccc; cursor:pointer; font:10px monospace;' +
            '}' +
            '#patrol-debug .pd-btn:active { background:#333; }' +
            '#patrol-debug .pd-slider { width:100%; margin:2px 0; }' +
            '#patrol-debug .pd-slider-row { margin-bottom:4px; }' +
            '#patrol-debug .pd-slider-label { display:flex; justify-content:space-between; color:#888; font-size:10px; }' +
            '</style>' +
            '<div class="pd-title">Patrol Debug</div>' +

            '<!-- Status -->' +
            '<div class="pd-row"><span class="pd-label">State</span><span class="pd-val" id="pd-state">home</span></div>' +
            '<div class="pd-row"><span class="pd-label">Position</span><span class="pd-val" id="pd-pos">-</span></div>' +
            '<div class="pd-row"><span class="pd-label">Platform</span><span class="pd-val" id="pd-platform">-</span></div>' +
            '<div class="pd-row"><span class="pd-label">Paws</span><span class="pd-val" id="pd-paws">0/8</span></div>' +
            '<div class="pd-row"><span class="pd-label">Cooldown</span><span class="pd-val" id="pd-cooldown">0s</span></div>' +
            '<div class="pd-row"><span class="pd-label">Device</span><span class="pd-val" id="pd-device">-</span></div>' +

            '<div class="pd-sep"></div>' +

            '<!-- Performance -->' +
            '<div class="pd-row"><span class="pd-label">FPS</span><span class="pd-val" id="pd-fps">--</span></div>' +
            '<div class="pd-row"><span class="pd-label">Frame avg</span><span class="pd-val" id="pd-avg">--ms</span></div>' +
            '<div class="pd-row"><span class="pd-label">Frame peak</span><span class="pd-val" id="pd-peak">--ms</span></div>' +
            '<div class="pd-row"><span class="pd-label">Over 1ms</span><span class="pd-val" id="pd-over">0%</span></div>' +

            '<div class="pd-sep"></div>' +

            '<!-- Buttons -->' +
            '<div class="pd-btns">' +
            '  <button class="pd-btn" id="pd-btn-force">Force Out</button>' +
            '  <button class="pd-btn" id="pd-btn-home">Force Home</button>' +
            '  <button class="pd-btn" id="pd-btn-pause">Pause</button>' +
            '  <button class="pd-btn" id="pd-btn-cooldown">Reset CD</button>' +
            '  <button class="pd-btn" id="pd-btn-terrain">Terrain</button>' +
            '</div>' +

            '<div class="pd-sep"></div>' +

            '<!-- Sliders -->' +
            '<div class="pd-slider-row">' +
            '  <div class="pd-slider-label"><span>Idle threshold</span><span id="pd-idle-val">8s</span></div>' +
            '  <input class="pd-slider" type="range" min="1000" max="30000" value="8000" step="1000" id="pd-idle-slider">' +
            '</div>' +
            '<div class="pd-slider-row">' +
            '  <div class="pd-slider-label"><span>Cooldown</span><span id="pd-cd-val">3min</span></div>' +
            '  <input class="pd-slider" type="range" min="0" max="600000" value="180000" step="10000" id="pd-cd-slider">' +
            '</div>' +
            '<div class="pd-slider-row">' +
            '  <div class="pd-slider-label"><span>Paw opacity</span><span id="pd-opacity-val">0.5</span></div>' +
            '  <input class="pd-slider" type="range" min="0.1" max="1.0" value="0.5" step="0.05" id="pd-opacity-slider">' +
            '</div>' +
            '<div class="pd-slider-row">' +
            '  <div class="pd-slider-label"><span>Speed</span><span id="pd-speed-val">30px/s</span></div>' +
            '  <input class="pd-slider" type="range" min="10" max="100" value="30" step="5" id="pd-speed-slider">' +
            '</div>' +
            '<div class="pd-slider-row">' +
            '  <div class="pd-slider-label"><span>Paw size</span><span id="pd-size-val">20px</span></div>' +
            '  <input class="pd-slider" type="range" min="8" max="32" value="20" step="2" id="pd-size-slider">' +
            '</div>';

        document.body.appendChild(panel);
        bindEvents();
    }

    function bindEvents() {
        // Buttons
        document.getElementById('pd-btn-force').addEventListener('click', function() {
            if (_stateMachine) {
                _stateMachine.forceState('peek');
                setTimeout(function() {
                    if (_stateMachine) _stateMachine.transition('peekDone');
                }, 300);
            }
        });

        document.getElementById('pd-btn-home').addEventListener('click', function() {
            if (_stateMachine) _stateMachine.reset();
        });

        var paused = false;
        document.getElementById('pd-btn-pause').addEventListener('click', function() {
            paused = !paused;
            this.textContent = paused ? 'Resume' : 'Pause';
            // Dispatch custom event for Patrol main module to handle
            document.dispatchEvent(new CustomEvent('patrol:debugPause', { detail: { paused: paused } }));
        });

        document.getElementById('pd-btn-cooldown').addEventListener('click', function() {
            if (_idleDetector) _idleDetector.resetCooldown();
        });

        document.getElementById('pd-btn-terrain').addEventListener('click', function() {
            if (_terrainOverlay) {
                var visible = _terrainOverlay.style.display !== 'none';
                _terrainOverlay.style.display = visible ? 'none' : 'block';
            }
        });

        // Sliders
        document.getElementById('pd-idle-slider').addEventListener('input', function() {
            var v = parseInt(this.value);
            document.getElementById('pd-idle-val').textContent = (v / 1000) + 's';
            if (_idleDetector) _idleDetector.setIdleThreshold(v);
        });

        document.getElementById('pd-cd-slider').addEventListener('input', function() {
            var v = parseInt(this.value);
            var label = v >= 60000 ? (v / 60000).toFixed(1) + 'min' : (v / 1000) + 's';
            document.getElementById('pd-cd-val').textContent = label;
            if (_idleDetector) _idleDetector.setCooldown(v);
        });

        document.getElementById('pd-opacity-slider').addEventListener('input', function() {
            var v = parseFloat(this.value);
            document.getElementById('pd-opacity-val').textContent = v.toFixed(2);
            document.dispatchEvent(new CustomEvent('patrol:debugParam', {
                detail: { key: 'opacity', value: v }
            }));
        });

        document.getElementById('pd-speed-slider').addEventListener('input', function() {
            var v = parseInt(this.value);
            document.getElementById('pd-speed-val').textContent = v + 'px/s';
            document.dispatchEvent(new CustomEvent('patrol:debugParam', {
                detail: { key: 'speed', value: v }
            }));
        });

        document.getElementById('pd-size-slider').addEventListener('input', function() {
            var v = parseInt(this.value);
            document.getElementById('pd-size-val').textContent = v + 'px';
            document.dispatchEvent(new CustomEvent('patrol:debugParam', {
                detail: { key: 'size', value: v }
            }));
        });
    }

    function startPerfMonitor() {
        var last = performance.now();

        function tick(now) {
            var dt = now - last;
            last = now;
            frameTimes.push(dt);
            totalFrames++;
            if (dt > 1) overBudgetCount++;
            if (frameTimes.length > 120) frameTimes.shift();

            rafId = requestAnimationFrame(tick);
        }

        rafId = requestAnimationFrame(tick);
    }

    function updateDisplay() {
        if (!panel) return;

        // State
        var stateEl = document.getElementById('pd-state');
        if (_stateMachine) {
            stateEl.textContent = _stateMachine.state;
        }

        // Paws
        var pawsEl = document.getElementById('pd-paws');
        if (_pawPool) {
            pawsEl.textContent = _pawPool.activeCount + '/8';
        }

        // Cooldown
        var cdEl = document.getElementById('pd-cooldown');
        if (_idleDetector) {
            var rem = _idleDetector.cooldownRemaining;
            cdEl.textContent = rem > 0 ? (rem / 1000).toFixed(0) + 's' : 'ready';
            cdEl.style.color = rem > 0 ? '#f90' : '#0f0';
        }

        // Device
        if (typeof DeviceProfile !== 'undefined') {
            document.getElementById('pd-device').textContent = DeviceProfile.tier;
        }

        // Performance
        if (frameTimes.length > 10) {
            var sum = 0, peak = 0;
            for (var i = 0; i < frameTimes.length; i++) {
                sum += frameTimes[i];
                if (frameTimes[i] > peak) peak = frameTimes[i];
            }
            var avg = sum / frameTimes.length;

            document.getElementById('pd-fps').textContent = Math.round(1000 / avg);
            document.getElementById('pd-avg').textContent = avg.toFixed(1) + 'ms';
            document.getElementById('pd-peak').textContent = peak.toFixed(1) + 'ms';

            var overPct = totalFrames > 0 ? ((overBudgetCount / totalFrames) * 100).toFixed(1) : '0';
            var overEl = document.getElementById('pd-over');
            overEl.textContent = overPct + '%';
            overEl.style.color = parseFloat(overPct) > 5 ? '#f00' : '#0f0';
        }

        setTimeout(updateDisplay, 500);
    }

    return {
        init: async function() {
            if (!isEnabled()) return;
            // 仅 admin 可开启调试面板
            _isAdmin = await checkAdmin();
            if (!_isAdmin) return;
            createPanel();
            startPerfMonitor();
            updateDisplay();
        },

        // Called by Patrol main module to provide references
        connect: function(opts) {
            _stateMachine = opts.stateMachine || null;
            _idleDetector = opts.idleDetector || null;
            _pawPool = opts.pawPool || null;
            _terrainOverlay = opts.terrainOverlay || null;
        },

        updatePosition: function(x, y) {
            if (!panel) return;
            document.getElementById('pd-pos').textContent =
                Math.round(x) + ', ' + Math.round(y);
        },

        updatePlatform: function(id) {
            if (!panel) return;
            document.getElementById('pd-platform').textContent = id || '-';
        },

        destroy: function() {
            if (rafId) cancelAnimationFrame(rafId);
            if (panel && panel.parentNode) panel.parentNode.removeChild(panel);
            panel = null;
            _stateMachine = null;
            _idleDetector = null;
            _pawPool = null;
        },

        get enabled() {
            return isEnabled();
        }
    };
})();
