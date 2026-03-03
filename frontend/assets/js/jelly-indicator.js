/**
 * JellyIndicator — Bouncy pill indicator with spring physics
 * Ported from ripple-test.html prototype (Telegram-style tab animation)
 */
(function() {
    'use strict';

    // ========== Spring Physics ==========

    function spring(pos, vel, target, stiffness, damping, dt) {
        var f = -stiffness * (pos - target) - damping * vel;
        vel += f * dt;
        pos += vel * dt;
        return { p: pos, v: vel };
    }

    function settled(p, v, t, threshold) {
        threshold = threshold || 0.3;
        return Math.abs(p - t) < threshold && Math.abs(v) < threshold;
    }

    // ========== Global Resize Handler ==========

    var _instances = [];
    var _resizeTimer = null;

    window.addEventListener('resize', function() {
        clearTimeout(_resizeTimer);
        _resizeTimer = setTimeout(function() {
            for (var i = 0; i < _instances.length; i++) {
                _instances[i].recalculate();
            }
        }, 150);
    });

    // ========== JellyIndicator Constructor ==========

    function JellyIndicator(opts) {
        this.container = typeof opts.container === 'string'
            ? document.querySelector(opts.container)
            : opts.container;
        this.itemSelector = opts.itemSelector;
        this.activeClass = opts.activeClass || 'active';
        this.axis = opts.axis || 'x'; // 'x' horizontal | 'y' vertical
        this.skipIndices = opts.skipIndices || [];
        this.padding = opts.padding != null ? opts.padding : 4;
        this.pillClass = opts.pillClass || '';
        this.bounce = opts.bounce !== false; // default true
        this._anim = null;
        this._pill = null;
        this._currentTarget = null;
        this._hidden = true;

        if (this.container) {
            this._createPill();
            _instances.push(this);
        }
    }

    // ========== Prototype Methods ==========

    JellyIndicator.prototype._createPill = function() {
        this._pill = document.createElement('div');
        this._pill.className = 'jelly-pill' + (this.pillClass ? ' ' + this.pillClass : '');
        this._pill.style.opacity = '0';
        // Ensure container has relative positioning for absolute pill
        var pos = getComputedStyle(this.container).position;
        if (pos === 'static') {
            this.container.style.position = 'relative';
        }
        this.container.insertBefore(this._pill, this.container.firstChild);
    };

    JellyIndicator.prototype._getRect = function(el) {
        var containerRect = this.container.getBoundingClientRect();
        var elRect = el.getBoundingClientRect();
        return {
            left: elRect.left - containerRect.left,
            top: elRect.top - containerRect.top,
            width: elRect.width,
            height: elRect.height
        };
    };

    JellyIndicator.prototype._setPill = function(pos, size, crossRect) {
        var pill = this._pill;
        var pad = this.padding;
        if (this.axis === 'x') {
            pill.style.left = (pos - pad) + 'px';
            pill.style.width = (size + pad * 2) + 'px';
            if (crossRect) {
                // Match element's vertical position exactly
                pill.style.top = (crossRect.top - pad) + 'px';
                pill.style.height = (crossRect.height + pad * 2) + 'px';
            } else {
                pill.style.top = pad + 'px';
                pill.style.height = 'calc(100% - ' + (pad * 2) + 'px)';
            }
        } else {
            pill.style.top = (pos - pad) + 'px';
            pill.style.height = (size + pad * 2) + 'px';
            if (crossRect) {
                pill.style.left = (crossRect.left - pad) + 'px';
                pill.style.width = (crossRect.width + pad * 2) + 'px';
            } else {
                pill.style.left = pad + 'px';
                pill.style.width = 'calc(100% - ' + (pad * 2) + 'px)';
            }
        }
    };

    JellyIndicator.prototype.moveTo = function(el) {
        if (!this._pill) return;

        // Hide pill
        if (!el) {
            this._hidden = true;
            this._currentTarget = null;
            this._pill.style.opacity = '0';
            if (this._anim) {
                cancelAnimationFrame(this._anim);
                this._anim = null;
            }
            return;
        }

        // Skip if same target
        if (el === this._currentTarget) return;

        var prevTarget = this._currentTarget;
        this._currentTarget = el;

        // If was hidden or no prev, just jump
        if (this._hidden || !prevTarget) {
            this.setPositionImmediate(el);
            return;
        }

        // Cancel any running animation
        if (this._anim) {
            cancelAnimationFrame(this._anim);
            this._anim = null;
        }

        var self = this;
        var prevRect = this._getRect(prevTarget);
        var targetRect = this._getRect(el);

        // If target is on a different row/column (cross-axis differs), snap instead of animate
        var crossDiff = this.axis === 'x'
            ? Math.abs(targetRect.top - prevRect.top)
            : Math.abs(targetRect.left - prevRect.left);
        if (crossDiff > 5) {
            this.setPositionImmediate(el);
            return;
        }

        var isHoriz = this.axis === 'x';
        var fromPos = isHoriz ? prevRect.left + prevRect.width / 2 : prevRect.top + prevRect.height / 2;
        var toPos = isHoriz ? targetRect.left + targetRect.width / 2 : targetRect.top + targetRect.height / 2;
        var fromSize = isHoriz ? prevRect.width : prevRect.height;
        var targetSize = isHoriz ? targetRect.width : targetRect.height;

        // Animation state
        var pos = fromPos, vPos = 0;
        var size = fromSize, vSize = 0;
        var useBounce = this.bounce;
        var minDim = Math.min(targetRect.width, targetRect.height);
        var squeezeTarget = targetSize - (targetSize - minDim) * 0.4;
        var phase = 0; // 0 = shrink+slide, 1 = expand+settle
        var last = performance.now();

        function step(now) {
            var dt = Math.min((now - last) / 1000, 0.03);
            last = now;

            // Position: smooth slide toward target
            var rp = spring(pos, vPos, toPos, 400, 30, dt);
            pos = rp.p; vPos = rp.v;

            if (useBounce) {
                // Bounce mode: squeeze while sliding, then expand with overshoot
                if (phase === 0) {
                    var rs = spring(size, vSize, squeezeTarget, 500, 35, dt);
                    size = rs.p; vSize = rs.v;
                    if (Math.abs(pos - toPos) < 6) {
                        phase = 1;
                        vSize = 5;
                    }
                } else {
                    var rs = spring(size, vSize, targetSize, 350, 22, dt);
                    size = rs.p; vSize = rs.v;
                }
            } else {
                // Simple mode: size smoothly interpolates, no squeeze/bounce
                var rs = spring(size, vSize, targetSize, 400, 30, dt);
                size = rs.p; vSize = rs.v;
                phase = 1; // always in "settle" phase for termination check
            }

            var clampedSize = useBounce ? Math.max(size, squeezeTarget * 0.9) : Math.max(size, 1);
            self._setPill(pos - clampedSize / 2, clampedSize, targetRect);

            if (phase === 1 && settled(pos, vPos, toPos, 0.3) && settled(size, vSize, targetSize, 0.4)) {
                self._setPill(toPos - targetSize / 2, targetSize, targetRect);
                self._anim = null;
                return;
            }
            self._anim = requestAnimationFrame(step);
        }
        this._anim = requestAnimationFrame(step);
    };

    JellyIndicator.prototype.setPositionImmediate = function(el) {
        if (!this._pill || !el) return;
        this._currentTarget = el;
        this._hidden = false;
        this._pill.style.opacity = '1';

        // Mark container as jelly-ready so CSS can drop fallback backgrounds
        if (!this.container.classList.contains('jelly-ready')) {
            this.container.classList.add('jelly-ready');
        }

        if (this._anim) {
            cancelAnimationFrame(this._anim);
            this._anim = null;
        }

        var rect = this._getRect(el);
        if (this.axis === 'x') {
            this._setPill(rect.left, rect.width, rect);
        } else {
            this._setPill(rect.top, rect.height, rect);
        }
    };

    JellyIndicator.prototype.recalculate = function() {
        if (this._currentTarget && !this._hidden && this.container.offsetWidth > 0) {
            this.setPositionImmediate(this._currentTarget);
        }
    };

    JellyIndicator.prototype.destroy = function() {
        if (this._anim) {
            cancelAnimationFrame(this._anim);
            this._anim = null;
        }
        if (this._pill && this._pill.parentNode) {
            this._pill.parentNode.removeChild(this._pill);
        }
        var idx = _instances.indexOf(this);
        if (idx !== -1) _instances.splice(idx, 1);
    };

    // ========== Factory ==========

    JellyIndicator.create = function(opts) {
        return new JellyIndicator(opts);
    };

    // ========== Export ==========

    window.JellyIndicator = JellyIndicator;
})();
