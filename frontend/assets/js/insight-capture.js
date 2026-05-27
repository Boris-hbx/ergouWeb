// ========== InsightCapture — 未归属候选池侧栏 (T-106 / spec § 5.4) ==========
//
// 右侧固定 320px 侧栏:粘贴 URL/文本 → 异步抓取 → 卡片列表 → 归到 Insight / 删除。
// 后端 API:
//   GET    /api/sources?unassigned=1   拉未归属候选
//   POST   /api/sources {url|content}  新建(后端识别 kind,async 抓取)
//   PATCH  /api/sources/:id {insight_id}  归属
//   DELETE /api/sources/:id            软删
//   POST   /api/sources/:id/refetch    重抓
//
// 抓取轮询:每 3 秒(后端是 async tokio::spawn,通常几秒内完成);
//   pending 数量归零后自动停止轮询,避免空转。
//
// 归属 UX:不弹 modal、不用 window.prompt(spec 通约禁用);
//   卡片右侧按钮 → 展开 native <select> 列出所有 Insight,选完即归属。

var InsightCapture = (function() {
    var _items = [];
    var _insights = [];
    var _refreshTimer = null;
    var _expandedSourceId = null;   // 当前展开归属 select 的 source id
    var _pendingShortText = '';     // T-112:短文本警示二选一暂存(spec § 5.4.1)

    function _esc(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
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
    function _statusBadge(st) {
        switch (st) {
            case 'pending': return '<span class="ins-cap-st ins-cap-st-pending">⏳ 抓取中</span>';
            case 'ok':      return '<span class="ins-cap-st ins-cap-st-ok">✓ 已抓</span>';
            case 'failed':  return '<span class="ins-cap-st ins-cap-st-failed">⚠ 失败</span>';
            case 'manual':  return '<span class="ins-cap-st ins-cap-st-manual">✎ 粘贴</span>';
            default:        return '';
        }
    }
    function _shortUrl(u) {
        try { var url = new URL(u); return url.hostname + url.pathname.slice(0, 30); }
        catch (_) { return u; }
    }
    // T-112 / spec § 5.4.2:无 title 时 content[:60]+"…"
    //   normalize 多行空白成单空格,避免渲染时换行撑高卡片
    function _previewContent(c) {
        if (!c) return '';
        var s = ('' + c).replace(/\s+/g, ' ').trim();
        if (!s) return '';
        return s.length > 60 ? s.slice(0, 60) + '…' : s;
    }

    function render() {
        var host = document.getElementById('insight-capture-sidebar');
        if (!host) return;
        var pendingCount = _items.filter(function(s) { return s.fetchStatus === 'pending'; }).length;
        host.innerHTML = ''
          + '<div class="ins-cap-head">'
          +   '<h3>候选池</h3>'
          +   '<span class="ins-cap-count">' + _items.length + ' 项</span>'
          + '</div>'
          + '<div class="ins-cap-input">'
          +   '<textarea id="ins-cap-text" placeholder="粘贴 URL(每行一个)或素材原文片段。&#10;想新建洞察请点上方「+ 新建洞察」。"></textarea>'
          +   _shortTextWarnHtml()
          +   '<button class="ins-cap-add" onclick="InsightCapture.submit()">+ 添加到候选池</button>'
          + '</div>'
          + (pendingCount ? '<div class="ins-cap-poll">⏳ 抓取中(' + pendingCount + ')...</div>' : '')
          + '<div class="ins-cap-list" id="ins-cap-list">'
          +   (_items.length === 0
              ? '<div class="ins-cap-empty">还没有候选。刷推时把链接丢这儿。</div>'
              : _items.map(_cardHtml).join(''))
          + '</div>';

        // 绑定 textarea 回车提交
        var ta = document.getElementById('ins-cap-text');
        if (ta) {
            ta.addEventListener('keydown', function(ev) {
                if (ev.key === 'Enter' && !ev.shiftKey) {
                    ev.preventDefault();
                    submit();
                }
            });
        }
    }

    // T-112 / spec § 5.4.1:短文本警示二选一 inline 提示条
    //   单次粘贴 < 20 字 + 无 URL 形态时触发;用户必须明示二选一才入库,不强制拦截
    function _shortTextWarnHtml() {
        if (!_pendingShortText) return '';
        return '<div class="ins-cap-warn">'
          + '<div class="ins-cap-warn-head">'
          +   '<span class="ins-cap-warn-icon">💭</span>'
          +   '<span class="ins-cap-warn-msg">看起来像研究主题而非素材。要不要新建洞察?</span>'
          +   '<button class="ins-cap-warn-x" onclick="InsightCapture.dismissShortWarn()" title="关闭(保留编辑)">✕</button>'
          + '</div>'
          + '<div class="ins-cap-warn-actions">'
          +   '<button class="ins-cap-warn-primary" onclick="InsightCapture.shortToInsight()">+ 新建洞察</button>'
          +   '<button class="ins-cap-warn-ghost" onclick="InsightCapture.shortAsTextSource()">就这样添加为 text source</button>'
          + '</div>'
          + '</div>';
    }

    function _cardHtml(s) {
        // T-112 / spec § 5.4.2:候选卡铁律——绝不允许整张视觉空白
        //   有 title → 用 title
        //   无 title 但有 content → content[:60]+"…"(粘贴的 text source 走这条)
        //   都无但有 url → shortUrl
        //   全无 → '(无标题)'
        var title = s.title
            || (s.content ? _previewContent(s.content) : '')
            || (s.url ? _shortUrl(s.url) : '')
            || '(无标题)';
        var expanded = (_expandedSourceId === s.id);
        var insightOpts = _insights.map(function(i) {
            return '<option value="' + i.id + '">' + _esc(i.title || '(无标题)') + '</option>';
        }).join('');
        var fetchErr = (s.fetchStatus === 'failed' && s.fetchError)
            ? '<div class="ins-cap-err">' + _esc(s.fetchError) + '</div>'
            : '';
        return '<div class="ins-cap-card" data-id="' + s.id + '">'
          +   '<div class="ins-cap-row">'
          +     '<div class="ins-cap-icon">' + _kindIcon(s.kind) + '</div>'
          +     '<div class="ins-cap-body">'
          +       '<div class="ins-cap-title">' + _esc(title) + '</div>'
          +       '<div class="ins-cap-meta">' + _statusBadge(s.fetchStatus) + (s.kind ? ' · ' + s.kind : '') + '</div>'
          +     '</div>'
          +     '<div class="ins-cap-actions">'
          +       (s.fetchStatus === 'failed' ? '<button class="ins-icon-btn" onclick="InsightCapture.refetch(' + s.id + ')" title="重试抓取">↻</button>' : '')
          +       '<button class="ins-icon-btn" onclick="InsightCapture.toggleAssign(' + s.id + ')" title="归到 Insight">→</button>'
          +       '<button class="ins-icon-btn ins-icon-btn-x" onclick="InsightCapture.remove(' + s.id + ')" title="删除">✕</button>'
          +     '</div>'
          +   '</div>'
          +   fetchErr
          +   (expanded
              ? '<div class="ins-cap-assign">'
                + (_insights.length === 0
                  ? '<span class="ins-cap-assign-empty">还没有 Insight,先新建一个</span>'
                  : '<select onchange="InsightCapture.doAssign(' + s.id + ', this.value)">'
                    + '<option value="">— 选择 Insight —</option>'
                    + insightOpts
                    + '</select>')
              + '</div>'
              : '')
          + '</div>';
    }

    async function refresh() {
        try {
            // 并行拉:候选池 + insight 列表(归属用)
            var results = await Promise.all([
                API.sourceList({ unassigned: 1 }),
                API.insightList({}),
            ]);
            _items = (results[0] && results[0].items) || [];
            _insights = (results[1] && results[1].items) || [];
            render();
            // 自动管理轮询
            var hasPending = _items.some(function(s) { return s.fetchStatus === 'pending'; });
            if (hasPending) _ensurePolling();
            else            _stopPolling();
        } catch (e) {
            console.error('[InsightCapture] refresh failed', e);
        }
    }

    function _ensurePolling() {
        if (_refreshTimer) return;
        _refreshTimer = setInterval(function() {
            refresh();
        }, 3000);
    }
    function _stopPolling() {
        if (_refreshTimer) { clearInterval(_refreshTimer); _refreshTimer = null; }
    }

    async function submit() {
        var ta = document.getElementById('ins-cap-text');
        if (!ta) return;
        var text = (ta.value || '').trim();
        if (!text) return;

        // T-112 / spec § 5.4.1:短文本警示二选一
        //   单条 < 20 字 + 不含 http(s):// → 不直接入库,弹 inline 提示条让用户明示二选一
        var hasUrl = /https?:\/\//i.test(text);
        if (!hasUrl && text.length < 20) {
            _pendingShortText = text;
            render();
            return;
        }

        var lines = text.split(/\n+/).map(function(l) { return l.trim(); }).filter(Boolean);
        if (lines.length === 0) return;

        // 全部都是 URL? 还是混入文本?
        // 简单策略:每行 startsWith http(s) → 当 URL;否则整段(多行合并)当 text
        var urls = lines.filter(function(l) { return /^https?:\/\//i.test(l); });
        var nonUrlText = lines.filter(function(l) { return !/^https?:\/\//i.test(l); }).join('\n');

        try {
            // URL 逐条建
            for (var i = 0; i < urls.length; i++) {
                await API.sourceCreate({ url: urls[i] });
            }
            // 非 URL 行合并作为一条 text source
            if (nonUrlText) {
                await API.sourceCreate({ content: nonUrlText });
            }
            ta.value = '';
            if (typeof showToast === 'function') {
                showToast('已添加 ' + (urls.length + (nonUrlText ? 1 : 0)) + ' 条候选,后台抓取中', 'success');
            }
            refresh();
        } catch (e) {
            console.error('[InsightCapture] submit failed', e);
            if (typeof showToast === 'function') showToast('添加失败:' + (e && e.message || ''), 'error');
        }
    }

    async function remove(id) {
        try {
            await API.sourceDelete(id);
            if (_expandedSourceId === id) _expandedSourceId = null;
            refresh();
        } catch (e) {
            console.error('[InsightCapture] delete failed', e);
        }
    }

    function toggleAssign(id) {
        _expandedSourceId = (_expandedSourceId === id) ? null : id;
        render();
    }

    async function doAssign(sourceId, insightIdStr) {
        var insightId = parseInt(insightIdStr, 10);
        if (!insightId) return;
        try {
            await API.sourceUpdate(sourceId, { insightId: insightId });
            _expandedSourceId = null;
            var ins = _insights.find(function(i) { return i.id === insightId; });
            if (typeof showToast === 'function') showToast('已归入「' + (ins && ins.title || ('#' + insightId)) + '」', 'success');
            refresh();
            // 通知详情页(如果当前打开的是这个 insight)
            if (typeof InsightDetail !== 'undefined' && InsightDetail.currentInsightId() === insightId) {
                InsightDetail.refresh();
            }
        } catch (e) {
            console.error('[InsightCapture] assign failed', e);
            if (typeof showToast === 'function') showToast('归入失败', 'error');
        }
    }

    async function refetch(id) {
        try {
            await API.sourceRefetch(id);
            if (typeof showToast === 'function') showToast('重新抓取中...', 'info');
            refresh();
        } catch (e) {
            console.error('[InsightCapture] refetch failed', e);
        }
    }

    // ============ T-112 短文本警示二选一处理(spec § 5.4.1) ============

    // 用户选「新建洞察」→ 打开 Insight.toggleNewForm + 预填 title
    function shortToInsight() {
        var text = _pendingShortText;
        _pendingShortText = '';
        var ta = document.getElementById('ins-cap-text');
        if (ta) ta.value = '';
        render();
        if (typeof Insight !== 'undefined' && Insight.toggleNewForm) {
            // 确保新建表单是打开的
            // 预填 title:用 setTimeout 等 render 完再写 DOM
            setTimeout(function() {
                var ti = document.getElementById('ins-new-title');
                if (!ti) {
                    Insight.toggleNewForm();
                    setTimeout(function() {
                        var ti2 = document.getElementById('ins-new-title');
                        if (ti2) { ti2.value = text; ti2.focus(); }
                    }, 30);
                } else {
                    ti.value = text;
                    ti.focus();
                }
            }, 0);
        }
    }

    // 用户选「就这样添加为 text source」→ 走原 sourceCreate(content)
    async function shortAsTextSource() {
        var text = _pendingShortText;
        _pendingShortText = '';
        var ta = document.getElementById('ins-cap-text');
        if (ta) ta.value = '';
        try {
            await API.sourceCreate({ content: text });
            if (typeof showToast === 'function') showToast('已作为 text source 添加', 'success');
            refresh();
        } catch (e) {
            console.error('[InsightCapture] shortAsTextSource failed', e);
            if (typeof showToast === 'function') showToast('添加失败', 'error');
            render();
        }
    }

    // ✕ 关闭警示但保留 textarea 内容(允许用户继续编辑后再提交)
    function dismissShortWarn() {
        _pendingShortText = '';
        render();
    }

    return {
        refresh: refresh,
        submit: submit,
        remove: remove,
        toggleAssign: toggleAssign,
        doAssign: doAssign,
        refetch: refetch,
        // T-112
        shortToInsight: shortToInsight,
        shortAsTextSource: shortAsTextSource,
        dismissShortWarn: dismissShortWarn,
    };
})();
