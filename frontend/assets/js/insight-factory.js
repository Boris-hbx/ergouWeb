// ========== Insight Factory (T-205) ==========
//
// Web-only shell for `/api/insight-factory/*`.
// Kept separate from `Insight` so the current `/insights` workflow and state
// machine stay untouched.

var InsightFactory = (function() {
    var _tasks = [];
    var _statusFilter = '';
    var _detailId = null;
    var _detail = null;
    var _jobs = [];
    var _reports = [];
    var _health = null;
    var _memories = [];
    var _memoryFilter = '';
    var _memoryEditingId = null;
    var _typeManual = false;
    var _showRaw = false;
    var _expandedReports = {};

    function _esc(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }

    function _statusLabel(s) {
        return ({
            idle: '未生成',
            pending: '排队中',
            running: '生成中',
            done: '已完成',
            failed: '失败'
        })[s] || s || '';
    }

    function _jobStatusLabel(s) {
        return ({
            pending: '排队中',
            running: '运行中',
            done: '完成',
            failed: '失败',
            blocked: '阻塞'
        })[s] || s || '';
    }

    function _modeLabel(s) {
        return ({ generate: '生成', revise: '修订', retry: '重试' })[s] || s || '';
    }

    function _typeLabel(t) {
        return ({ url: '链接', topic: '主题', prompt: '指令', note: '随想' })[t] || t || '';
    }

    function _templateLabel(t) {
        return ({ survey: '综述型', decision: '决策型', watch: '追踪型' })[t] || (t || '自动');
    }

    function _memoryTypeLabel(t) {
        return ({
            project_fact: '工程事实',
            boris_profile: 'Boris 画像',
            report_preference: '报告偏好',
            insight_summary: '历史洞察'
        })[t] || t || '';
    }

    function _shortTime(iso) {
        if (!iso) return '';
        try {
            var d = new Date(iso);
            var diffH = (Date.now() - d.getTime()) / 36e5;
            if (diffH < 1) return Math.max(1, Math.round(diffH * 60)) + ' 分钟前';
            if (diffH < 24) return Math.round(diffH) + ' 小时前';
            if (diffH < 24 * 7) return Math.round(diffH / 24) + ' 天前';
            return d.getFullYear() + '-' + String(d.getMonth() + 1).padStart(2, '0') + '-' + String(d.getDate()).padStart(2, '0');
        } catch (_) {
            return iso;
        }
    }

    function detectInputType(text) {
        var t = (text || '').trim();
        var lower = t.toLowerCase();
        if ((lower.indexOf('http://') >= 0 || lower.indexOf('https://') >= 0) && t.split(/\s+/).length === 1) {
            return 'url';
        }
        var n = Array.from(t).length;
        if (n <= 80) return 'topic';
        var markers = ['帮我', '请', '分析', '总结', '对比', '写一份', '写一篇', '整理', '给我', '梳理', '评估'];
        for (var i = 0; i < markers.length; i++) {
            if (t.indexOf(markers[i]) >= 0) return 'prompt';
        }
        return 'note';
    }

    function _setUrl(path, replace) {
        if (!window.history || !path) return;
        var next = path || '/insight-factory';
        if (window.location.pathname === next) return;
        var fn = replace ? 'replaceState' : 'pushState';
        window.history[fn]({ insightFactory: true }, '', next);
    }

    function _showDetail(on) {
        var listEl = document.getElementById('insight-factory-list-view');
        var detEl = document.getElementById('insight-factory-detail-view');
        if (listEl) listEl.classList.toggle('ins-hidden', !!on);
        if (detEl) detEl.classList.toggle('ins-hidden', !on);
    }

    function _showShell() {
        var hub = document.getElementById('work-hub');
        var tableView = document.getElementById('work-table-view');
        var insightView = document.getElementById('work-insight-view');
        var doneView = document.getElementById('work-done-view');
        var factoryView = document.getElementById('work-insight-factory-view');
        if (hub) hub.style.display = 'none';
        if (tableView) tableView.style.display = 'none';
        if (insightView) insightView.style.display = 'none';
        if (doneView) doneView.style.display = 'none';
        if (factoryView) factoryView.style.display = '';
    }

    function openHub(opts) {
        opts = opts || {};
        _detailId = null;
        _detail = null;
        _showShell();
        _showDetail(false);
        if (!opts.noHistory) _setUrl('/insight-factory', !!opts.replace);
        _renderList();
        refreshList();
        refreshHealth();
        refreshMemories();
    }

    function backToHub() {
        var detEl = document.getElementById('insight-factory-detail-view');
        if (detEl && !detEl.classList.contains('ins-hidden')) {
            _detailId = null;
            _detail = null;
            _showDetail(false);
            _setUrl('/insight-factory');
            refreshList();
            return;
        }
        var factoryView = document.getElementById('work-insight-factory-view');
        var hub = document.getElementById('work-hub');
        if (factoryView) factoryView.style.display = 'none';
        if (hub) hub.style.display = '';
        if (window.location.pathname.indexOf('/insight-factory') === 0) _setUrl('/', false);
    }

    function _statusFilterHtml() {
        var opts = [['', '全部'], ['idle', '未生成'], ['pending', '排队中'], ['running', '生成中'], ['done', '已完成'], ['failed', '失败']];
        return '<select class="ins-filter" onchange="InsightFactory.setFilter(this.value)">'
            + opts.map(function(o) {
                return '<option value="' + o[0] + '"' + (_statusFilter === o[0] ? ' selected' : '') + '>' + o[1] + '</option>';
            }).join('')
            + '</select>';
    }

    function _healthHtml() {
        var h = _health || {};
        var status = h.status || 'unknown';
        var gate = h.quotaGate || 'unknown';
        var cls = (status === 'ok' || status === 'ready') ? 'ok' : (status === 'placeholder' || status === 'blocked') ? 'warn' : 'muted';
        return '<div class="inf-health inf-health-' + cls + '">'
            + '<span class="inf-health-dot"></span>'
            + '<span>provider ' + _esc(h.provider || 'codex') + '</span>'
            + '<span>gate ' + _esc(gate) + '</span>'
            + '<span>API fallback ' + (h.apiKeyFallback ? 'on' : 'off') + '</span>'
            + '</div>';
    }

    function _captureHtml() {
        var typeOptions = ['url', 'topic', 'prompt', 'note'].map(function(t) {
            return '<option value="' + t + '">' + _typeLabel(t) + '</option>';
        }).join('');
        var tmplOptions = '<option value="">自动</option>'
            + ['survey', 'decision', 'watch'].map(function(t) {
                return '<option value="' + t + '">' + _templateLabel(t) + '</option>';
            }).join('');
        return '<div class="inf-capture">'
            + '<textarea id="inf-cap-text" class="ins-cap-textarea" rows="4" '
            + 'placeholder="贴链接、写主题、输入 prompt 或记录随想。Ctrl+Enter 创建并生成。" '
            + 'oninput="InsightFactory.onCaptureInput()" '
            + 'onkeydown="if((event.ctrlKey||event.metaKey)&&event.key===\'Enter\')InsightFactory.submitNew()"></textarea>'
            + '<div class="ins-cap-foot">'
            + '<label class="ins-cap-type-label">类型<select id="inf-cap-type" class="ins-cap-type" onchange="InsightFactory.onTypeManual()">' + typeOptions + '</select></label>'
            + '<label class="ins-cap-type-label">模板<select id="inf-cap-template" class="ins-cap-type">' + tmplOptions + '</select></label>'
            + '<span class="ins-cap-type-hint" id="inf-cap-type-hint"></span>'
            + '<div class="ins-cap-spacer"></div>'
            + '<button class="ins-btn ins-btn-primary" onclick="InsightFactory.submitNew()">创建并生成</button>'
            + '</div>'
            + '</div>';
    }

    function _renderList() {
        var host = document.getElementById('insight-factory-list-view');
        if (!host) return;
        host.innerHTML = '<div class="inf-list-main">'
            + '<div class="ins-list-head">'
            + '<h2>洞察工厂</h2>'
            + '<span class="ins-count-pill">' + _tasks.length + ' 个</span>'
            + '<div class="ins-list-spacer"></div>'
            + _healthHtml()
            + _statusFilterHtml()
            + '</div>'
            + _captureHtml()
            + _memoryPanelHtml()
            + '<div class="ins-list-table">' + _tableHtml() + '</div>'
            + '</div>';
        _syncDetectedType();
    }

    function _memoryPanelHtml() {
        var types = ['', 'project_fact', 'boris_profile', 'report_preference', 'insight_summary'];
        var filterOptions = types.map(function(t) {
            return '<option value="' + t + '"' + (_memoryFilter === t ? ' selected' : '') + '>'
                + (t ? _memoryTypeLabel(t) : '全部记忆') + '</option>';
        }).join('');
        var editing = _memories.filter(function(m) { return m.id === _memoryEditingId; })[0] || null;
        var type = editing ? editing.type : 'report_preference';
        var typeOptions = ['project_fact', 'boris_profile', 'report_preference', 'insight_summary'].map(function(t) {
            return '<option value="' + t + '"' + (type === t ? ' selected' : '') + '>' + _memoryTypeLabel(t) + '</option>';
        }).join('');
        var rows = _memories.length
            ? _memories.map(_memoryItemHtml).join('')
            : '<div class="inf-memory-empty">暂无工厂记忆。新增后会在下一次生成/修订时注入 worker 上下文。</div>';
        return '<section class="inf-memory-panel">'
            + '<div class="inf-memory-head">'
            + '<div><h3>工厂记忆</h3><div class="inf-memory-sub">仅用于洞察工厂，不写入二狗通用记忆。</div></div>'
            + '<select class="ins-filter" onchange="InsightFactory.setMemoryFilter(this.value)">' + filterOptions + '</select>'
            + '</div>'
            + '<div class="inf-memory-editor">'
            + '<select id="inf-mem-type" class="ins-cap-type">' + typeOptions + '</select>'
            + '<input id="inf-mem-title" class="inf-memory-title" value="' + _esc(editing ? editing.title : '') + '" placeholder="标题">'
            + '<input id="inf-mem-importance" class="inf-memory-importance" type="number" min="1" max="5" value="' + _esc(editing ? editing.importance : 3) + '" title="重要度">'
            + '<textarea id="inf-mem-body" class="inf-memory-body" rows="2" placeholder="记忆内容：工程事实、Boris 偏好、报告风格约束或历史洞察摘要。">' + _esc(editing ? editing.body : '') + '</textarea>'
            + '<div class="inf-memory-actions">'
            + '<label class="inf-memory-enabled"><input id="inf-mem-enabled" type="checkbox"' + (!editing || editing.enabled ? ' checked' : '') + '>启用</label>'
            + '<button class="ins-btn ins-btn-primary" onclick="InsightFactory.saveMemory()">' + (editing ? '保存记忆' : '新增记忆') + '</button>'
            + (editing ? '<button class="ins-btn ins-btn-secondary" onclick="InsightFactory.cancelMemoryEdit()">取消</button>' : '')
            + '</div>'
            + '</div>'
            + '<div class="inf-memory-list">' + rows + '</div>'
            + '</section>';
    }

    function _memoryItemHtml(m) {
        return '<div class="inf-memory-item' + (m.enabled ? '' : ' inf-memory-off') + '">'
            + '<div class="inf-memory-item-main">'
            + '<div class="inf-memory-item-top">'
            + '<span class="ins-tag ins-tag-type">' + _memoryTypeLabel(m.type) + '</span>'
            + '<strong>' + _esc(m.title) + '</strong>'
            + '<span class="inf-memory-score">★' + _esc(m.importance) + '</span>'
            + '</div>'
            + '<div class="inf-memory-text">' + _esc(m.body) + '</div>'
            + '</div>'
            + '<div class="inf-memory-row-actions">'
            + '<button class="ins-link-btn" onclick="InsightFactory.toggleMemory(' + m.id + ', ' + (!m.enabled) + ')">' + (m.enabled ? '禁用' : '启用') + '</button>'
            + '<button class="ins-link-btn" onclick="InsightFactory.editMemory(' + m.id + ')">编辑</button>'
            + '<button class="ins-link-btn ins-danger-link" onclick="InsightFactory.deleteMemory(' + m.id + ')">删除</button>'
            + '</div>'
            + '</div>';
    }

    function _tableHtml() {
        if (!_tasks.length) {
            return '<div class="ins-list-empty">'
                + '<div class="ins-list-empty-icon">⚙</div>'
                + '<div class="ins-list-empty-title">还没有工厂任务</div>'
                + '<div class="ins-list-empty-sub">从上方录入素材，工厂会创建 pending job，后续 worker 写回 v1/v2 报告。</div>'
                + '</div>';
        }
        return '<table class="ins-table inf-table">'
            + '<thead><tr><th class="ins-th-id">ID</th><th>标题</th><th>类型</th><th>状态</th><th>版本</th><th>更新</th><th></th></tr></thead>'
            + '<tbody>' + _tasks.map(_rowHtml).join('') + '</tbody>'
            + '</table>';
    }

    function _rowHtml(t) {
        var ver = t.latestVersion ? 'v' + t.latestVersion : '—';
        return '<tr class="ins-row" onclick="InsightFactory.openDetail(' + t.id + ')">'
            + '<td class="ins-row-id">' + t.id + '</td>'
            + '<td class="ins-row-title">' + _esc(t.title || '(无标题)') + '</td>'
            + '<td><span class="ins-tag ins-tag-type">' + _typeLabel(t.inputType) + '</span></td>'
            + '<td><span class="ins-pill inf-st-' + _esc(t.status) + '">' + _statusLabel(t.status) + '</span></td>'
            + '<td class="ins-row-ver">' + ver + '</td>'
            + '<td>' + _shortTime(t.latestReportAt || t.updatedAt) + '</td>'
            + '<td class="ins-row-actions" onclick="event.stopPropagation()">'
            + '<button class="ins-icon-btn ins-icon-btn-x" onclick="InsightFactory.confirmDelete(' + t.id + ')" title="删除">×</button>'
            + '</td>'
            + '</tr>';
    }

    async function refreshHealth() {
        try {
            var resp = await API.factoryWorkerHealth();
            if (resp && resp.success) _health = resp;
            _renderList();
        } catch (e) {
            console.error('[InsightFactory] health', e);
            _health = { provider: 'codex', status: 'unknown', quotaGate: 'unknown', apiKeyFallback: false };
            _renderList();
        }
    }

    async function refreshList() {
        try {
            var params = _statusFilter ? { status: _statusFilter } : {};
            var resp = await API.factoryTaskList(params);
            _tasks = (resp && resp.items) || [];
            _renderList();
        } catch (e) {
            console.error('[InsightFactory] refreshList', e);
            if (typeof showToast === 'function') showToast('加载洞察工厂列表失败', 'error');
        }
    }

    async function refreshMemories() {
        try {
            var params = {};
            if (_memoryFilter) params.type = _memoryFilter;
            var resp = await API.factoryMemoryList(params);
            _memories = (resp && resp.items) || [];
            _renderList();
        } catch (e) {
            console.error('[InsightFactory] memories', e);
            if (typeof showToast === 'function') showToast('加载工厂记忆失败', 'error');
        }
    }

    function setFilter(v) {
        _statusFilter = v || '';
        refreshList();
    }

    function setMemoryFilter(v) {
        _memoryFilter = v || '';
        _memoryEditingId = null;
        refreshMemories();
    }

    function editMemory(id) {
        _memoryEditingId = Number(id);
        _renderList();
    }

    function cancelMemoryEdit() {
        _memoryEditingId = null;
        _renderList();
    }

    async function saveMemory() {
        var typeEl = document.getElementById('inf-mem-type');
        var titleEl = document.getElementById('inf-mem-title');
        var bodyEl = document.getElementById('inf-mem-body');
        var impEl = document.getElementById('inf-mem-importance');
        var enEl = document.getElementById('inf-mem-enabled');
        var title = titleEl ? (titleEl.value || '').trim() : '';
        var body = bodyEl ? (bodyEl.value || '').trim() : '';
        if (!title || !body) {
            if (typeof showToast === 'function') showToast('记忆标题和内容不能为空', 'warning');
            return;
        }
        var payload = {
            type: typeEl ? typeEl.value : 'report_preference',
            title: title,
            body: body,
            importance: Math.max(1, Math.min(5, Number(impEl && impEl.value ? impEl.value : 3))),
            enabled: !!(enEl && enEl.checked),
            source: 'manual'
        };
        try {
            if (_memoryEditingId) {
                await API.factoryMemoryUpdate(_memoryEditingId, payload);
            } else {
                await API.factoryMemoryCreate(payload);
            }
            _memoryEditingId = null;
            await refreshMemories();
            if (typeof showToast === 'function') showToast('工厂记忆已保存', 'success');
        } catch (e) {
            console.error('[InsightFactory] saveMemory', e);
            if (typeof showToast === 'function') showToast('保存工厂记忆失败', 'error');
        }
    }

    async function toggleMemory(id, enabled) {
        try {
            await API.factoryMemoryUpdate(id, { enabled: !!enabled });
            await refreshMemories();
        } catch (e) {
            console.error('[InsightFactory] toggleMemory', e);
            if (typeof showToast === 'function') showToast('更新记忆状态失败', 'error');
        }
    }

    async function deleteMemory(id) {
        if (!confirm('删除这条工厂记忆?')) return;
        try {
            await API.factoryMemoryDelete(id);
            if (_memoryEditingId === id) _memoryEditingId = null;
            await refreshMemories();
            if (typeof showToast === 'function') showToast('已删除工厂记忆', 'info');
        } catch (e) {
            console.error('[InsightFactory] deleteMemory', e);
            if (typeof showToast === 'function') showToast('删除工厂记忆失败', 'error');
        }
    }

    function onCaptureInput() {
        if (!_typeManual) _syncDetectedType();
    }

    function onTypeManual() {
        _typeManual = true;
        var hint = document.getElementById('inf-cap-type-hint');
        if (hint) hint.textContent = '已手动指定';
    }

    function _syncDetectedType() {
        var ta = document.getElementById('inf-cap-text');
        var sel = document.getElementById('inf-cap-type');
        var hint = document.getElementById('inf-cap-type-hint');
        if (!ta || !sel) return;
        var txt = ta.value || '';
        var t = detectInputType(txt);
        sel.value = t;
        if (hint) hint.textContent = txt.trim() ? '自动识别为「' + _typeLabel(t) + '」' : '';
    }

    async function submitNew() {
        var ta = document.getElementById('inf-cap-text');
        var typeSel = document.getElementById('inf-cap-type');
        var tmplSel = document.getElementById('inf-cap-template');
        if (!ta) return;
        var content = (ta.value || '').trim();
        if (!content) {
            if (typeof showToast === 'function') showToast('录入内容不能为空', 'warning');
            ta.focus();
            return;
        }
        var data = {
            inputContent: content,
            inputType: typeSel && typeSel.value ? typeSel.value : detectInputType(content),
            createGenerateJob: true,
            provider: 'codex'
        };
        if (tmplSel && tmplSel.value) data.template = tmplSel.value;
        try {
            var resp = await API.factoryTaskCreate(data);
            if (resp && resp.success && resp.item) {
                _typeManual = false;
                if (typeof showToast === 'function') showToast('已创建生成任务', 'success');
                openDetail(resp.item.id);
            }
        } catch (e) {
            console.error('[InsightFactory] create', e);
            if (typeof showToast === 'function') showToast('创建失败', 'error');
        }
    }

    function confirmDelete(id) {
        var t = _tasks.filter(function(x) { return x.id === id; })[0];
        var title = t ? t.title : id;
        var msg = '删除「' + _esc(title) + '」?<br><small style="color:#6B7280">洞察工厂任务会软删除。</small>';
        if (window.AppUtils && AppUtils.showConfirm) {
            AppUtils.showConfirm(msg, function() { _doDelete(id); }, { confirmText: '删除', danger: true });
        } else if (confirm('删除「' + title + '」?')) {
            _doDelete(id);
        }
    }

    async function _doDelete(id) {
        try {
            await API.factoryTaskDelete(id);
            if (_detailId === id) {
                _detailId = null;
                _detail = null;
                _showDetail(false);
                _setUrl('/insight-factory');
            }
            await refreshList();
            if (typeof showToast === 'function') showToast('已删除', 'info');
        } catch (e) {
            console.error('[InsightFactory] delete', e);
            if (typeof showToast === 'function') showToast('删除失败', 'error');
        }
    }

    async function openDetail(id, opts) {
        opts = opts || {};
        _detailId = Number(id);
        _showRaw = false;
        _showShell();
        _showDetail(true);
        if (!opts.noHistory) _setUrl('/insight-factory/' + _detailId, !!opts.replace);
        var host = document.getElementById('insight-factory-detail-view');
        if (host) host.innerHTML = '<div class="ins-det-loading">加载中...</div>';
        await _loadDetail();
    }

    async function _loadDetail() {
        if (_detailId == null) return;
        try {
            var resp = await API.factoryTaskGet(_detailId);
            if (resp && resp.success) _detail = resp.item;
            _jobs = [];
            _reports = [];
            try {
                var jr = await API.factoryTaskJobs(_detailId);
                if (jr && jr.success) _jobs = jr.items || [];
            } catch (e1) {
                console.error('[InsightFactory] jobs', e1);
            }
            try {
                var rr = await API.factoryTaskReports(_detailId);
                if (rr && rr.success) _reports = rr.items || [];
            } catch (e2) {
                console.error('[InsightFactory] reports', e2);
            }
            _renderDetail();
        } catch (e) {
            console.error('[InsightFactory] detail', e);
            if (typeof showToast === 'function') showToast('加载详情失败', 'error');
        }
    }

    function _renderDetail() {
        var host = document.getElementById('insight-factory-detail-view');
        if (!host || !_detail) return;
        host.innerHTML = '<div class="ins-det inf-det">'
            + _infoHtml(_detail)
            + _jobHtml(_detail)
            + _reportHtml(_detail)
            + _feedbackHtml(_detail)
            + _historyHtml()
            + '</div>';
        var rb = host.querySelector('.ins-report-body');
        if (rb && typeof InsightMd !== 'undefined' && InsightMd.decorate) InsightMd.decorate(rb);
        host.querySelectorAll('.inf-history-report .ins-report-body').forEach(function(el) {
            if (typeof InsightMd !== 'undefined' && InsightMd.decorate) InsightMd.decorate(el);
        });
    }

    function _infoHtml(t) {
        var tmplSel = '<option value=""' + (!t.template ? ' selected' : '') + '>自动</option>'
            + ['survey', 'decision', 'watch'].map(function(x) {
                return '<option value="' + x + '"' + (t.template === x ? ' selected' : '') + '>' + _templateLabel(x) + '</option>';
            }).join('');
        return '<section class="ins-det-card ins-det-info">'
            + '<div class="ins-det-info-top">'
            + '<input id="inf-det-title" class="ins-det-title-input" value="' + _esc(t.title) + '" onchange="InsightFactory.saveTitle(this.value)" placeholder="(无标题)">'
            + '<span class="ins-pill inf-st-' + _esc(t.status) + '">' + _statusLabel(t.status) + '</span>'
            + '</div>'
            + '<div class="ins-det-meta">'
            + '<span class="ins-tag ins-tag-type">' + _typeLabel(t.inputType) + '</span>'
            + '<label class="ins-det-tmpl">模板 <select onchange="InsightFactory.saveTemplate(this.value)">' + tmplSel + '</select></label>'
            + '<button class="ins-link-btn" onclick="InsightFactory.toggleRaw()">' + (_showRaw ? '收起原文 ▲' : '展开原文 ▼') + '</button>'
            + '<button class="ins-link-btn" onclick="InsightFactory.reload()">刷新</button>'
            + '</div>'
            + (_showRaw ? '<div class="ins-det-raw">' + _esc(t.inputContent) + '</div>' : '')
            + (_showRaw && t.sourceSnapshot ? '<div class="ins-det-raw ins-det-snapshot"><div class="ins-det-raw-label">抓取快照</div>' + _esc(t.sourceSnapshot) + '</div>' : '')
            + '</section>';
    }

    function _jobHtml(t) {
        var active = t.activeJob;
        var latestBad = _latestFailedJob();
        var body = '';
        if (active) {
            body = '<div class="inf-job-active">'
                + '<span class="ins-pill inf-job-' + _esc(active.status) + '">' + _jobStatusLabel(active.status) + '</span>'
                + '<span>' + _modeLabel(active.mode) + ' job #' + active.id + '</span>'
                + '<span>provider ' + _esc(active.provider) + '</span>'
                + (active.feedbackNote ? '<div class="inf-job-note">' + _esc(active.feedbackNote) + '</div>' : '')
                + '</div>';
        } else if (t.status === 'idle') {
            body = '<div class="inf-job-empty">尚未创建生成 job。</div>'
                + '<button class="ins-btn ins-btn-primary" onclick="InsightFactory.generate()">生成 v1</button>';
        } else if (t.status === 'failed' || latestBad) {
            var bad = latestBad || {};
            body = '<div class="inf-job-error">'
                + '<div class="inf-job-error-title">最近一次 job 失败</div>'
                + '<div class="inf-job-error-msg">' + _esc(bad.errorMessage || '无错误摘要') + '</div>'
                + (bad.id ? '<button class="ins-btn ins-btn-danger ins-btn-sm" onclick="InsightFactory.retry(' + bad.id + ')">重试</button>' : '')
                + '</div>';
        } else {
            body = '<div class="inf-job-empty">没有 active job。</div>';
        }
        return '<section class="ins-det-card inf-job-card">'
            + '<div class="ins-det-card-title">Worker 状态</div>'
            + body
            + '</section>';
    }

    function _latestReport() {
        return (_reports || []).slice().sort(function(a, b) { return b.version - a.version; })[0] || _detail.latestReport;
    }

    function _latestFailedJob() {
        return (_jobs || []).filter(function(j) {
            return j.status === 'failed' || j.status === 'blocked';
        })[0] || null;
    }

    function _reportHtml(t) {
        var rep = _latestReport();
        var active = t.activeJob;
        var inner = '';
        if (rep && rep.contentMd) {
            var cover = (typeof InsightMd !== 'undefined' && InsightMd.cover)
                ? InsightMd.cover({ template: rep.template, version: rep.version, createdAt: rep.createdAt, modelUsed: rep.modelUsed }, false)
                : '';
            inner = cover + '<div class="ins-report-body">' + (typeof InsightMd !== 'undefined' ? InsightMd.render(rep.contentMd) : _esc(rep.contentMd)) + '</div>';
        } else if (active) {
            inner = '<div class="ins-det-pending"><span class="ins-spin">⏳</span> 等待 worker 写回报告...</div>';
        } else {
            inner = '<div class="ins-det-pending">暂无报告。可创建生成 job，或等待 worker 处理。</div>';
        }
        return '<section class="ins-det-card ins-det-report">'
            + '<div class="ins-det-card-title">最新报告 <button class="ins-link-btn" onclick="InsightFactory.reload()">↻ 刷新</button></div>'
            + inner
            + '</section>';
    }

    function _feedbackHtml(t) {
        var rep = _latestReport();
        var active = t.activeJob;
        if (!rep || active) return '';
        return '<section class="ins-det-card ins-det-feedback">'
            + '<div class="ins-det-card-title">写反馈生成下一版</div>'
            + '<textarea id="inf-fb-text" class="ins-fb-textarea" rows="3" placeholder="写下要改的方向、删补重点或新的约束。"></textarea>'
            + '<div class="ins-fb-foot"><button class="ins-btn ins-btn-primary" onclick="InsightFactory.submitFeedback()">提交修订</button></div>'
            + '</section>';
    }

    function _historyHtml() {
        var reps = (_reports || []).slice().sort(function(a, b) { return b.version - a.version; });
        if (!reps.length) return '';
        return '<section class="ins-det-card ins-det-history">'
            + '<div class="ins-det-card-title">版本历史</div>'
            + '<div class="inf-history-list">' + reps.map(_historyItemHtml).join('') + '</div>'
            + '</section>';
    }

    function _historyItemHtml(r) {
        var open = !!_expandedReports[r.id];
        var note = r.revisionNote ? '<div class="ins-hist-note">' + _esc(r.revisionNote) + '</div>' : '<div class="ins-hist-note ins-hist-first">首次生成</div>';
        var body = open
            ? '<div class="inf-history-report"><div class="ins-report-body">' + (typeof InsightMd !== 'undefined' ? InsightMd.render(r.contentMd) : _esc(r.contentMd)) + '</div></div>'
            : '';
        return '<div class="ins-hist-item">'
            + '<div class="inf-history-head" onclick="InsightFactory.toggleReport(' + r.id + ')">'
            + '<span class="ins-hist-ver">v' + r.version + ' · ' + _shortTime(r.createdAt) + '</span>'
            + '<span class="ins-hist-caret">' + (open ? '▲' : '▼') + '</span>'
            + '</div>'
            + note
            + body
            + '</div>';
    }

    function toggleReport(id) {
        _expandedReports[id] = !_expandedReports[id];
        _renderDetail();
    }

    function toggleRaw() {
        _showRaw = !_showRaw;
        _renderDetail();
    }

    function reload() {
        _loadDetail();
    }

    async function saveTitle(v) {
        var title = (v || '').trim();
        if (!_detail || title === _detail.title) return;
        try {
            var resp = await API.factoryTaskUpdate(_detailId, { title: title });
            if (resp && resp.success) _detail = resp.item;
            await refreshList();
        } catch (e) {
            console.error('[InsightFactory] saveTitle', e);
            if (typeof showToast === 'function') showToast('保存标题失败', 'error');
        }
    }

    async function saveTemplate(v) {
        if (!_detail || !v || v === _detail.template) return;
        try {
            var resp = await API.factoryTaskUpdate(_detailId, { template: v });
            if (resp && resp.success) {
                _detail = resp.item;
                _renderDetail();
            }
        } catch (e) {
            console.error('[InsightFactory] saveTemplate', e);
            if (typeof showToast === 'function') showToast('保存模板失败', 'error');
        }
    }

    async function generate() {
        if (!_detailId) return;
        try {
            var resp = await API.factoryTaskGenerate(_detailId, { provider: 'codex' });
            if (resp && resp.success) {
                if (typeof showToast === 'function') showToast('已创建生成 job', 'success');
                await _loadDetail();
            }
        } catch (e) {
            console.error('[InsightFactory] generate', e);
            if (typeof showToast === 'function') showToast('创建生成 job 失败', 'error');
        }
    }

    async function submitFeedback() {
        var ta = document.getElementById('inf-fb-text');
        if (!ta) return;
        var note = (ta.value || '').trim();
        if (!note) {
            if (typeof showToast === 'function') showToast('反馈不能为空', 'warning');
            ta.focus();
            return;
        }
        try {
            var resp = await API.factoryTaskFeedback(_detailId, { feedbackNote: note, provider: 'codex' });
            if (resp && resp.success) {
                if (typeof showToast === 'function') showToast('已创建修订 job', 'success');
                await _loadDetail();
            }
        } catch (e) {
            console.error('[InsightFactory] feedback', e);
            if (typeof showToast === 'function') showToast('提交反馈失败', 'error');
        }
    }

    async function retry(id) {
        try {
            var resp = await API.factoryJobRetry(id);
            if (resp && resp.success) {
                if (typeof showToast === 'function') showToast('已创建重试 job', 'success');
                await _loadDetail();
            }
        } catch (e) {
            console.error('[InsightFactory] retry', e);
            if (typeof showToast === 'function') showToast('重试失败', 'error');
        }
    }

    function _openFromPath(replace) {
        var path = window.location.pathname || '';
        if (path === '/insight-factory' || path === '/insight-factory/') {
            if (typeof switchPage === 'function' && window.currentPage !== 'work') switchPage('work');
            openHub({ noHistory: true, replace: replace });
            return true;
        }
        var m = path.match(/^\/insight-factory\/(\d+)\/?$/);
        if (m) {
            if (typeof switchPage === 'function' && window.currentPage !== 'work') switchPage('work');
            openDetail(Number(m[1]), { noHistory: true, replace: replace });
            return true;
        }
        return false;
    }

    function initRoute() {
        _openFromPath(true);
        window.addEventListener('popstate', function() {
            if (!_openFromPath(true) && typeof Work !== 'undefined' && window.currentPage === 'work') {
                Work.showHub();
            }
        });
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', initRoute);
    } else {
        setTimeout(initRoute, 0);
    }

    return {
        openHub: openHub,
        backToHub: backToHub,
        refreshList: refreshList,
        refreshHealth: refreshHealth,
        refreshMemories: refreshMemories,
        setFilter: setFilter,
        setMemoryFilter: setMemoryFilter,
        editMemory: editMemory,
        cancelMemoryEdit: cancelMemoryEdit,
        saveMemory: saveMemory,
        toggleMemory: toggleMemory,
        deleteMemory: deleteMemory,
        onCaptureInput: onCaptureInput,
        onTypeManual: onTypeManual,
        submitNew: submitNew,
        confirmDelete: confirmDelete,
        openDetail: openDetail,
        toggleRaw: toggleRaw,
        reload: reload,
        saveTitle: saveTitle,
        saveTemplate: saveTemplate,
        generate: generate,
        submitFeedback: submitFeedback,
        retry: retry,
        toggleReport: toggleReport,
        detectInputType: detectInputType
    };
})();
