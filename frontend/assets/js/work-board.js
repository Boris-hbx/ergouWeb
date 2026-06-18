// ========== WorkBoard — 工作任务表的看板视图 (T-094) ==========
//
// 按 status 分 4 列:待办 / 进行中 / 阻塞 / 已完成。
// 卡片可拖到目标列,落下后:status 写回;若 status = done 则 progress 也自动 = 100
// (后端 PATCH 时会强制做这件事,前端预先更新只是为了 UI 立即响应)。
//
// 卡片只显示精简元数据(spec § 6.2):标题 / 责任人头像 / 层级 / 频率 / 截止日 / 简介小图标。

var WorkBoard = (function() {
    var _drag = { id: null };

    function render() {
        var host = document.getElementById('wt-board-view');
        if (!host) return;

        var rows = _visibleRows();
        var STATUS = WorkTable._STATUS;
        WorkGridEngine.renderBoard({
            host: host,
            columns: STATUS,
            rows: rows,
            dragState: _drag,
            skipColumn: function(col) {
                // T-113:看板砍'已完成'列,只保留 3 列(待办 / 进行中 / 阻塞)
                //   完成路径走单元格 ☐ → progress dialog → 100% → 二次确认 → 彩纸(保护 B.3 完成确认链)
                //   不允许通过拖动改 status=done 绕过确认链
                return col.key === 'done';
            },
            rowsForColumn: function(list, col) {
                return list.filter(function(t) { return t.status === col.key; });
            },
            columnHeaderHtml: function(col, items) {
                return '<div class="wt-col-head">'
                  +   '<span class="wt-pdot" style="width:9px;height:9px;flex:0 0 9px;background:' + col.dot + '"></span>'
                  +   col.label
                  +   '<span class="wt-count">' + items.length + '</span>'
                  + '</div>';
            },
            cardHtml: _card,
            onDrop: function(id, newStatus) {
                var patch = { status: newStatus };
                if (newStatus === 'done') patch.progress = 100;
                Work.updateRow(id, patch);
            },
        });
    }

    function _visibleRows() {
        // T-098:先过时间镜头 Tab,再过责任人(两层叠加,与表格视图一致)
        var rows = Work.applyTimeTabFilter(Work.rows());
        var fEl = document.getElementById('wt-filter');
        var f = fEl ? fEl.value : '';
        if (!f) return rows;
        return rows.filter(function(t) { return t.assignee === f; });
    }

    function _card(t) {
        var esc = WorkTable._esc;
        var today = WorkTable._todayMD();
        var overdue = (t.status !== 'done' && t.due && t.due < today) ? ' overdue' : '';
        var descIcon = (t.desc && ('' + t.desc).trim())
            ? '<span class="wt-desc-expand" '
              + 'onclick="event.stopPropagation();WorkTable.openText(' + t.id + ',\'desc\')" '
              + 'title="查看简介">📄</span>'
            : '';
        // T-119:协作者头像组(<=3,超出 +N)
        var collabs = Array.isArray(t.collaborators) ? t.collaborators : [];
        var collabHtml = '';
        if (collabs.length > 0) {
            var shown = collabs.slice(0, 3);
            var extra = collabs.length - shown.length;
            collabHtml = '<span class="wt-collab-stack" title="协作者:' + esc(collabs.join('、')) + '">'
                + shown.map(function(c) {
                    return '<span class="wt-avatar wt-avatar-xs" style="background:' + Work.colorOf(c) + '">' + esc(('' + c).slice(0, 1)) + '</span>';
                  }).join('')
                + (extra > 0 ? '<span class="wt-collab-more">+' + extra + '</span>' : '')
                + '</span>';
        }
        // T-100:卡片点击(空白处)打开详情;内部 desc 图标依然 stopPropagation 走原弹层。
        return '<div class="wt-card" draggable="true" data-id="' + t.id + '" '
            +    'onclick="WorkBoard._openFromCard(event,' + t.id + ')">'
            +   '<div class="wt-card-title">' + esc(t.title || '(无标题)') + '</div>'
            +   '<div class="wt-card-meta">'
            +     WorkTable._avatar(t.assignee || '?')
            +     '<span style="font-size:.8rem;color:var(--text-secondary)">' + esc(t.assignee || '—') + '</span>'
            +     collabHtml
            +     (t.level ? '<span class="wt-pill wt-lv">' + esc(t.level) + '</span>' : '')
            +     (t.freq && t.freq !== '一次性'
                    ? '<span class="wt-pill wt-fq" title="周期任务:完成后自动顺延到下一期">' + esc(Work.cycleChip(t)) + '</span>'
                    : (t.freq ? '<span class="wt-pill wt-fq">' + esc(t.freq) + '</span>' : ''))
            +     descIcon
            +     (t.due ? '<span class="wt-due' + overdue + '" style="font-size:.78rem;margin-left:auto">📅 ' + esc(t.due) + '</span>' : '')
            +   '</div>'
            + '</div>';
    }

    // T-100:卡片空白处单击打开抽屉;落在描述图标等可点元素上 → 跳过。
    function _openFromCard(ev, id) {
        if (typeof WorkDetail === 'undefined') return;
        if (ev && ev.target && ev.target.closest && ev.target.closest('.wt-desc-expand, .wt-pill')) return;
        var ids = _visibleRows().map(function(r) { return r.id; });
        WorkDetail.openDetail(id, ids);
    }

    return { render: render, _openFromCard: _openFromCard };
})();
