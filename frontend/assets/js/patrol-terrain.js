/**
 * patrol-terrain.js — 二狗巡游地形感知系统
 * SPEC-057 Phase 2: 地形感知
 *
 * 包含: TerrainScanner, PathPlanner
 * 依赖: patrol-utils.js (DeviceProfile)
 */

// ============================================================
// 1. TerrainScanner — DOM 地形扫描与平台缓存
// ============================================================

var TerrainScanner = (function() {
    'use strict';

    // Platform types
    var PLATFORM = 'platform';
    var GROUND = 'ground';
    var OBSTACLE = 'obstacle';

    // CSS selector → terrain type mapping
    var SELECTOR_MAP = [
        // Platforms (walkable top edges)
        { selector: '.quadrant:not(.collapsed)', type: PLATFORM },
        { selector: '.task-item', type: PLATFORM },
        { selector: '.expense-card', type: PLATFORM },
        { selector: '.review-card', type: PLATFORM },
        { selector: '.routine-item', type: PLATFORM },
        { selector: '.settings-section', type: PLATFORM },
        { selector: '.english-card', type: PLATFORM },
        { selector: '.life-card', type: PLATFORM },
        { selector: '.health-card', type: PLATFORM },

        // Ground (walk along)
        { selector: 'hr', type: GROUND },
        { selector: '.divider', type: GROUND },
        { selector: '.separator', type: GROUND },
        { selector: '.quadrant-header', type: GROUND },

        // Obstacles (avoid)
        { selector: 'button:not(.quadrant-header button)', type: OBSTACLE },
        { selector: '.btn', type: OBSTACLE },
        { selector: '.mobile-fab', type: OBSTACLE },
        { selector: '.mobile-nav', type: OBSTACLE }
    ];

    var _cache = [];        // [{el, type, rect, dirty, id}]
    var _idCounter = 0;
    var _observer = null;
    var _viewContainer = null;
    var _overlayEl = null;   // debug terrain overlay

    function isInViewport(rect) {
        return rect.bottom > 0 &&
               rect.top < window.innerHeight &&
               rect.right > 0 &&
               rect.left < window.innerWidth &&
               rect.width > 10 && rect.height > 10; // skip tiny elements
    }

    function scanElements(container) {
        var results = [];
        var root = container || document.body;

        for (var i = 0; i < SELECTOR_MAP.length; i++) {
            var mapping = SELECTOR_MAP[i];
            var els;
            try {
                els = root.querySelectorAll(mapping.selector);
            } catch (e) {
                continue;
            }
            for (var j = 0; j < els.length; j++) {
                var el = els[j];
                // Skip hidden elements
                if (el.offsetParent === null && el.style.display === 'none') continue;
                var rect = el.getBoundingClientRect();
                if (!isInViewport(rect)) continue;

                results.push({
                    el: el,
                    type: mapping.type,
                    rect: {
                        top: rect.top,
                        bottom: rect.bottom,
                        left: rect.left,
                        right: rect.right,
                        width: rect.width,
                        height: rect.height
                    },
                    dirty: false,
                    id: 'p' + (++_idCounter)
                });
            }
        }

        return results;
    }

    function setupObserver() {
        if (_observer) _observer.disconnect();

        _observer = new MutationObserver(function() {
            // Mark all cached platforms as dirty
            for (var i = 0; i < _cache.length; i++) {
                _cache[i].dirty = true;
            }
        });

        if (_viewContainer) {
            _observer.observe(_viewContainer, {
                childList: true,
                subtree: true,
                attributes: true,
                attributeFilter: ['class', 'style']
            });
        }
    }

    function updateOverlay() {
        if (!_overlayEl) return;
        _overlayEl.innerHTML = '';
        for (var i = 0; i < _cache.length; i++) {
            var p = _cache[i];
            var div = document.createElement('div');
            div.style.cssText =
                'position:fixed;pointer-events:none;border:1px solid;' +
                'top:' + p.rect.top + 'px;' +
                'left:' + p.rect.left + 'px;' +
                'width:' + p.rect.width + 'px;' +
                'height:' + p.rect.height + 'px;';
            if (p.type === PLATFORM) {
                div.style.background = 'rgba(0,255,0,0.15)';
                div.style.borderColor = 'rgba(0,255,0,0.4)';
            } else if (p.type === GROUND) {
                div.style.background = 'rgba(0,0,255,0.15)';
                div.style.borderColor = 'rgba(0,0,255,0.4)';
            } else if (p.type === OBSTACLE) {
                div.style.background = 'rgba(255,0,0,0.15)';
                div.style.borderColor = 'rgba(255,0,0,0.4)';
            }
            _overlayEl.appendChild(div);
        }
    }

    return {
        PLATFORM: PLATFORM,
        GROUND: GROUND,
        OBSTACLE: OBSTACLE,

        /** Full viewport scan */
        scan: function() {
            _cache = scanElements(_viewContainer);
            updateOverlay();
            return _cache;
        },

        /** Scan ahead from position in a direction, return nearest 2-3 platforms */
        scanAhead: function(x, y, direction) {
            // Refresh dirty items first
            var hasDirty = false;
            for (var i = 0; i < _cache.length; i++) {
                if (_cache[i].dirty) { hasDirty = true; break; }
            }
            if (hasDirty) {
                _cache = scanElements(_viewContainer);
                updateOverlay();
            }

            // Filter platforms ahead of (x, y) in the given direction
            var ahead = [];
            for (var j = 0; j < _cache.length; j++) {
                var p = _cache[j];
                if (p.type === OBSTACLE) continue;

                var centerX = (p.rect.left + p.rect.right) / 2;
                var centerY = (p.rect.top + p.rect.bottom) / 2;

                // Check if platform is "ahead" based on direction (degrees, 0=up, 90=right)
                var dx = centerX - x;
                var dy = centerY - y;
                var dist = Math.sqrt(dx * dx + dy * dy);
                if (dist < 20) continue; // skip current platform
                if (dist > 300) continue; // too far

                // Rough direction check (within 120 degree cone)
                var angleToTarget = ((Math.atan2(dy, dx) * 180 / Math.PI) + 90 + 360) % 360;
                var angleDiff = Math.abs(angleToTarget - direction);
                if (angleDiff > 180) angleDiff = 360 - angleDiff;
                if (angleDiff < 90) {
                    ahead.push({ platform: p, dist: dist });
                }
            }

            // Sort by distance, return nearest 3
            ahead.sort(function(a, b) { return a.dist - b.dist; });
            var result = [];
            for (var k = 0; k < Math.min(3, ahead.length); k++) {
                result.push(ahead[k].platform);
            }
            return result;
        },

        /** Find platform at given coordinates */
        getPlatformAt: function(x, y) {
            for (var i = 0; i < _cache.length; i++) {
                var r = _cache[i].rect;
                if (x >= r.left && x <= r.right && y >= r.top - 10 && y <= r.bottom + 10) {
                    return _cache[i];
                }
            }
            return null;
        },

        /** Check if a platform is still valid (not dirty/gone) */
        isPlatformValid: function(platformId) {
            for (var i = 0; i < _cache.length; i++) {
                if (_cache[i].id === platformId) {
                    if (_cache[i].dirty) {
                        // Re-check element
                        var rect = _cache[i].el.getBoundingClientRect();
                        if (rect.width < 5 || rect.height < 5) return false;
                        _cache[i].rect = {
                            top: rect.top, bottom: rect.bottom,
                            left: rect.left, right: rect.right,
                            width: rect.width, height: rect.height
                        };
                        _cache[i].dirty = false;
                    }
                    return true;
                }
            }
            return false;
        },

        setViewContainer: function(el) {
            _viewContainer = el;
            setupObserver();
        },

        invalidate: function() {
            for (var i = 0; i < _cache.length; i++) {
                _cache[i].dirty = true;
            }
        },

        setOverlay: function(el) {
            _overlayEl = el;
        },

        get platforms() {
            return _cache;
        },

        destroy: function() {
            if (_observer) {
                _observer.disconnect();
                _observer = null;
            }
            _cache = [];
            _viewContainer = null;
            if (_overlayEl) {
                _overlayEl.innerHTML = '';
                _overlayEl = null;
            }
        }
    };
})();


