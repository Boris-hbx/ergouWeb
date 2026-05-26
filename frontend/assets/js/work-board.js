// ========== WorkBoard — 工作任务表的看板视图 (T-094) ==========
//
// 按 status 分 4 列:待办 / 进行中 / 阻塞 / 已完成。
// 卡片可拖到目标列,落下后:status 写回;若 status = done 则 progress 也自动 = 100
// (后端 PATCH 时会强制做这件事,前端预先更新只是为了 UI 立即响应)。
//
// 卡片只显示精简元数据(spec § 6.2):标题 / 责任人头像 / 层级 / 频率 / 截止日 / 简介小图标。

var WorkBoard = (function() {
    var _dragId = null;

    function render() {
        var host = document.getElementById('wt-board-view');
        if (!host) return;

        var rows = _visibleRows();
        var STATUS = WorkTable._STATUS;
        host.innerHTML = '';
        STATUS.forEach(function(col) {
            var items = rows.filter(function(t) { return t.status === col.key; });
            var div = document.createElement('div');
            div.className = 'wt-col';
            div.dataset.col = col.key;
            div.innerHTML =
                '<div class="wt-col-head">'
              +   '<span class="wt-pdot" style="width:9px;height:9px;flex:0 0 9px;background:' + col.dot + '"></span>'
              +   col.label
              +   '<span class="wt-count">' + items.length + '</span>'
              + '</div>'
              + '<div class="wt-col-body">'
              +   items.map(_card).join('')
              + '</div>';
            host.appendChild(div);
        });

        // 绑定拖动
        host.querySelectorAll('.wt-card').forEach(function(c) {
            c.addEventListener('dragstart', function() {
                _dragId = +c.dataset.id;
                c.classList.add('dragging');
            });
            c.addEventListener('dragend', function() { c.classList.remove('dragging'); });
        });
        host.querySelectorAll('.wt-col').forEach(function(col) {
            var body = col.querySelector('.wt-col-body');
            col.addEventListener('dragover', function(e) {
                e.preventDefault();
                body.classList.add('drop-hover');
            });
            col.addEventListener('dragleave', function() {
                body.classList.remove('drop-hover');
            });
            col.addEventListener('drop', function(e) {
                e.preventDefault();
                body.classList.remove('drop-hover');
                if (_dragId == null) return;
                var newStatus = col.dataset.col;
                var patch = { status: newStatus };
                if (newStatus === 'done') patch.progress = 100;
                Work.updateRow(_dragId, patch);
                _dragId = null;
            });
        });
    }

    function _visibleRows() {
        var fEl = document.getElementById('wt-filter');
        var f = fEl ? fEl.value : '';
        var rows = Work.rows();
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
        return '<div class="wt-card" draggable="true" data-id="' + t.id + '">'
            +   '<div class="wt-card-title">' + esc(t.title || '(无标题)') + '</div>'
            +   '<div class="wt-card-meta">'
            +     WorkTable._avatar(t.assignee || '?')
            +     '<span style="font-size:.8rem;color:var(--text-secondary)">' + esc(t.assignee || '—') + '</span>'
            +     (t.level ? '<span class="wt-pill wt-lv">' + esc(t.level) + '</span>' : '')
            +     (t.freq  ? '<span class="wt-pill wt-fq">' + esc(t.freq)  + '</span>' : '')
            +     descIcon
            +     (t.due ? '<span class="wt-due' + overdue + '" style="font-size:.78rem;margin-left:auto">📅 ' + esc(t.due) + '</span>' : '')
            +   '</div>'
            + '</div>';
    }

    return { render: render };
})();
