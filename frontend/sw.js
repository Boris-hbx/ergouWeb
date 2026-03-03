const CACHE_NAME = 'next-v22';

// Global error handlers
self.addEventListener('error', event => {
    console.error('[SW] uncaught:', event.error || event.message);
});
self.addEventListener('unhandledrejection', event => {
    console.error('[SW] unhandled rejection:', event.reason);
});
const STATIC_ASSETS = [
    '/',
    '/index.html',
    '/login.html',
    '/assets/css/base.css',
    '/assets/css/style.css',
    '/assets/css/components.css',
    '/assets/css/mobile.css',
    '/assets/css/abao.css',
    '/assets/js/api.js',
    '/assets/js/utils.js',
    '/assets/js/jelly-indicator.js',
    '/assets/js/app.js',
    '/assets/js/tasks.js',
    '/assets/js/modal.js',
    '/assets/js/drag.js',
    '/assets/js/review.js',
    '/assets/js/routines.js',
    '/assets/js/features.js',
    '/assets/js/abao.js',
    '/assets/js/settings.js',
    '/assets/js/notifications.js',
    '/assets/js/life.js',
    '/assets/js/expense.js',
    '/assets/js/trip.js',
    '/assets/js/utils.js',
    '/assets/js/health.js',
    '/assets/js/observability.js',
];

// Install: cache static assets
self.addEventListener('install', event => {
    event.waitUntil(
        caches.open(CACHE_NAME)
            .then(cache => cache.addAll(STATIC_ASSETS))
            .then(() => self.skipWaiting())
    );
});

// Activate: clean old caches
self.addEventListener('activate', event => {
    event.waitUntil(
        caches.keys().then(keys =>
            Promise.all(
                keys.filter(key => key !== CACHE_NAME)
                    .map(key => caches.delete(key))
            )
        ).then(() => self.clients.claim())
    );
});

// Push: show notification when push arrives
self.addEventListener('push', event => {
    let data = { title: '提醒', body: '', type: 'reminder' };
    try {
        if (event.data) data = Object.assign(data, event.data.json());
    } catch(e) { console.error('[SW] push data parse error:', e); }

    const options = {
        body: data.body || '',
        icon: '/assets/icons/icon-192.png',
        badge: '/assets/icons/icon-192.png',
        tag: 'reminder-' + Date.now(),
        data: data,
        requireInteraction: true,
        actions: [
            { action: 'acknowledge', title: '知道了' },
            { action: 'snooze', title: '5分钟后' }
        ]
    };

    event.waitUntil(
        self.registration.showNotification(data.title, options)
            .then(() => {
                // Update app badge
                if ('setAppBadge' in self.navigator) {
                    return self.registration.getNotifications()
                        .then(notifs => self.navigator.setAppBadge(notifs.length));
                }
            })
    );
});

// Notification click handler
self.addEventListener('notificationclick', event => {
    const notification = event.notification;
    const action = event.action;
    notification.close();

    if (action === 'snooze') {
        // Snooze via API — try to find reminder_id from data
        event.waitUntil(
            fetch('/api/reminders/' + (notification.data && notification.data.reminder_id || 'unknown') + '/snooze', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                credentials: 'same-origin',
                body: JSON.stringify({ minutes: 5 })
            }).catch(e => { console.error('[SW] snooze failed:', e); })
        );
        return;
    }

    // Default click or 'acknowledge' — open the app
    event.waitUntil(
        clients.matchAll({ type: 'window', includeUncontrolled: true })
            .then(windowClients => {
                // Focus existing window if any
                for (const client of windowClients) {
                    if (client.url.includes('/') && 'focus' in client) {
                        return client.focus();
                    }
                }
                // Open new window
                return clients.openWindow('/');
            })
            .then(() => {
                // Clear badge
                if ('clearAppBadge' in self.navigator) {
                    return self.navigator.clearAppBadge();
                }
            })
    );
});

// Fetch: network first, fallback to cache
self.addEventListener('fetch', event => {
    const url = new URL(event.request.url);

    // API requests: always network, no SW interference
    if (url.pathname.startsWith('/api/')) {
        return;
    }

    // HTML pages: always fetch fresh from network (never cache)
    // This prevents stale HTML (e.g. wrong charset) after deployments
    const isHtml = url.pathname === '/'
        || url.pathname.endsWith('.html')
        || url.pathname === '';
    if (isHtml) {
        event.respondWith(fetch(event.request));
        return;
    }

    // Static assets: network first, fallback to cache
    event.respondWith(
        fetch(event.request)
            .then(response => {
                // Cache successful responses
                if (response.ok) {
                    const responseClone = response.clone();
                    caches.open(CACHE_NAME).then(cache => {
                        cache.put(event.request, responseClone);
                    });
                }
                return response;
            })
            .catch(err => {
                // Fallback to cache
                console.error('[SW] fetch failed:', event.request.url, err);
                return caches.match(event.request);
            })
    );
});
