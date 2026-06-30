// ========== Praxis 记录回看 (T-286 / SPEC praxis §6) ==========
// 只读：翻历史今日经营记录，列表 + 筛选(日期/板块/类型) + 详情。
// 复用 GET /api/praxis/journal（from/to 走后端，board/type 前端筛），无新表/新端点。
var PraxisReview = (function() {
    var _entries = [];      // 服务端按 from/to 拉回的原始集合
    var _selectedId = null;

    var BOARD_NAMES = {
        strategy: '战略定位', skill: '能力产品', ops: '运营管理', fin: '财务健康',
        brand: '品牌市场', rel: '关键关系', cog: '竞争认知', risk: '风险预案'
    };
    var VALUE_LABELS = {
        build: '主动建设', respond: '被动响应', firefight: '救火',
        maintain: '维护', rest: '休整'
    };

    function open() {
        toggleView(true);
        fillSelects();
        _selectedId = null;
        renderDetailEmpty();
        load();
    }

    function close() {
        toggleView(false);
        if (window.Praxis && typeof Praxis.refreshJournalStatus === 'function') {
            Praxis.refreshJournalStatus();
        }
    }

    function toggleView(show) {
        ['praxis-cockpit', 'praxis-today-entry', 'praxis-review-entry'].forEach(function(id) {
            var el = document.getElementById(id);
            if (el) el.style.display = show ? 'none' : '';
        });
        var jview = document.getElementById('praxis-journal-view');
        if (jview) jview.style.display = 'none';
        var rview = document.getElementById('praxis-review-view');
        if (rview) rview.style.display = show ? '' : 'none';
    }

    function fillSelects() {
        var board = document.getElementById('pr-board');
        if (board && !board.dataset.filled) {
            board.innerHTML = '<option value="">全部板块</option>' +
                Object.keys(BOARD_NAMES).map(function(k) {
                    return '<option value="' + k + '">' + BOARD_NAMES[k] + '</option>';
                }).join('');
            board.dataset.filled = '1';
        }
        var type = document.getElementById('pr-type');
        if (type && !type.dataset.filled) {
            type.innerHTML = '<option value="">全部类型</option>' +
                Object.keys(VALUE_LABELS).map(function(k) {
                    return '<option value="' + k + '">' + VALUE_LABELS[k] + '</option>';
                }).join('');
            type.dataset.filled = '1';
        }
    }

    function val(id) {
        var el = document.getElementById(id);
        return el ? el.value : '';
    }

    async function load() {
        var params = { limit: 200 };
        if (val('pr-from')) params.from = val('pr-from');
        if (val('pr-to')) params.to = val('pr-to');
        try {
            var res = await API.praxisJournalList(params);
            if (!res || res.success === false) throw new Error((res && res.error) || '加载失败');
            _entries = res.items || [];
        } catch (err) {
            console.error('[PraxisReview] load failed', err);
            if (typeof showToast === 'function') showToast('记录回看加载失败', 'error');
            _entries = [];
        }
        renderList();
    }

    // from/to 变了要回服务端重拉；board/type 是纯前端筛。
    function applyFilter() {
        load();
    }

    function resetFilter() {
        ['pr-from', 'pr-to'].forEach(function(id) { var el = document.getElementById(id); if (el) el.value = ''; });
        ['pr-board', 'pr-type'].forEach(function(id) { var el = document.getElementById(id); if (el) el.value = ''; });
        load();
    }

    function structuredOf(e) {
        return (e && e.structured && typeof e.structured === 'object') ? e.structured : null;
    }

    function filtered() {
        var board = val('pr-board');
        var type = val('pr-type');
        return _entries.filter(function(e) {
            var s = structuredOf(e);
            if (board) {
                if (!s || !Array.isArray(s.boards) || s.boards.indexOf(board) === -1) return false;
            }
            if (type) {
                if (!s || s.value !== type) return false;
            }
            return true;
        });
    }

    function renderList() {
        var list = document.getElementById('pr-list');
        var count = document.getElementById('pr-count');
        if (!list) return;
        var items = filtered();
        if (count) count.textContent = '共 ' + items.length + ' 条';
        if (!items.length) {
            list.innerHTML = '<div class="pr-list-empty">没有匹配的记录</div>';
            return;
        }
        list.innerHTML = items.map(function(e) {
            var s = structuredOf(e);
            var title = s && s.summary ? s.summary : (e.rawText || '（无内容）');
            var type = s && s.value && VALUE_LABELS[s.value] ? '<span class="pr-row-type">' + esc(VALUE_LABELS[s.value]) + '</span>' : '';
            var boards = (s && Array.isArray(s.boards) ? s.boards : []).slice(0, 4).map(function(b) {
                return '<span class="pr-row-board">' + esc(BOARD_NAMES[b] || b) + '</span>';
            }).join('');
            var sel = e.id === _selectedId ? ' selected' : '';
            return '<button class="pr-row' + sel + '" onclick="PraxisReview.select(' + e.id + ')">' +
                '<div class="pr-row-head"><span class="pr-row-date">' + esc(e.entryDate) + '</span>' + type + '</div>' +
                '<div class="pr-row-title">' + esc(snippet(title)) + '</div>' +
                (boards ? '<div class="pr-row-boards">' + boards + '</div>' : '') +
                '</button>';
        }).join('');
    }

    function snippet(text) {
        var t = String(text || '').replace(/\s+/g, ' ').trim();
        return t.length > 60 ? t.slice(0, 60) + '…' : t;
    }

    function select(id) {
        _selectedId = id;
        renderList();
        var e = _entries.find(function(x) { return x.id === id; });
        if (e) renderDetail(e);
    }

    function renderDetailEmpty() {
        var d = document.getElementById('pr-detail');
        if (d) d.innerHTML = '<div class="pr-detail-empty">点左侧某条记录看详情</div>';
    }

    function renderDetail(e) {
        var d = document.getElementById('pr-detail');
        if (!d) return;
        var s = structuredOf(e);
        var html = '<div class="pr-detail-head">' +
            '<h4>' + esc(e.entryDate) + '</h4>' +
            '<button class="eg-btn" onclick="PraxisReview.edit(\'' + esc(e.entryDate) + '\')">编辑</button>' +
            '</div>';
        html += '<div class="pr-detail-sec"><h5>原文</h5><p class="pr-raw">' +
            (e.rawText ? esc(e.rawText) : '<em>（空）</em>') + '</p></div>';
        if (s) {
            if (s.value && VALUE_LABELS[s.value]) {
                html += '<div class="pj-card-type pj-type-' + esc(s.value) + '">' + esc(VALUE_LABELS[s.value]) + '</div>';
            }
            if (s.summary) html += '<div class="pr-detail-sec"><h5>摘要</h5><p>' + esc(s.summary) + '</p></div>';
            var boards = Array.isArray(s.boards) ? s.boards : [];
            if (boards.length) {
                html += '<div class="pr-detail-sec"><h5>涉及板块</h5><div class="pj-card-boards">' +
                    boards.map(function(b) { return '<span class="pj-board-tag">' + esc(BOARD_NAMES[b] || b) + '</span>'; }).join('') +
                    '</div></div>';
            }
            var events = Array.isArray(s.events) ? s.events : [];
            if (events.length) {
                html += '<div class="pr-detail-sec"><h5>关键事件</h5><ul>' +
                    events.map(function(x) { return '<li>' + esc(x) + '</li>'; }).join('') + '</ul></div>';
            }
            var risks = Array.isArray(s.risks) ? s.risks : [];
            if (risks.length) {
                html += '<div class="pr-detail-sec pj-card-risks"><h5>风险</h5><ul>' +
                    risks.map(function(x) { return '<li>' + esc(x) + '</li>'; }).join('') + '</ul></div>';
            }
            if (s.tomorrow) {
                html += '<div class="pj-card-tomorrow"><h5>明日调整</h5><p>' + esc(s.tomorrow) + '</p></div>';
            }
        } else {
            html += '<div class="pr-detail-sec"><em>这条还没分析过。</em></div>';
        }
        d.innerHTML = html;
    }

    // 编辑 → 回今日经营页（只读回看的编辑入口，spec §6）。
    function edit(date) {
        if (window.PraxisJournal && typeof PraxisJournal.open === 'function') {
            PraxisJournal.open(date);
        }
    }

    function esc(value) {
        return String(value == null ? '' : value)
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;').replace(/'/g, '&#39;');
    }

    return {
        open: open,
        close: close,
        applyFilter: applyFilter,
        resetFilter: resetFilter,
        select: select,
        edit: edit
    };
})();
