// ========== Behavior Tracker (T-219 / SPEC analytics) ==========
//
// Zero-dependency, declarative behavior tracking for the web (desktop) client.
// - clicks: global event delegation on elements carrying `data-track`
// - pageview/dwell: hooks the top-level `switchPage` router + admin sub-sections
// - batch upload to `/api/events/batch` via navigator.sendBeacon (fetch fallback)
//
// Best-effort by design: never throws into the UI, never blocks, never retries.
// Analytics data may be dropped, but it must never break the main experience.
// `user_id` is NOT sent — the backend takes it from the session guard.

var Tracker = (function() {
    var ENDPOINT = '/api/events/batch';
    var FLUSH_SIZE = 20;          // flush when the queue reaches this many events
    var FLUSH_INTERVAL_MS = 10000; // ...or every 10s
    var LABEL_MAX = 64;            // truncate element label text
    var MAX_DWELL_MS = 24 * 3600 * 1000; // ignore absurd dwell (clock jumps / sleep)
    var VALID = { pageview: 1, click: 1, dwell: 1, input: 1, custom: 1 };

    var _queue = [];
    var _sessionId = null;
    var _route = null;   // current view, e.g. 'work' / 'admin/users'
    var _enterTs = 0;    // perf timestamp when current route was entered
    var _timer = null;
    var _started = false;

    function _perf() {
        return (window.performance && performance.now) ? performance.now() : Date.now();
    }

    function _uuid() {
        try {
            if (window.crypto && crypto.randomUUID) return crypto.randomUUID();
        } catch (_) { /* fall through */ }
        return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {
            var r = Math.random() * 16 | 0;
            return (c === 'x' ? r : (r & 0x3 | 0x8)).toString(16);
        });
    }

    function _getSessionId() {
        try {
            var k = 'ergou_track_sid';
            var v = sessionStorage.getItem(k);
            if (!v) { v = _uuid(); sessionStorage.setItem(k, v); }
            return v;
        } catch (_) {
            return _sessionId || (_sessionId = _uuid());
        }
    }

    function _rfc3339() {
        try { return new Date().toISOString(); } catch (_) { return ''; }
    }

    function _enqueue(ev) {
        if (!ev || !VALID[ev.event_type]) return;
        ev.client_ts = _rfc3339();
        _queue.push(ev);
        if (_queue.length >= FLUSH_SIZE) flush();
    }

    function flush() {
        if (!_queue.length) return;
        var batch = _queue.splice(0, _queue.length);
        var payload;
        try {
            payload = JSON.stringify({ session_id: _getSessionId(), events: batch });
        } catch (e) {
            console.error('[Tracker] serialize', e);
            return;
        }
        try {
            if (navigator.sendBeacon) {
                var blob = new Blob([payload], { type: 'application/json' });
                if (navigator.sendBeacon(ENDPOINT, blob)) return;
            }
            fetch(ENDPOINT, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: payload,
                credentials: 'same-origin',
                keepalive: true
            }).catch(function(e) { console.error('[Tracker] flush', e); });
        } catch (e) {
            console.error('[Tracker] flush', e);
        }
    }

    function _emitDwell() {
        if (!_route || !_enterTs) return;
        var ms = Math.round(_perf() - _enterTs);
        if (ms > 0 && ms < MAX_DWELL_MS) {
            _enqueue({ event_type: 'dwell', route: _route, dwell_ms: ms });
        }
    }

    // Record entry into a view: close out the previous view's dwell, emit pageview.
    function setRoute(route) {
        if (!route || route === _route) return;
        _emitDwell();
        _route = route;
        _enterTs = _perf();
        _enqueue({ event_type: 'pageview', route: _route });
    }

    // Manual / custom event hook for code-driven tracking.
    function track(targetId, opts) {
        opts = opts || {};
        var label = opts.label;
        if (label && label.length > LABEL_MAX) label = label.slice(0, LABEL_MAX);
        _enqueue({
            event_type: opts.event_type || 'custom',
            target_id: targetId,
            target_label: label,
            route: opts.route || _route,
            dwell_ms: opts.dwell_ms,
            meta: opts.meta
        });
    }

    function _onClick(e) {
        var node = e.target && e.target.closest ? e.target.closest('[data-track]') : null;
        if (!node) return;
        var id = node.getAttribute('data-track');
        if (!id) return;
        var label = (node.getAttribute('data-track-label') || node.textContent || '').trim();
        if (label.length > LABEL_MAX) label = label.slice(0, LABEL_MAX);
        _enqueue({ event_type: 'click', target_id: id, target_label: label, route: _route });
    }

    function _hookRouting() {
        // Top-level pages: wrap the global switchPage router.
        if (typeof window.switchPage === 'function' && !window.switchPage.__tracked) {
            var orig = window.switchPage;
            var wrapped = function(page) {
                var r = orig.apply(this, arguments);
                try { setRoute(page); } catch (_) { /* never block nav */ }
                return r;
            };
            wrapped.__tracked = true;
            window.switchPage = wrapped;
        }
        // Admin sub-sections: refine route when an admin nav item is clicked.
        document.addEventListener('click', function(e) {
            var item = e.target && e.target.closest
                ? e.target.closest('.admin-nav-item[data-section]') : null;
            if (item) {
                try { setRoute('admin/' + item.getAttribute('data-section')); } catch (_) { /* noop */ }
            }
        }, true);
    }

    function init() {
        if (_started) return;
        _started = true;
        try {
            document.addEventListener('click', _onClick, true);
            _hookRouting();
            setRoute(window.currentPage || 'todo'); // initial pageview
            _timer = setInterval(flush, FLUSH_INTERVAL_MS);

            document.addEventListener('visibilitychange', function() {
                if (document.visibilityState === 'hidden') {
                    _emitDwell();        // count time up to hide
                    _enterTs = _perf();  // don't count the hidden gap
                    flush();
                } else {
                    _enterTs = _perf();  // resume timing on return
                }
            });
            window.addEventListener('pagehide', function() { _emitDwell(); flush(); });
            window.addEventListener('beforeunload', function() { _emitDwell(); flush(); });
        } catch (e) {
            console.error('[Tracker] init', e);
        }
    }

    return { init: init, track: track, setRoute: setRoute, flush: flush };
})();
