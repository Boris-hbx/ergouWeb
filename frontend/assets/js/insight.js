// ========== Insight — 洞察模块入口 + 列表页 (T-106 P1) ==========
//
// 工作 Hub 第 2 张卡的"洞察"入口由 Insight.openHub() 触发。
// 模块内部有两层视图:列表(`insight-list-view`)/详情(`insight-detail-view`),由 Insight.openDetail / InsightDetail.close 切换。
// 候选池(`insight-capture-sidebar`)在列表页右侧固定;详情页内 source 由 InsightDetail 自管。
//
// 数据流(列表):
//   API.insightList({status?}) → 渲染表格行;
//   每行点击 → Insight.openDetail(id) → InsightDetail.open(id);
//   候选池由 InsightCapture 模块管理(并行刷新)。

var Insight = (function() {
    var _insights = [];
    var _statusFilter = '';
    var _showNewForm = false;

    function _esc(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }

    function _statusLabel(s) {
        return ({
            collecting: '收集中',
            ready:      '等待生成',
            processing: '生成中',
            editing:    '编辑中',
            published:  '已发布',
            archived:   '已归档',
        })[s] || s;
    }
    function _statusClass(s) { return 'ins-st-' + s; }
    function _templateLabel(t) {
        return ({ survey: '综述型', decision: '决策型', watch: '追踪型' })[t] || t;
    }
    function _shortTime(iso) {
        if (!iso) return '';
        try {
            var d = new Date(iso);
            var now = new Date();
            var diffH = (now - d) / 36e5;
            if (diffH < 1) return Math.max(1, Math.round(diffH * 60)) + ' 分钟前';
            if (diffH < 24) return Math.round(diffH) + ' 小时前';
            if (diffH < 24 * 7) return Math.round(diffH / 24) + ' 天前';
            return d.getFullYear() + '-' + String(d.getMonth()+1).padStart(2,'0') + '-' + String(d.getDate()).padStart(2,'0');
        } catch (_) { return iso; }
    }

    // ============ Hub 切换 ============
    function openHub() {
        // 隐藏工作 Hub + 任务表视图,显示洞察视图
        var hub = document.getElementById('work-hub');
        var tableView = document.getElementById('work-table-view');
        var insView = document.getElementById('work-insight-view');
        if (hub) hub.style.display = 'none';
        if (tableView) tableView.style.display = 'none';
        if (insView) insView.style.display = '';

        // 默认进列表页(关掉详情)
        var listEl = document.getElementById('insight-list-view');
        var detEl  = document.getElementById('insight-detail-view');
        if (listEl) listEl.classList.remove('ins-hidden');
        if (detEl)  detEl.classList.add('ins-hidden');

        _renderList();   // 先渲染骨架
        refreshList();   // 异步加载数据
    }

    function backToHub() {
        // 关详情(如果开着)
        var detEl = document.getElementById('insight-detail-view');
        if (detEl && !detEl.classList.contains('ins-hidden')) {
            if (typeof InsightDetail !== 'undefined') InsightDetail.close();
            return;
        }
        // 列表页 → 回工作 Hub
        var insView = document.getElementById('work-insight-view');
        var hub = document.getElementById('work-hub');
        if (insView) insView.style.display = 'none';
        if (hub) hub.style.display = '';
    }

    // ============ 列表渲染 ============
    function _renderList() {
        var host = document.getElementById('insight-list-view');
        if (!host) return;
        host.innerHTML = ''
          + '<div class="ins-list-main">'
          +   '<div class="ins-list-head">'
          +     '<h2>洞察</h2>'
          +     '<span class="ins-count-pill">' + _insights.length + ' 个</span>'
          +     '<div class="ins-list-spacer"></div>'
          +     _statusFilterHtml()
          +     '<button class="ins-btn ins-btn-primary" onclick="Insight.toggleNewForm()">+ 新建洞察</button>'
          +   '</div>'
          +   _newFormHtml()
          +   '<div class="ins-list-table">' + _tableHtml() + '</div>'
          + '</div>'
          + '<aside class="ins-capture-aside" id="insight-capture-sidebar"></aside>';

        // 候选池侧栏由 InsightCapture 渲染
        if (typeof InsightCapture !== 'undefined') InsightCapture.refresh();

        // 自动 focus 新建标题输入
        if (_showNewForm) {
            var ti = document.getElementById('ins-new-title');
            if (ti) ti.focus();
        }
    }

    function _statusFilterHtml() {
        var opts = [
            ['',           '全部'],
            ['collecting', '收集中'],
            ['ready',      '等待生成'],
            ['processing', '生成中'],
            ['editing',    '编辑中'],
            ['published',  '已发布'],
            ['archived',   '已归档'],
        ];
        return '<select class="ins-filter" onchange="Insight.setFilter(this.value)">'
          + opts.map(function(o) {
              return '<option value="' + o[0] + '"' + (_statusFilter === o[0] ? ' selected' : '') + '>' + o[1] + '</option>';
            }).join('')
          + '</select>';
    }

    function _newFormHtml() {
        if (!_showNewForm) return '';
        return '<div class="ins-new-form">'
          + '<input id="ins-new-title" type="text" placeholder="主题/标题(必填)" '
          +   'onkeydown="if(event.key===\'Enter\')Insight.submitNew();if(event.key===\'Escape\')Insight.toggleNewForm()">'
          + '<input id="ins-new-topic" type="text" placeholder="一句话描述(可选)" '
          +   'onkeydown="if(event.key===\'Enter\')Insight.submitNew();if(event.key===\'Escape\')Insight.toggleNewForm()">'
          + '<select id="ins-new-template">'
          +   '<option value="survey">综述型</option>'
          +   '<option value="decision">决策型</option>'
          +   '<option value="watch">追踪型</option>'
          + '</select>'
          + '<button class="ins-btn ins-btn-primary" onclick="Insight.submitNew()">创建</button>'
          + '<button class="ins-btn ins-btn-ghost" onclick="Insight.toggleNewForm()">取消</button>'
          + '</div>';
    }

    function _tableHtml() {
        if (_insights.length === 0) {
            return '<div class="ins-list-empty">'
              + '<div class="ins-list-empty-icon">🔍</div>'
              + '<div class="ins-list-empty-title">还没有洞察</div>'
              + '<div class="ins-list-empty-sub">点上面「+ 新建洞察」开始一个研究项目;或先从右侧候选池捕获素材。</div>'
              + '</div>';
        }
        return '<table class="ins-table">'
          + '<thead><tr>'
          +   '<th>标题</th>'
          +   '<th>主题</th>'
          +   '<th>模板</th>'
          +   '<th>状态</th>'
          +   '<th>更新</th>'
          +   '<th></th>'
          + '</tr></thead>'
          + '<tbody>' + _insights.map(_rowHtml).join('') + '</tbody>'
          + '</table>';
    }

    function _rowHtml(ins) {
        return '<tr class="ins-row" onclick="Insight.openDetail(' + ins.id + ')">'
          + '<td class="ins-row-title">' + _esc(ins.title) + (ins.currentReportId ? ' <span class="ins-row-ver">v?</span>' : '') + '</td>'
          + '<td class="ins-row-topic">' + _esc(ins.topic || '—') + '</td>'
          + '<td>' + _templateLabel(ins.template) + '</td>'
          + '<td><span class="ins-pill ' + _statusClass(ins.status) + '">' + _statusLabel(ins.status) + '</span></td>'
          + '<td>' + _shortTime(ins.updatedAt) + '</td>'
          + '<td class="ins-row-actions" onclick="event.stopPropagation()">'
          +   (ins.status !== 'archived'
              ? '<button class="ins-icon-btn" onclick="Insight.archive(' + ins.id + ')" title="归档">📦</button>'
              : '<button class="ins-icon-btn" onclick="Insight.unarchive(' + ins.id + ')" title="取消归档">📤</button>')
          +   '<button class="ins-icon-btn ins-icon-btn-x" onclick="Insight.confirmDelete(' + ins.id + ', \'' + _esc(ins.title) + '\')" title="删除">✕</button>'
          + '</td>'
          + '</tr>';
    }

    // ============ 数据加载 ============
    async function refreshList() {
        try {
            var params = _statusFilter ? { status: _statusFilter } : {};
            var resp = await API.insightList(params);
            _insights = (resp && resp.items) || [];
            _renderList();
        } catch (e) {
            console.error('[Insight] refreshList err', e);
            if (typeof showToast === 'function') showToast('加载洞察列表失败', 'error');
        }
    }

    // ============ 操作 ============
    function setFilter(v) {
        _statusFilter = v || '';
        refreshList();
    }

    function toggleNewForm() {
        _showNewForm = !_showNewForm;
        _renderList();
    }

    async function submitNew() {
        var titleEl = document.getElementById('ins-new-title');
        var topicEl = document.getElementById('ins-new-topic');
        var tmplEl  = document.getElementById('ins-new-template');
        if (!titleEl) return;
        var title = (titleEl.value || '').trim();
        if (!title) {
            if (typeof showToast === 'function') showToast('标题不能为空', 'warning');
            titleEl.focus();
            return;
        }
        var data = { title: title };
        if (topicEl && topicEl.value) data.topic = topicEl.value.trim();
        if (tmplEl && tmplEl.value)   data.template = tmplEl.value;
        try {
            var resp = await API.insightCreate(data);
            if (resp && resp.success) {
                _showNewForm = false;
                if (typeof showToast === 'function') showToast('已创建', 'success');
                await refreshList();
                // 直接进详情页(顺手开始加 source)
                openDetail(resp.item.id);
            }
        } catch (e) {
            console.error('[Insight] create err', e);
            if (typeof showToast === 'function') showToast('创建失败', 'error');
        }
    }

    function openDetail(id) {
        if (typeof InsightDetail !== 'undefined') {
            InsightDetail.open(id);
        }
    }

    async function archive(id) {
        try {
            await API.insightUpdate(id, { status: 'archived' });
            await refreshList();
        } catch (e) { console.error('[Insight] archive err', e); }
    }
    async function unarchive(id) {
        // 取消归档:回到 collecting(若无报告)或 editing(若有)
        var ins = _insights.find(function(x) { return x.id === id; });
        if (!ins) return;
        var to = ins.currentReportId ? 'editing' : 'collecting';
        try {
            await API.insightUpdate(id, { status: to });
            await refreshList();
        } catch (e) { console.error('[Insight] unarchive err', e); }
    }

    function confirmDelete(id, title) {
        var msg = '彻底删除「' + _esc(title) + '」?<br>'
            + '<small style="color:#6B7280">软删除,不可在 UI 恢复(数据库保留 30 天)。包括的 source / report / 分享链接也会失效。</small>';
        if (window.AppUtils && AppUtils.showConfirm) {
            AppUtils.showConfirm(msg, function() { _doDelete(id); }, { confirmText: '删除', danger: true });
        } else if (confirm('删除「' + title + '」?')) {
            _doDelete(id);
        }
    }
    async function _doDelete(id) {
        try {
            await API.insightDelete(id);
            if (typeof showToast === 'function') showToast('已删除', 'info');
            await refreshList();
        } catch (e) {
            console.error('[Insight] delete err', e);
            if (typeof showToast === 'function') showToast('删除失败', 'error');
        }
    }

    return {
        openHub: openHub,
        backToHub: backToHub,
        refreshList: refreshList,
        setFilter: setFilter,
        toggleNewForm: toggleNewForm,
        submitNew: submitNew,
        openDetail: openDetail,
        archive: archive,
        unarchive: unarchive,
        confirmDelete: confirmDelete,
    };
})();
