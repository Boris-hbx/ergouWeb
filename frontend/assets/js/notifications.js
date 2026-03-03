// ========== 通知系统 (轮询 + 铃铛 + 横幅 + Web Push) ==========
var Notifications = (function() {
    var pollTimer = null;
    var POLL_INTERVAL = 30000; // 30 seconds
    var panelOpen = false;
    var lastItems = [];
    var shownBannerIds = {}; // track which reminders already showed a banner
    var pushSubscribed = false; // whether push is active

    function init() {
        startPolling();
        // Try to register push silently if already granted
        checkAndRegisterPush();
        // Close panel on outside click
        document.addEventListener('click', function(e) {
            if (panelOpen) {
                var panel = document.getElementById('notif-panel');
                var bell = document.getElementById('notif-bell-wrapper');
                if (panel && !panel.contains(e.target) && bell && !bell.contains(e.target)) {
                    closePanel();
                }
            }
        });
    }

    // ===== Web Push Subscription =====

    function isPushSupported() {
        return 'serviceWorker' in navigator && 'PushManager' in window && 'Notification' in window;
    }

    function isPushGranted() {
        return isPushSupported() && Notification.permission === 'granted';
    }

    // Silently register if permission is already granted
    async function checkAndRegisterPush() {
        if (!isPushSupported()) return;
        if (Notification.permission === 'granted') {
            await subscribePush();
        }
        updatePushStatus();
    }

    // Request permission and subscribe — called from UI or Abao prompt
    async function requestAndSubscribe() {
        if (!isPushSupported()) {
            if (typeof showToast === 'function') showToast('你的浏览器不支持推送通知', 'error');
            return false;
        }
        try {
            var perm = await Notification.requestPermission();
            if (perm !== 'granted') {
                if (typeof showToast === 'function') showToast('推送权限被拒绝', 'error');
                return false;
            }
            var ok = await subscribePush();
            if (ok && typeof showToast === 'function') showToast('推送通知已开启', 'success');
            updatePushStatus();
            return ok;
        } catch(e) {
            console.error('[Push] requestAndSubscribe error:', e);
            return false;
        }
    }

    async function subscribePush() {
        try {
            var reg = await navigator.serviceWorker.ready;
            // Get VAPID public key
            var keyData = await API.getVapidPublicKey();
            if (!keyData || !keyData.success || !keyData.key) {
                console.warn('[Push] VAPID key not available');
                return false;
            }
            var vapidKey = urlBase64ToUint8Array(keyData.key);

            // Subscribe
            var sub = await reg.pushManager.subscribe({
                userVisibleOnly: true,
                applicationServerKey: vapidKey
            });

            // Send subscription to server
            var subJson = sub.toJSON();
            var result = await API.subscribePush({
                endpoint: subJson.endpoint,
                p256dh: subJson.keys.p256dh,
                auth: subJson.keys.auth
            });

            if (result && result.success) {
                pushSubscribed = true;
                return true;
            }
            return false;
        } catch(e) {
            console.error('[Push] subscribe error:', e);
            return false;
        }
    }

    async function unsubscribePush() {
        try {
            var reg = await navigator.serviceWorker.ready;
            var sub = await reg.pushManager.getSubscription();
            if (sub) {
                var endpoint = sub.endpoint;
                await sub.unsubscribe();
                await API.unsubscribePush(endpoint).catch(function(e) {
                    console.error('[notifications] unsubscribePush:', e);
                });
            }
            pushSubscribed = false;
            updatePushStatus();
            if (typeof showToast === 'function') showToast('推送通知已关闭');
        } catch(e) {
            console.error('[Push] unsubscribe error:', e);
        }
    }

    function updatePushStatus() {
        var statusEl = document.getElementById('push-status-text');
        var toggleBtn = document.getElementById('push-toggle-btn');
        if (!statusEl || !toggleBtn) return;

        if (!isPushSupported()) {
            statusEl.textContent = '你的浏览器不支持推送通知';
            toggleBtn.style.display = 'none';
            return;
        }

        if (Notification.permission === 'denied') {
            statusEl.textContent = '推送已被浏览器禁止，请在浏览器设置中开启';
            toggleBtn.style.display = 'none';
            return;
        }

        if (Notification.permission === 'granted' && pushSubscribed) {
            statusEl.textContent = '已开启';
            toggleBtn.textContent = '关闭推送';
            toggleBtn.style.display = '';
            toggleBtn.onclick = function() { unsubscribePush(); };
        } else {
            statusEl.textContent = '未开启';
            toggleBtn.textContent = '开启推送';
            toggleBtn.style.display = '';
            toggleBtn.onclick = function() { requestAndSubscribe(); };
        }
    }

    function urlBase64ToUint8Array(base64String) {
        var padding = '='.repeat((4 - base64String.length % 4) % 4);
        var base64 = (base64String + padding).replace(/-/g, '+').replace(/_/g, '/');
        var rawData = window.atob(base64);
        var outputArray = new Uint8Array(rawData.length);
        for (var i = 0; i < rawData.length; ++i) {
            outputArray[i] = rawData.charCodeAt(i);
        }
        return outputArray;
    }

    function startPolling() {
        poll(); // immediate first poll
        if (pollTimer) clearInterval(pollTimer);
        pollTimer = setInterval(poll, POLL_INTERVAL);
    }

    async function poll() {
        try {
            var data = await API.getUnreadNotifications();
            if (!data || !data.success) return;
            updateBadge(data.count);
            lastItems = data.items || [];
            if (panelOpen) renderPanel();
            // Show banners for new triggered reminders
            showBannersForNew(lastItems);
        } catch(e) {
            console.error('[notifications] poll:', e);
        }
    }

    function updateBadge(count) {
        var badge = document.getElementById('notif-badge');
        if (!badge) return;
        if (count > 0) {
            badge.textContent = count > 99 ? '99+' : count;
            badge.style.display = '';
        } else {
            badge.style.display = 'none';
        }
        // Desktop badge API
        if ('setAppBadge' in navigator) {
            if (count > 0) navigator.setAppBadge(count);
            else navigator.clearAppBadge();
        }
    }

    function togglePanel(e) {
        if (e) e.stopPropagation();
        if (panelOpen) {
            closePanel();
        } else {
            openPanel();
        }
    }

    function openPanel() {
        var panel = document.getElementById('notif-panel');
        if (!panel) return;
        panelOpen = true;
        panel.style.display = '';
        renderPanel();
    }

    function closePanel() {
        var panel = document.getElementById('notif-panel');
        if (!panel) return;
        panelOpen = false;
        panel.style.display = 'none';
    }

    function renderPanel() {
        var body = document.getElementById('notif-panel-body');
        if (!body) return;

        if (!lastItems || lastItems.length === 0) {
            body.innerHTML = '<div class="notif-empty">暂无通知</div>';
            return;
        }

        var html = '';
        for (var i = 0; i < lastItems.length; i++) {
            var item = lastItems[i];
            var timeStr = formatRelativeTime(item.created_at);
            var icon = item.type === 'reminder' ? '🔔' : '📢';
            html += '<div class="notif-item" data-id="' + escapeHtml(item.id) + '">';
            html += '<div class="notif-item-icon">' + icon + '</div>';
            html += '<div class="notif-item-content">';
            html += '<div class="notif-item-title">' + escapeHtml(item.title) + '</div>';
            html += '<div class="notif-item-body">' + escapeHtml(item.body) + '</div>';
            html += '<div class="notif-item-time">' + timeStr + '</div>';
            html += '</div>';
            html += '<div class="notif-item-actions">';
            if (item.type === 'reminder' && item.reminder_id) {
                html += '<button class="notif-ack-btn" onclick="Notifications.acknowledgeFromPanel(\'' + escapeHtml(item.reminder_id) + '\', \'' + escapeHtml(item.id) + '\')" title="知道了">✓</button>';
                html += '<button class="notif-snooze-btn" onclick="Notifications.snoozeFromPanel(\'' + escapeHtml(item.reminder_id) + '\', \'' + escapeHtml(item.id) + '\')" title="5分钟后再提醒">⏰</button>';
            } else {
                html += '<button class="notif-ack-btn" onclick="Notifications.dismissNotif(\'' + escapeHtml(item.id) + '\')" title="已读">✓</button>';
            }
            html += '</div>';
            html += '</div>';
        }
        body.innerHTML = html;
    }

    function showBannersForNew(items) {
        var container = document.getElementById('notif-banner-container');
        if (!container) return;

        // Only show banners for reminder-type notifications not yet shown
        var reminderItems = items.filter(function(it) {
            return it.type === 'reminder' && it.reminder_id && !shownBannerIds[it.id];
        });

        // If more than 3 pending, show aggregate banner
        if (reminderItems.length > 3) {
            // Only show aggregate if we haven't shown it yet
            if (!shownBannerIds['_aggregate']) {
                shownBannerIds['_aggregate'] = true;
                showBanner({
                    id: '_aggregate',
                    title: '你有 ' + reminderItems.length + ' 条提醒未处理',
                    body: '点击铃铛查看详情',
                    isAggregate: true
                });
            }
            // Mark all as shown so we don't re-banner
            reminderItems.forEach(function(it) { shownBannerIds[it.id] = true; });
            return;
        }

        for (var i = 0; i < reminderItems.length; i++) {
            var item = reminderItems[i];
            shownBannerIds[item.id] = true;
            showBanner(item);
        }
    }

    function showBanner(item) {
        var container = document.getElementById('notif-banner-container');
        if (!container) return;

        var banner = document.createElement('div');
        banner.className = 'notif-banner';
        banner.setAttribute('data-notif-id', item.id);

        var html = '<div class="notif-banner-content">';
        html += '<div class="notif-banner-icon">🔔</div>';
        html += '<div class="notif-banner-text">';
        html += '<div class="notif-banner-title">' + escapeHtml(item.title) + '</div>';
        html += '<div class="notif-banner-body">' + escapeHtml(item.body || '') + '</div>';
        html += '</div>';
        html += '</div>';
        html += '<div class="notif-banner-actions">';
        if (item.isAggregate) {
            html += '<button class="notif-banner-btn notif-banner-primary" onclick="Notifications.togglePanel(); Notifications.dismissBanner(this)">查看</button>';
        } else {
            html += '<button class="notif-banner-btn notif-banner-primary" onclick="Notifications.ackBanner(\'' + escapeHtml(item.reminder_id || '') + '\', \'' + escapeHtml(item.id) + '\', this)">知道了</button>';
            html += '<button class="notif-banner-btn" onclick="Notifications.snoozeBanner(\'' + escapeHtml(item.reminder_id || '') + '\', \'' + escapeHtml(item.id) + '\', this)">5分钟后</button>';
            if (item.todo_id) {
                html += '<button class="notif-banner-btn" onclick="Notifications.openTodo(\'' + escapeHtml(item.todo_id) + '\', this)">打开任务</button>';
            }
        }
        html += '</div>';

        banner.innerHTML = html;
        container.appendChild(banner);
        // Trigger animation
        requestAnimationFrame(function() {
            banner.classList.add('notif-banner-show');
        });
    }

    function dismissBanner(btn) {
        var banner = btn.closest('.notif-banner');
        if (banner) {
            banner.classList.remove('notif-banner-show');
            banner.classList.add('notif-banner-hide');
            setTimeout(function() { banner.remove(); }, 300);
        }
    }

    async function ackBanner(reminderId, notifId, btn) {
        if (reminderId) {
            await API.acknowledgeReminder(reminderId).catch(function(e) {
                console.error('[notifications] ackBanner:', e);
            });
        }
        dismissBanner(btn);
        poll();
    }

    async function snoozeBanner(reminderId, notifId, btn) {
        if (reminderId) {
            await API.snoozeReminder(reminderId, 5).catch(function(e) {
                console.error('[notifications] snoozeBanner:', e);
            });
        }
        dismissBanner(btn);
        poll();
        if (typeof showToast === 'function') showToast('5分钟后再提醒你');
    }

    function openTodo(todoId, btn) {
        dismissBanner(btn);
        closePanel();
        // Open todo detail modal
        if (typeof openTaskDetail === 'function') {
            openTaskDetail(todoId);
        }
    }

    async function acknowledgeFromPanel(reminderId, notifId) {
        await API.acknowledgeReminder(reminderId).catch(function(e) {
            console.error('[notifications] acknowledgeFromPanel:', e);
        });
        poll();
    }

    async function snoozeFromPanel(reminderId, notifId) {
        await API.snoozeReminder(reminderId, 5).catch(function(e) {
            console.error('[notifications] snoozeFromPanel:', e);
        });
        poll();
        if (typeof showToast === 'function') showToast('5分钟后再提醒你');
    }

    async function dismissNotif(notifId) {
        await API.markNotificationRead(notifId).catch(function(e) {
            console.error('[notifications] dismissNotif:', e);
        });
        poll();
    }

    async function readAll() {
        await API.markAllNotificationsRead().catch(function(e) {
            console.error('[notifications] readAll:', e);
        });
        // Clear all banners
        var container = document.getElementById('notif-banner-container');
        if (container) container.innerHTML = '';
        shownBannerIds = {};
        poll();
    }

    function formatRelativeTime(isoStr) {
        try {
            var dt = new Date(isoStr);
            var now = new Date();
            var diff = Math.floor((now - dt) / 1000);
            if (diff < 60) return '刚刚';
            if (diff < 3600) return Math.floor(diff / 60) + '分钟前';
            if (diff < 86400) return Math.floor(diff / 3600) + '小时前';
            return Math.floor(diff / 86400) + '天前';
        } catch(e) {
            return '';
        }
    }

    function escapeHtml(s) {
        if (!s) return '';
        return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
    }

    // Auto-init when DOM is ready
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }

    return {
        poll: poll,
        togglePanel: togglePanel,
        readAll: readAll,
        acknowledgeFromPanel: acknowledgeFromPanel,
        snoozeFromPanel: snoozeFromPanel,
        dismissNotif: dismissNotif,
        ackBanner: ackBanner,
        snoozeBanner: snoozeBanner,
        dismissBanner: dismissBanner,
        openTodo: openTodo,
        // Push subscription
        isPushSupported: isPushSupported,
        isPushGranted: isPushGranted,
        requestAndSubscribe: requestAndSubscribe,
        unsubscribePush: unsubscribePush,
        updatePushStatus: updatePushStatus
    };
})();
