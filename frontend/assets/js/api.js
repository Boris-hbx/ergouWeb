// ========== API 封装层 (REST fetch + Cookie 认证) ==========
var API = (function() {
    var BASE = '/api';

    async function request(method, path, body) {
        var opts = {
            method: method,
            headers: {},
            credentials: 'same-origin'
        };
        if (body !== undefined) {
            opts.headers['Content-Type'] = 'application/json';
            opts.body = JSON.stringify(body);
        }
        var _t0 = Date.now();
        try {
            var resp = await fetch(BASE + path, opts);
            var data = await resp.json();
            // Breadcrumb: record API call
            if (typeof Observability !== 'undefined') {
                Observability.addBreadcrumb('api', method + ' ' + path + ' → ' + resp.status + ' (' + (Date.now() - _t0) + 'ms)');
            }
            if (resp.status === 401) {
                // Session expired, redirect to login
                window.location.href = '/login.html';
                throw new Error('UNAUTHORIZED');
            }
            if (resp.status === 403 && data && data.error === 'ACCOUNT_PENDING') {
                if (typeof showToast === 'function') showToast('账户审核中，暂时无法操作', 'warning');
                throw new Error('ACCOUNT_PENDING');
            }
            if (resp.status === 403 && data && data.error === 'GUEST_RESTRICTED') {
                if (typeof showToast === 'function') showToast('体验模式不支持此功能，注册账户解锁', 'warning');
                throw new Error('GUEST_RESTRICTED');
            }
            if (resp.status === 403 && data && data.error === 'GUEST_AI_EXHAUSTED') {
                if (typeof showToast === 'function') showToast('AI 体验次数已用完 — 注册账户解锁无限使用', 'warning');
                updateGuestAiCount(0);
                throw new Error('GUEST_AI_EXHAUSTED');
            }
            // Sync AI remaining count from any response
            if (data && data.ai_remaining !== undefined) {
                updateGuestAiCount(data.ai_remaining);
            }
            return data;
        } catch (err) {
            if (err.message === 'UNAUTHORIZED' || err.message === 'ACCOUNT_PENDING' || err.message === 'GUEST_RESTRICTED' || err.message === 'GUEST_AI_EXHAUSTED') throw err;
            // Breadcrumb: record failed API call
            if (typeof Observability !== 'undefined') {
                Observability.addBreadcrumb('api', method + ' ' + path + ' → ERR (' + (Date.now() - _t0) + 'ms)');
            }
            console.error('[API] error:', method, path, err);
            throw err;
        }
    }

    function updateGuestAiCount(remaining) {
        window._guestAiRemaining = remaining;
        var el = document.getElementById('guest-ai-count');
        if (el) el.textContent = remaining;
        if (typeof updateAllGuestAiHints === 'function') updateAllGuestAiHints();
    }

    return {
        // ===== Auth APIs =====
        register: async function(username, password, displayName) {
            return await request('POST', '/auth/register', {
                username: username,
                password: password,
                display_name: displayName || undefined
            });
        },

        login: async function(username, password) {
            return await request('POST', '/auth/login', {
                username: username,
                password: password
            });
        },

        logout: async function() {
            return await request('POST', '/auth/logout');
        },

        getMe: async function() {
            return await request('GET', '/auth/me');
        },

        changePassword: async function(oldPassword, newPassword) {
            return await request('POST', '/auth/change-password', {
                old_password: oldPassword,
                new_password: newPassword
            });
        },

        updateAvatar: async function(avatar) {
            return await request('PUT', '/auth/avatar', { avatar: avatar });
        },

        // ===== Settings APIs =====
        getAiModel: async function() {
            return await request('GET', '/settings/ai-model');
        },

        setAiModel: async function(model) {
            return await request('PUT', '/settings/ai-model', { model: model });
        },

        getTimezone: async function() {
            return await request('GET', '/settings/timezone');
        },

        setTimezone: async function(tz) {
            return await request('PUT', '/settings/timezone', { timezone: tz });
        },

        // ===== Todo APIs =====
        getTodos: async function(tab) {
            var path = '/todos';
            if (tab) path += '?tab=' + encodeURIComponent(tab);
            return await request('GET', path);
        },

        getTodo: async function(id) {
            return await request('GET', '/todos/' + encodeURIComponent(id));
        },

        createTodo: async function(data) {
            return await request('POST', '/todos', data);
        },

        updateTodo: async function(id, data) {
            return await request('PUT', '/todos/' + encodeURIComponent(id), data);
        },

        deleteTodo: async function(id) {
            return await request('DELETE', '/todos/' + encodeURIComponent(id));
        },

        restoreTodo: async function(id) {
            return await request('POST', '/todos/' + encodeURIComponent(id) + '/restore');
        },

        permanentDeleteTodo: async function(id) {
            return await request('DELETE', '/todos/' + encodeURIComponent(id) + '/permanent');
        },

        batchUpdateTodos: async function(updates) {
            return await request('PUT', '/todos/batch', updates);
        },

        getTodoCounts: async function(tab) {
            return await request('GET', '/todos/counts?tab=' + encodeURIComponent(tab));
        },

        // ===== Routine APIs =====
        getRoutines: async function() {
            return await request('GET', '/routines');
        },

        createRoutine: async function(text) {
            return await request('POST', '/routines', { text: text });
        },

        toggleRoutine: async function(id) {
            return await request('POST', '/routines/' + encodeURIComponent(id) + '/toggle');
        },

        deleteRoutine: async function(id) {
            return await request('DELETE', '/routines/' + encodeURIComponent(id));
        },

        // ===== Review APIs =====
        getReviews: async function() {
            return await request('GET', '/reviews');
        },

        createReview: async function(data) {
            return await request('POST', '/reviews', data);
        },

        updateReview: async function(id, data) {
            return await request('PUT', '/reviews/' + encodeURIComponent(id), data);
        },

        completeReview: async function(id) {
            return await request('POST', '/reviews/' + encodeURIComponent(id) + '/complete');
        },

        uncompleteReview: async function(id) {
            return await request('POST', '/reviews/' + encodeURIComponent(id) + '/uncomplete');
        },

        deleteReview: async function(id) {
            return await request('DELETE', '/reviews/' + encodeURIComponent(id));
        },

        // ===== Quote API =====
        getRandomQuote: async function() {
            return await request('GET', '/quotes/random');
        },

        // ===== Chat APIs (阿宝) =====
        sendChat: async function(message, conversationId) {
            return await request('POST', '/chat', {
                message: message,
                conversation_id: conversationId || undefined
            });
        },

        getConversations: async function() {
            return await request('GET', '/conversations');
        },

        getConversationMessages: async function(convId) {
            return await request('GET', '/conversations/' + encodeURIComponent(convId) + '/messages');
        },

        deleteConversation: async function(convId) {
            return await request('DELETE', '/conversations/' + encodeURIComponent(convId));
        },

        renameConversation: async function(convId, title) {
            return await request('POST', '/conversations/' + encodeURIComponent(convId) + '/rename', { title: title });
        },

        getChatUsage: async function() {
            return await request('GET', '/chat/usage');
        },

        // ===== English Scenario (Learn) APIs =====
        getScenarios: async function(archived, category) {
            var path = '/english/scenarios';
            var params = [];
            if (archived !== undefined) params.push('archived=' + archived);
            if (category) params.push('category=' + encodeURIComponent(category));
            if (params.length) path += '?' + params.join('&');
            return await request('GET', path);
        },

        createScenario: async function(data) {
            return await request('POST', '/english/scenarios', data);
        },

        getScenario: async function(id) {
            return await request('GET', '/english/scenarios/' + encodeURIComponent(id));
        },

        updateScenario: async function(id, data) {
            return await request('PUT', '/english/scenarios/' + encodeURIComponent(id), data);
        },

        deleteScenario: async function(id) {
            return await request('DELETE', '/english/scenarios/' + encodeURIComponent(id));
        },

        generateScenario: async function(id) {
            return await request('POST', '/english/scenarios/' + encodeURIComponent(id) + '/generate');
        },

        archiveScenario: async function(id) {
            return await request('POST', '/english/scenarios/' + encodeURIComponent(id) + '/archive');
        },

        // ===== Friends APIs =====
        getFriends: async function() {
            return await request('GET', '/friends');
        },

        getFriendRequests: async function() {
            return await request('GET', '/friends/requests');
        },

        sendFriendRequest: async function(username) {
            return await request('POST', '/friends/request', { username: username });
        },

        acceptFriend: async function(id) {
            return await request('POST', '/friends/' + encodeURIComponent(id) + '/accept');
        },

        declineFriend: async function(id) {
            return await request('POST', '/friends/' + encodeURIComponent(id) + '/decline');
        },

        deleteFriend: async function(id) {
            return await request('DELETE', '/friends/' + encodeURIComponent(id));
        },

        searchUsers: async function(q) {
            return await request('GET', '/friends/search?q=' + encodeURIComponent(q));
        },

        // ===== Share APIs =====
        shareItem: async function(friendId, itemType, itemId, message) {
            return await request('POST', '/share', {
                friend_id: friendId,
                item_type: itemType,
                item_id: itemId,
                message: message || undefined
            });
        },

        getSharedSent: async function(itemType, itemId) {
            return await request('GET', '/share/sent?item_type=' + encodeURIComponent(itemType) + '&item_id=' + encodeURIComponent(itemId));
        },

        getSharedInbox: async function(type) {
            var path = '/share/inbox';
            if (type) path += '?type=' + encodeURIComponent(type);
            return await request('GET', path);
        },

        getSharedInboxCount: async function() {
            return await request('GET', '/share/inbox/count');
        },

        acceptShared: async function(id) {
            return await request('POST', '/share/' + encodeURIComponent(id) + '/accept');
        },

        dismissShared: async function(id) {
            return await request('POST', '/share/' + encodeURIComponent(id) + '/dismiss');
        },

        // ===== Reminder APIs =====
        getReminders: async function(status) {
            var path = '/reminders';
            if (status) path += '?status=' + encodeURIComponent(status);
            return await request('GET', path);
        },

        createReminder: async function(data) {
            return await request('POST', '/reminders', data);
        },

        updateReminder: async function(id, data) {
            return await request('PUT', '/reminders/' + encodeURIComponent(id), data);
        },

        cancelReminder: async function(id) {
            return await request('DELETE', '/reminders/' + encodeURIComponent(id));
        },

        acknowledgeReminder: async function(id) {
            return await request('POST', '/reminders/' + encodeURIComponent(id) + '/acknowledge');
        },

        snoozeReminder: async function(id, minutes) {
            return await request('POST', '/reminders/' + encodeURIComponent(id) + '/snooze', { minutes: minutes || 5 });
        },

        getReminderPendingCount: async function() {
            return await request('GET', '/reminders/pending-count');
        },

        // ===== Notification APIs =====
        getUnreadNotifications: async function() {
            return await request('GET', '/notifications/unread');
        },

        markNotificationRead: async function(id) {
            return await request('POST', '/notifications/' + encodeURIComponent(id) + '/read');
        },

        markAllNotificationsRead: async function() {
            return await request('POST', '/notifications/read-all');
        },

        // ===== Push Subscription APIs =====
        getVapidPublicKey: async function() {
            return await request('GET', '/push/vapid-public-key');
        },

        subscribePush: async function(subscription) {
            return await request('POST', '/push/subscribe', subscription);
        },

        unsubscribePush: async function(endpoint) {
            return await request('DELETE', '/push/subscribe', { endpoint: endpoint });
        },

        // ===== Contacts APIs =====
        getContacts: async function() {
            return await request('GET', '/contacts');
        },

        createContact: async function(data) {
            return await request('POST', '/contacts', data);
        },

        updateContact: async function(id, data) {
            return await request('PUT', '/contacts/' + encodeURIComponent(id), data);
        },

        deleteContact: async function(id) {
            return await request('DELETE', '/contacts/' + encodeURIComponent(id));
        },

        // ===== Collaboration APIs =====
        setTodoCollaborator: async function(todoId, friendId) {
            return await request('POST', '/collaborate/todos/' + encodeURIComponent(todoId), { friend_id: friendId });
        },

        removeTodoCollaborator: async function(todoId) {
            return await request('DELETE', '/collaborate/todos/' + encodeURIComponent(todoId));
        },

        setRoutineCollaborator: async function(routineId, friendId) {
            return await request('POST', '/collaborate/routines/' + encodeURIComponent(routineId), { friend_id: friendId });
        },

        removeRoutineCollaborator: async function(routineId) {
            return await request('DELETE', '/collaborate/routines/' + encodeURIComponent(routineId));
        },

        // ===== Confirmation APIs =====
        getPendingConfirmations: async function() {
            return await request('GET', '/collaborate/confirmations/pending');
        },

        respondConfirmation: async function(id, response) {
            return await request('POST', '/collaborate/confirmations/' + encodeURIComponent(id) + '/respond', { response: response });
        },

        withdrawConfirmation: async function(id) {
            return await request('POST', '/collaborate/confirmations/' + encodeURIComponent(id) + '/withdraw');
        },

        // ===== Moment API =====
        getMoment: async function() {
            return await request('GET', '/moment');
        },

        // ===== Expense APIs =====
        getExpenses: async function(from, to, tags) {
            var params = [];
            if (from) params.push('from=' + encodeURIComponent(from));
            if (to) params.push('to=' + encodeURIComponent(to));
            if (tags) params.push('tags=' + encodeURIComponent(tags));
            var path = '/expenses' + (params.length ? '?' + params.join('&') : '');
            return await request('GET', path);
        },

        createExpense: async function(data) {
            return await request('POST', '/expenses', data);
        },

        getExpense: async function(id) {
            return await request('GET', '/expenses/' + encodeURIComponent(id));
        },

        updateExpense: async function(id, data) {
            return await request('PUT', '/expenses/' + encodeURIComponent(id), data);
        },

        deleteExpense: async function(id) {
            return await request('DELETE', '/expenses/' + encodeURIComponent(id));
        },

        getExpenseTags: async function() {
            return await request('GET', '/expenses/tags');
        },

        parseExpenseReceipts: async function(id) {
            return await request('POST', '/expenses/' + encodeURIComponent(id) + '/parse');
        },

        getExpenseStats: async function(period, date) {
            var params = 'period=' + encodeURIComponent(period);
            if (date) params += '&date=' + encodeURIComponent(date);
            return await request('GET', '/expenses/stats?' + params);
        },

        parseExpensePreview: async function(images, text) {
            var body = { images: images || [] };
            if (text) body.text = text;
            return await request('POST', '/expenses/parse-preview', body);
        },

        deleteExpensePhoto: async function(photoId) {
            return await request('DELETE', '/expenses/photos/' + encodeURIComponent(photoId));
        },

        // ===== Trip APIs (差旅) =====
        getTrips: async function() {
            return await request('GET', '/trips');
        },

        createTrip: async function(data) {
            return await request('POST', '/trips', data);
        },

        getTrip: async function(id) {
            return await request('GET', '/trips/' + encodeURIComponent(id));
        },

        updateTrip: async function(id, data) {
            return await request('PUT', '/trips/' + encodeURIComponent(id), data);
        },

        deleteTrip: async function(id) {
            return await request('DELETE', '/trips/' + encodeURIComponent(id));
        },

        createTripItem: async function(tripId, data) {
            return await request('POST', '/trips/' + encodeURIComponent(tripId) + '/items', data);
        },

        updateTripItem: async function(itemId, data) {
            return await request('PUT', '/trips/items/' + encodeURIComponent(itemId), data);
        },

        deleteTripItem: async function(itemId) {
            return await request('DELETE', '/trips/items/' + encodeURIComponent(itemId));
        },

        uploadTripItemPhotos: async function(itemId, files) {
            var formData = new FormData();
            for (var i = 0; i < files.length; i++) {
                formData.append('photos', files[i]);
            }
            var resp = await fetch(BASE + '/trips/items/' + encodeURIComponent(itemId) + '/photos', {
                method: 'POST',
                credentials: 'same-origin',
                body: formData
            });
            return await resp.json();
        },

        deleteTripPhoto: async function(photoId) {
            return await request('DELETE', '/trips/photos/' + encodeURIComponent(photoId));
        },

        // AI分析：图片(base64数组) + 可选文字 → 差旅条目数组
        analyzeTripItem: async function(data) {
            return await request('POST', '/trips/analyze', data);
        },

        addTripCollaborator: async function(tripId, friendId, role) {
            return await request('POST', '/trips/' + encodeURIComponent(tripId) + '/collaborators', {
                friend_id: friendId,
                role: role || 'viewer'
            });
        },

        removeTripCollaborator: async function(tripId, userId) {
            return await request('DELETE', '/trips/' + encodeURIComponent(tripId) + '/collaborators/' + encodeURIComponent(userId));
        },

        // ===== Admin APIs =====
        getAdminDashboard: async function() {
            return await request('GET', '/admin/dashboard');
        },
        getPendingUsers: async function() {
            return await request('GET', '/admin/pending-users');
        },
        approveUser: async function(id) {
            return await request('POST', '/admin/users/' + encodeURIComponent(id) + '/approve');
        },
        rejectUser: async function(id) {
            return await request('POST', '/admin/users/' + encodeURIComponent(id) + '/reject');
        },
        getSecurityEvents: async function() {
            return await request('GET', '/admin/security-events');
        },
        restoreUser: async function(id) {
            return await request('POST', '/admin/users/' + encodeURIComponent(id) + '/restore');
        },
        // Generic admin request helper for new admin panel endpoints
        adminRequest: async function(method, path, body) {
            return await request(method, path, body);
        },

        // ===== Memory APIs =====
        getMemories: async function() {
            return await request('GET', '/settings/memories');
        },
        deleteMemory: async function(id) {
            return await request('DELETE', '/settings/memories/' + encodeURIComponent(id));
        },
        deleteAllMemories: async function() {
            return await request('DELETE', '/settings/memories');
        },

        // 环境检测 (always web now)
        isTauri: function() { return false; }
    };
})();
