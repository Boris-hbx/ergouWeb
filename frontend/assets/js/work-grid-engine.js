// ========== WorkGridEngine - generic column + row rendering helpers (T-222) ==========
//
// This file is intentionally business-agnostic. WorkTable / WorkBoard /
// WorkDistribution keep task-specific cells, status semantics, and save callbacks.

var WorkGridEngine = (function() {
    function esc(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;').replace(/'/g, '&#39;');
    }

    function renderTable(opts) {
        opts = opts || {};
        var host = opts.host;
        var columns = opts.columns || [];
        var rows = opts.rows || [];
        if (!host) return;

        var serialWidth = opts.serialWidth || 54;
        var span = columns.length + 1;
        var sumW = serialWidth;
        for (var i = 0; i < columns.length; i++) {
            sumW += (parseInt(columns[i].width, 10) || opts.defaultWidth || 130);
        }

        var colgroup = '<colgroup><col style="width:' + serialWidth + 'px">'
            + columns.map(function(c) {
                return '<col style="width:' + (c.width || opts.defaultWidth || 130) + 'px">';
            }).join('')
            + '</colgroup>';

        var thead = typeof opts.headerHtml === 'function'
            ? opts.headerHtml(columns)
            : _defaultHeader(columns);

        var n = 0;
        function block(list) {
            return list.map(function(row) {
                n++;
                return opts.rowHtml(row, n);
            }).join('');
        }

        var body = '';
        if (typeof opts.groupRows === 'function') {
            var groups = opts.groupRows(rows) || [];
            groups.forEach(function(g) {
                body += '<tr class="wt-group-row"><td colspan="' + span + '">' + esc(g.label) + ' · ' + (g.items || []).length + '</td></tr>';
                body += block(g.items || []);
            });
        } else {
            body = block(rows);
        }

        if (typeof opts.addRowHtml === 'function') {
            body += opts.addRowHtml(span);
        }

        host.innerHTML = '<div class="wt-table-scroll">'
            + '<table class="' + (opts.tableClass || 'wt-table') + '" style="width:' + sumW + 'px">'
            + colgroup + thead + '<tbody>' + body + '</tbody></table></div>';
    }

    function _defaultHeader(columns) {
        return '<thead><tr><th class="wt-num-th">#</th>'
            + columns.map(function(c) { return '<th>' + esc(c.name || c.key) + '</th>'; }).join('')
            + '</tr></thead>';
    }

    function renderBoard(opts) {
        opts = opts || {};
        var host = opts.host;
        if (!host) return;
        var columns = opts.columns || [];
        var rows = opts.rows || [];
        var dragState = opts.dragState || {};

        host.innerHTML = '';
        columns.forEach(function(col) {
            if (typeof opts.skipColumn === 'function' && opts.skipColumn(col)) return;
            var items = typeof opts.rowsForColumn === 'function'
                ? (opts.rowsForColumn(rows, col) || [])
                : rows.filter(function(r) { return r[opts.columnKey || 'status'] === col.key; });
            var div = document.createElement('div');
            div.className = opts.columnClass || 'wt-col';
            div.dataset.col = col.key;
            div.innerHTML = (typeof opts.columnHeaderHtml === 'function' ? opts.columnHeaderHtml(col, items) : '')
                + '<div class="' + (opts.columnBodyClass || 'wt-col-body') + '">'
                + items.map(function(row) { return opts.cardHtml(row, col); }).join('')
                + '</div>';
            host.appendChild(div);
        });

        bindBoardDrag({
            host: host,
            dragState: dragState,
            cardSelector: opts.cardSelector || '.wt-card',
            columnSelector: opts.columnSelector || '.wt-col',
            bodySelector: opts.bodySelector || '.wt-col-body',
            onDrop: opts.onDrop,
        });
    }

    function bindBoardDrag(opts) {
        var host = opts.host;
        if (!host) return;
        host.querySelectorAll(opts.cardSelector).forEach(function(card) {
            card.addEventListener('dragstart', function() {
                opts.dragState.id = +card.dataset.id;
                card.classList.add('dragging');
                card.classList.add('wt-dragging');
            });
            card.addEventListener('dragend', function() {
                card.classList.remove('dragging');
                card.classList.remove('wt-dragging');
            });
        });
        host.querySelectorAll(opts.columnSelector).forEach(function(col) {
            var body = col.querySelector(opts.bodySelector);
            col.addEventListener('dragover', function(e) {
                e.preventDefault();
                if (body) body.classList.add('drop-hover');
            });
            col.addEventListener('dragleave', function() {
                if (body) body.classList.remove('drop-hover');
            });
            col.addEventListener('drop', function(e) {
                e.preventDefault();
                if (body) body.classList.remove('drop-hover');
                if (opts.dragState.id == null) return;
                if (typeof opts.onDrop === 'function') {
                    opts.onDrop(opts.dragState.id, col.dataset.col, col);
                }
                opts.dragState.id = null;
            });
        });
    }

    function groupByDimension(rows, col, getValues, valueLabel, unmarkedLabel) {
        var map = Object.create(null);
        var order = [];
        var unmarked = [];
        rows.forEach(function(row) {
            var values = getValues(row, col) || [];
            if (values.length === 0) {
                unmarked.push({ row: row, task: row, extra: 0 });
                return;
            }
            values.forEach(function(v) {
                var label = valueLabel ? valueLabel(col, v) : v;
                if (!map[label]) {
                    map[label] = [];
                    order.push(label);
                }
                map[label].push({ row: row, task: row, extra: values.length - 1 });
            });
        });
        order.sort(function(a, b) {
            var d = map[b].length - map[a].length;
            if (d !== 0) return d;
            return order.indexOf(a) - order.indexOf(b);
        });
        var groups = order.map(function(n) {
            return { name: n, items: map[n], tasks: map[n], isUnmarked: false };
        });
        if (unmarked.length > 0) {
            groups.push({ name: unmarkedLabel || '未标记', items: unmarked, tasks: unmarked, isUnmarked: true });
        }
        return groups;
    }

    function renderBubbles(opts) {
        opts = opts || {};
        var host = opts.host;
        if (!host) return;
        host.innerHTML = '';
        (opts.groups || []).forEach(function(g) {
            var b = document.createElement('div');
            b.className = typeof opts.className === 'function' ? opts.className(g) : 'wt-bubble';
            var size = typeof opts.size === 'function' ? opts.size(g) : 72;
            b.style.width = size + 'px';
            b.style.height = size + 'px';
            b.dataset.tag = g.name;
            b.innerHTML = opts.html(g);
            if (typeof opts.onClick === 'function') b.onclick = function() { opts.onClick(g); };
            host.appendChild(b);
        });
    }

    function renderCards(opts) {
        opts = opts || {};
        var host = opts.host;
        if (!host) return;
        host.innerHTML = '';
        (opts.groups || []).forEach(function(g) {
            host.appendChild(opts.card(g));
        });
    }

    return {
        esc: esc,
        renderTable: renderTable,
        renderBoard: renderBoard,
        bindBoardDrag: bindBoardDrag,
        groupByDimension: groupByDimension,
        renderBubbles: renderBubbles,
        renderCards: renderCards,
    };
})();
