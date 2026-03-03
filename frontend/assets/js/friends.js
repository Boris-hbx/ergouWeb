// ========== 好友与分享模块 (IIFE) ==========
var Friends = (function() {
    var friends = [];
    var friendRequests = [];
    var sharedItems = [];
    var initialized = false;

    function init() {
        if (initialized) return;
        initialized = true;
        bindEvents();
    }

    function bindEvents() {
        // Add friend button in settings
        var addFriendBtn = document.getElementById('add-friend-btn');
        if (addFriendBtn) addFriendBtn.onclick = openSearchModal;

        // Search modal
        var searchOverlay = document.getElementById('friend-search-overlay');
        if (searchOverlay) searchOverlay.onclick = closeSearchModal;

        var searchInput = document.getElementById('friend-search-input');
        if (searchInput) {
            var debounceTimer;
            searchInput.oninput = function() {
                clearTimeout(debounceTimer);
                debounceTimer = setTimeout(function() {
                    searchUsers(searchInput.value.trim());
                }, 300);
            };
        }

    }

    // ─── Friends Management (Settings page) ───

    async function loadFriendsData() {
        init();
        if (window._userStatus === 'guest') return; // Guest: skip friend loading
        try {
            var [friendsResp, requestsResp] = await Promise.all([
                API.getFriends(),
                API.getFriendRequests()
            ]);
            if (friendsResp.success) friends = friendsResp.items || [];
            if (requestsResp.success) friendRequests = requestsResp.items || [];
            renderFriendsList();
            renderFriendRequests();
        } catch (e) {
            console.error('[Friends] load failed:', e);
        }
    }

    function renderFriendsList() {
        var container = document.getElementById('friends-list');
        if (!container) return;

        if (friends.length === 0) {
            container.innerHTML = '<div class="friends-empty">还没有好友</div>';
            return;
        }

        container.innerHTML = friends.map(function(f) {
            var name = f.display_name || f.username;
            var initial = name.charAt(0).toUpperCase();
            return '<div class="friend-item">' +
                '<div class="friend-avatar">' + initial + '</div>' +
                '<div class="friend-info">' +
                    '<span class="friend-name">' + escapeHtml(name) + '</span>' +
                    '<span class="friend-username">@' + escapeHtml(f.username) + '</span>' +
                '</div>' +
                '<button class="friend-remove-btn" onclick="Friends.removeFriend(\'' + f.friendship_id + '\')" title="删除好友">&times;</button>' +
            '</div>';
        }).join('');
    }

    function renderFriendRequests() {
        var container = document.getElementById('friend-requests');
        if (!container) return;

        if (friendRequests.length === 0) {
            container.innerHTML = '';
            return;
        }

        container.innerHTML = '<h5>待处理的好友请求</h5>' + friendRequests.map(function(r) {
            var name = r.from_user.display_name || r.from_user.username;
            return '<div class="friend-request-item">' +
                '<span class="friend-request-name">' + escapeHtml(name) + ' (@' + escapeHtml(r.from_user.username) + ')</span>' +
                '<div class="friend-request-actions">' +
                    '<button class="btn-accept" onclick="Friends.acceptRequest(\'' + r.id + '\')">接受</button>' +
                    '<button class="btn-decline" onclick="Friends.declineRequest(\'' + r.id + '\')">拒绝</button>' +
                '</div>' +
            '</div>';
        }).join('');
    }

    async function acceptRequest(id) {
        try {
            var resp = await API.acceptFriend(id);
            if (resp.success) {
                showToast('已接受', 'success');
                if (typeof ShareModal !== 'undefined') ShareModal.invalidateCache();
                loadFriendsData();
            } else {
                showToast(resp.message || '操作失败', 'error');
            }
        } catch (e) {
            showToast('操作失败', 'error');
        }
    }

    async function declineRequest(id) {
        try {
            var resp = await API.declineFriend(id);
            if (resp.success) {
                showToast('已拒绝', 'success');
                friendRequests = friendRequests.filter(function(r) { return r.id !== id; });
                renderFriendRequests();
            }
        } catch (e) {
            showToast('操作失败', 'error');
        }
    }

    async function removeFriend(friendshipId) {
        if (!confirm('确定删除这位好友吗？')) return;
        try {
            var resp = await API.deleteFriend(friendshipId);
            if (resp.success) {
                showToast('已删除好友', 'success');
                if (typeof ShareModal !== 'undefined') ShareModal.invalidateCache();
                friends = friends.filter(function(f) { return f.friendship_id !== friendshipId; });
                renderFriendsList();
            }
        } catch (e) {
            showToast('操作失败', 'error');
        }
    }

    // ─── Search Modal ───

    function openSearchModal() {
        var overlay = document.getElementById('friend-search-overlay');
        if (overlay) overlay.style.display = 'flex';
        var input = document.getElementById('friend-search-input');
        if (input) { input.value = ''; input.focus(); }
        var results = document.getElementById('friend-search-results');
        if (results) results.innerHTML = '';
    }

    function closeSearchModal() {
        var overlay = document.getElementById('friend-search-overlay');
        if (overlay) overlay.style.display = 'none';
    }

    async function searchUsers(query) {
        var results = document.getElementById('friend-search-results');
        if (!results) return;
        if (!query || query.length < 1) {
            results.innerHTML = '';
            return;
        }

        try {
            var resp = await API.searchUsers(query);
            if (resp.success && resp.items) {
                if (resp.items.length === 0) {
                    results.innerHTML = '<div class="search-empty">没有找到用户</div>';
                    return;
                }
                results.innerHTML = resp.items.map(function(u) {
                    var name = u.display_name || u.username;
                    var isFriend = friends.some(function(f) { return f.id === u.id; });
                    var btn = isFriend
                        ? '<span class="already-friend">已是好友</span>'
                        : '<button class="btn-add-friend" onclick="Friends.sendRequest(\'' + escapeHtml(u.username) + '\')">添加</button>';
                    return '<div class="search-user-item">' +
                        '<span class="search-user-name">' + escapeHtml(name) + ' (@' + escapeHtml(u.username) + ')</span>' +
                        btn +
                    '</div>';
                }).join('');
            }
        } catch (e) {
            results.innerHTML = '<div class="search-empty">搜索失败</div>';
        }
    }

    async function sendRequest(username) {
        try {
            var resp = await API.sendFriendRequest(username);
            if (resp.success) {
                showToast('好友请求已发送', 'success');
                closeSearchModal();
            } else {
                showToast(resp.message || '发送失败', 'error');
            }
        } catch (e) {
            showToast('发送失败', 'error');
        }
    }

    // ─── Share Modal (delegated to ShareModal component) ───

    function openShareModal(itemType, itemId) {
        if (typeof ShareModal !== 'undefined') {
            ShareModal.openShare(itemType, itemId);
        }
    }

    function closeShareModal() {
        if (typeof ShareModal !== 'undefined') {
            ShareModal.close();
        }
    }

    // ─── Shared Inbox ───

    async function loadSharedInbox() {
        if (window._userStatus === 'guest') return;
        init();
        try {
            var resp = await API.getSharedInbox();
            if (resp.success) {
                sharedItems = resp.items || [];
                renderSharedInbox();
                updateInboxBadge();

                // Show/hide shared section in sidebar
                var sharedSection = document.getElementById('shared-section');
                if (sharedSection) {
                    sharedSection.style.display = sharedItems.length > 0 ? '' : 'none';
                }

                // Render share banners on all module pages
                renderShareBanners();
            }
        } catch (e) {
            console.error('[Friends] inbox load failed:', e);
        }
    }

    var bannerConfig = {
        scenario: { elementId: 'english-share-banner', label: '学习笔记' },
        todo:     { elementId: 'todo-share-banner',    label: '待办事项' },
        review:   { elementId: 'review-share-banner',  label: '例行审视' },
        routine:  { elementId: 'todo-share-banner',    label: '日常例行' },
        expense:  { elementId: 'life-share-banner',    label: '账单记录' }
    };

    function renderShareBanners() {
        // 按 elementId 聚合计数和标签（多种 type 可能共享同一个 banner 容器）
        var byElement = {}; // elementId -> { count, labels[] }
        sharedItems.forEach(function(item) {
            var cfg = bannerConfig[item.item_type];
            if (!cfg) return;
            var eid = cfg.elementId;
            if (!byElement[eid]) byElement[eid] = { count: 0, labels: [] };
            byElement[eid].count += 1;
            if (byElement[eid].labels.indexOf(cfg.label) < 0) {
                byElement[eid].labels.push(cfg.label);
            }
        });

        // 找到所有 elementId（可能有 banner 但此次无数据，需清空）
        var allElementIds = {};
        Object.keys(bannerConfig).forEach(function(t) { allElementIds[bannerConfig[t].elementId] = true; });

        Object.keys(allElementIds).forEach(function(eid) {
            var banner = document.getElementById(eid);
            if (!banner) return;
            var info = byElement[eid];
            if (!info || info.count === 0) {
                banner.innerHTML = '';
                return;
            }
            var labelStr = info.labels.join('、');
            banner.innerHTML = '<div class="share-banner" onclick="Friends.openShareInbox()">' +
                '<span class="share-banner-icon">📬</span>' +
                '<span class="share-banner-text">你收到 ' + info.count + ' 条好友分享的' + labelStr + '</span>' +
                '<span class="share-banner-arrow">\u203A</span>' +
            '</div>';
        });
    }

    async function openShareInbox() {
        // 确保数据已加载，防止铃铛点开空白
        if (sharedItems.length === 0) {
            await loadSharedInbox();
        }

        var isMobile = window.innerWidth <= 768;

        if (isMobile) {
            // On mobile: show right sidebar as full-screen inbox page
            document.body.classList.add('page-inbox');

            // Hide all main views
            var views = ['todo-view', 'review-view', 'english-view', 'life-view', 'settings-view'];
            views.forEach(function(id) {
                var el = document.getElementById(id);
                if (el) el.style.display = 'none';
            });

            // Hide mobile FAB
            var fab = document.getElementById('mobile-fab');
            if (fab) fab.style.display = 'none';
        } else {
            // On desktop: navigate to todo page
            if (typeof switchPage === 'function') switchPage('todo');
        }

        // Expand and show shared section
        var section = document.getElementById('shared-section');
        if (section) {
            section.style.display = '';
            if (!section.classList.contains('expanded')) {
                section.classList.add('expanded');
            }
            if (!isMobile) {
                section.scrollIntoView({ behavior: 'smooth', block: 'start' });
            }
        }
    }

    function closeShareInbox() {
        document.body.classList.remove('page-inbox');
        // Restore current page view
        if (typeof switchPage === 'function') {
            var current = typeof currentPage !== 'undefined' ? currentPage : 'todo';
            // Force re-render by resetting currentPage
            if (typeof currentPage !== 'undefined') currentPage = '';
            switchPage(current);
        }
    }

    function renderSharedInbox() {
        var container = document.getElementById('shared-inbox-section');
        if (!container) return;

        if (sharedItems.length === 0) {
            container.innerHTML = '';
            return;
        }

        var typeIcons = { todo: '✓', review: '🔄', scenario: '📖', routine: '🔁', expense: '💰' };
        var typeLabels = { todo: '任务', review: '例行事项', scenario: '英语场景', routine: '日常习惯', expense: '记账' };

        var isMobile = window.innerWidth <= 768;
        var html = '';
        if (isMobile) {
            html += '<div class="shared-inbox-header">' +
                '<button class="shared-inbox-back" onclick="Friends.closeShareInbox()">‹ 返回</button>' +
                '<h4 class="shared-inbox-title">好友分享</h4>' +
            '</div>';
        } else {
            html += '<h4 class="shared-inbox-title">好友分享</h4>';
        }
        html += sharedItems.map(function(item) {
            var icon = typeIcons[item.item_type] || '📦';
            var label = typeLabels[item.item_type] || item.item_type;
            var title = '';
            if (item.item_snapshot) {
                title = item.item_snapshot.text || item.item_snapshot.title || '';
                if (!title && item.item_type === 'expense') {
                    var s = item.item_snapshot;
                    var sym = s.currency === 'CNY' ? '¥' : 'CA$';
                    title = sym + (s.amount || 0).toFixed(2);
                    if (s.notes) title += ' ' + s.notes;
                    if (s.date) title += ' (' + s.date + ')';
                }
                if (!title) title = '(未命名)';
            }

            return '<div class="shared-item-card">' +
                '<div class="shared-item-header">' +
                    '<span class="shared-item-icon">' + icon + '</span>' +
                    '<span class="shared-item-type">' + label + '</span>' +
                    '<span class="shared-item-from">来自 ' + escapeHtml(item.sender_name || '好友') + '</span>' +
                '</div>' +
                '<div class="shared-item-title">' + escapeHtml(title) + '</div>' +
                (item.message ? '<div class="shared-item-msg">' + escapeHtml(item.message) + '</div>' : '') +
                '<div class="shared-item-actions">' +
                    '<button class="btn-accept" onclick="Friends.acceptShared(\'' + item.id + '\')">收下</button>' +
                    '<button class="btn-decline" onclick="Friends.dismissShared(\'' + item.id + '\')">忽略</button>' +
                '</div>' +
            '</div>';
        }).join('');

        container.innerHTML = html;
    }

    async function acceptShared(id) {
        try {
            var resp = await API.acceptShared(id);
            if (resp.success) {
                showToast('已收下', 'success');
                sharedItems = sharedItems.filter(function(s) { return s.id !== id; });
                renderSharedInbox();
                updateInboxBadge();

                // Navigate to the newly created item
                if (resp.new_id && resp.item_type) {
                    navigateToAcceptedItem(resp.item_type, resp.new_id);
                }
            }
        } catch (e) {
            showToast('操作失败', 'error');
        }
    }

    function navigateToAcceptedItem(itemType, newId) {
        // Remove inbox overlay without triggering switchPage (avoid conflict)
        document.body.classList.remove('page-inbox');

        // Force switchPage to work by clearing currentPage
        if (typeof currentPage !== 'undefined') currentPage = '';

        switch (itemType) {
            case 'todo':
                if (typeof switchPage === 'function') switchPage('todo');
                setTimeout(function() {
                    if (typeof showTaskCard === 'function') showTaskCard(newId);
                }, 600);
                break;

            case 'review':
                if (typeof switchPage === 'function') switchPage('review');
                setTimeout(function() {
                    // 尝试高亮新条目，否则 toast 提示
                    var el = document.querySelector('[data-review-id="' + newId + '"]');
                    if (el) {
                        el.scrollIntoView({ behavior: 'smooth', block: 'center' });
                        el.classList.add('item-new-flash');
                    } else {
                        if (typeof showToast === 'function') showToast('例行事项已收下，在例行审视页面可见', 'info');
                    }
                }, 700);
                break;

            case 'routine':
                if (typeof switchPage === 'function') switchPage('todo');
                setTimeout(function() {
                    // 刷新 routines 列表，然后高亮新条目
                    if (typeof loadRoutines === 'function') loadRoutines();
                    setTimeout(function() {
                        var el = document.querySelector('[data-routine-id="' + newId + '"]');
                        if (el) {
                            el.scrollIntoView({ behavior: 'smooth', block: 'center' });
                            el.classList.add('item-new-flash');
                        } else {
                            if (typeof showToast === 'function') showToast('日常例行已收下，在今日页面下方可见', 'info');
                        }
                    }, 400);
                }, 400);
                break;

            case 'scenario':
                if (typeof switchPage === 'function') switchPage('english');
                setTimeout(function() {
                    if (typeof English !== 'undefined') English.openDetail(newId);
                }, 600);
                break;

            case 'expense':
                if (typeof switchPage === 'function') switchPage('life');
                setTimeout(function() {
                    if (typeof Life !== 'undefined') Life.openFeature('expense');
                    // 记录 pendingOpenId，让 Expense 模块数据加载后自动打开
                    if (typeof Expense !== 'undefined') {
                        Expense.pendingOpenId = newId;
                    }
                }, 400);
                break;
        }
    }

    async function dismissShared(id) {
        try {
            var resp = await API.dismissShared(id);
            if (resp.success) {
                sharedItems = sharedItems.filter(function(s) { return s.id !== id; });
                renderSharedInbox();
                updateInboxBadge();
            }
        } catch (e) {
            showToast('操作失败', 'error');
        }
    }

    // ─── Badge ───

    async function updateInboxBadge() {
        if (window._userStatus === 'guest') return; // Guest: no inbox
        try {
            var resp = await API.getSharedInboxCount();
            if (resp.success) {
                var badge = document.getElementById('inbox-badge');
                var wrapper = document.getElementById('inbox-bell-wrapper');
                if (resp.count > 0) {
                    if (badge) {
                        badge.textContent = resp.count;
                        badge.style.display = '';
                    }
                    if (wrapper) wrapper.style.display = '';
                } else {
                    if (badge) badge.style.display = 'none';
                    if (wrapper) wrapper.style.display = 'none';
                }
            }
        } catch (e) {
            console.error('[friends] updateInboxBadge:', e);
        }
    }

    // Auto-check inbox badge on load
    setTimeout(function() {
        updateInboxBadge();
    }, 2000);

    return {
        init: init,
        loadFriendsData: loadFriendsData,
        loadSharedInbox: loadSharedInbox,
        acceptRequest: acceptRequest,
        declineRequest: declineRequest,
        removeFriend: removeFriend,
        sendRequest: sendRequest,
        openShareModal: openShareModal,
        closeShareModal: closeShareModal,
        acceptShared: acceptShared,
        dismissShared: dismissShared,
        updateInboxBadge: updateInboxBadge,
        openShareInbox: openShareInbox,
        closeShareInbox: closeShareInbox
    };
})();
