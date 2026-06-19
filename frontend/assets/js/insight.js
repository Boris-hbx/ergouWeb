// ========== Insight v0.3 — 洞察模块 (T-123) ==========
//
// v0.3 大重构(spec v0.3):废弃 v0.2 的 source 候选池 / annotation 锚点 / publish 分享,
// 简化为「单层 insight_task + 单 textarea 反馈」。
//
// 工作 Hub「📍 洞察」卡 → Insight.openHub()。
// 模块内两层视图(div 显隐,非 URL 路由):
//   列表页  `insight-list-view`   —— 顶部录入区(textarea + input_type 自动识别)+ 任务列表
//   详情页  `insight-detail-view` —— 三块:任务信息 / 最新报告(markdown)/ 反馈框
//
// Hybrid:Web 只录入 + 看 + 写反馈;生成/修订由 Claude Code 跑 `/insight :id`。
// 报告 markdown 复用 InsightMd.render()。

var Insight = (function() {
    var _tasks = [];
    var _statusFilter = '';
    var _detailId = null;
    var _detail = null;       // 当前详情 task(含内嵌 report)
    var _typeManual = false;  // 用户是否手动改过录入类型下拉
    var _showRaw = false;     // 详情页原文折叠状态
    var _shares = [];         // T-126:当前详情的分享列表(active+revoked)
    var _history = [];        // T-127:当前详情的报告版本列表(version DESC)
    var _showHistory = false; // T-127:修订历史折叠状态

    function _esc(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }

    // ---- 标签 / 文案 ----
    function _statusLabel(s) {
        return ({ ready: '待处理', processing: '生成中', done: '已完成' })[s] || s;
    }
    function _typeLabel(t) {
        return ({ url: '链接', topic: '主题', prompt: '指令', note: '随想' })[t] || t;
    }
    function _templateLabel(t) {
        return ({ survey: '综述型', decision: '决策型', watch: '追踪型' })[t] || (t || '自动');
    }
    function _shortTime(iso) {
        if (!iso) return '';
        try {
            var d = new Date(iso);
            var diffH = (Date.now() - d.getTime()) / 36e5;
            if (diffH < 1)  return Math.max(1, Math.round(diffH * 60)) + ' 分钟前';
            if (diffH < 24) return Math.round(diffH) + ' 小时前';
            if (diffH < 24 * 7) return Math.round(diffH / 24) + ' 天前';
            return d.getFullYear() + '-' + String(d.getMonth()+1).padStart(2,'0') + '-' + String(d.getDate()).padStart(2,'0');
        } catch (_) { return iso; }
    }

    // 前端自动识别 input_type(spec § 五,与后端 detect_input_type 一致,作录入实时提示)
    function detectInputType(text) {
        var t = (text || '').trim();
        var lower = t.toLowerCase();
        if ((lower.indexOf('http://') >= 0 || lower.indexOf('https://') >= 0)
            && t.split(/\s+/).length === 1) {
            return 'url';
        }
        var n = Array.from(t).length;
        if (n <= 80) return 'topic';
        var markers = ['帮我','请','分析','总结','对比','写一份','写一篇','整理','给我','梳理','评估'];
        for (var i = 0; i < markers.length; i++) {
            if (t.indexOf(markers[i]) >= 0) return 'prompt';
        }
        return 'note';
    }

    // ============ Hub 切换 ============
    function openHub() {
        var hub = document.getElementById('work-hub');
        var tableView = document.getElementById('work-table-view');
        var insView = document.getElementById('work-insight-view');
        if (hub) hub.style.display = 'none';
        if (tableView) tableView.style.display = 'none';
        if (insView) insView.style.display = '';
        _showDetail(false);
        _renderList();
        refreshList();
    }

    function backToHub() {
        // 详情开着 → 回列表
        var detEl = document.getElementById('insight-detail-view');
        if (detEl && !detEl.classList.contains('ins-hidden')) {
            _detailId = null;
            _detail = null;
            _showDetail(false);
            refreshList();
            return;
        }
        // 列表 → 工作 Hub
        var insView = document.getElementById('work-insight-view');
        var hub = document.getElementById('work-hub');
        if (insView) insView.style.display = 'none';
        if (hub) hub.style.display = '';
    }

    function _showDetail(on) {
        var listEl = document.getElementById('insight-list-view');
        var detEl  = document.getElementById('insight-detail-view');
        if (listEl) listEl.classList.toggle('ins-hidden', !!on);
        if (detEl)  detEl.classList.toggle('ins-hidden', !on);
    }

    // ============ 列表页 ============
    function _renderList() {
        var host = document.getElementById('insight-list-view');
        if (!host) return;
        host.innerHTML =
            '<div class="ins-list-main">'
          +   '<div class="ins-list-head">'
          +     '<h2>📍 候选洞察任务资源池</h2>'
          +     '<span class="ins-count-pill">' + _tasks.length + ' 个</span>'
          +     '<div class="ins-list-spacer"></div>'
          +     _statusFilterHtml()
          +   '</div>'
          +   _captureHtml()
          +   '<div class="ins-list-table">' + _tableHtml() + '</div>'
          + '</div>';
        var ta = document.getElementById('ins-cap-text');
        if (ta) _syncDetectedType();
    }

    function _statusFilterHtml() {
        var opts = [['','全部'], ['ready','待处理'], ['processing','生成中'], ['done','已完成']];
        return '<select class="ins-filter" onchange="Insight.setFilter(this.value)">'
          + opts.map(function(o) {
              return '<option value="' + o[0] + '"' + (_statusFilter === o[0] ? ' selected' : '') + '>' + o[1] + '</option>';
            }).join('')
          + '</select>';
    }

    // 顶部录入区:textarea + 自动识别类型下拉(可手动改)+ 创建
    function _captureHtml() {
        var sel = ['url','topic','prompt','note'].map(function(t) {
            return '<option value="' + t + '">' + _typeLabel(t) + '</option>';
        }).join('');
        return '<div class="ins-capture">'
          + '<textarea id="ins-cap-text" class="ins-cap-textarea" rows="3" '
          +   'placeholder="贴个链接 / 写个主题 / 扔个 prompt / 记个随想…（Ctrl+Enter 创建）" '
          +   'oninput="Insight.onCaptureInput()" '
          +   'onkeydown="if((event.ctrlKey||event.metaKey)&&event.key===\'Enter\')Insight.submitNew()"></textarea>'
          + '<div class="ins-cap-foot">'
          +   '<label class="ins-cap-type-label">类型'
          +     '<select id="ins-cap-type" class="ins-cap-type" onchange="Insight.onTypeManual()">' + sel + '</select>'
          +   '</label>'
          +   '<span class="ins-cap-type-hint" id="ins-cap-type-hint"></span>'
          +   '<div class="ins-cap-spacer"></div>'
          +   '<button class="eg-btn eg-btn--primary" onclick="Insight.submitNew()">创建</button>'
          + '</div>'
          + '</div>';
    }

    function onCaptureInput() {
        if (!_typeManual) _syncDetectedType();
    }
    function onTypeManual() {
        _typeManual = true;
        var hint = document.getElementById('ins-cap-type-hint');
        if (hint) hint.textContent = '已手动指定';
    }
    function _syncDetectedType() {
        var ta = document.getElementById('ins-cap-text');
        var sel = document.getElementById('ins-cap-type');
        var hint = document.getElementById('ins-cap-type-hint');
        if (!ta || !sel) return;
        var txt = ta.value || '';
        var t = detectInputType(txt);
        sel.value = t;
        if (hint) hint.textContent = txt.trim() ? '自动识别为「' + _typeLabel(t) + '」' : '';
    }

    function _tableHtml() {
        if (_tasks.length === 0) {
            return '<div class="ins-list-empty">'
              + '<div class="ins-list-empty-icon">📍</div>'
              + '<div class="ins-list-empty-title">还没有洞察</div>'
              + '<div class="ins-list-empty-sub">在上面录入框贴链接、写主题、扔 prompt 或记随想,创建后让 Claude Code 跑 <code>/insight</code> 生成报告。</div>'
              + '</div>';
        }
        return '<table class="ins-table">'
          + '<thead><tr><th class="ins-th-id">ID</th><th>标题</th><th>类型</th><th>状态</th><th>版本</th><th>更新</th><th></th></tr></thead>'
          + '<tbody>' + _tasks.map(_rowHtml).join('') + '</tbody>'
          + '</table>';
    }

    function _rowHtml(t) {
        var ver = t.latestVersion ? 'v' + t.latestVersion : '—';
        return '<tr class="ins-row" onclick="Insight.openDetail(' + t.id + ')">'
          + '<td class="ins-row-id">' + t.id + '</td>'
          + '<td class="ins-row-title">' + _esc(t.title || '(无标题)') + '</td>'
          + '<td><span class="ins-tag ins-tag-type">' + _typeLabel(t.inputType) + '</span></td>'
          + '<td><span class="ins-pill ins-st-' + t.status + '">' + _statusLabel(t.status) + '</span></td>'
          + '<td class="ins-row-ver">' + ver + '</td>'
          + '<td>' + _shortTime(t.latestReportAt || t.updatedAt) + '</td>'
          + '<td class="ins-row-actions" onclick="event.stopPropagation()">'
          +   '<button class="ins-icon-btn ins-icon-btn-x" onclick="Insight.confirmDelete(' + t.id + ',\'' + _esc(t.title).replace(/'/g, "\\'") + '\')" title="删除">✕</button>'
          + '</td>'
          + '</tr>';
    }

    async function refreshList() {
        try {
            var params = _statusFilter ? { status: _statusFilter } : {};
            var resp = await API.insightTaskList(params);
            _tasks = (resp && resp.items) || [];
            _renderList();
        } catch (e) {
            console.error('[Insight] refreshList', e);
            if (typeof showToast === 'function') showToast('加载洞察列表失败', 'error');
        }
    }

    function setFilter(v) { _statusFilter = v || ''; refreshList(); }

    async function submitNew() {
        var ta = document.getElementById('ins-cap-text');
        var sel = document.getElementById('ins-cap-type');
        if (!ta) return;
        var content = (ta.value || '').trim();
        if (!content) {
            if (typeof showToast === 'function') showToast('录入内容不能为空', 'warning');
            ta.focus();
            return;
        }
        var data = { inputContent: content };
        if (sel && sel.value) data.inputType = sel.value;
        try {
            var resp = await API.insightTaskCreate(data);
            if (resp && resp.success) {
                _typeManual = false;
                if (typeof showToast === 'function') showToast('已创建,等 Claude Code 处理', 'success');
                openDetail(resp.item.id);   // 创建后跳详情
            }
        } catch (e) {
            console.error('[Insight] create', e);
            if (typeof showToast === 'function') showToast('创建失败', 'error');
        }
    }

    function confirmDelete(id, title) {
        var msg = '删除「' + _esc(title) + '」?<br>'
            + '<small style="color:#6B7280">软删除(数据库保留 30 天),报告一并失效。</small>';
        if (window.AppUtils && AppUtils.showConfirm) {
            AppUtils.showConfirm(msg, function() { _doDelete(id); }, { confirmText: '删除', danger: true });
        } else if (confirm('删除「' + title + '」?')) {
            _doDelete(id);
        }
    }
    async function _doDelete(id) {
        try {
            await API.insightTaskDelete(id);
            if (typeof showToast === 'function') showToast('已删除', 'info');
            if (_detailId === id) { _detailId = null; _detail = null; _showDetail(false); }
            await refreshList();
        } catch (e) {
            console.error('[Insight] delete', e);
            if (typeof showToast === 'function') showToast('删除失败', 'error');
        }
    }

    // ============ 详情页 ============
    async function openDetail(id) {
        _detailId = id;
        _showRaw = false;
        _showDetail(true);
        var host = document.getElementById('insight-detail-view');
        if (host) host.innerHTML = '<div class="ins-det-loading">加载中…</div>';
        await _loadDetail();
    }

    async function _loadDetail() {
        if (_detailId == null) return;
        try {
            var resp = await API.insightTaskGet(_detailId);
            if (resp && resp.success) {
                _detail = resp.item;
                // T-126/T-127:并行拉分享状态 + 报告历史(失败各自隔离,不挡主渲染)
                _shares = []; _history = [];
                try { var sr = await API.insightTaskShares(_detailId);  if (sr && sr.success) _shares = sr.items || []; }
                catch (e) { console.error('[Insight] shares', e); }
                try { var rr = await API.insightTaskReports(_detailId); if (rr && rr.success) _history = rr.items || []; }
                catch (e) { console.error('[Insight] reports', e); }
                _renderDetail();
            }
        } catch (e) {
            console.error('[Insight] loadDetail', e);
            if (typeof showToast === 'function') showToast('加载详情失败', 'error');
        }
    }

    function _renderDetail() {
        var host = document.getElementById('insight-detail-view');
        if (!host || !_detail) return;
        var t = _detail;
        host.innerHTML =
            '<div class="ins-det">'
          +   _detailInfoHtml(t)
          +   _detailReportHtml(t)
          +   _detailHistoryHtml(t)
          +   _detailFeedbackHtml(t)
          + '</div>';
        // T-129:渲染后把「思辨」节包成 callout
        var rb = host.querySelector('.ins-report-body');
        if (rb && typeof InsightMd !== 'undefined' && InsightMd.decorate) InsightMd.decorate(rb);
    }

    // 块 1:任务信息
    function _detailInfoHtml(t) {
        var tmplSel = '<option value=""' + (!t.template ? ' selected' : '') + '>自动(LLM 选)</option>'
          + ['survey','decision','watch'].map(function(x) {
              return '<option value="' + x + '"' + (t.template === x ? ' selected' : '') + '>' + _templateLabel(x) + '</option>';
            }).join('');
        var raw = _showRaw
            ? '<div class="ins-det-raw">' + _esc(t.inputContent) + '</div>'
            : '';
        var snapshot = (t.sourceSnapshot && _showRaw)
            ? '<div class="ins-det-raw ins-det-snapshot"><div class="ins-det-raw-label">抓取快照</div>' + _esc(t.sourceSnapshot) + '</div>'
            : '';
        return '<section class="ins-det-card ins-det-info">'
          + '<div class="ins-det-info-top">'
          +   '<input id="ins-det-title" class="ins-det-title-input" value="' + _esc(t.title) + '" '
          +     'onchange="Insight.saveTitle(this.value)" placeholder="(无标题)">'
          +   '<span class="ins-pill ins-st-' + t.status + '">' + _statusLabel(t.status) + '</span>'
          + '</div>'
          + '<div class="ins-det-meta">'
          +   '<span class="ins-tag ins-tag-type">' + _typeLabel(t.inputType) + '</span>'
          +   '<label class="ins-det-tmpl">模板 '
          +     '<select onchange="Insight.saveTemplate(this.value)">' + tmplSel + '</select>'
          +   '</label>'
          +   '<button class="ins-link-btn" onclick="Insight.toggleRaw()">' + (_showRaw ? '收起原文 ▲' : '展开原文 ▼') + '</button>'
          + '</div>'
          + raw
          + snapshot
          + '</section>';
    }

    // 块 2:最新报告
    function _detailReportHtml(t) {
        var rep = t.report;
        var inner;
        if (t.status === 'processing') {
            inner = '<div class="ins-det-pending">'
              + '<span class="ins-spin">⏳</span> 正在生成…'
              + '<button class="ins-btn ins-btn-ghost ins-btn-sm" onclick="Insight.abort()">中止</button>'
              + '</div>';
        } else if (rep && rep.contentMd) {
            // 有报告就渲染(done;或 ready 修订中仍可看旧版)
            var banner = (t.status === 'ready')
                ? '<div class="ins-det-rep-banner">📝 已提交反馈,等待 Claude Code 出新版(下方为上一版 v' + rep.version + ')</div>'
                : '';
            var cover = (typeof InsightMd !== 'undefined' && InsightMd.cover)
                ? InsightMd.cover({ template: rep.template, version: rep.version, createdAt: rep.createdAt, modelUsed: rep.modelUsed }, false)
                : '';
            inner = banner
              + cover
              + '<div class="ins-report-body">' + (typeof InsightMd !== 'undefined' ? InsightMd.render(rep.contentMd) : _esc(rep.contentMd)) + '</div>';
        } else {
            inner = '<div class="ins-det-pending">'
              + '⏳ 等待 Claude Code 处理 —— 在 CC 端跑 <code>/insight ' + t.id + '</code>'
              + '</div>';
        }
        return '<section class="ins-det-card ins-det-report">'
          + '<div class="ins-det-card-title">最新报告 '
          +   '<button class="ins-link-btn" onclick="Insight.reload()" title="刷新状态">↻ 刷新</button>'
          + '</div>'
          + inner
          + ((rep && rep.contentMd) ? _detailShareHtml() : '')
          + '</section>';
    }

    // 块 2.5:分享(T-126)—— 有报告时显示;active 则显链接+撤销,否则显分享按钮+含思辨勾选
    function _detailShareHtml() {
        var active = (_shares || []).filter(function(s) { return !s.revokedAt; })[0];
        if (active) {
            var url = _shareUrl(active.token);
            return '<div class="ins-share ins-share-active">'
              + '<span class="ins-share-label">🔗 已分享' + (active.includeNotes ? '' : '(干净版)') + '</span>'
              + '<input class="ins-share-link" readonly value="' + _esc(url) + '" onclick="this.select()">'
              + '<button class="ins-btn ins-btn-sm" onclick="Insight.copyShareLink(\'' + active.token + '\')">复制</button>'
              + '<button class="ins-btn ins-btn-ghost ins-btn-sm" onclick="Insight.retractShare()">撤销</button>'
              + '</div>';
        }
        return '<div class="ins-share">'
          + '<button class="ins-btn ins-btn-sm" onclick="Insight.publishShare()">🔗 分享</button>'
          + '<label class="ins-share-notes"><input type="checkbox" id="ins-share-notes" checked> 含思辨</label>'
          + '</div>';
    }

    // 块 4:修订/反馈历史(T-127)—— 复用已拉的 _history(version DESC);仅 >1 版时显示折叠区
    function _detailHistoryHtml() {
        var reps = (_history || []).slice().sort(function(a, b) { return b.version - a.version; });
        if (reps.length <= 1) return '';
        var head = '<div class="ins-det-card-title ins-hist-head" onclick="Insight.toggleHistory()">'
          + '修订历史(' + reps.length + ' 版)<span class="ins-hist-caret">' + (_showHistory ? '▲' : '▼') + '</span></div>';
        var body = '';
        if (_showHistory) {
            body = '<div class="ins-hist-list">' + reps.map(function(r) {
                var note = (r.revisionNote && r.revisionNote.trim())
                    ? '<div class="ins-hist-note">' + _esc(r.revisionNote) + '</div>'
                    : '<div class="ins-hist-note ins-hist-first">首次生成</div>';
                return '<div class="ins-hist-item"><div class="ins-hist-ver">v' + r.version + ' · ' + _shortTime(r.createdAt) + '</div>' + note + '</div>';
            }).join('') + '</div>';
        }
        return '<section class="ins-det-card ins-det-history">' + head + body + '</section>';
    }

    function _shareUrl(token) { return (window.location.origin || '') + '/r/' + token; }

    // 块 3:反馈框
    function _detailFeedbackHtml(t) {
        if (t.status === 'done') {
            return '<section class="ins-det-card ins-det-feedback">'
              + '<div class="ins-det-card-title">写反馈让 CC 改一版</div>'
              + '<textarea id="ins-fb-text" class="ins-fb-textarea" rows="3" '
              +   'placeholder="想让报告怎么改?写一句…提交后状态回「待处理」,下次跑 /insight 时按反馈修订">' + _esc(t.feedbackNote || '') + '</textarea>'
              + '<div class="ins-fb-foot">'
              +   '<button class="eg-btn eg-btn--primary" onclick="Insight.submitFeedback()">提交反馈</button>'
              + '</div>'
              + '</section>';
        }
        if (t.status === 'ready' && t.feedbackNote) {
            return '<section class="ins-det-card ins-det-feedback ins-det-feedback-readonly">'
              + '<div class="ins-det-card-title">待修订反馈</div>'
              + '<div class="ins-fb-readonly">' + _esc(t.feedbackNote) + '</div>'
              + '</section>';
        }
        return '';   // ready 无反馈 / processing 不显示反馈框
    }

    // ---- 详情操作 ----
    function toggleRaw() { _showRaw = !_showRaw; _renderDetail(); }
    function reload() { _loadDetail(); }

    async function saveTitle(v) {
        var title = (v || '').trim();
        if (!_detail || title === _detail.title) return;
        try {
            var resp = await API.insightTaskUpdate(_detailId, { title: title });
            if (resp && resp.success) { _detail = resp.item; }
        } catch (e) {
            console.error('[Insight] saveTitle', e);
            if (typeof showToast === 'function') showToast('保存标题失败', 'error');
        }
    }

    async function saveTemplate(v) {
        // 空 = "自动",后端校验只收 survey/decision/watch;留空不回写(保持 LLM 自选)
        if (!_detail || !v || v === _detail.template) return;
        try {
            var resp = await API.insightTaskUpdate(_detailId, { template: v });
            if (resp && resp.success) { _detail = resp.item; _renderDetail(); }
        } catch (e) {
            console.error('[Insight] saveTemplate', e);
            if (typeof showToast === 'function') showToast('保存模板失败', 'error');
        }
    }

    async function abort() {
        try {
            var resp = await API.insightTaskRelease(_detailId);
            if (resp && resp.success) {
                _detail = resp.item;
                _renderDetail();
                if (typeof showToast === 'function') showToast('已中止', 'info');
            }
        } catch (e) {
            console.error('[Insight] abort', e);
            if (typeof showToast === 'function') showToast('中止失败', 'error');
        }
    }

    async function submitFeedback() {
        var ta = document.getElementById('ins-fb-text');
        if (!ta) return;
        var note = (ta.value || '').trim();
        if (!note) {
            if (typeof showToast === 'function') showToast('反馈不能为空', 'warning');
            ta.focus();
            return;
        }
        try {
            var resp = await API.insightTaskUpdate(_detailId, { feedbackNote: note });
            if (resp && resp.success) {
                _detail = resp.item;
                _renderDetail();
                if (typeof showToast === 'function') showToast('反馈已提交,等 CC 修订', 'success');
            }
        } catch (e) {
            console.error('[Insight] submitFeedback', e);
            if (typeof showToast === 'function') showToast('提交反馈失败', 'error');
        }
    }

    // ---- 分享 / 历史操作(T-126 / T-127)----
    async function publishShare() {
        var cb = document.getElementById('ins-share-notes');
        var includeNotes = cb ? cb.checked : true;
        try {
            var resp = await API.insightTaskPublish(_detailId, { includeNotes: includeNotes });
            if (resp && resp.success) {
                await _loadDetail();   // 重新拉分享状态并重渲
                if (typeof showToast === 'function') showToast('已生成分享链接', 'success');
            }
        } catch (e) {
            console.error('[Insight] publishShare', e);
            if (typeof showToast === 'function') showToast((e && e.message) || '分享失败', 'error');
        }
    }

    async function retractShare() {
        try {
            var resp = await API.insightTaskRetract(_detailId);
            if (resp && resp.success) {
                await _loadDetail();
                if (typeof showToast === 'function') showToast('已撤销分享', 'info');
            }
        } catch (e) {
            console.error('[Insight] retractShare', e);
            if (typeof showToast === 'function') showToast((e && e.message) || '撤销失败', 'error');
        }
    }

    function copyShareLink(token) {
        var url = _shareUrl(token);
        if (navigator.clipboard && navigator.clipboard.writeText) {
            navigator.clipboard.writeText(url).then(
                function() { if (typeof showToast === 'function') showToast('链接已复制', 'success'); },
                function() { if (typeof showToast === 'function') showToast('复制失败,请手动选中复制', 'warning'); }
            );
        } else if (typeof showToast === 'function') {
            showToast('请手动选中链接复制', 'info');
        }
    }

    function toggleHistory() { _showHistory = !_showHistory; _renderDetail(); }

    return {
        openHub: openHub,
        backToHub: backToHub,
        refreshList: refreshList,
        setFilter: setFilter,
        onCaptureInput: onCaptureInput,
        onTypeManual: onTypeManual,
        submitNew: submitNew,
        openDetail: openDetail,
        confirmDelete: confirmDelete,
        toggleRaw: toggleRaw,
        reload: reload,
        saveTitle: saveTitle,
        saveTemplate: saveTemplate,
        abort: abort,
        submitFeedback: submitFeedback,
        publishShare: publishShare,
        retractShare: retractShare,
        copyShareLink: copyShareLink,
        toggleHistory: toggleHistory,
        detectInputType: detectInputType,
    };
})();
