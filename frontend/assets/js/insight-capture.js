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
          +   '<textarea id="ins-cap-text" placeholder="粘贴 URL(一行一个)或一段文本&#10;回车提交;Shift+Enter 换行"></textarea>'
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

    function _cardHtml(s) {
        var title = s.title || (s.url ? _shortUrl(s.url) : '(无标题)');
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

    return {
        refresh: refresh,
        submit: submit,
        remove: remove,
        toggleAssign: toggleAssign,
        doAssign: doAssign,
        refetch: refetch,
    };
})();
