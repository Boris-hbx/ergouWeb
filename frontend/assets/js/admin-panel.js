// ========== Admin Panel — Full Console ==========
var AdminPanel = (function() {
    var _role = null; // 'owner' | 'admin' | null
    var _currentSection = 'overview';
    var _initialized = false;

    // ── Helpers ──
    function esc(s) {
        if (!s) return '';
        var d = document.createElement('div');
        d.textContent = s;
        return d.innerHTML;
    }
    function fmt(n) {
        if (n == null) return '0';
        if (n >= 1000000) return (n / 1000000).toFixed(1) + 'M';
        if (n >= 1000) return (n / 1000).toFixed(1) + 'K';
        return String(n);
    }
    function fmtBytes(bytes) {
        if (!bytes) return '0 B';
        if (bytes >= 1073741824) return (bytes / 1073741824).toFixed(1) + ' GB';
        if (bytes >= 1048576) return (bytes / 1048576).toFixed(1) + ' MB';
        if (bytes >= 1024) return (bytes / 1024).toFixed(1) + ' KB';
        return bytes + ' B';
    }
    function fmtDuration(ms) {
        ms = Number(ms || 0);
        if (ms <= 0) return '0s';
        var sec = Math.round(ms / 1000);
        if (sec < 60) return sec + 's';
        var min = Math.floor(sec / 60);
        var rest = sec % 60;
        if (min < 60) return min + 'm ' + rest + 's';
        var hour = Math.floor(min / 60);
        return hour + 'h ' + (min % 60) + 'm';
    }
    function shortDate(s) { return s ? s.substring(0, 10) : '-'; }
    function shortDateTime(s) { return s ? s.substring(0, 16).replace('T', ' ') : '-'; }
    function fmtUptime(secs) {
        var d = Math.floor(secs / 86400);
        var h = Math.floor((secs % 86400) / 3600);
        var m = Math.floor((secs % 3600) / 60);
        if (d > 0) return d + 'd ' + h + 'h';
        if (h > 0) return h + 'h ' + m + 'm';
        return m + 'm';
    }
    function roleBadge(role) {
        return '<span class="admin-badge role-' + esc(role) + '">' + esc(role) + '</span>';
    }
    function statusBadge(status) {
        return '<span class="admin-badge status-' + esc(status) + '">' + esc(status) + '</span>';
    }
    function severityBadge(sev) {
        return '<span class="admin-badge severity-' + esc(sev) + '">' + esc(sev) + '</span>';
    }

    // ── Init ──
    function init() {
        var user = window._currentUser;
        if (!user || (user.role !== 'admin' && user.role !== 'owner')) {
            // Hide admin nav
            var navEl = document.getElementById('admin-nav-link');
            if (navEl) navEl.style.display = 'none';
            return;
        }
        _role = user.role;
        var navEl = document.getElementById('admin-nav-link');
        if (navEl) navEl.style.display = '';

        if (_initialized) {
            // Just refresh current section
            loadSection(_currentSection);
            return;
        }
        _initialized = true;

        // Bind sidebar nav clicks
        var navItems = document.querySelectorAll('.admin-nav-item');
        navItems.forEach(function(el) {
            el.addEventListener('click', function() {
                showSection(el.dataset.section);
            });
        });

        showSection('overview');
    }

    function showSection(id) {
        _currentSection = id;
        // Update nav active
        document.querySelectorAll('.admin-nav-item').forEach(function(el) {
            el.classList.toggle('active', el.dataset.section === id);
        });
        // Toggle sections
        document.querySelectorAll('.admin-section').forEach(function(el) {
            el.classList.toggle('active', el.id === 'admin-sec-' + id);
        });
        loadSection(id);
    }

    function loadSection(id) {
        switch (id) {
            case 'overview': Overview.load(); break;
            case 'users': Users.load(); break;
            case 'chats': Chats.load(); break;
            case 'ai': AI.load(); break;
            case 'risk': Risk.load(); break;
            case 'analytics': Analytics.load(); break;
            case 'system': System.load(); break;
            case 'audit': Audit.load(); break;
            case 'people': People.load(); break;
            case 'patrol': PatrolLab.load(); break;
        }
    }

    // ═══════════════════════════════════════════
    // OVERVIEW Section
    // ═══════════════════════════════════════════
    var Overview = {
        load: async function() {
            var el = document.getElementById('admin-overview-content');
            if (!el) return;
            el.innerHTML = '<div class="admin-loading-text">加载中...</div>';
            try {
                var data = await API.getAdminDashboard();
                if (!data.success) { el.innerHTML = '<div class="admin-empty">加载失败</div>'; return; }
                this.render(el, data);
            } catch(e) {
                el.innerHTML = '<div class="admin-empty">加载失败</div>';
            }
        },
        render: function(el, data) {
            var html = '<div class="admin-cards-grid">';
            html += this.card(data.users.total, '总用户', '', 'users');
            html += this.card(data.users.dau, '今日活跃', 'DAU', '');
            html += this.card(data.users.pending || 0, '待审批', '', 'users', data.users.pending > 0 ? 'warning' : '');

            var aiToday = data.ai && data.ai.today ? data.ai.today.input_tokens + data.ai.today.output_tokens : 0;
            html += this.card(fmt(aiToday), 'AI Tokens (今日)', '', 'ai');

            var secCount = 0;
            if (data.security_events_today != null) secCount = data.security_events_today;
            html += this.card(secCount, '安全事件 (今日)', '', 'risk', secCount > 0 ? 'danger' : '');
            html += '</div>';

            // Quick stats
            if (data.features) {
                html += '<div style="margin-top:8px;"><strong>功能使用</strong></div>';
                html += '<div class="admin-cards-grid" style="margin-top:8px;">';
                var f = data.features;
                html += this.miniCard(f.todos, 'Todos');
                html += this.miniCard(f.routines, '例行');
                html += this.miniCard(f.conversations, '对话');
                html += this.miniCard(f.expenses, '记账');
                html += this.miniCard(f.trips, '差旅');
                html += this.miniCard(f.friendships, '好友');
                html += '</div>';
            }
            el.innerHTML = html;

            // Bind card clicks for navigation
            el.querySelectorAll('[data-goto]').forEach(function(card) {
                card.addEventListener('click', function() {
                    showSection(card.dataset.goto);
                });
            });
        },
        card: function(value, label, sub, goto, cls) {
            return '<div class="admin-summary-card ' + (cls || '') + '"' +
                (goto ? ' data-goto="' + goto + '"' : '') + '>' +
                '<div class="card-value">' + value + '</div>' +
                '<div class="card-label">' + esc(label) + '</div>' +
                (sub ? '<div class="card-sub">' + esc(sub) + '</div>' : '') +
                '</div>';
        },
        miniCard: function(value, label) {
            return '<div class="admin-summary-card" style="padding:10px;"><div class="card-value" style="font-size:20px;">' + fmt(value) + '</div><div class="card-label">' + esc(label) + '</div></div>';
        }
    };

    // ═══════════════════════════════════════════
    // USERS Section
    // ═══════════════════════════════════════════
    var Users = {
        _params: { search: '', role: '', status: '', sort: 'created_at', order: 'desc' },
        _detail: null,

        load: async function() {
            var el = document.getElementById('admin-users-content');
            if (!el) return;
            this.renderFilters(el);
            await this.fetchList();
        },

        renderFilters: function(el) {
            if (document.getElementById('admin-users-filters')) return;
            var filtersHtml = '<div class="admin-filters" id="admin-users-filters">' +
                '<input class="admin-filter-input" type="text" placeholder="搜索用户名..." id="admin-users-search" style="width:180px;">' +
                '<select class="admin-filter-select" id="admin-users-role-filter">' +
                '<option value="">全部角色</option><option value="owner">Owner</option><option value="admin">Admin</option><option value="user">User</option></select>' +
                '<select class="admin-filter-select" id="admin-users-status-filter">' +
                '<option value="">全部状态</option><option value="active">Active</option><option value="pending">Pending</option><option value="suspended">Suspended</option><option value="rejected">Rejected</option></select>' +
                '<button class="admin-refresh-btn" id="admin-users-refresh">刷新</button>' +
                '</div>' +
                '<div id="admin-users-table"></div>' +
                '<div id="admin-users-detail"></div>';
            el.innerHTML = filtersHtml;
            var self = this;
            document.getElementById('admin-users-search').addEventListener('input', debounce(function() {
                self._params.search = this.value;
                self.fetchList();
            }, 300));
            document.getElementById('admin-users-role-filter').addEventListener('change', function() {
                self._params.role = this.value;
                self.fetchList();
            });
            document.getElementById('admin-users-status-filter').addEventListener('change', function() {
                self._params.status = this.value;
                self.fetchList();
            });
            document.getElementById('admin-users-refresh').addEventListener('click', function() {
                self.fetchList();
            });
        },

        fetchList: async function() {
            var table = document.getElementById('admin-users-table');
            if (!table) return;
            table.innerHTML = '<div class="admin-loading-text">加载中...</div>';
            try {
                var p = this._params;
                var qs = '?sort=' + p.sort + '&order=' + p.order;
                if (p.search) qs += '&search=' + encodeURIComponent(p.search);
                if (p.role) qs += '&role=' + p.role;
                if (p.status) qs += '&status=' + p.status;
                var data = await API.adminRequest('GET', '/admin/users' + qs);
                if (!data.success) { table.innerHTML = '<div class="admin-empty">加载失败</div>'; return; }
                this.renderTable(table, data.users || []);
            } catch(e) {
                table.innerHTML = '<div class="admin-empty">加载失败</div>';
            }
        },

        renderTable: function(el, users) {
            if (!users.length) { el.innerHTML = '<div class="admin-empty">无用户</div>'; return; }
            var html = '<table class="admin-panel-table"><thead><tr>' +
                '<th>用户</th><th>角色</th><th>状态</th><th>注册时间</th><th>最近活跃</th><th>操作</th>' +
                '</tr></thead><tbody>';
            var self = this;
            for (var i = 0; i < users.length; i++) {
                var u = users[i];
                html += '<tr>';
                html += '<td><strong>' + esc(u.display_name || u.username) + '</strong><br><span style="font-size:11px;opacity:0.5;">@' + esc(u.username) + '</span></td>';
                html += '<td>' + roleBadge(u.role) + '</td>';
                html += '<td>' + statusBadge(u.status) + '</td>';
                html += '<td>' + shortDate(u.created_at) + '</td>';
                html += '<td>' + shortDate(u.last_active) + '</td>';
                html += '<td><div class="admin-btn-group">';
                // Context-specific actions
                if (u.status === 'pending') {
                    html += '<button class="admin-btn admin-btn-success" data-action="approve" data-id="' + esc(u.id) + '">通过</button>';
                    html += '<button class="admin-btn admin-btn-danger" data-action="reject" data-id="' + esc(u.id) + '">拒绝</button>';
                } else if (u.status === 'active' && u.role !== 'owner') {
                    html += '<button class="admin-btn admin-btn-secondary" data-action="force-logout" data-id="' + esc(u.id) + '">强制登出</button>';
                    html += '<button class="admin-btn admin-btn-danger" data-action="suspend" data-id="' + esc(u.id) + '">封禁</button>';
                    if (_role === 'owner') {
                        if (u.role === 'user') {
                            html += '<button class="admin-btn admin-btn-primary" data-action="set-admin" data-id="' + esc(u.id) + '">设管理员</button>';
                        } else if (u.role === 'admin') {
                            html += '<button class="admin-btn admin-btn-secondary" data-action="revoke-admin" data-id="' + esc(u.id) + '">撤销管理员</button>';
                        }
                    }
                } else if (u.status === 'suspended') {
                    html += '<button class="admin-btn admin-btn-success" data-action="restore" data-id="' + esc(u.id) + '">恢复</button>';
                }
                html += '</div></td>';
                html += '</tr>';
            }
            html += '</tbody></table>';
            el.innerHTML = html;

            // Bind action buttons
            el.querySelectorAll('[data-action]').forEach(function(btn) {
                btn.addEventListener('click', function(e) {
                    e.stopPropagation();
                    self.handleAction(btn.dataset.action, btn.dataset.id);
                });
            });
        },

        handleAction: async function(action, userId) {
            var self = this;
            try {
                var data;
                switch (action) {
                    case 'approve':
                        data = await API.approveUser(userId);
                        if (data.success) showToast('已通过', 'success');
                        break;
                    case 'reject':
                        if (!confirm('确定拒绝该用户？')) return;
                        data = await API.rejectUser(userId);
                        if (data.success) showToast('已拒绝', 'success');
                        break;
                    case 'force-logout':
                        if (!confirm('确定强制该用户登出？')) return;
                        data = await API.adminRequest('POST', '/admin/users/' + encodeURIComponent(userId) + '/force-logout');
                        if (data.success) showToast('已强制登出', 'success');
                        break;
                    case 'suspend':
                        if (!confirm('确定封禁该用户？')) return;
                        data = await API.adminRequest('POST', '/admin/users/' + encodeURIComponent(userId) + '/suspend');
                        if (data.success) showToast('已封禁', 'success');
                        break;
                    case 'restore':
                        data = await API.restoreUser(userId);
                        if (data.success) showToast('已恢复', 'success');
                        break;
                    case 'set-admin':
                        if (!confirm('确定将该用户设为管理员？')) return;
                        data = await API.adminRequest('PUT', '/admin/users/' + encodeURIComponent(userId) + '/role', { role: 'admin' });
                        if (data.success) showToast('已设为管理员', 'success');
                        break;
                    case 'revoke-admin':
                        if (!confirm('确定撤销该用户的管理员权限？')) return;
                        data = await API.adminRequest('PUT', '/admin/users/' + encodeURIComponent(userId) + '/role', { role: 'user' });
                        if (data.success) showToast('已撤销管理员', 'success');
                        break;
                }
                if (data && !data.success) {
                    showToast(data.message || '操作失败', 'error');
                }
                self.fetchList();
            } catch(e) {
                showToast('操作失败', 'error');
            }
        }
    };

    // ═══════════════════════════════════════════
    // CHATS Section (Conversation Monitor)
    // 三级导航: 用户列表 → 对话列表 → 消息流
    // ═══════════════════════════════════════════
    var Chats = {
        _filters: { date_from: '', date_to: '' },
        _selectedUserId: null,
        _selectedConvId: null,
        _users: [],
        _convos: [],

        load: async function() {
            var el = document.getElementById('admin-chats-content');
            if (!el) return;
            this.renderLayout(el);
            await this.fetchUsers();
        },

        renderLayout: function(el) {
            el.innerHTML =
                '<div class="admin-filters" id="admin-chats-filters">' +
                    '<input class="admin-filter-input" type="date" id="admin-chats-from"' +
                        (this._filters.date_from ? ' value="' + this._filters.date_from + '"' : '') + '>' +
                    '<span style="opacity:0.5;">至</span>' +
                    '<input class="admin-filter-input" type="date" id="admin-chats-to"' +
                        (this._filters.date_to ? ' value="' + this._filters.date_to + '"' : '') + '>' +
                    '<button class="admin-refresh-btn" id="admin-chats-search-btn">筛选</button>' +
                '</div>' +
                '<div class="admin-chats-layout">' +
                    '<div class="admin-chats-user-list" id="admin-chats-user-list">' +
                        '<div class="admin-loading-text">加载中...</div>' +
                    '</div>' +
                    '<div class="admin-chats-right">' +
                        '<div class="admin-chats-conv-list" id="admin-chats-conv-list">' +
                            '<div class="admin-empty" style="padding:20px;">选择用户查看对话</div>' +
                        '</div>' +
                        '<div class="admin-chats-msg-area" id="admin-chats-msg-area">' +
                            '<div class="admin-empty" style="padding:20px;">选择对话查看消息</div>' +
                        '</div>' +
                    '</div>' +
                '</div>';
            var self = this;
            document.getElementById('admin-chats-search-btn').addEventListener('click', function() {
                self._filters.date_from = document.getElementById('admin-chats-from').value;
                self._filters.date_to = document.getElementById('admin-chats-to').value;
                self._selectedUserId = null;
                self._selectedConvId = null;
                self.fetchUsers();
            });
        },

        fetchUsers: async function() {
            var list = document.getElementById('admin-chats-user-list');
            if (!list) return;
            list.innerHTML = '<div class="admin-loading-text">加载中...</div>';
            try {
                var qs = '?';
                if (this._filters.date_from) qs += 'date_from=' + this._filters.date_from + '&';
                if (this._filters.date_to) qs += 'date_to=' + this._filters.date_to + '&';
                var data = await API.adminRequest('GET', '/admin/conversations/users' + qs);
                if (!data.success) { list.innerHTML = '<div class="admin-empty">加载失败</div>'; return; }
                this._users = data.users || [];
                this.renderUserList(list);
                // Auto-select: use pre-set user or first in list
                if (this._users.length > 0) {
                    var targetId = this._selectedUserId || this._users[0].user_id;
                    this.selectUser(targetId);
                }
            } catch(e) {
                console.error('[Chats]', e);
                list.innerHTML = '<div class="admin-empty">加载失败</div>';
            }
        },

        renderUserList: function(el) {
            if (!this._users.length) { el.innerHTML = '<div class="admin-empty">无对话用户</div>'; return; }
            var html = '';
            var self = this;
            for (var i = 0; i < this._users.length; i++) {
                var u = this._users[i];
                var active = this._selectedUserId === u.user_id ? ' active' : '';
                html += '<div class="admin-chats-user-item' + active + '" data-uid="' + esc(u.user_id) + '">';
                html += '<div class="chats-user-name">' + esc(u.user_name) + '</div>';
                html += '<div class="chats-user-stats">' + u.conv_count + ' 对话 · ' + fmt(u.msg_count) + ' 消息</div>';
                html += '<div class="chats-user-stats">' + fmt(u.token_sum) + ' tokens · ' + shortDate(u.last_active) + '</div>';
                html += '</div>';
            }
            el.innerHTML = html;
            el.querySelectorAll('[data-uid]').forEach(function(item) {
                item.addEventListener('click', function() {
                    self.selectUser(item.dataset.uid);
                });
            });
        },

        selectUser: async function(userId) {
            this._selectedUserId = userId;
            this._selectedConvId = null;
            // Update active state in user list
            var list = document.getElementById('admin-chats-user-list');
            if (list) {
                list.querySelectorAll('.admin-chats-user-item').forEach(function(item) {
                    item.classList.toggle('active', item.dataset.uid === userId);
                });
            }
            // Fetch conversations for this user
            var convList = document.getElementById('admin-chats-conv-list');
            var msgArea = document.getElementById('admin-chats-msg-area');
            if (convList) convList.innerHTML = '<div class="admin-loading-text">加载中...</div>';
            if (msgArea) msgArea.innerHTML = '<div class="admin-empty" style="padding:20px;">选择对话查看消息</div>';
            try {
                var qs = '?user_id=' + encodeURIComponent(userId) + '&limit=50';
                if (this._filters.date_from) qs += '&date_from=' + this._filters.date_from;
                if (this._filters.date_to) qs += '&date_to=' + this._filters.date_to;
                var data = await API.adminRequest('GET', '/admin/conversations' + qs);
                if (!data.success) { if (convList) convList.innerHTML = '<div class="admin-empty">加载失败</div>'; return; }
                this._convos = data.conversations || [];
                this.renderConvList(convList);
                // Auto-select first conversation
                if (this._convos.length > 0) {
                    this.selectConversation(this._convos[0].id);
                }
            } catch(e) {
                console.error('[Chats]', e);
                if (convList) convList.innerHTML = '<div class="admin-empty">加载失败</div>';
            }
        },

        renderConvList: function(el) {
            if (!el) return;
            if (!this._convos.length) { el.innerHTML = '<div class="admin-empty">该用户无对话</div>'; return; }
            var html = '';
            var self = this;
            for (var i = 0; i < this._convos.length; i++) {
                var c = this._convos[i];
                var active = this._selectedConvId === c.id ? ' active' : '';
                html += '<div class="admin-chats-conv-item' + active + '" data-cid="' + esc(c.id) + '">';
                html += '<div class="chats-conv-title">' + esc(c.title || '(无标题)') + '</div>';
                html += '<div class="chats-conv-meta">' + (c.message_count || 0) + ' 消息 · ' + fmt(c.token_sum || 0) + ' tokens · ' + shortDateTime(c.updated_at) + '</div>';
                html += '</div>';
            }
            el.innerHTML = html;
            el.querySelectorAll('[data-cid]').forEach(function(item) {
                item.addEventListener('click', function() {
                    self.selectConversation(item.dataset.cid);
                });
            });
        },

        selectConversation: async function(convId) {
            this._selectedConvId = convId;
            // Update active state in conv list
            var convList = document.getElementById('admin-chats-conv-list');
            if (convList) {
                convList.querySelectorAll('.admin-chats-conv-item').forEach(function(item) {
                    item.classList.toggle('active', item.dataset.cid === convId);
                });
            }
            var msgArea = document.getElementById('admin-chats-msg-area');
            if (!msgArea) return;
            msgArea.innerHTML = '<div class="admin-loading-text">加载消息中...</div>';
            try {
                var data = await API.adminRequest('GET', '/admin/conversations/' + encodeURIComponent(convId) + '/messages');
                if (!data.success) { msgArea.innerHTML = '<div class="admin-empty">加载失败</div>'; return; }
                this.renderMessages(msgArea, data.messages || []);
            } catch(e) {
                console.error('[Chats]', e);
                msgArea.innerHTML = '<div class="admin-empty">加载失败</div>';
            }
        },

        // Called from Risk section to jump to a specific conversation
        openConversation: async function(convId) {
            this._selectedConvId = convId;
            var el = document.getElementById('admin-chats-content');
            if (!el) return;
            // Render a simplified view for cross-section jump
            var msgArea = document.getElementById('admin-chats-msg-area');
            if (!msgArea) {
                this.renderLayout(el);
                msgArea = document.getElementById('admin-chats-msg-area');
            }
            if (!msgArea) return;
            msgArea.innerHTML = '<div class="admin-loading-text">加载消息中...</div>';
            try {
                var data = await API.adminRequest('GET', '/admin/conversations/' + encodeURIComponent(convId) + '/messages');
                if (!data.success) { msgArea.innerHTML = '<div class="admin-empty">加载失败</div>'; return; }
                this.renderMessages(msgArea, data.messages || []);
            } catch(e) {
                console.error('[Chats]', e);
                msgArea.innerHTML = '<div class="admin-empty">加载失败</div>';
            }
        },

        renderMessages: function(el, messages) {
            if (!messages.length) { el.innerHTML = '<div class="admin-empty">该对话无消息</div>'; return; }
            var html = '<div class="admin-chat-messages">';
            for (var i = 0; i < messages.length; i++) {
                var m = messages[i];
                var cls = m.role === 'user' ? 'user' : 'assistant';
                html += '<div class="admin-chat-msg ' + cls + '">';
                html += esc(m.content || m.content_text || '');
                html += '<div class="msg-meta">' + shortDateTime(m.created_at);
                if (m.token_count) html += ' · ' + fmt(m.token_count) + ' tokens';
                html += '</div></div>';
            }
            html += '</div>';
            el.innerHTML = html;
        }
    };

    // ═══════════════════════════════════════════
    // AI Section
    // ═══════════════════════════════════════════
    var AI = {
        load: async function() {
            var el = document.getElementById('admin-ai-content');
            if (!el) return;
            el.innerHTML = '<div class="admin-loading-text">加载中...</div>';
            try {
                var results = await Promise.all([
                    API.adminRequest('GET', '/admin/ai-usage'),
                    API.adminRequest('GET', '/admin/ai-usage/providers')
                ]);
                var usage = results[0];
                var providers = results[1];
                this.render(el, usage, providers);
            } catch(e) {
                el.innerHTML = '<div class="admin-empty">加载失败</div>';
            }
        },

        render: function(el, usage, providers) {
            var html = '';

            // Provider status
            if (providers.success && providers.providers) {
                html += '<h4 style="margin:0 0 12px;">模型提供商状态</h4>';
                html += '<div class="admin-provider-grid">';
                var provs = providers.providers;
                for (var key in provs) {
                    var p = provs[key];
                    html += '<div class="admin-provider-card">';
                    html += '<div class="admin-provider-dot ' + (p.configured ? 'configured' : 'not-configured') + '"></div>';
                    html += '<div><strong>' + esc(key) + '</strong><br><span style="font-size:11px;opacity:0.5;">' + (p.configured ? '已配置' : '未配置') + '</span></div>';
                    html += '</div>';
                }
                html += '</div>';
            }

            // Token consumption by period
            if (usage.success) {
                html += '<h4 style="margin:20px 0 12px;">Token 消耗</h4>';
                html += '<table class="admin-token-grid"><thead><tr>';
                html += '<th>周期</th><th>消息数</th><th>输入 Tokens</th><th>输出 Tokens</th><th>合计</th>';
                html += '</tr></thead><tbody>';
                var periods = [
                    { label: '今日', data: usage.today },
                    { label: '近7天', data: usage.week },
                    { label: '近30天', data: usage.month }
                ];
                for (var i = 0; i < periods.length; i++) {
                    var p = periods[i];
                    var d = p.data || {};
                    html += '<tr><td>' + p.label + '</td>';
                    html += '<td>' + fmt(d.messages) + '</td>';
                    html += '<td>' + fmt(d.input_tokens) + '</td>';
                    html += '<td>' + fmt(d.output_tokens) + '</td>';
                    html += '<td>' + fmt((d.input_tokens || 0) + (d.output_tokens || 0)) + '</td>';
                    html += '</tr>';
                }
                html += '</tbody></table>';

                // By model
                if (usage.by_model && usage.by_model.length) {
                    html += '<h4 style="margin:20px 0 12px;">按模型统计</h4>';
                    html += '<table class="admin-panel-table"><thead><tr>';
                    html += '<th>模型</th><th>消息数</th><th>输入 Tokens</th><th>输出 Tokens</th>';
                    html += '</tr></thead><tbody>';
                    for (var j = 0; j < usage.by_model.length; j++) {
                        var m = usage.by_model[j];
                        html += '<tr><td>' + esc(m.model || '(unknown)') + '</td>';
                        html += '<td>' + fmt(m.messages) + '</td>';
                        html += '<td>' + fmt(m.input_tokens) + '</td>';
                        html += '<td>' + fmt(m.output_tokens) + '</td></tr>';
                    }
                    html += '</tbody></table>';
                }

                // Per-user ranking
                if (usage.per_user && usage.per_user.length) {
                    html += '<h4 style="margin:20px 0 12px;">用户消耗排行</h4>';
                    html += '<table class="admin-panel-table"><thead><tr>';
                    html += '<th>用户</th><th>消息数</th><th>总 Tokens</th>';
                    html += '</tr></thead><tbody>';
                    for (var k = 0; k < usage.per_user.length; k++) {
                        var u = usage.per_user[k];
                        html += '<tr style="cursor:pointer;" data-user-chat="' + esc(u.user_id) + '">';
                        html += '<td>' + esc(u.display_name || u.user_id) + '</td>';
                        html += '<td>' + fmt(u.messages) + '</td>';
                        html += '<td>' + fmt((u.input_tokens || 0) + (u.output_tokens || 0)) + '</td></tr>';
                    }
                    html += '</tbody></table>';
                }
            }
            el.innerHTML = html;

            // Bind user click to jump to chat monitor
            el.querySelectorAll('[data-user-chat]').forEach(function(row) {
                row.addEventListener('click', function() {
                    var uid = row.dataset.userChat;
                    Chats._selectedUserId = uid;
                    Chats._selectedConvId = null;
                    showSection('chats');
                });
            });
        }
    };

    // ═══════════════════════════════════════════
    // RISK Section (Security Events)
    // ═══════════════════════════════════════════
    var Risk = {
        _offset: 0,
        _limit: 30,
        _filters: { severity: '', event_type: '', user_id: '' },

        load: async function() {
            var el = document.getElementById('admin-risk-content');
            if (!el) return;
            if (!document.getElementById('admin-risk-filters')) {
                this.renderFilters(el);
            }
            await this.fetchList();
        },

        renderFilters: function(el) {
            el.innerHTML = '<div class="admin-filters" id="admin-risk-filters">' +
                '<select class="admin-filter-select" id="admin-risk-severity">' +
                '<option value="">全部严重性</option><option value="critical">Critical</option><option value="high">High</option><option value="medium">Medium</option><option value="low">Low</option></select>' +
                '<input class="admin-filter-input" type="text" placeholder="事件类型..." id="admin-risk-type" style="width:140px;">' +
                '<input class="admin-filter-input" type="text" placeholder="用户ID..." id="admin-risk-user" style="width:140px;">' +
                '<button class="admin-refresh-btn" id="admin-risk-search-btn">搜索</button>' +
                '</div>' +
                '<div id="admin-risk-summary"></div>' +
                '<div id="admin-risk-list"></div>';
            var self = this;
            document.getElementById('admin-risk-search-btn').addEventListener('click', function() {
                self._filters.severity = document.getElementById('admin-risk-severity').value;
                self._filters.event_type = document.getElementById('admin-risk-type').value;
                self._filters.user_id = document.getElementById('admin-risk-user').value;
                self._offset = 0;
                self.fetchList();
            });
        },

        fetchList: async function() {
            var list = document.getElementById('admin-risk-list');
            if (!list) return;
            list.innerHTML = '<div class="admin-loading-text">加载中...</div>';
            try {
                var qs = '?limit=' + this._limit + '&offset=' + this._offset;
                if (this._filters.severity) qs += '&severity=' + this._filters.severity;
                if (this._filters.event_type) qs += '&event_type=' + encodeURIComponent(this._filters.event_type);
                if (this._filters.user_id) qs += '&user_id=' + encodeURIComponent(this._filters.user_id);
                var data = await API.adminRequest('GET', '/admin/security-events-v2' + qs);
                if (!data.success) { list.innerHTML = '<div class="admin-empty">加载失败</div>'; return; }
                this.renderSummary(data.risk_users || []);
                this.renderList(list, data.events || [], data.total || 0);
            } catch(e) {
                list.innerHTML = '<div class="admin-empty">加载失败</div>';
            }
        },

        renderSummary: function(riskUsers) {
            var el = document.getElementById('admin-risk-summary');
            if (!el) return;
            if (!riskUsers.length) { el.innerHTML = ''; return; }
            var html = '<h4 style="margin:0 0 8px;">高风险用户</h4><div class="admin-risk-users">';
            for (var i = 0; i < riskUsers.length; i++) {
                var u = riskUsers[i];
                html += '<div class="admin-risk-user-card">';
                html += '<div class="risk-name">' + esc(u.user_name || u.user_id) + '</div>';
                html += '<div class="risk-stats">' + u.event_count + ' 次事件 · 最近: ' + shortDate(u.last_event) + '</div>';
                html += '<button class="admin-btn admin-btn-danger" style="margin-top:6px;" data-risk-suspend="' + esc(u.user_id) + '">封禁</button>';
                html += '</div>';
            }
            html += '</div>';
            el.innerHTML = html;
            var self = this;
            el.querySelectorAll('[data-risk-suspend]').forEach(function(btn) {
                btn.addEventListener('click', function() {
                    if (!confirm('确定封禁该用户？')) return;
                    API.adminRequest('POST', '/admin/users/' + encodeURIComponent(btn.dataset.riskSuspend) + '/suspend').then(function(d) {
                        if (d.success) { showToast('已封禁', 'success'); self.fetchList(); }
                        else showToast(d.message || '操作失败', 'error');
                    });
                });
            });
        },

        renderList: function(el, events, total) {
            if (!events.length) { el.innerHTML = '<div class="admin-empty">无安全事件</div>'; return; }
            var html = '<table class="admin-panel-table"><thead><tr>' +
                '<th>时间</th><th>严重性</th><th>类型</th><th>用户</th><th>描述</th><th>操作</th>' +
                '</tr></thead><tbody>';
            var self = this;
            for (var i = 0; i < events.length; i++) {
                var e = events[i];
                html += '<tr>';
                html += '<td>' + shortDateTime(e.created_at) + '</td>';
                html += '<td>' + severityBadge(e.severity) + '</td>';
                html += '<td>' + esc(e.event_type) + '</td>';
                html += '<td>' + esc(e.user_name || e.user_id || '-') + '</td>';
                html += '<td style="max-width:300px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">' + esc(e.description) + '</td>';
                html += '<td><div class="admin-btn-group">';
                if (e.conversation_id) {
                    html += '<button class="admin-btn admin-btn-secondary" data-view-conv="' + esc(e.conversation_id) + '">查看对话</button>';
                }
                if (!e.reviewed) {
                    html += '<button class="admin-btn admin-btn-primary" data-review-event="' + esc(e.id) + '">已审阅</button>';
                }
                html += '</div></td></tr>';
            }
            html += '</tbody></table>';
            // Pagination
            html += '<div class="admin-pagination">';
            html += '<button ' + (this._offset === 0 ? 'disabled' : '') + ' id="admin-risk-prev">上一页</button>';
            html += '<span>' + (this._offset + 1) + '-' + Math.min(this._offset + this._limit, total) + ' / ' + total + '</span>';
            html += '<button ' + (this._offset + this._limit >= total ? 'disabled' : '') + ' id="admin-risk-next">下一页</button>';
            html += '</div>';
            el.innerHTML = html;

            // Bind actions
            el.querySelectorAll('[data-view-conv]').forEach(function(btn) {
                btn.addEventListener('click', function(ev) {
                    ev.stopPropagation();
                    Chats.openConversation(btn.dataset.viewConv);
                    showSection('chats');
                });
            });
            el.querySelectorAll('[data-review-event]').forEach(function(btn) {
                btn.addEventListener('click', function(ev) {
                    ev.stopPropagation();
                    API.adminRequest('POST', '/admin/security-events/' + encodeURIComponent(btn.dataset.reviewEvent) + '/review').then(function(d) {
                        if (d.success) { showToast('已标记为已审阅', 'success'); self.fetchList(); }
                    });
                });
            });
            var prevBtn = document.getElementById('admin-risk-prev');
            var nextBtn = document.getElementById('admin-risk-next');
            if (prevBtn) prevBtn.addEventListener('click', function() { self._offset -= self._limit; self.fetchList(); });
            if (nextBtn) nextBtn.addEventListener('click', function() { self._offset += self._limit; self.fetchList(); });
        }
    };

    // ═══════════════════════════════════════════
    // ANALYTICS Section (Behavior Analytics)
    // ═══════════════════════════════════════════
    var Analytics = {
        _range: '7d',
        _userId: '',
        _customFrom: '',
        _customTo: '',
        _users: [],
        _charts: {},

        load: async function() {
            var el = document.getElementById('admin-analytics-content');
            if (!el) return;
            if (!document.getElementById('admin-analytics-shell')) {
                this.renderShell(el);
            }
            await this.fetchAll();
        },

        renderShell: function(el) {
            el.innerHTML = '<div class="admin-analytics" id="admin-analytics-shell">' +
                '<div class="admin-analytics-toolbar">' +
                '<div class="admin-filter-group">' +
                '<label>时间</label>' +
                '<select class="admin-filter-select" id="analytics-range">' +
                '<option value="today">今天</option>' +
                '<option value="7d" selected>近 7 天</option>' +
                '<option value="30d">近 30 天</option>' +
                '<option value="custom">自定义</option>' +
                '</select>' +
                '</div>' +
                '<div class="admin-filter-group analytics-custom-range" id="analytics-custom-range" style="display:none;">' +
                '<input class="admin-filter-input" type="date" id="analytics-from">' +
                '<span>至</span>' +
                '<input class="admin-filter-input" type="date" id="analytics-to">' +
                '</div>' +
                '<div class="admin-filter-group">' +
                '<label>用户</label>' +
                '<select class="admin-filter-select" id="analytics-user"><option value="">全部用户</option></select>' +
                '</div>' +
                '<button class="admin-refresh-btn" id="analytics-refresh">刷新</button>' +
                '</div>' +
                '<div id="analytics-status"></div>' +
                '<div id="analytics-body"></div>' +
                '</div>';
            var self = this;
            document.getElementById('analytics-range').addEventListener('change', function() {
                self._range = this.value;
                document.getElementById('analytics-custom-range').style.display = self._range === 'custom' ? '' : 'none';
                self.fetchAll();
            });
            document.getElementById('analytics-user').addEventListener('change', function() {
                self._userId = this.value;
                self.fetchAll();
            });
            document.getElementById('analytics-from').addEventListener('change', function() {
                self._customFrom = this.value;
                if (self._range === 'custom') self.fetchAll();
            });
            document.getElementById('analytics-to').addEventListener('change', function() {
                self._customTo = this.value;
                if (self._range === 'custom') self.fetchAll();
            });
            document.getElementById('analytics-refresh').addEventListener('click', function() {
                self.fetchAll();
            });
        },

        params: function(includeUser) {
            var p = [];
            var now = new Date();
            var from = null;
            var to = new Date(now.getTime());
            if (this._range === 'today') {
                from = new Date(now.getFullYear(), now.getMonth(), now.getDate());
            } else if (this._range === '30d') {
                from = new Date(now.getTime() - 29 * 86400000);
            } else if (this._range === 'custom') {
                if (this._customFrom) from = new Date(this._customFrom + 'T00:00:00');
                if (this._customTo) to = new Date(this._customTo + 'T23:59:59');
            } else {
                from = new Date(now.getTime() - 6 * 86400000);
            }
            if (from) p.push('from=' + encodeURIComponent(from.toISOString()));
            if (to) p.push('to=' + encodeURIComponent(to.toISOString()));
            if (includeUser && this._userId) p.push('user_id=' + encodeURIComponent(this._userId));
            return p.length ? '?' + p.join('&') : '';
        },

        fetchAll: async function() {
            var status = document.getElementById('analytics-status');
            var body = document.getElementById('analytics-body');
            if (!status || !body) return;
            status.innerHTML = '<div class="admin-loading-text">加载行为数据中...</div>';
            try {
                var qs = this.params(true);
                var results = await Promise.all([
                    API.adminRequest('GET', '/admin/analytics/overview' + qs),
                    API.adminRequest('GET', '/admin/analytics/top-targets' + qs + (qs ? '&' : '?') + 'limit=12'),
                    API.adminRequest('GET', '/admin/analytics/feature-usage' + qs),
                    API.adminRequest('GET', '/admin/analytics/dwell' + qs),
                    API.adminRequest('GET', '/admin/analytics/users' + this.params(false))
                ]);
                for (var i = 0; i < results.length; i++) {
                    if (!results[i].success) throw new Error(results[i].error || 'analytics api failed');
                }
                this._users = results[4].items || [];
                this.renderUserOptions();
                this.render(body, {
                    overview: results[0],
                    topTargets: results[1].items || [],
                    featureUsage: results[2].items || [],
                    dwell: results[3].items || [],
                    users: this._users
                });
                status.innerHTML = '';
                await this.loadTrail();
            } catch(e) {
                console.error('[admin-analytics]', e);
                status.innerHTML = '<div class="admin-empty">加载失败，请重试</div>';
                body.innerHTML = '';
            }
        },

        renderUserOptions: function() {
            var select = document.getElementById('analytics-user');
            if (!select) return;
            var current = this._userId;
            var html = '<option value="">全部用户</option>';
            for (var i = 0; i < this._users.length; i++) {
                var u = this._users[i];
                html += '<option value="' + esc(u.user_id) + '">' + esc(u.display_name || u.user_id) + '</option>';
            }
            select.innerHTML = html;
            select.value = current;
        },

        render: function(el, data) {
            this.destroyCharts();
            var overview = data.overview || {};
            var html = '<div class="admin-cards-grid analytics-kpis">';
            html += this.kpi(overview.active_users, '活跃用户');
            html += this.kpi(overview.sessions, '会话数');
            html += this.kpi(overview.total_events, '总事件');
            html += this.kpi(overview.events_per_user, '人均事件');
            html += '</div>';

            html += '<div class="analytics-grid">' +
                '<div class="analytics-panel analytics-wide"><div class="analytics-panel-title">时段活跃</div><div class="analytics-chart-wrap"><canvas id="analytics-hour-chart"></canvas></div></div>' +
                '<div class="analytics-panel"><div class="analytics-panel-title">按钮点击排行</div><div id="analytics-top-targets"></div></div>' +
                '<div class="analytics-panel analytics-wide"><div class="analytics-panel-title">功能使用与停留</div><div id="analytics-feature-usage"></div></div>' +
                '<div class="analytics-panel"><div class="analytics-panel-title">停留排行</div><div id="analytics-dwell"></div></div>' +
                '<div class="analytics-panel analytics-wide"><div class="analytics-panel-title">行为轨迹</div><div id="analytics-trail"></div></div>' +
                '<div class="analytics-panel analytics-wide"><div class="analytics-panel-title">用户分群</div><div id="analytics-users"></div></div>' +
                '</div>';
            el.innerHTML = html;
            this.renderHourChart(overview.by_hour || []);
            this.renderTopTargets(document.getElementById('analytics-top-targets'), data.topTargets);
            this.renderFeatureUsage(document.getElementById('analytics-feature-usage'), data.featureUsage);
            this.renderDwell(document.getElementById('analytics-dwell'), data.dwell);
            this.renderUsers(document.getElementById('analytics-users'), data.users);
        },

        kpi: function(value, label) {
            return '<div class="admin-summary-card analytics-kpi"><div class="card-value">' + fmt(value || 0) + '</div><div class="card-label">' + esc(label) + '</div></div>';
        },

        renderHourChart: function(byHour) {
            var canvas = document.getElementById('analytics-hour-chart');
            if (!canvas) return;
            var labels = [];
            var values = [];
            for (var i = 0; i < 24; i++) {
                labels.push((i < 10 ? '0' : '') + i + ':00');
                values.push(Number(byHour[i] || 0));
            }
            if (typeof Chart === 'undefined') {
                canvas.parentElement.innerHTML = '<div class="admin-empty">Chart.js 未加载</div>';
                return;
            }
            this._charts.hour = new Chart(canvas, {
                type: 'bar',
                data: { labels: labels, datasets: [{ label: '事件数', data: values, backgroundColor: '#4f8cff' }] },
                options: {
                    responsive: true,
                    maintainAspectRatio: false,
                    plugins: { legend: { display: false } },
                    scales: { y: { beginAtZero: true, ticks: { precision: 0 } } }
                }
            });
        },

        renderTopTargets: function(el, items) {
            if (!el) return;
            if (!items.length) { el.innerHTML = '<div class="admin-empty">暂无点击数据</div>'; return; }
            var max = Math.max.apply(null, items.map(function(x) { return x.clicks || 0; })) || 1;
            var html = '<div class="analytics-bars">';
            items.forEach(function(item) {
                var pct = Math.max(4, Math.round((item.clicks || 0) / max * 100));
                html += '<div class="analytics-bar-row">' +
                    '<div class="analytics-bar-meta"><strong>' + esc(item.target_label || item.target_id) + '</strong><span>' + esc(item.target_id) + '</span></div>' +
                    '<div class="analytics-bar-track"><div style="width:' + pct + '%"></div></div>' +
                    '<div class="analytics-bar-value">' + fmt(item.clicks) + '</div>' +
                    '</div>';
            });
            el.innerHTML = html + '</div>';
        },

        renderFeatureUsage: function(el, items) {
            if (!el) return;
            if (!items.length) { el.innerHTML = '<div class="admin-empty">暂无功能使用数据</div>'; return; }
            var html = '<table class="admin-panel-table"><thead><tr><th>功能页</th><th>访问</th><th>总停留</th><th>平均停留</th></tr></thead><tbody>';
            items.slice(0, 16).forEach(function(item) {
                html += '<tr><td><code>' + esc(item.route) + '</code></td><td>' + fmt(item.pageviews) + '</td><td>' + fmtDuration(item.total_dwell_ms) + '</td><td>' + fmtDuration(item.avg_dwell_ms) + '</td></tr>';
            });
            el.innerHTML = html + '</tbody></table>';
        },

        renderDwell: function(el, items) {
            if (!el) return;
            if (!items.length) { el.innerHTML = '<div class="admin-empty">暂无停留数据</div>'; return; }
            var html = '<table class="admin-panel-table"><thead><tr><th>功能页</th><th>中位</th><th>均值</th><th>样本</th></tr></thead><tbody>';
            items.slice(0, 12).forEach(function(item) {
                html += '<tr><td><code>' + esc(item.route) + '</code></td><td>' + fmtDuration(item.median_dwell_ms) + '</td><td>' + fmtDuration(item.avg_dwell_ms) + '</td><td>' + fmt(item.samples) + '</td></tr>';
            });
            el.innerHTML = html + '</tbody></table>';
        },

        renderUsers: function(el, users) {
            if (!el) return;
            if (!users.length) { el.innerHTML = '<div class="admin-empty">暂无用户行为数据</div>'; return; }
            var html = '<table class="admin-panel-table"><thead><tr><th>用户</th><th>角色</th><th>事件</th><th>会话</th><th>最近活跃</th><th>操作</th></tr></thead><tbody>';
            users.forEach(function(u) {
                html += '<tr><td>' + esc(u.display_name || u.user_id) + '</td><td>' + roleBadge(u.role || 'user') + '</td><td>' + fmt(u.events) + '</td><td>' + fmt(u.sessions) + '</td><td>' + shortDateTime(u.last_active) + '</td>' +
                    '<td><button class="admin-btn admin-btn-secondary" data-analytics-user="' + esc(u.user_id) + '">钻取</button></td></tr>';
            });
            el.innerHTML = html + '</tbody></table>';
            var self = this;
            el.querySelectorAll('[data-analytics-user]').forEach(function(btn) {
                btn.addEventListener('click', function() {
                    self._userId = btn.dataset.analyticsUser;
                    var sel = document.getElementById('analytics-user');
                    if (sel) sel.value = self._userId;
                    self.fetchAll();
                });
            });
        },

        loadTrail: async function() {
            var el = document.getElementById('analytics-trail');
            if (!el) return;
            var targetUser = this._userId || (this._users[0] && this._users[0].user_id);
            if (!targetUser) { el.innerHTML = '<div class="admin-empty">暂无行为轨迹</div>'; return; }
            el.innerHTML = '<div class="admin-loading-text">加载轨迹中...</div>';
            try {
                var data = await API.adminRequest('GET', '/admin/analytics/trail?user_id=' + encodeURIComponent(targetUser) + '&limit=80');
                if (!data.success) { el.innerHTML = '<div class="admin-empty">加载失败，请重试</div>'; return; }
                this.renderTrail(el, data.items || []);
            } catch(e) {
                console.error('[admin-analytics-trail]', e);
                el.innerHTML = '<div class="admin-empty">加载失败，请重试</div>';
            }
        },

        renderTrail: function(el, items) {
            if (!items.length) { el.innerHTML = '<div class="admin-empty">暂无行为轨迹</div>'; return; }
            var html = '<div class="analytics-trail">';
            items.forEach(function(item) {
                var main = item.target_label || item.target_id || item.route || item.event_type;
                html += '<div class="analytics-trail-item">' +
                    '<div class="analytics-trail-time">' + shortDateTime(item.client_ts) + '</div>' +
                    '<div class="analytics-trail-dot"></div>' +
                    '<div class="analytics-trail-main"><strong>' + esc(item.event_type) + '</strong> ' + esc(main || '-') +
                    '<div>' + esc(item.route || '-') + (item.dwell_ms ? ' · ' + fmtDuration(item.dwell_ms) : '') + '</div></div>' +
                    '</div>';
            });
            el.innerHTML = html + '</div>';
        },

        destroyCharts: function() {
            for (var key in this._charts) {
                if (this._charts[key] && this._charts[key].destroy) this._charts[key].destroy();
            }
            this._charts = {};
        }
    };

    // ═══════════════════════════════════════════
    // SYSTEM Section
    // ═══════════════════════════════════════════
    var System = {
        load: async function() {
            var el = document.getElementById('admin-system-content');
            if (!el) return;
            el.innerHTML = '<div class="admin-loading-text">加载中...</div>';
            try {
                var data = await API.adminRequest('GET', '/admin/system-status');
                if (!data.success) { el.innerHTML = '<div class="admin-empty">加载失败</div>'; return; }
                this.render(el, data);
            } catch(e) {
                el.innerHTML = '<div class="admin-empty">加载失败</div>';
            }
        },

        render: function(el, data) {
            var html = '<div class="admin-system-grid">';
            // Server info
            html += '<div class="admin-system-card"><h4>服务器</h4>';
            html += sysItem('版本', data.version || '-');
            html += sysItem('运行时间', fmtUptime(data.uptime_secs || 0));
            html += '</div>';
            // Database
            html += '<div class="admin-system-card"><h4>数据库</h4>';
            html += sysItem('文件大小', fmtBytes(data.database ? data.database.file_size : 0));
            if (data.database && data.database.tables) {
                var t = data.database.tables;
                for (var key in t) {
                    html += sysItem(key, fmt(t[key]) + ' 行');
                }
            }
            html += '</div>';
            // Storage
            html += '<div class="admin-system-card"><h4>存储</h4>';
            html += sysItem('上传文件数', data.storage ? fmt(data.storage.upload_files) : '0');
            html += sysItem('上传总大小', fmtBytes(data.storage ? data.storage.upload_size : 0));
            html += '</div>';
            // Errors
            html += '<div class="admin-system-card"><h4>错误</h4>';
            html += sysItem('近24小时', data.errors ? data.errors.last_24h : 0);
            html += sysItem('近7天', data.errors ? data.errors.last_7d : 0);
            html += '</div>';
            html += '</div>';
            el.innerHTML = html;
        }
    };

    function sysItem(label, value) {
        return '<div class="admin-system-item"><span class="sys-label">' + esc(label) + '</span><span class="sys-value">' + value + '</span></div>';
    }

    // ═══════════════════════════════════════════
    // AUDIT Section
    // ═══════════════════════════════════════════
    var Audit = {
        _offset: 0,
        _limit: 30,
        _filters: { admin_user: '', action_type: '' },
        _total: 0,

        load: async function() {
            var el = document.getElementById('admin-audit-content');
            if (!el) return;
            if (!document.getElementById('admin-audit-filters')) {
                this.renderFilters(el);
            }
            await this.fetchList();
        },

        renderFilters: function(el) {
            el.innerHTML = '<div class="admin-filters" id="admin-audit-filters">' +
                '<input class="admin-filter-input" type="text" placeholder="管理员..." id="admin-audit-admin" style="width:140px;">' +
                '<input class="admin-filter-input" type="text" placeholder="操作类型..." id="admin-audit-action" style="width:140px;">' +
                '<button class="admin-refresh-btn" id="admin-audit-search-btn">搜索</button>' +
                '</div>' +
                '<div id="admin-audit-list"></div>';
            var self = this;
            document.getElementById('admin-audit-search-btn').addEventListener('click', function() {
                self._filters.admin_user = document.getElementById('admin-audit-admin').value;
                self._filters.action_type = document.getElementById('admin-audit-action').value;
                self._offset = 0;
                self.fetchList();
            });
        },

        fetchList: async function() {
            var list = document.getElementById('admin-audit-list');
            if (!list) return;
            list.innerHTML = '<div class="admin-loading-text">加载中...</div>';
            try {
                var qs = '?limit=' + this._limit + '&offset=' + this._offset;
                if (this._filters.admin_user) qs += '&admin_user=' + encodeURIComponent(this._filters.admin_user);
                if (this._filters.action_type) qs += '&action_type=' + encodeURIComponent(this._filters.action_type);
                var data = await API.adminRequest('GET', '/admin/audit-log' + qs);
                if (!data.success) { list.innerHTML = '<div class="admin-empty">加载失败</div>'; return; }
                this._total = data.total || 0;
                this.renderList(list, data.entries || []);
            } catch(e) {
                list.innerHTML = '<div class="admin-empty">加载失败</div>';
            }
        },

        renderList: function(el, entries) {
            if (!entries.length) { el.innerHTML = '<div class="admin-empty">无审计记录</div>'; return; }
            var html = '<table class="admin-panel-table"><thead><tr>' +
                '<th>时间</th><th>管理员</th><th>操作</th><th>目标用户</th><th>资源</th><th>详情</th>' +
                '</tr></thead><tbody>';
            for (var i = 0; i < entries.length; i++) {
                var e = entries[i];
                html += '<tr>';
                html += '<td>' + shortDateTime(e.created_at) + '</td>';
                html += '<td>' + esc(e.admin_name || e.admin_user_id) + '</td>';
                html += '<td><code>' + esc(e.action_type) + '</code></td>';
                html += '<td>' + esc(e.target_name || e.target_user_id || '-') + '</td>';
                html += '<td>' + esc(e.target_resource || '-') + '</td>';
                html += '<td style="max-width:200px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">' + esc(e.details || '-') + '</td>';
                html += '</tr>';
            }
            html += '</tbody></table>';
            // Pagination
            html += '<div class="admin-pagination">';
            html += '<button ' + (this._offset === 0 ? 'disabled' : '') + ' id="admin-audit-prev">上一页</button>';
            html += '<span>' + (this._offset + 1) + '-' + Math.min(this._offset + this._limit, this._total) + ' / ' + this._total + '</span>';
            html += '<button ' + (this._offset + this._limit >= this._total ? 'disabled' : '') + ' id="admin-audit-next">下一页</button>';
            html += '</div>';
            el.innerHTML = html;
            var self = this;
            var prevBtn = document.getElementById('admin-audit-prev');
            var nextBtn = document.getElementById('admin-audit-next');
            if (prevBtn) prevBtn.addEventListener('click', function() { self._offset -= self._limit; self.fetchList(); });
            if (nextBtn) nextBtn.addEventListener('click', function() { self._offset += self._limit; self.fetchList(); });
        }
    };

    // ── People — 人物档案 ──
    var People = {
        _data: [],
        _users: [],

        load: async function() {
            var el = document.getElementById('admin-people-content');
            if (!el) return;
            el.innerHTML = '<div class="admin-loading-text">加载中...</div>';
            try {
                // Load users for the dropdown
                var usersData = await API.adminRequest('GET', '/admin/users');
                if (usersData.success) this._users = usersData.users || [];
                // Load people
                var data = await API.adminRequest('GET', '/admin/people');
                if (!data.success) { el.innerHTML = '<div class="admin-empty">加载失败</div>'; return; }
                this._data = data.people || [];
                this.render(el);
            } catch(e) {
                console.error('[admin-people]', e);
                el.innerHTML = '<div class="admin-empty">加载失败</div>';
            }
        },

        render: function(el) {
            var self = this;
            var html = '<div style="margin-bottom:12px;display:flex;gap:8px;align-items:center;flex-wrap:wrap;">';
            // User filter
            html += '<select id="people-user-filter" class="admin-select" style="min-width:140px;">';
            html += '<option value="">全部用户</option>';
            this._users.forEach(function(u) {
                html += '<option value="' + esc(u.id) + '">' + esc(u.display_name || u.username) + '</option>';
            });
            html += '</select>';
            html += '<button class="admin-btn admin-btn-primary" id="people-add-btn">+ 新增人物</button>';
            html += '</div>';

            if (this._data.length === 0) {
                html += '<div class="admin-empty">暂无人物档案</div>';
            } else {
                html += '<div class="admin-table-wrap"><table class="admin-table"><thead><tr>';
                html += '<th>名字</th><th>关系</th><th>称呼</th><th>态度</th><th>备注</th><th>来源</th><th>用户</th><th>操作</th>';
                html += '</tr></thead><tbody>';
                this._data.forEach(function(p) {
                    html += '<tr>';
                    html += '<td>' + esc(p.name) + '</td>';
                    html += '<td>' + esc(p.relationship) + '</td>';
                    html += '<td>' + esc(p.nickname || '-') + '</td>';
                    html += '<td style="max-width:200px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">' + esc(p.attitude || '-') + '</td>';
                    html += '<td style="max-width:150px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">' + esc(p.notes || '-') + '</td>';
                    html += '<td><span class="admin-badge">' + esc(p.created_by) + '</span></td>';
                    html += '<td>' + esc(p.username || p.user_id) + '</td>';
                    html += '<td style="white-space:nowrap;">';
                    html += '<button class="admin-btn admin-btn-sm" data-action="edit-person" data-id="' + esc(p.id) + '">编辑</button> ';
                    html += '<button class="admin-btn admin-btn-sm admin-btn-danger" data-action="delete-person" data-id="' + esc(p.id) + '" data-name="' + esc(p.name) + '">删除</button>';
                    html += '</td></tr>';
                });
                html += '</tbody></table></div>';
            }
            el.innerHTML = html;

            // Bind filter
            var filterEl = document.getElementById('people-user-filter');
            if (filterEl) {
                filterEl.addEventListener('change', async function() {
                    var uid = this.value;
                    var url = '/admin/people' + (uid ? '?user_id=' + encodeURIComponent(uid) : '');
                    try {
                        var data = await API.adminRequest('GET', url);
                        if (data.success) { self._data = data.people || []; self.render(el); }
                    } catch(e) { console.error('[admin-people]', e); }
                });
            }

            // Bind add
            var addBtn = document.getElementById('people-add-btn');
            if (addBtn) addBtn.addEventListener('click', function() { self.showForm(el, null); });

            // Bind edit/delete
            el.querySelectorAll('[data-action="edit-person"]').forEach(function(btn) {
                btn.addEventListener('click', function() {
                    var person = self._data.find(function(p) { return p.id === btn.dataset.id; });
                    if (person) self.showForm(el, person);
                });
            });
            el.querySelectorAll('[data-action="delete-person"]').forEach(function(btn) {
                btn.addEventListener('click', function() {
                    if (!confirm('确定删除人物 "' + btn.dataset.name + '"？')) return;
                    self.deletePerson(btn.dataset.id);
                });
            });
        },

        showForm: function(parentEl, person) {
            var self = this;
            var isEdit = !!person;
            var overlay = document.createElement('div');
            overlay.className = 'admin-modal-overlay';
            overlay.innerHTML = '<div class="admin-modal" style="max-width:480px;">' +
                '<div class="admin-modal-title">' + (isEdit ? '编辑人物' : '新增人物') + '</div>' +
                '<div class="admin-form">' +
                (isEdit ? '' : '<div class="admin-form-group"><label>所属用户</label>' +
                    '<select id="person-user-id" class="admin-select">' +
                    this._users.map(function(u) { return '<option value="' + esc(u.id) + '">' + esc(u.display_name || u.username) + '</option>'; }).join('') +
                    '</select></div>') +
                '<div class="admin-form-group"><label>名字 *</label><input type="text" id="person-name" class="admin-input" value="' + esc(person ? person.name : '') + '"></div>' +
                '<div class="admin-form-group"><label>关系 *</label><input type="text" id="person-relationship" class="admin-input" placeholder="wife/friend/colleague/family/assistant" value="' + esc(person ? person.relationship : '') + '"></div>' +
                '<div class="admin-form-group"><label>二狗称呼</label><input type="text" id="person-nickname" class="admin-input" placeholder="如:夫人、Ruby姐姐" value="' + esc(person ? person.nickname : '') + '"></div>' +
                '<div class="admin-form-group"><label>态度</label><input type="text" id="person-attitude" class="admin-input" placeholder="如:经常夸她，特别尊重" value="' + esc(person ? person.attitude : '') + '"></div>' +
                '<div class="admin-form-group"><label>备注</label><textarea id="person-notes" class="admin-input" rows="2" placeholder="生日、喜好等">' + esc(person ? person.notes : '') + '</textarea></div>' +
                '</div>' +
                '<div class="admin-modal-actions">' +
                '<button class="admin-btn" id="person-cancel">取消</button>' +
                '<button class="admin-btn admin-btn-primary" id="person-save">' + (isEdit ? '保存' : '添加') + '</button>' +
                '</div></div>';
            document.body.appendChild(overlay);

            overlay.querySelector('#person-cancel').addEventListener('click', function() { overlay.remove(); });
            overlay.addEventListener('click', function(e) { if (e.target === overlay) overlay.remove(); });

            overlay.querySelector('#person-save').addEventListener('click', async function() {
                var name = document.getElementById('person-name').value.trim();
                var relationship = document.getElementById('person-relationship').value.trim();
                if (!name || !relationship) { showToast('名字和关系为必填', 'error'); return; }

                var body = {
                    name: name,
                    relationship: relationship,
                    nickname: document.getElementById('person-nickname').value.trim(),
                    attitude: document.getElementById('person-attitude').value.trim(),
                    notes: document.getElementById('person-notes').value.trim()
                };

                try {
                    var result;
                    if (isEdit) {
                        result = await API.adminRequest('PUT', '/admin/people/' + encodeURIComponent(person.id), body);
                    } else {
                        body.user_id = document.getElementById('person-user-id').value;
                        result = await API.adminRequest('POST', '/admin/people', body);
                    }
                    if (result.success) {
                        overlay.remove();
                        showToast(isEdit ? '已更新' : '已添加', 'success');
                        self.load();
                    } else {
                        showToast(result.error || '操作失败', 'error');
                    }
                } catch(e) {
                    console.error('[admin-people]', e);
                    showToast('操作失败', 'error');
                }
            });
        },

        deletePerson: async function(id) {
            try {
                var result = await API.adminRequest('DELETE', '/admin/people/' + encodeURIComponent(id));
                if (result.success) {
                    showToast('已删除', 'success');
                    this.load();
                } else {
                    showToast(result.error || '删除失败', 'error');
                }
            } catch(e) {
                console.error('[admin-people]', e);
                showToast('删除失败', 'error');
            }
        }
    };

    // ── PatrolLab — 二狗实验室 ──
    var PatrolLab = {
        _refs: null,       // { sm, idle, pawPool, terrainOverlay }
        _rafId: null,
        _frameTimes: [],
        _overBudget: 0,
        _totalFrames: 0,
        _updateTimer: null,
        _paused: false,

        load: function() {
            var el = document.getElementById('admin-patrol-content');
            if (!el) return;

            // Get refs from Patrol system
            if (typeof Patrol !== 'undefined' && Patrol.getDebugRefs) {
                this._refs = Patrol.getDebugRefs();
            }

            this.render(el);
            this.startPerfMonitor();
            this.startDisplayUpdate();
        },

        render: function(el) {
            var self = this;
            var connected = !!this._refs;

            var html = '';

            // ── Status + controls (horizontal groups) ──
            html += '<div class="admin-patrol-status">';

            // Real-time status
            html += '<div class="admin-patrol-group">';
            html += '<div class="admin-patrol-group-title">实时状态</div>';
            html += '<div class="ap-row"><span class="ap-label">状态</span><span class="ap-val" id="ap-state">' + (connected ? 'off_duty' : '未连接') + '</span></div>';
            html += '<div class="ap-row"><span class="ap-label">位置</span><span class="ap-val" id="ap-pos">-</span></div>';
            html += '<div class="ap-row"><span class="ap-label">爪印</span><span class="ap-val" id="ap-paws">0/8</span></div>';
            html += '<div class="ap-row"><span class="ap-label">冷却</span><span class="ap-val" id="ap-cooldown">ready</span></div>';
            html += '<div class="ap-row"><span class="ap-label">设备</span><span class="ap-val" id="ap-device">-</span></div>';
            html += '</div>';

            // Performance
            html += '<div class="admin-patrol-group">';
            html += '<div class="admin-patrol-group-title">性能</div>';
            html += '<div class="ap-row"><span class="ap-label">FPS</span><span class="ap-val" id="ap-fps">--</span></div>';
            html += '<div class="ap-row"><span class="ap-label">帧均</span><span class="ap-val" id="ap-avg">--ms</span></div>';
            html += '<div class="ap-row"><span class="ap-label">峰值</span><span class="ap-val" id="ap-peak">--ms</span></div>';
            html += '<div class="ap-row"><span class="ap-label">超标</span><span class="ap-val" id="ap-over">0%</span></div>';
            html += '</div>';

            // Controls
            html += '<div class="admin-patrol-group">';
            html += '<div class="admin-patrol-group-title">操控</div>';
            html += '<div class="admin-patrol-btns">';
            html += '<button class="admin-btn admin-btn-primary ap-ctrl" data-action="force">出场</button>';
            html += '<button class="admin-btn admin-btn-secondary ap-ctrl" data-action="home">回家</button>';
            html += '<button class="admin-btn admin-btn-secondary ap-ctrl" data-action="pause">暂停</button>';
            html += '<button class="admin-btn admin-btn-secondary ap-ctrl" data-action="cooldown">重置冷却</button>';
            html += '<button class="admin-btn admin-btn-secondary ap-ctrl" data-action="terrain">地形</button>';
            html += '</div>';
            if (!connected) {
                html += '<div class="ap-hint">值班系统未初始化（仅移动端）</div>';
            }
            html += '</div>';

            html += '</div>'; // end status

            // ── Bottom: parameter sliders ──
            html += '<div class="admin-patrol-sliders">';
            html += '<div class="admin-patrol-group-title">参数调节</div>';
            html += '<div class="admin-patrol-slider-grid">';

            html += this.renderSlider('ap-idle', '闲置阈值', 1000, 30000, 8000, 1000, 's', function(v) { return (v / 1000) + 's'; });
            html += this.renderSlider('ap-speed', '步速', 10, 100, 30, 5, 'px/s', function(v) { return v + 'px/s'; });
            html += this.renderSlider('ap-cd', '冷却时间', 0, 600000, 180000, 10000, '', function(v) { return v >= 60000 ? (v / 60000).toFixed(1) + 'min' : (v / 1000) + 's'; });
            html += this.renderSlider('ap-paw-size', '爪印大小', 8, 32, 20, 2, 'px', function(v) { return v + 'px'; });
            html += this.renderSlider('ap-paw-opacity', '爪印透明', 0.1, 1.0, 0.5, 0.05, '', function(v) { return v.toFixed(2); });

            html += '</div>';
            html += '</div>';

            el.innerHTML = html;
            this.bindEvents();
        },

        renderSlider: function(id, label, min, max, val, step, unit, fmtFn) {
            return '<div class="ap-slider-item">' +
                '<div class="ap-slider-header"><span>' + label + '</span><span id="' + id + '-val">' + fmtFn(val) + '</span></div>' +
                '<input type="range" class="ap-slider" min="' + min + '" max="' + max + '" value="' + val + '" step="' + step + '" id="' + id + '-slider">' +
                '</div>';
        },

        bindEvents: function() {
            var self = this;

            // Control buttons
            var ctrls = document.querySelectorAll('.ap-ctrl');
            ctrls.forEach(function(btn) {
                btn.addEventListener('click', function() {
                    var action = btn.dataset.action;
                    self.handleControl(action, btn);
                });
            });

            // Parameter sliders → dispatch events
            this.bindParamSlider('ap-idle', function(v) {
                if (self._refs && self._refs.idle) self._refs.idle.setIdleThreshold(parseInt(v));
            }, function(v) { return (parseInt(v) / 1000) + 's'; });

            this.bindParamSlider('ap-speed', function(v) {
                document.dispatchEvent(new CustomEvent('patrol:debugParam', { detail: { key: 'speed', value: parseInt(v) } }));
            }, function(v) { return parseInt(v) + 'px/s'; });

            this.bindParamSlider('ap-cd', function(v) {
                if (self._refs && self._refs.idle) self._refs.idle.setCooldown(parseInt(v));
            }, function(v) { v = parseInt(v); return v >= 60000 ? (v / 60000).toFixed(1) + 'min' : (v / 1000) + 's'; });

            this.bindParamSlider('ap-paw-size', function(v) {
                document.dispatchEvent(new CustomEvent('patrol:debugParam', { detail: { key: 'size', value: parseInt(v) } }));
            }, function(v) { return parseInt(v) + 'px'; });

            this.bindParamSlider('ap-paw-opacity', function(v) {
                document.dispatchEvent(new CustomEvent('patrol:debugParam', { detail: { key: 'opacity', value: parseFloat(v) } }));
            }, function(v) { return parseFloat(v).toFixed(2); });
        },

        bindParamSlider: function(prefix, onInput, fmtFn) {
            var slider = document.getElementById(prefix + '-slider');
            var valEl = document.getElementById(prefix + '-val');
            if (!slider || !valEl) return;
            slider.addEventListener('input', function() {
                valEl.textContent = fmtFn(this.value);
                onInput(this.value);
            });
        },

        handleControl: function(action, btn) {
            var refs = this._refs;
            switch (action) {
                case 'force':
                    if (refs && refs.sm) {
                        refs.sm.forceState('on_duty');
                        setTimeout(function() {
                            if (refs.sm) refs.sm.transition('peekDone');
                        }, 300);
                    }
                    break;
                case 'home':
                    if (refs && refs.sm) refs.sm.reset();
                    break;
                case 'pause':
                    this._paused = !this._paused;
                    btn.textContent = this._paused ? '继续' : '暂停';
                    document.dispatchEvent(new CustomEvent('patrol:debugPause', { detail: { paused: this._paused } }));
                    break;
                case 'cooldown':
                    if (refs && refs.idle) refs.idle.resetCooldown();
                    break;
                case 'terrain':
                    if (refs && refs.terrainOverlay) {
                        var vis = refs.terrainOverlay.style.display !== 'none';
                        refs.terrainOverlay.style.display = vis ? 'none' : 'block';
                    }
                    break;
            }
        },

        startPerfMonitor: function() {
            if (this._rafId) return;
            var self = this;
            var last = performance.now();

            function tick(now) {
                var dt = now - last;
                last = now;
                self._frameTimes.push(dt);
                self._totalFrames++;
                if (dt > 1) self._overBudget++;
                if (self._frameTimes.length > 120) self._frameTimes.shift();
                self._rafId = requestAnimationFrame(tick);
            }

            self._rafId = requestAnimationFrame(tick);
        },

        startDisplayUpdate: function() {
            var self = this;
            function update() {
                if (!document.getElementById('ap-state')) return; // section gone
                self.updateDisplay();
                self._updateTimer = setTimeout(update, 500);
            }
            update();
        },

        stopPerfMonitor: function() {
            if (this._rafId) {
                cancelAnimationFrame(this._rafId);
                this._rafId = null;
            }
        },

        stopDisplayUpdate: function() {
            if (this._updateTimer) {
                clearTimeout(this._updateTimer);
                this._updateTimer = null;
            }
        },

        updateDisplay: function() {
            var refs = this._refs;

            // State
            var stateEl = document.getElementById('ap-state');
            if (stateEl) {
                stateEl.textContent = (refs && refs.sm) ? refs.sm.state : '未连接';
            }

            // Paws
            var pawsEl = document.getElementById('ap-paws');
            if (pawsEl && refs && refs.pawPool) {
                pawsEl.textContent = refs.pawPool.activeCount + '/8';
            }

            // Cooldown
            var cdEl = document.getElementById('ap-cooldown');
            if (cdEl && refs && refs.idle) {
                var rem = refs.idle.cooldownRemaining;
                cdEl.textContent = rem > 0 ? (rem / 1000).toFixed(0) + 's' : 'ready';
                cdEl.style.color = rem > 0 ? 'var(--warning-color, #e6a817)' : 'var(--success-color, #2ea44f)';
            }

            // Device
            var devEl = document.getElementById('ap-device');
            if (devEl && typeof DeviceProfile !== 'undefined') {
                devEl.textContent = DeviceProfile.tier;
            }

            // Position — read from PatrolDebug if available
            var posEl = document.getElementById('ap-pos');
            if (posEl) {
                var pdPos = document.getElementById('pd-pos');
                if (pdPos) posEl.textContent = pdPos.textContent;
            }

            // Performance
            if (this._frameTimes.length > 10) {
                var sum = 0, peak = 0;
                for (var i = 0; i < this._frameTimes.length; i++) {
                    sum += this._frameTimes[i];
                    if (this._frameTimes[i] > peak) peak = this._frameTimes[i];
                }
                var avg = sum / this._frameTimes.length;
                var fpsEl = document.getElementById('ap-fps');
                if (fpsEl) fpsEl.textContent = Math.round(1000 / avg);
                var avgEl = document.getElementById('ap-avg');
                if (avgEl) avgEl.textContent = avg.toFixed(1) + 'ms';
                var peakEl = document.getElementById('ap-peak');
                if (peakEl) peakEl.textContent = peak.toFixed(1) + 'ms';

                var overPct = this._totalFrames > 0 ? ((this._overBudget / this._totalFrames) * 100).toFixed(1) : '0';
                var overEl = document.getElementById('ap-over');
                if (overEl) {
                    overEl.textContent = overPct + '%';
                    overEl.style.color = parseFloat(overPct) > 5 ? 'var(--danger-color, #da3633)' : 'var(--success-color, #2ea44f)';
                }
            }
        },

        destroy: function() {
            if (this._rafId) { cancelAnimationFrame(this._rafId); this._rafId = null; }
            if (this._updateTimer) { clearTimeout(this._updateTimer); this._updateTimer = null; }
            this._frameTimes = [];
            this._overBudget = 0;
            this._totalFrames = 0;
            this._refs = null;
            this._paused = false;
        }
    };

    // ── Utility: debounce ──
    function debounce(fn, delay) {
        var timer;
        return function() {
            var ctx = this, args = arguments;
            clearTimeout(timer);
            timer = setTimeout(function() { fn.apply(ctx, args); }, delay);
        };
    }

    return {
        init: init,
        showSection: showSection
    };
})();
