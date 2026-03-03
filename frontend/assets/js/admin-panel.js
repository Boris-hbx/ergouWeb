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
            case 'system': System.load(); break;
            case 'audit': Audit.load(); break;
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
    // ═══════════════════════════════════════════
    var Chats = {
        _offset: 0,
        _limit: 20,
        _filters: { user_id: '', date_from: '', date_to: '' },
        _total: 0,

        load: async function() {
            var el = document.getElementById('admin-chats-content');
            if (!el) return;
            if (!document.getElementById('admin-chats-filters')) {
                this.renderFilters(el);
            }
            await this.fetchList();
        },

        renderFilters: function(el) {
            el.innerHTML = '<div class="admin-filters" id="admin-chats-filters">' +
                '<input class="admin-filter-input" type="text" placeholder="用户ID筛选..." id="admin-chats-user" style="width:160px;">' +
                '<input class="admin-filter-input" type="date" id="admin-chats-from">' +
                '<span style="opacity:0.5;">至</span>' +
                '<input class="admin-filter-input" type="date" id="admin-chats-to">' +
                '<button class="admin-refresh-btn" id="admin-chats-search-btn">搜索</button>' +
                '</div>' +
                '<div id="admin-chats-list"></div>' +
                '<div id="admin-chats-detail" style="display:none;"></div>';
            var self = this;
            document.getElementById('admin-chats-search-btn').addEventListener('click', function() {
                self._filters.user_id = document.getElementById('admin-chats-user').value;
                self._filters.date_from = document.getElementById('admin-chats-from').value;
                self._filters.date_to = document.getElementById('admin-chats-to').value;
                self._offset = 0;
                self.fetchList();
            });
        },

        fetchList: async function() {
            var list = document.getElementById('admin-chats-list');
            if (!list) return;
            list.innerHTML = '<div class="admin-loading-text">加载中...</div>';
            document.getElementById('admin-chats-detail').style.display = 'none';
            try {
                var qs = '?limit=' + this._limit + '&offset=' + this._offset;
                if (this._filters.user_id) qs += '&user_id=' + encodeURIComponent(this._filters.user_id);
                if (this._filters.date_from) qs += '&date_from=' + this._filters.date_from;
                if (this._filters.date_to) qs += '&date_to=' + this._filters.date_to;
                var data = await API.adminRequest('GET', '/admin/conversations' + qs);
                if (!data.success) { list.innerHTML = '<div class="admin-empty">加载失败</div>'; return; }
                this._total = data.total || 0;
                this.renderList(list, data.conversations || []);
            } catch(e) {
                list.innerHTML = '<div class="admin-empty">加载失败</div>';
            }
        },

        renderList: function(el, convos) {
            if (!convos.length) { el.innerHTML = '<div class="admin-empty">无对话记录</div>'; return; }
            var html = '<table class="admin-panel-table"><thead><tr>' +
                '<th>标题</th><th>用户</th><th>消息数</th><th>Tokens</th><th>更新时间</th>' +
                '</tr></thead><tbody>';
            var self = this;
            for (var i = 0; i < convos.length; i++) {
                var c = convos[i];
                html += '<tr style="cursor:pointer;" data-conv-id="' + esc(c.id) + '">';
                html += '<td>' + esc(c.title || '(无标题)') + '</td>';
                html += '<td>' + esc(c.user_name || c.user_id) + '</td>';
                html += '<td>' + (c.message_count || 0) + '</td>';
                html += '<td>' + fmt(c.token_sum || 0) + '</td>';
                html += '<td>' + shortDateTime(c.updated_at) + '</td>';
                html += '</tr>';
            }
            html += '</tbody></table>';
            // Pagination
            html += '<div class="admin-pagination">';
            html += '<button ' + (this._offset === 0 ? 'disabled' : '') + ' id="admin-chats-prev">上一页</button>';
            html += '<span>' + (this._offset + 1) + '-' + Math.min(this._offset + this._limit, this._total) + ' / ' + this._total + '</span>';
            html += '<button ' + (this._offset + this._limit >= this._total ? 'disabled' : '') + ' id="admin-chats-next">下一页</button>';
            html += '</div>';
            el.innerHTML = html;

            // Bind row click
            el.querySelectorAll('[data-conv-id]').forEach(function(row) {
                row.addEventListener('click', function() {
                    self.openConversation(row.dataset.convId);
                });
            });
            // Pagination
            var prevBtn = document.getElementById('admin-chats-prev');
            var nextBtn = document.getElementById('admin-chats-next');
            if (prevBtn) prevBtn.addEventListener('click', function() { self._offset -= self._limit; self.fetchList(); });
            if (nextBtn) nextBtn.addEventListener('click', function() { self._offset += self._limit; self.fetchList(); });
        },

        openConversation: async function(id) {
            var detail = document.getElementById('admin-chats-detail');
            if (!detail) return;
            detail.style.display = '';
            detail.innerHTML = '<div class="admin-loading-text">加载消息中...</div>';
            try {
                var data = await API.adminRequest('GET', '/admin/conversations/' + encodeURIComponent(id) + '/messages');
                if (!data.success) { detail.innerHTML = '<div class="admin-empty">加载失败</div>'; return; }
                this.renderMessages(detail, data.messages || []);
            } catch(e) {
                detail.innerHTML = '<div class="admin-empty">加载失败</div>';
            }
        },

        renderMessages: function(el, messages) {
            var html = '<div class="admin-detail-panel">';
            html += '<div class="admin-detail-header"><div class="admin-detail-title">对话详情 (' + messages.length + ' 条消息)</div>';
            html += '<button class="admin-refresh-btn" id="admin-chats-close-detail">关闭</button></div>';
            html += '<div class="admin-chat-messages">';
            for (var i = 0; i < messages.length; i++) {
                var m = messages[i];
                var cls = m.role === 'user' ? 'user' : 'assistant';
                html += '<div class="admin-chat-msg ' + cls + '">';
                html += esc(m.content || m.content_text || '');
                html += '<div class="msg-meta">' + shortDateTime(m.created_at);
                if (m.token_count) html += ' · ' + fmt(m.token_count) + ' tokens';
                html += '</div></div>';
            }
            html += '</div></div>';
            el.innerHTML = html;
            document.getElementById('admin-chats-close-detail').addEventListener('click', function() {
                el.style.display = 'none';
            });
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
                    var userInput = document.getElementById('admin-chats-user');
                    if (userInput) userInput.value = uid;
                    Chats._filters.user_id = uid;
                    Chats._offset = 0;
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
