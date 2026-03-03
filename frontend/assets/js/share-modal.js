// ========== 统一分享组件 ShareModal ==========
var ShareModal = (function() {
    var _mode = null; // 'share' | 'collaborate'
    var _friendsCache = null;
    var _friendsCacheTime = 0;
    var FRIENDS_CACHE_TTL = 5 * 60 * 1000; // 5 minutes
    var _shareContext = { itemType: '', itemId: '' };
    var _collabOptions = null;

    function _getOverlay() {
        return document.getElementById('share-modal-overlay');
    }

    function _escapeHtml(str) {
        var div = document.createElement('div');
        div.textContent = str || '';
        return div.innerHTML;
    }

    // ─── Public: share mode (一次性分享) ───
    function openShare(itemType, itemId) {
        _mode = 'share';
        _shareContext.itemType = itemType;
        _shareContext.itemId = itemId;
        _collabOptions = null;

        var overlay = _getOverlay();
        if (!overlay) return;

        overlay.innerHTML = '<div class="share-modal" onclick="event.stopPropagation()">'
            + '<h3>分享给好友</h3>'
            + '<div class="share-friends-list" id="share-friends-list">'
            + '<div class="share-loading">加载中...</div>'
            + '</div>'
            + '<div class="share-message-area">'
            + '<input type="text" id="share-message-input" class="share-message-input" placeholder="附言（可选）" maxlength="200" />'
            + '</div>'
            + '<div class="share-modal-close">'
            + '<button class="btn btn-secondary" onclick="ShareModal.close()">取消</button>'
            + '</div></div>';

        overlay.style.display = 'flex';
        _loadShareContent();
    }

    // ─── Public: collaborate mode (持久协作) ───
    function openCollaborate(options) {
        _mode = 'collaborate';
        _shareContext = {};
        _collabOptions = options || {};

        var overlay = _getOverlay();
        if (!overlay) return;

        var collabs = _collabOptions.collaborators || [];
        var collabHtml = '';
        if (collabs.length > 0) {
            collabHtml = '<div class="share-collab-section">'
                + '<div class="share-collab-label">当前协作者</div>'
                + '<div class="share-collab-list">';
            collabs.forEach(function(c) {
                var name = _escapeHtml(c.display_name || c.username || '');
                var initial = name.charAt(0).toUpperCase();
                var roleLabel = c.role === 'editor' ? '编辑' : '查看';
                collabHtml += '<div class="share-friend-item share-collab-item">'
                    + '<div class="friend-avatar">' + initial + '</div>'
                    + '<span class="friend-name">' + name + '</span>'
                    + '<span class="share-role-label">' + roleLabel + '</span>'
                    + '<button class="share-remove-btn" onclick="ShareModal._removeCollab(\'' + _escapeHtml(c.user_id) + '\')">&times;</button>'
                    + '</div>';
            });
            collabHtml += '</div></div>';
        }

        overlay.innerHTML = '<div class="share-modal" onclick="event.stopPropagation()">'
            + '<h3>共享行程</h3>'
            + collabHtml
            + '<div class="share-collab-section">'
            + '<div class="share-collab-label">添加好友</div>'
            + '<div class="share-friends-list" id="share-friends-list">'
            + '<div class="share-loading">加载中...</div>'
            + '</div>'
            + '<div class="share-role-picker">'
            + '<label>权限</label>'
            + '<select id="share-role-select">'
            + '<option value="viewer">查看（只读）</option>'
            + '<option value="editor">编辑（可更新报销状态）</option>'
            + '</select>'
            + '</div>'
            + '</div>'
            + '<div class="share-modal-close">'
            + '<button class="btn btn-secondary" onclick="ShareModal.close()">关闭</button>'
            + '</div></div>';

        overlay.style.display = 'flex';
        _loadCollabFriends();
    }

    // ─── Close ───
    function close() {
        var overlay = _getOverlay();
        if (overlay) overlay.style.display = 'none';
        _mode = null;
    }

    // ─── Cache ───
    function invalidateCache() {
        _friendsCache = null;
        _friendsCacheTime = 0;
    }

    // ─── Load friends (cached) ───
    async function _loadFriends() {
        if (_friendsCache && (Date.now() - _friendsCacheTime < FRIENDS_CACHE_TTL)) {
            return _friendsCache;
        }
        try {
            var resp = await API.getFriends();
            if (resp.success) {
                var items = resp.items || [];
                // Only cache non-empty results to avoid stale empty state
                if (items.length > 0) {
                    _friendsCache = items;
                    _friendsCacheTime = Date.now();
                }
                return items;
            }
        } catch (e) {
            console.error('[ShareModal] loadFriends error:', e);
        }
        return [];
    }

    // ─── Share mode rendering ───
    async function _loadShareContent() {
        var friends = await _loadFriends();
        var container = document.getElementById('share-friends-list');
        if (!container) return;

        if (friends.length === 0) {
            container.innerHTML = '<div class="share-empty">还没有好友，去设置页面添加吧</div>';
            return;
        }

        // Check who already received this item
        var sentMap = {};
        try {
            if (_shareContext.itemType && _shareContext.itemId) {
                var sentResp = await API.getSharedSent(_shareContext.itemType, _shareContext.itemId);
                if (sentResp.success && sentResp.items) {
                    sentResp.items.forEach(function(s) {
                        sentMap[s.recipient_id] = s.status;
                    });
                }
            }
        } catch (e) { console.error('[share-modal] parseFriends:', e); }

        container.innerHTML = friends.map(function(f) {
            var name = f.display_name || f.username;
            var initial = name.charAt(0).toUpperCase();
            var alreadySent = sentMap[f.id];

            if (alreadySent) {
                var statusLabel = alreadySent === 'accepted' ? '已收下' :
                                  alreadySent === 'dismissed' ? '已忽略' : '已分享';
                return '<div class="share-friend-item share-friend-sent">'
                    + '<div class="friend-avatar">' + initial + '</div>'
                    + '<span class="friend-name">' + _escapeHtml(name) + '</span>'
                    + '<span class="share-sent-label">' + statusLabel + '</span>'
                    + '</div>';
            }

            return '<div class="share-friend-item" onclick="ShareModal._doShare(\'' + f.id + '\')">'
                + '<div class="friend-avatar">' + initial + '</div>'
                + '<span class="friend-name">' + _escapeHtml(name) + '</span>'
                + '</div>';
        }).join('');
    }

    // ─── Collaborate mode rendering ───
    async function _loadCollabFriends() {
        var friends = await _loadFriends();
        var container = document.getElementById('share-friends-list');
        if (!container) return;

        // Exclude existing collaborators
        var existing = (_collabOptions.collaborators || []).map(function(c) { return c.user_id; });
        var available = friends.filter(function(f) { return existing.indexOf(f.id) < 0; });

        if (available.length === 0) {
            container.innerHTML = '<div class="share-empty">没有可添加的好友</div>';
            return;
        }

        container.innerHTML = available.map(function(f) {
            var name = f.display_name || f.username;
            var initial = name.charAt(0).toUpperCase();
            return '<div class="share-friend-item" onclick="ShareModal._addCollab(\'' + f.id + '\')">'
                + '<div class="friend-avatar">' + initial + '</div>'
                + '<span class="friend-name">' + _escapeHtml(name) + '</span>'
                + '</div>';
        }).join('');
    }

    // ─── Share action ───
    async function _doShare(friendId) {
        try {
            var msgInput = document.getElementById('share-message-input');
            var message = msgInput ? msgInput.value.trim() : '';
            var resp = await API.shareItem(friendId, _shareContext.itemType, _shareContext.itemId, message || undefined);
            if (resp.success) {
                showToast('分享成功', 'success');
                close();
            } else {
                showToast(resp.message || '分享失败', 'error');
            }
        } catch (e) {
            showToast('分享失败', 'error');
        }
    }

    // ─── Collaborate actions ───
    async function _addCollab(friendId) {
        if (!_collabOptions || !_collabOptions.onAdd) return;
        var role = (document.getElementById('share-role-select') || {}).value || 'viewer';
        _collabOptions.onAdd(friendId, role);
    }

    function _removeCollab(userId) {
        if (!confirm('确定移除此协作者？')) return;
        if (!_collabOptions || !_collabOptions.onRemove) return;
        _collabOptions.onRemove(userId);
    }

    return {
        openShare: openShare,
        openCollaborate: openCollaborate,
        close: close,
        invalidateCache: invalidateCache,
        // Internal (exposed for onclick handlers)
        _doShare: _doShare,
        _addCollab: _addCollab,
        _removeCollab: _removeCollab
    };
})();
