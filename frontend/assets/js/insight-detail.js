// ========== InsightDetail — 洞察详情页主控 (T-106 P1) ==========
//
// 按 insight.status 切 4 形态:
//   collecting   → source 编辑栏 + 模板下拉 + 「准备好生成」按钮
//   ready / processing → 等待 Claude Code 处理的横幅 + 取消按钮
//   editing      → 当前 report 渲染(只读) + 「发布」按钮 + sources 引用列表
//   published    → 醒目横幅(链接 + 复制 + 打开 + 撤回) + 只读报告
//
// P1 不做(留 P2 / P3):
//   - annotation gutter(点段落加备注)
//   - 让 CC 改一版 dialog(insight-revise.js)
//   - 历史版本下拉
//   - inline markdown 编辑(已发布版本自动 fork)
//   - citation popover([^N] 原文片段)
//
// 数据流:
//   open(id) → API.insightGet(id, include=sources) → 若有 currentReportId,
//   再 GET reports/{version};若 published,再 GET share。

var InsightDetail = (function() {
    var _id = null;
    var _state = {
        insight: null,    // { id, title, topic, template, status, currentReportId, ... }
        sources: [],      // [{ id, kind, url, title, author, content, fetchStatus, ... }]
        report: null,     // 当前展示的 report(可能是 current 也可能是历史版本)
        allReports: [],   // P2:所有版本(版本下拉用)
        viewingVersion: null,  // P2:null = 看 currentReport;非空 = 看历史版本(只读)
        share: null,      // 当前 active share { token, url, ... } (status=published 时拉)
        editingMd: false, // P2:inline markdown 编辑模式开关
    };

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

    function _statusClass(s) {
        return 'ins-st-' + s;
    }

    function _kindIcon(k) {
        switch (k) {
            case 'youtube': return '🎬';
            case 'x':       return '🐦';
            case 'github':  return '🐙';
            case 'pdf':     return '📄';
            case 'text':    return '📝';
            default:        return '🌐';
        }
    }

    function _templateLabel(t) {
        return ({ survey: '综述型', decision: '决策型', watch: '追踪型' })[t] || t;
    }

    function currentInsightId() { return _id; }

    async function open(id) {
        _id = id;
        _state = { insight: null, sources: [], report: null, allReports: [], viewingVersion: null, share: null, editingMd: false };
        // 切到详情容器
        var listEl = document.getElementById('insight-list-view');
        var detEl  = document.getElementById('insight-detail-view');
        if (listEl) listEl.classList.add('ins-hidden');
        if (detEl)  detEl.classList.remove('ins-hidden');
        _renderLoading();
        await refresh();
    }

    function close() {
        _id = null;
        _state = { insight: null, sources: [], report: null, allReports: [], viewingVersion: null, share: null, editingMd: false };
        var listEl = document.getElementById('insight-list-view');
        var detEl  = document.getElementById('insight-detail-view');
        if (listEl) listEl.classList.remove('ins-hidden');
        if (detEl)  detEl.classList.add('ins-hidden');
        // 通知列表刷新(创建/删除 source 等可能影响列表)
        if (typeof Insight !== 'undefined') Insight.refreshList();
    }

    async function refresh() {
        if (_id == null) return;
        try {
            var resp = await API.insightGet(_id, true);
            if (!resp || !resp.success) {
                _renderError('未找到洞察');
                return;
            }
            _state.insight = resp.item;
            _state.sources = (resp.item && resp.item.sources) || [];
            _state.report = null;
            _state.share = null;
            _state.allReports = [];
            // 拉所有 report(历史版本下拉 + 决定当前展示哪一个)
            if (_state.insight.currentReportId) {
                try {
                    var rs = await API.insightListReports(_id);
                    if (rs && rs.items) _state.allReports = rs.items;
                } catch (_) {}
                // 决定 _state.report:历史模式 → viewingVersion 对应版本;否则 → currentReportId
                if (_state.viewingVersion) {
                    _state.report = _state.allReports.find(function(r) { return r.version === _state.viewingVersion; }) || null;
                }
                if (!_state.report) {
                    _state.report = _state.allReports.find(function(r) { return r.id === _state.insight.currentReportId; })
                                  || (_state.allReports[0] || null);
                }
            }
            // published → 拉当前 active share
            if (_state.insight.status === 'published') {
                try {
                    var sh = await API.insightGetShare(_id);
                    if (sh && sh.items) {
                        _state.share = sh.items.find(function(s) { return !s.revokedAt; }) || null;
                    }
                } catch (_) {}
            }
            _render();
        } catch (e) {
            console.error('[InsightDetail] refresh err', e);
            _renderError('加载失败');
        }
    }

    function _renderLoading() {
        var host = document.getElementById('insight-detail-view');
        if (!host) return;
        host.innerHTML = '<div class="ins-loading">加载中…</div>';
    }
    function _renderError(msg) {
        var host = document.getElementById('insight-detail-view');
        if (!host) return;
        host.innerHTML = '<div class="ins-error">' + _esc(msg) + ' <button class="ins-btn" onclick="InsightDetail.close()">返回列表</button></div>';
    }

    function _render() {
        var host = document.getElementById('insight-detail-view');
        if (!host) return;
        var ins = _state.insight;
        if (!ins) { _renderError('数据为空'); return; }
        // 渲染前 detach 旧的 annotation 绑定
        if (typeof InsightAnnotations !== 'undefined') InsightAnnotations.detach();

        var topBar = ''
          + '<div class="ins-det-topbar">'
          +   '<button class="ins-back" onclick="InsightDetail.close()">← 列表</button>'
          +   '<div class="ins-det-title-block">'
          +     '<input class="ins-det-title" type="text" value="' + _esc(ins.title) + '" '
          +        'onblur="InsightDetail.updateField(\'title\', this.value)" '
          +        'placeholder="洞察标题">'
          +     '<input class="ins-det-topic" type="text" value="' + _esc(ins.topic || '') + '" '
          +        'onblur="InsightDetail.updateField(\'topic\', this.value)" '
          +        'placeholder="一句话描述这个研究主题(可选)">'
          +   '</div>'
          +   '<span class="ins-pill ' + _statusClass(ins.status) + '">' + _statusLabel(ins.status) + '</span>'
          + '</div>';

        var body = '';
        switch (ins.status) {
            case 'collecting': body = _renderCollecting(); break;
            case 'ready':      body = _renderWaiting('ready');      break;
            case 'processing': body = _renderWaiting('processing'); break;
            case 'editing':    body = _renderEditing();   break;
            case 'published':  body = _renderPublished(); break;
            case 'archived':   body = _renderArchived();  break;
            default:           body = '<div class="ins-error">未知状态:' + _esc(ins.status) + '</div>';
        }

        host.innerHTML = topBar + '<div class="ins-det-body">' + body + '</div>';

        // P2.3:editing 状态(看当前 currentReport、非历史、非 inline 编辑模式)
        //       挂 annotation 系统
        var r = _state.report;
        var isCurrent = ins && r && r.id === ins.currentReportId;
        if (ins.status === 'editing' && isCurrent && !_state.viewingVersion && !_state.editingMd
            && typeof InsightAnnotations !== 'undefined') {
            var annMdEl = host.querySelector('.ins-report-md');
            if (annMdEl) InsightAnnotations.attach(_id, r.id, annMdEl);
        }

        // P3:无论 editing/published/历史版本,只要展示了 markdown 报告就挂 citation popover
        //    inline 编辑模式下 .ins-report-md 不存在,自然跳过
        if (r && typeof InsightCitation !== 'undefined') {
            var citeMdEl = host.querySelector('.ins-report-md');
            if (citeMdEl) {
                InsightCitation.attach(citeMdEl, r.citations || [], _state.sources || []);
            }
        }
    }

    // ---------- collecting ----------
    function _renderCollecting() {
        var ins = _state.insight;
        var canGenerate = _state.sources.length > 0;
        return ''
          + '<div class="ins-det-cols">'
          +   '<div class="ins-det-col-left">'
          +     _renderSourceList(true /* editable */)
          +   '</div>'
          +   '<div class="ins-det-col-right">'
          +     '<div class="ins-placeholder">'
          +       '<div class="ins-placeholder-icon">📝</div>'
          +       '<div class="ins-placeholder-title">还没生成报告</div>'
          +       '<div class="ins-placeholder-sub">整理好素材后选择模板,点下面按钮让 Claude Code 跑一次</div>'
          +     '</div>'
          +     '<div class="ins-action-panel">'
          +       '<label class="ins-action-label">模板:'
          +         '<select onchange="InsightDetail.updateField(\'template\', this.value)">'
          +           ['survey', 'decision', 'watch'].map(function(t) {
                        return '<option value="' + t + '"' + (ins.template === t ? ' selected' : '') + '>'
                            + _templateLabel(t) + '</option>';
                      }).join('')
          +         '</select>'
          +       '</label>'
          +       '<button class="ins-btn ins-btn-primary" '
          +              (canGenerate ? '' : 'disabled')
          +              ' onclick="InsightDetail.markReady()">↓ 准备好生成 ↓</button>'
          +       (canGenerate
              ? '<div class="ins-action-hint">点击后状态切到「等待生成」,在 Claude Code 里说: 处理 Insight #' + ins.id + '</div>'
              : '<div class="ins-action-hint ins-warn">至少加 1 个 source 才能生成</div>')
          +     '</div>'
          +   '</div>'
          + '</div>';
    }

    // ---------- ready / processing ----------
    function _renderWaiting(status) {
        var ins = _state.insight;
        var isReady = status === 'ready';
        return ''
          + '<div class="ins-waiting-banner ' + (isReady ? 'ready' : 'processing') + '">'
          +   '<div class="ins-waiting-title">' + (isReady ? '⏳ 等待 Claude Code 处理' : '🌀 Claude Code 处理中...') + '</div>'
          +   '<div class="ins-waiting-hint">在 Claude Code 里说: <code>处理 Insight #' + ins.id + '</code></div>'
          +   (isReady
              ? '<button class="ins-btn ins-btn-ghost" onclick="InsightDetail.cancelReady()">取消(回到 ' + (ins.currentReportId ? 'editing' : 'collecting') + ')</button>'
              : '')
          + '</div>'
          + '<div class="ins-det-cols">'
          +   '<div class="ins-det-col-left">' + _renderSourceList(false) + '</div>'
          +   '<div class="ins-det-col-right">' + (_state.report ? _renderReportRO() : '') + '</div>'
          + '</div>';
    }

    // ---------- editing ----------
    function _renderEditing() {
        var r = _state.report;
        if (!r) {
            // 罕见:state=editing 但 report 没拉到,容错
            return '<div class="ins-error">无法加载当前报告版本</div>';
        }
        var ins = _state.insight;
        var viewingHistory = _state.viewingVersion && r.id !== ins.currentReportId;
        var actionsHtml;
        if (viewingHistory) {
            actionsHtml = '<button class="ins-btn ins-btn-primary" onclick="InsightDetail.viewVersion(\'\')">回到当前 (v' + _currentVersion() + ')</button>';
        } else {
            actionsHtml = '<button class="ins-btn ins-btn-ghost" onclick="InsightDetail.reviseDialog()">✎ 让 CC 改一版</button>'
                + '<button class="ins-btn ins-btn-primary" onclick="InsightDetail.publish()">📤 发布</button>';
        }
        return ''
          + '<div class="ins-report-bar">'
          +   '<span class="ins-report-version">v' + r.version + '</span>'
          +   '<span class="ins-report-meta">· ' + _shortTime(r.createdAt) + ' · ' + _templateLabel(r.template)
          +     (viewingHistory ? ' · <span class="ins-history-tag">📜 历史只读</span>' : '')
          +   '</span>'
          +   _renderVersionSelect()
          +   '<div class="ins-report-actions">' + actionsHtml + '</div>'
          + '</div>'
          + '<div class="ins-det-cols">'
          +   '<div class="ins-det-col-left">' + _renderSourceList(!viewingHistory) + '</div>'
          +   '<div class="ins-det-col-right">' + _renderReportRO() + '</div>'
          + '</div>';
    }

    // ---------- published ----------
    function _renderPublished() {
        var ins = _state.insight;
        var r = _state.report;
        var sh = _state.share;
        var viewingHistory = _state.viewingVersion && ins.currentReportId && r && r.id !== ins.currentReportId;
        var shareUrl = sh ? (location.origin + '/r/' + sh.token) : '';
        return ''
          + '<div class="ins-published-banner">'
          +   '<div class="ins-pub-row">'
          +     '<span class="ins-pub-icon">🔗</span>'
          +     '<span class="ins-pub-label">已发布</span>'
          +     '<span class="ins-pub-version">v' + (r ? r.version : '?') + (viewingHistory ? ' 📜历史' : '') + '</span>'
          +     (sh
              ? '<a class="ins-pub-link" href="/r/' + _esc(sh.token) + '" target="_blank">' + _esc(shareUrl) + '</a>'
              : '<span class="ins-pub-link ins-warn">无 active 分享链接</span>')
          +   '</div>'
          +   '<div class="ins-pub-actions">'
          +     _renderVersionSelect()
          +     (viewingHistory
              ? '<button class="ins-btn ins-btn-primary" onclick="InsightDetail.viewVersion(\'\')">回到当前 (v' + _currentVersion() + ')</button>'
              : ''
              + (sh ? '<button class="ins-btn ins-btn-ghost" onclick="InsightDetail.copyShareLink()">📋 复制</button>' : '')
              + (sh ? '<button class="ins-btn ins-btn-ghost" onclick="window.open(\'/r/' + _esc(sh.token) + '\')">↗ 打开</button>' : '')
              + '<button class="ins-btn ins-btn-danger" onclick="InsightDetail.retract()">⤺ 撤回</button>')
          +   '</div>'
          + '</div>'
          + '<div class="ins-det-cols">'
          +   '<div class="ins-det-col-left">' + _renderSourceList(false) + '</div>'
          +   '<div class="ins-det-col-right">' + (r ? _renderReportRO() : '') + '</div>'
          + '</div>';
    }

    function _renderArchived() {
        return '<div class="ins-archived"><strong>这个洞察已归档。</strong>'
          + '<button class="ins-btn" onclick="InsightDetail.unarchive()">取消归档</button></div>';
    }

    // ---------- source list (左栏共用) ----------
    function _renderSourceList(editable) {
        var ins = _state.insight;
        var sources = _state.sources;
        var headHtml = '<div class="ins-source-head">'
          + '<h3>📦 来源 <span class="ins-count">' + sources.length + '</span></h3>'
          + (editable
            ? '<button class="ins-btn ins-btn-sm" onclick="InsightDetail.addSourcePrompt()">+ 新增</button>'
            : '')
          + '</div>';
        var addHtml = editable
          ? '<div class="ins-source-add" id="ins-source-add" style="display:none">'
            + '<textarea id="ins-source-add-text" placeholder="粘贴 URL(一行一个)或一段文字"></textarea>'
            + '<div class="ins-source-add-actions">'
            +   '<button class="ins-btn ins-btn-ghost" onclick="InsightDetail.addSourceCancel()">取消</button>'
            +   '<button class="ins-btn ins-btn-primary" onclick="InsightDetail.addSourceSubmit()">添加</button>'
            + '</div>'
            + '</div>'
          : '';
        var emptyHtml = sources.length === 0
          ? '<div class="ins-source-empty">还没有来源。' + (editable ? '点上面「+ 新增」加 URL,或从右侧候选池拖入。' : '') + '</div>'
          : '';
        var listHtml = sources.map(function(s) {
            return _sourceRowHtml(s, editable);
        }).join('');
        return headHtml + addHtml + emptyHtml + '<div class="ins-source-list">' + listHtml + '</div>';
    }

    function _sourceRowHtml(s, editable) {
        var title = s.title || (s.url ? _shortUrl(s.url) : '(无标题)');
        var statusBadge = _sourceStatusBadge(s);
        var urlLink = s.url ? '<a class="ins-src-url" href="' + _esc(s.url) + '" target="_blank">↗</a>' : '';
        var starredCls = s.starred ? ' ins-src-starred' : '';
        return '<div class="ins-src-card' + starredCls + '" data-id="' + s.id + '">'
          + '<div class="ins-src-row">'
          +   '<div class="ins-src-icon">' + _kindIcon(s.kind) + '</div>'
          +   '<div class="ins-src-body">'
          +     '<div class="ins-src-title">' + _esc(title) + ' ' + urlLink + '</div>'
          +     '<div class="ins-src-meta">' + statusBadge
                  + (s.author ? ' · ' + _esc(s.author) : '')
                  + (s.content ? ' · ' + s.content.length + ' 字符' : '')
                  + '</div>'
          +   '</div>'
          +   (editable
              ? '<div class="ins-src-actions">'
                + '<button class="ins-icon-btn" title="' + (s.starred ? '取消重点' : '标为重点') + '" '
                +   'onclick="InsightDetail.toggleStar(' + s.id + ')">' + (s.starred ? '⭐' : '☆') + '</button>'
                + (s.fetchStatus === 'failed' ? '<button class="ins-icon-btn" onclick="InsightDetail.refetchSource(' + s.id + ')" title="重抓">↻</button>' : '')
                + '<button class="ins-icon-btn" onclick="InsightDetail.removeSourceFromInsight(' + s.id + ')" title="移出此 Insight (回候选池)">⤴</button>'
                + '<button class="ins-icon-btn ins-icon-btn-x" onclick="InsightDetail.deleteSource(' + s.id + ')" title="彻底删除">✕</button>'
              + '</div>'
              : '')
          + '</div>'
          + (s.fetchStatus === 'failed' && s.fetchError
              ? '<div class="ins-src-err">⚠ ' + _esc(s.fetchError) + '</div>'
              : '')
          + '</div>';
    }

    function _sourceStatusBadge(s) {
        switch (s.fetchStatus) {
            case 'pending': return '<span class="ins-src-st pending">⏳ 抓取中</span>';
            case 'ok':      return '<span class="ins-src-st ok">✓ ' + s.kind + '</span>';
            case 'failed':  return '<span class="ins-src-st failed">⚠ 失败</span>';
            case 'manual':  return '<span class="ins-src-st manual">✎ 粘贴</span>';
            default:        return _esc(s.kind || '');
        }
    }

    function _shortUrl(u) {
        try { var url = new URL(u); return url.hostname + url.pathname.slice(0, 30); }
        catch (_) { return u; }
    }
    function _shortTime(iso) {
        if (!iso) return '';
        try {
            var d = new Date(iso);
            return d.getFullYear() + '-' + String(d.getMonth()+1).padStart(2,'0') + '-' + String(d.getDate()).padStart(2,'0')
                + ' ' + String(d.getHours()).padStart(2,'0') + ':' + String(d.getMinutes()).padStart(2,'0');
        } catch (_) { return iso; }
    }

    // ---------- report render(只读 or inline edit) ----------
    function _renderReportRO() {
        var r = _state.report;
        var ins = _state.insight;
        var isCurrent = ins && r && r.id === ins.currentReportId;
        var canEdit = isCurrent
            && (ins.status === 'editing' || ins.status === 'collecting')
            && !_state.viewingVersion;

        if (_state.editingMd && canEdit) {
            return _renderReportEditor();
        }

        var html = (typeof InsightMd !== 'undefined') ? InsightMd.render(r.contentMd) : '<pre>' + _esc(r.contentMd) + '</pre>';
        // sources 引用列表(只展示 source_ids 中提到的那些)
        var refSrcs = [];
        try {
            var ids = Array.isArray(r.sourceIds) ? r.sourceIds : JSON.parse(r.sourceIds || '[]');
            ids.forEach(function(sid) {
                var s = _state.sources.find(function(x) { return x.id === sid; });
                if (s) refSrcs.push(s);
            });
        } catch (_) {}
        var sourcesHtml = '';
        if (refSrcs.length) {
            sourcesHtml = '<div class="ins-report-sources">'
              + '<h4>引用来源(' + refSrcs.length + ')</h4>'
              + '<ol>'
              + refSrcs.map(function(s) {
                  var link = s.url ? '<a href="' + _esc(s.url) + '" target="_blank">' + _esc(s.title || s.url) + '</a>' : _esc(s.title);
                  return '<li data-src-id="' + s.id + '">' + _kindIcon(s.kind) + ' ' + link + (s.author ? ' · ' + _esc(s.author) : '') + '</li>';
                }).join('')
              + '</ol>'
              + '</div>';
        }
        var editBtn = canEdit
            ? '<button class="ins-btn ins-btn-sm ins-btn-ghost ins-md-edit-btn" '
              + 'onclick="InsightDetail.startEditMd()" title="编辑 markdown(Cmd+E)">✎ 编辑</button>'
            : '';
        return '<div class="ins-report-wrap">'
          + editBtn
          + '<div class="ins-report-md">' + html + '</div>'
          + sourcesHtml
          + '</div>';
    }

    function _renderReportEditor() {
        var r = _state.report;
        var rawMd = r.contentMd || '';
        var isPublished = !!r.publishedAt;
        return '<div class="ins-report-wrap ins-report-editing">'
          + '<div class="ins-md-edit-bar">'
          +   '<span class="ins-md-edit-label">✎ 编辑 v' + r.version + ' markdown'
          +     (isPublished ? ' · <span class="ins-warn-tag">⚠ 已发布版本,保存会 fork 为新版本</span>' : '')
          +   '</span>'
          +   '<div class="ins-md-edit-actions">'
          +     '<button class="ins-btn ins-btn-ghost ins-btn-sm" onclick="InsightDetail.cancelEditMd()">取消 (Esc)</button>'
          +     '<button class="ins-btn ins-btn-primary ins-btn-sm" onclick="InsightDetail.saveEditMd()">保存 (Cmd+S)</button>'
          +   '</div>'
          + '</div>'
          + '<textarea id="ins-md-textarea" class="ins-md-textarea" '
          +   'oninput="InsightDetail.autoGrowTextarea(this)" '
          +   'onkeydown="InsightDetail.handleMdKey(event)"'
          + '>' + _esc(rawMd) + '</textarea>'
          + '</div>';
    }

    // ============ 操作 ============
    async function updateField(field, value) {
        var patch = {};
        patch[field] = value;
        try {
            var resp = await API.insightUpdate(_id, patch);
            if (resp && resp.success) _state.insight = resp.item;
        } catch (e) {
            console.error('[InsightDetail] updateField err', field, e);
            if (typeof showToast === 'function') showToast('保存失败', 'error');
        }
    }

    async function markReady() {
        try {
            await API.insightUpdate(_id, { status: 'ready' });
            await refresh();
            if (typeof showToast === 'function') showToast('已标记准备好,在 Claude Code 里说: 处理 Insight #' + _id, 'success');
        } catch (e) {
            console.error('[InsightDetail] markReady err', e);
            if (typeof showToast === 'function') showToast('操作失败', 'error');
        }
    }

    async function cancelReady() {
        var ins = _state.insight;
        var backTo = ins.currentReportId ? 'editing' : 'collecting';
        try {
            await API.insightUpdate(_id, { status: backTo });
            await refresh();
        } catch (e) {
            console.error('[InsightDetail] cancelReady err', e);
        }
    }

    function addSourcePrompt() {
        var el = document.getElementById('ins-source-add');
        if (el) {
            el.style.display = '';
            var ta = document.getElementById('ins-source-add-text');
            if (ta) ta.focus();
        }
    }
    function addSourceCancel() {
        var el = document.getElementById('ins-source-add');
        if (el) el.style.display = 'none';
        var ta = document.getElementById('ins-source-add-text');
        if (ta) ta.value = '';
    }
    async function addSourceSubmit() {
        var ta = document.getElementById('ins-source-add-text');
        if (!ta) return;
        var text = (ta.value || '').trim();
        if (!text) return;
        var lines = text.split(/\n+/).map(function(l) { return l.trim(); }).filter(Boolean);
        var urls = lines.filter(function(l) { return /^https?:\/\//i.test(l); });
        var nonUrlText = lines.filter(function(l) { return !/^https?:\/\//i.test(l); }).join('\n');
        try {
            for (var i = 0; i < urls.length; i++) {
                await API.sourceCreate({ url: urls[i], insightId: _id });
            }
            if (nonUrlText) {
                await API.sourceCreate({ content: nonUrlText, insightId: _id });
            }
            addSourceCancel();
            if (typeof showToast === 'function') showToast('已添加 ' + (urls.length + (nonUrlText ? 1 : 0)) + ' 条来源', 'success');
            await refresh();
        } catch (e) {
            console.error('[InsightDetail] addSource err', e);
            if (typeof showToast === 'function') showToast('添加失败', 'error');
        }
    }

    async function toggleStar(sourceId) {
        var s = _state.sources.find(function(x) { return x.id === sourceId; });
        if (!s) return;
        try {
            await API.sourceUpdate(sourceId, { starred: !s.starred });
            await refresh();
        } catch (e) { console.error('[InsightDetail] toggleStar', e); }
    }
    async function refetchSource(sourceId) {
        try {
            await API.sourceRefetch(sourceId);
            if (typeof showToast === 'function') showToast('重抓中...', 'info');
            setTimeout(refresh, 1500);
        } catch (e) { console.error('[InsightDetail] refetch', e); }
    }
    async function removeSourceFromInsight(sourceId) {
        // 解除归属 = insightId 设为 null;后端 UpdateSourceRequest 用 Option<Option<i64>>
        // 这里传 null 表示 set to NULL
        try {
            await API.sourceUpdate(sourceId, { insightId: null });
            if (typeof showToast === 'function') showToast('已移回候选池', 'info');
            await refresh();
            if (typeof InsightCapture !== 'undefined') InsightCapture.refresh();
        } catch (e) { console.error('[InsightDetail] removeSource', e); }
    }
    async function deleteSource(sourceId) {
        try {
            await API.sourceDelete(sourceId);
            await refresh();
        } catch (e) { console.error('[InsightDetail] deleteSource', e); }
    }

    // ---------- 历史版本下拉 ----------
    function _currentVersion() {
        var cur = _state.allReports.find(function(r) { return r.id === (_state.insight && _state.insight.currentReportId); });
        return cur ? cur.version : '?';
    }
    function _renderVersionSelect() {
        if (!_state.allReports || _state.allReports.length <= 1) return '';
        var r = _state.report;
        var ins = _state.insight;
        return '<select class="ins-version-select" onchange="InsightDetail.viewVersion(this.value)">'
          + _state.allReports.map(function(rep) {
              var marks = [];
              if (ins && rep.id === ins.currentReportId) marks.push('当前');
              if (rep.publishedAt && rep.retractedAt) marks.push('已撤回');
              else if (rep.publishedAt) marks.push('已发布');
              if (rep.generatedBy === 'boris-inline') marks.push('手改');
              var label = 'v' + rep.version + ' · ' + _shortTime(rep.createdAt);
              if (marks.length) label += ' · ' + marks.join('、');
              var selected = (r && rep.version === r.version) ? ' selected' : '';
              return '<option value="' + rep.version + '"' + selected + '>' + _esc(label) + '</option>';
            }).join('')
          + '</select>';
    }
    function viewVersion(v) {
        if (!v || v === '') {
            _state.viewingVersion = null;
        } else {
            _state.viewingVersion = parseInt(v, 10);
        }
        refresh();
    }

    // ---------- inline markdown 编辑 (P2.5) ----------
    function startEditMd() {
        var r = _state.report;
        if (!r) return;
        // 已发布过 → 提前弹 fork 确认(虽然后端会自动 fork,但 spec § 5.3.3 要求先问)
        if (r.publishedAt) {
            if (typeof InsightDialog !== 'undefined') {
                InsightDialog.showForkConfirm(r.version, function() {
                    _state.editingMd = true;
                    _render();
                    setTimeout(_focusMdTextarea, 60);
                });
            }
        } else {
            _state.editingMd = true;
            _render();
            setTimeout(_focusMdTextarea, 60);
        }
    }
    function _focusMdTextarea() {
        var ta = document.getElementById('ins-md-textarea');
        if (ta) {
            ta.focus();
            autoGrowTextarea(ta);
            // 光标放末尾
            ta.setSelectionRange(ta.value.length, ta.value.length);
        }
    }
    function autoGrowTextarea(ta) {
        if (!ta) return;
        ta.style.height = 'auto';
        ta.style.height = Math.max(360, ta.scrollHeight + 4) + 'px';
    }
    function cancelEditMd() {
        _state.editingMd = false;
        _render();
    }
    async function saveEditMd() {
        var ta = document.getElementById('ins-md-textarea');
        if (!ta) return;
        var newMd = ta.value;
        var r = _state.report;
        if (!r) return;
        try {
            var resp = await API.insightPatchReport(_id, r.version, { contentMd: newMd });
            if (resp && resp.success) {
                if (typeof showToast === 'function') {
                    // 如果服务端 fork 出新版本,resp.item.version 会 > r.version
                    if (resp.item.version > r.version) {
                        showToast('已发布版本不可改写,已 fork 为 v' + resp.item.version, 'success');
                    } else {
                        showToast('已保存 v' + r.version, 'success');
                    }
                }
                _state.editingMd = false;
                await refresh();
            } else {
                if (typeof showToast === 'function') showToast(resp && resp.error || '保存失败', 'error');
            }
        } catch (e) {
            console.error('[InsightDetail] saveEditMd err', e);
            if (typeof showToast === 'function') showToast('保存失败', 'error');
        }
    }
    function handleMdKey(ev) {
        if (ev.key === 'Escape') {
            ev.preventDefault();
            cancelEditMd();
        } else if ((ev.metaKey || ev.ctrlKey) && (ev.key === 's' || ev.key === 'S' || ev.key === 'Enter')) {
            ev.preventDefault();
            saveEditMd();
        }
    }

    function reviseDialog() {
        if (typeof InsightDialog !== 'undefined') {
            InsightDialog.showRevise(_id, function() { refresh(); });
        }
    }

    function publish() {
        // P2:走 InsightDialog 的发布 dialog(预览 + show_notes 复选框)
        if (typeof InsightDialog !== 'undefined') {
            InsightDialog.showPublish(_id, _state.report, function() { refresh(); });
        }
    }

    function retract() {
        if (typeof InsightDialog !== 'undefined') {
            InsightDialog.showRetract(_id, function() { refresh(); });
        }
    }

    async function copyShareLink() {
        var sh = _state.share;
        if (!sh) return;
        var url = location.origin + '/r/' + sh.token;
        try {
            await navigator.clipboard.writeText(url);
            if (typeof showToast === 'function') showToast('已复制:' + url, 'success');
        } catch (e) {
            // 回退:用 textarea + execCommand
            var ta = document.createElement('textarea');
            ta.value = url;
            document.body.appendChild(ta);
            ta.select();
            try { document.execCommand('copy'); } catch (_) {}
            document.body.removeChild(ta);
            if (typeof showToast === 'function') showToast('已复制', 'success');
        }
    }

    async function unarchive() {
        try {
            await API.insightUpdate(_id, { status: _state.insight.currentReportId ? 'editing' : 'collecting' });
            await refresh();
        } catch (e) { console.error('[InsightDetail] unarchive', e); }
    }

    function notImpl(feature) {
        if (typeof showToast === 'function') showToast(feature + ' — 留 P2 实现', 'info');
    }

    return {
        open: open,
        close: close,
        refresh: refresh,
        currentInsightId: currentInsightId,
        updateField: updateField,
        markReady: markReady,
        cancelReady: cancelReady,
        addSourcePrompt: addSourcePrompt,
        addSourceCancel: addSourceCancel,
        addSourceSubmit: addSourceSubmit,
        toggleStar: toggleStar,
        refetchSource: refetchSource,
        removeSourceFromInsight: removeSourceFromInsight,
        deleteSource: deleteSource,
        publish: publish,
        retract: retract,
        reviseDialog: reviseDialog,
        viewVersion: viewVersion,
        startEditMd: startEditMd,
        cancelEditMd: cancelEditMd,
        saveEditMd: saveEditMd,
        autoGrowTextarea: autoGrowTextarea,
        handleMdKey: handleMdKey,
        copyShareLink: copyShareLink,
        unarchive: unarchive,
        notImpl: notImpl,
    };
})();
