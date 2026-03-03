// ========== Observability: 全局错误拦截 + Breadcrumb + 上报 ==========
var Observability = (function() {
    // --- Breadcrumb ring buffer (max 20) ---
    var _breadcrumbs = [];
    var MAX_BREADCRUMBS = 20;

    // --- Session flood protection ---
    var _reportCount = 0;
    var MAX_REPORTS_PER_SESSION = 10;
    var _reportedMessages = {}; // dedup by error_message

    // --- Offline buffer key ---
    var OFFLINE_KEY = 'obs_offline_errors';
    var MAX_OFFLINE = 5;

    function init() {
        _installGlobalHandlers();
        _flushOfflineBuffer();
    }

    // ─── Breadcrumb ───

    function addBreadcrumb(category, message) {
        var entry = {
            ts: new Date().toTimeString().slice(0, 8),
            cat: category,
            msg: message
        };
        _breadcrumbs.push(entry);
        if (_breadcrumbs.length > MAX_BREADCRUMBS) {
            _breadcrumbs.shift();
        }
    }

    function getBreadcrumbs() {
        return _breadcrumbs.slice();
    }

    // ─── Global error handlers ───

    function _installGlobalHandlers() {
        window.onerror = function(message, source, lineno, colno, error) {
            _captureError({
                error_message: String(message),
                stack: error && error.stack ? error.stack : (source + ':' + lineno + ':' + colno)
            });
        };

        window.addEventListener('unhandledrejection', function(event) {
            var reason = event.reason;
            var message = reason instanceof Error ? reason.message : String(reason);
            var stack = reason instanceof Error ? reason.stack : '';
            _captureError({
                error_message: 'Unhandled rejection: ' + message,
                stack: stack || ''
            });
        });
    }

    // ─── Capture + report ───

    function _captureError(info) {
        // Dedup
        if (_reportedMessages[info.error_message]) return;
        _reportedMessages[info.error_message] = true;

        // Session limit
        if (_reportCount >= MAX_REPORTS_PER_SESSION) return;
        _reportCount++;

        var payload = {
            error_message: info.error_message,
            stack: info.stack || '',
            app_version: _getAppVersion(),
            url: window.location.href,
            user_agent: navigator.userAgent,
            screen_size: screen.width + 'x' + screen.height,
            network_online: navigator.onLine,
            user_id: _getUserId(),
            breadcrumbs: getBreadcrumbs(),
            timestamp: new Date().toISOString()
        };

        _sendReport(payload);
    }

    function _sendReport(payload) {
        fetch('/api/client-errors', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload)
        }).catch(function() {
            // Offline: buffer to localStorage
            _bufferOffline(payload);
        });
    }

    function _bufferOffline(payload) {
        try {
            var buf = JSON.parse(localStorage.getItem(OFFLINE_KEY) || '[]');
            if (buf.length >= MAX_OFFLINE) return;
            buf.push(payload);
            localStorage.setItem(OFFLINE_KEY, JSON.stringify(buf));
        } catch (e) { /* quota exceeded, skip */ }
    }

    function _flushOfflineBuffer() {
        try {
            var raw = localStorage.getItem(OFFLINE_KEY);
            if (!raw) return;
            var buf = JSON.parse(raw);
            if (!buf.length) return;
            localStorage.removeItem(OFFLINE_KEY);
            buf.forEach(function(payload) {
                fetch('/api/client-errors', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(payload)
                }).catch(function() { /* give up on retry failure */ });
            });
        } catch (e) { /* corrupted data, clear */ localStorage.removeItem(OFFLINE_KEY); }
    }

    // ─── Helpers ───

    function _getAppVersion() {
        // Extract from first <link> or <script> ?v= parameter
        var link = document.querySelector('link[rel="stylesheet"][href*="?v="]');
        if (link) {
            var match = link.href.match(/\?v=([^&]+)/);
            if (match) return match[1];
        }
        return 'unknown';
    }

    function _getUserId() {
        return (window._currentUser && window._currentUser.id) || localStorage.getItem('username') || '';
    }

    // ─── Eruda DevTools ───

    function _initEruda() {
        // Check URL param
        var params = new URLSearchParams(window.location.search);
        if (params.has('debug')) {
            if (params.get('debug') === '0') {
                localStorage.removeItem('eruda_enabled');
                if (window.eruda) { try { eruda.destroy(); } catch(e){} }
                return;
            }
            if (params.get('debug') === '1') {
                localStorage.setItem('eruda_enabled', '1');
            }
        }

        // Load if enabled
        if (localStorage.getItem('eruda_enabled') === '1') {
            _loadEruda();
        }

        // Hidden gesture: 5 taps on version/about element
        _setupErudaGesture();
    }

    function _loadEruda() {
        if (window.eruda) return; // already loaded
        var script = document.createElement('script');
        script.src = '/assets/vendor/eruda.min.js';
        script.onload = function() {
            if (window.eruda) eruda.init();
        };
        script.onerror = function() { /* silent fail */ };
        document.head.appendChild(script);
    }

    var _tapCount = 0;
    var _tapTimer = null;

    function _setupErudaGesture() {
        // Look for version/about element to attach gesture
        document.addEventListener('click', function(e) {
            var target = e.target;
            // Match version text or about link in settings
            if (!target.closest || !target.closest('#about-version, .about-version, [data-eruda-tap]')) return;

            _tapCount++;
            clearTimeout(_tapTimer);
            _tapTimer = setTimeout(function() { _tapCount = 0; }, 3000);

            if (_tapCount >= 5) {
                _tapCount = 0;
                if (localStorage.getItem('eruda_enabled') === '1') {
                    localStorage.removeItem('eruda_enabled');
                    if (window.eruda) { try { eruda.destroy(); } catch(e){} }
                    if (typeof showToast === 'function') showToast('DevTools 已关闭', 'info');
                } else {
                    localStorage.setItem('eruda_enabled', '1');
                    _loadEruda();
                    if (typeof showToast === 'function') showToast('DevTools 已开启', 'info');
                }
            }
        });
    }

    // ─── Public API ───

    return {
        init: init,
        initEruda: _initEruda,
        addBreadcrumb: addBreadcrumb,
        getBreadcrumbs: getBreadcrumbs
    };
})();

// Auto-init on load
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', function() {
        Observability.init();
        Observability.initEruda();
    });
} else {
    Observability.init();
    Observability.initEruda();
}