// ============================================================
// 2. PathPlanner — 地形感知路径规划
// ============================================================

var PathPlanner = (function() {
    'use strict';

    /**
     * Generate a path along a platform's top edge.
     * Returns [{x, y, platformId, onEdge}]
     */
    function walkAlongPlatform(platform, startFromLeft, steps) {
        var r = platform.rect;
        var path = [];
        var n = steps || Math.max(5, Math.min(10, Math.floor(r.width / 40)));
        var margin = 16; // don't walk off the very edge

        var x1 = r.left + margin;
        var x2 = r.right - margin;
        var y = r.top - 2; // just above top edge

        if (startFromLeft) {
            for (var i = 0; i < n; i++) {
                var t = i / (n - 1);
                path.push({
                    x: x1 + t * (x2 - x1),
                    y: y,
                    platformId: platform.id,
                    onEdge: (i === 0 || i === n - 1)
                });
            }
        } else {
            for (var j = 0; j < n; j++) {
                var t2 = j / (n - 1);
                path.push({
                    x: x2 - t2 * (x2 - x1),
                    y: y,
                    platformId: platform.id,
                    onEdge: (j === 0 || j === n - 1)
                });
            }
        }

        return path;
    }

    /**
     * Generate a jump arc between two points.
     * Returns 3-4 path points forming a parabola.
     */
    function jumpArc(fromX, fromY, toX, toY) {
        var midX = (fromX + toX) / 2;
        var jumpHeight = Math.min(40, Math.abs(toX - fromX) * 0.3 + 15);
        var midY = Math.min(fromY, toY) - jumpHeight;

        return [
            { x: fromX, y: fromY, onEdge: true },
            { x: midX, y: midY },
            { x: toX, y: toY, onEdge: true }
        ];
    }

    /**
     * Check if any obstacles overlap a path segment.
     * Returns obstacle rect or null.
     */
    function findObstacleOnPath(path, platforms) {
        var obstacles = platforms.filter(function(p) {
            return p.type === TerrainScanner.OBSTACLE;
        });

        for (var i = 0; i < path.length; i++) {
            var pt = path[i];
            for (var j = 0; j < obstacles.length; j++) {
                var r = obstacles[j].rect;
                if (pt.x >= r.left - 5 && pt.x <= r.right + 5 &&
                    pt.y >= r.top - 5 && pt.y <= r.bottom + 5) {
                    return obstacles[j];
                }
            }
        }
        return null;
    }

    /**
     * Generate detour around an obstacle.
     */
    function detourAround(obstacle, path, obstacleIndex) {
        var r = obstacle.rect;
        var beforePt = path[Math.max(0, obstacleIndex - 1)];
        var afterIdx = obstacleIndex;

        // Find where path exits the obstacle
        while (afterIdx < path.length) {
            var pt = path[afterIdx];
            if (pt.x < r.left - 5 || pt.x > r.right + 5 ||
                pt.y < r.top - 5 || pt.y > r.bottom + 5) {
                break;
            }
            afterIdx++;
        }
        var afterPt = path[Math.min(path.length - 1, afterIdx)];

        // Go above the obstacle
        var detourY = r.top - 12;
        var detour = [
            { x: beforePt.x, y: beforePt.y },
            { x: r.left - 8, y: detourY },
            { x: r.right + 8, y: detourY },
            { x: afterPt.x, y: afterPt.y }
        ];

        // Splice into path
        var newPath = path.slice(0, Math.max(0, obstacleIndex - 1));
        newPath = newPath.concat(detour);
        newPath = newPath.concat(path.slice(Math.min(path.length, afterIdx)));
        return newPath;
    }

    /**
     * Phase 1 fallback: random path in safe area.
     */
    function randomPath(startX, startY, params) {
        var vw = window.innerWidth;
        var vh = window.innerHeight;
        var stride = (params && params.stride) || 40;
        var steps = 5 + Math.floor(Math.random() * 4);
        var path = [];

        var angle = Math.random() * 360;
        var rad = angle * Math.PI / 180;
        var curveBias = (Math.random() - 0.5) * 0.02;
        var x = startX;
        var y = startY;

        for (var i = 0; i < steps; i++) {
            rad += curveBias;
            x += Math.cos(rad) * stride;
            y += Math.sin(rad) * stride;
            x = Math.max(16, Math.min(vw - 16, x));
            y = Math.max(44, Math.min(vh - 60, y));
            path.push({ x: x, y: y });
        }

        return path;
    }

    return {
        /**
         * Plan a path from startX,startY using available platforms.
         * @param {number} startX
         * @param {number} startY
         * @param {Array} platforms - from TerrainScanner.platforms
         * @param {object} params - {stride, lastHeading}
         * @returns {Array} [{x, y, platformId?, onEdge?}]
         */
        planPath: function(startX, startY, platforms, params) {
            if (!platforms || platforms.length === 0) {
                return randomPath(startX, startY, params);
            }

            // Filter to walkable platforms (platform + ground)
            var walkable = platforms.filter(function(p) {
                return p.type === TerrainScanner.PLATFORM ||
                       p.type === TerrainScanner.GROUND;
            });

            if (walkable.length === 0) {
                return randomPath(startX, startY, params);
            }

            // Find nearest platform to start position
            var best = null;
            var bestDist = Infinity;
            for (var i = 0; i < walkable.length; i++) {
                var p = walkable[i];
                var cx = (p.rect.left + p.rect.right) / 2;
                var cy = p.rect.top;
                var d = Math.sqrt((cx - startX) * (cx - startX) + (cy - startY) * (cy - startY));
                if (d < bestDist) {
                    bestDist = d;
                    best = p;
                }
            }

            if (!best || bestDist > 200) {
                return randomPath(startX, startY, params);
            }

            // Determine walk direction (left-to-right or right-to-left)
            var centerX = (best.rect.left + best.rect.right) / 2;
            var startFromLeft = startX <= centerX;

            // Generate path along this platform
            var path = walkAlongPlatform(best, startFromLeft);

            // If start is far from platform, prepend a jump arc
            if (bestDist > 30) {
                var firstPt = path[0];
                var jumpPath = jumpArc(startX, startY, firstPt.x, firstPt.y);
                path = jumpPath.concat(path.slice(1));
            }

            // Check for obstacles and add detours
            var obstacle = findObstacleOnPath(path, platforms);
            if (obstacle) {
                // Find first path point inside obstacle
                for (var k = 0; k < path.length; k++) {
                    var pt = path[k];
                    var r = obstacle.rect;
                    if (pt.x >= r.left - 5 && pt.x <= r.right + 5 &&
                        pt.y >= r.top - 5 && pt.y <= r.bottom + 5) {
                        path = detourAround(obstacle, path, k);
                        break;
                    }
                }
            }

            return path;
        },

        /** Exposed for fallback use */
        randomPath: randomPath
    };
})();
