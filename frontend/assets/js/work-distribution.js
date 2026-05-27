// ========== WorkDistribution — 工作任务表第 5 视图:分布视图 (T-108) ==========
//
// 顶部维度选择器 + 气泡总览 + 卡片墙(复用 work-person 的形态)。
// 视觉依据:`frontend/work-distribution-preview.html`(PM 单文件设计稿,Boris 已认可)。
// spec § 6.5。
//
// 与人员视图(T-099)的差异:
//   人员视图 = 固定按 assignee 分组;
//   分布视图 = 动态选维度,可按任意 multi / select / status 列分组。
//
// 数据流:
//   Work.columns() 过滤出 multi/select/status 列 → 维度选择器
//   Work.rows() → applyTimeTabFilter → _groupByDim(col) → 渲染气泡 + 卡片
//   multi 字段:一个任务多值时,在每个对应卡片里都出现一次,任务行右侧显示「+N」
//
// 数据隔离:WorkDistribution 不写任何任务字段(没有"拖动改标签"),只读 + 跳转抽屉。

var WorkDistribution = (function() {
    var UNMARKED = '未标记';
    var _curDim = null;  // 当前维度的列 key

    function _esc(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;').replace(/'/g, '&#39;');
    }

    function _todayYMD() {
        var d = new Date();
        return d.getFullYear() + '-' + ('0'+(d.getMonth()+1)).slice(-2) + '-' + ('0'+d.getDate()).slice(-2);
    }
    function _normalizeDue(due) {
        if (!due) return '';
        var s = '' + due;
        if (s.length === 10 && s.charAt(4) === '-' && s.charAt(7) === '-') return s;
        if (s.length === 5 && s.charAt(2) === '-') return (new Date()).getFullYear() + '-' + s;
        return '';
    }
    function _addDaysYMD(ymd, n) {
        var d = new Date(ymd + 'T00:00:00');
        d.setDate(d.getDate() + n);
        return d.getFullYear() + '-' + ('0'+(d.getMonth()+1)).slice(-2) + '-' + ('0'+d.getDate()).slice(-2);
    }
    function _daysBetween(fromY, toY) {
        var a = new Date(fromY + 'T00:00:00');
        var b = new Date(toY + 'T00:00:00');
        return Math.max(1, Math.floor((b - a) / 86400000));
    }
    function _weekEnd(today) {
        var d = new Date(today + 'T00:00:00');
        var day = d.getDay();
        var toSun = (day === 0) ? 0 : 7 - day;
        return _addDaysYMD(today, toSun);
    }
    function _dueLabel(t) {
        if (!t.due) return { text: '—', cls: '' };
        var due = _normalizeDue(t.due);
        var today = _todayYMD();
        if (!due) return { text: ('' + t.due), cls: '' };
        if (t.status !== 'done' && due < today) {
            var n = _daysBetween(due, today);
            return { text: '逾期' + n + '天', cls: 'wt-person-due-overdue' };
        }
        var tmr = _addDaysYMD(today, 1);
        if (due === today) return { text: '今天', cls: 'wt-person-due-soon' };
        if (due === tmr) return { text: '明天', cls: 'wt-person-due-soon' };
        if (due <= _weekEnd(today)) return { text: '本周', cls: '' };
        return { text: due.slice(5), cls: '' };
    }

    // ── 维度:取 multi / select / status 列 ──
    function _getDimensions() {
        return Work.columns().filter(function(c) {
            return c.type === 'multi' || c.type === 'select' || c.type === 'status';
        });
    }

    // ── 取一个任务在维度列下的值(返回数组,空数组 = 未标记) ──
    //   multi:返回该任务的多个值(从 customFields[key] 或 t[key])
    //   select/status:返回单元素数组
    function _getValues(task, col) {
        var key = col.key;
        // 内置列读 task[key];自定义列读 task.customFields[key]
        var raw;
        if (key === 'status' || key === 'priority' || key === 'level' || key === 'freq' || key === 'assignee') {
            raw = task[key];
        } else if (key === 'tags') {
            // 内置「标签」列:可能在 task.tags 直字段(将来扩展)或 customFields.tags 中
            raw = (task.tags != null) ? task.tags : (task.customFields && task.customFields[key]);
        } else {
            raw = (task.customFields && task.customFields[key]);
        }
        if (col.type === 'multi') {
            if (Array.isArray(raw)) {
                return raw.filter(function(v) { return v != null && ('' + v).trim() !== ''; });
            }
            if (typeof raw === 'string' && raw.trim()) {
                // 兼容存为 JSON 字符串的情况
                try {
                    var arr = JSON.parse(raw);
                    if (Array.isArray(arr)) {
                        return arr.filter(function(v) { return v != null && ('' + v).trim() !== ''; });
                    }
                } catch (_) {}
                return [raw];
            }
            return [];
        }
        // single value
        if (raw == null) return [];
        var s = ('' + raw).trim();
        return s ? [s] : [];
    }

    // ── 值的显示 label(status 转中文,优先级转高/中/低) ──
    function _valueLabel(col, value) {
        if (col.type === 'status' && typeof WorkTable !== 'undefined') {
            var s = WorkTable._statusBy(value);
            if (s) return s.label;
        }
        if (col.key === 'priority' && typeof WorkTable !== 'undefined') {
            var p = WorkTable._prioBy(value);
            if (p) return p.label;
        }
        return value;
    }

    // ── 按维度分组:returns [{ name, tasks:[{task,extra}], isUnmarked }] ──
    //   multi:同一任务在每个值都出现一次;extra = 该任务在此维度的总值数 - 1
    function _groupByDim(rows, col) {
        var map = Object.create(null);
        var order = [];   // 维持首次出现顺序,稳定排序
        var unmarked = [];
        rows.forEach(function(t) {
            var values = _getValues(t, col);
            if (values.length === 0) {
                unmarked.push({ task: t, extra: 0 });
                return;
            }
            values.forEach(function(v) {
                var label = _valueLabel(col, v);
                if (!map[label]) {
                    map[label] = [];
                    order.push(label);
                }
                map[label].push({ task: t, extra: values.length - 1 });
            });
        });
        // 默认按任务数降序;并列时按首次出现顺序
        order.sort(function(a, b) {
            var d = map[b].length - map[a].length;
            if (d !== 0) return d;
            return order.indexOf(a) - order.indexOf(b);
        });
        var groups = order.map(function(n) {
            return { name: n, tasks: map[n], isUnmarked: false };
        });
        // 未标记永远置末(spec § 6.5)
        if (unmarked.length > 0) {
            groups.push({ name: UNMARKED, tasks: unmarked, isUnmarked: true });
        }
        return groups;
    }

    // ── 气泡颜色:优先级 逾期 > P0 > 正常 > 仅低优 ──
    function _bubbleClass(g) {
        if (g.isUnmarked) return 'wt-bubble-low';
        var today = _todayYMD();
        var hasOverdue = g.tasks.some(function(it) {
            var t = it.task;
            var due = _normalizeDue(t.due);
            return t.status !== 'done' && due && due < today;
        });
        if (hasOverdue) return 'wt-bubble-overdue';
        var hasP0 = g.tasks.some(function(it) { return it.task.priority === 'high'; });
        if (hasP0) return 'wt-bubble-p0';
        var onlyLow = g.tasks.every(function(it) { return it.task.priority === 'low'; });
        if (onlyLow) return 'wt-bubble-low';
        return 'wt-bubble-normal';
    }

    function _bubbleSize(n) {
        var min = 60, max = 110;
        var t = Math.min(1, Math.log2(n + 1) / 4);
        return Math.round(min + (max - min) * t);
    }

    function _overdueCount(items) {
        var today = _todayYMD();
        return items.filter(function(it) {
            var t = it.task;
            var due = _normalizeDue(t.due);
            return t.status !== 'done' && due && due < today;
        }).length;
    }
    function _p0Count(items) {
        return items.filter(function(it) { return it.task.priority === 'high'; }).length;
    }

    // ============ 主渲染 ============
    function render() {
        var host = document.getElementById('wt-distribution-view');
        if (!host) return;

        var dims = _getDimensions();
        if (dims.length === 0) {
            host.innerHTML = '<div class="wt-empty-state">'
                + '<div class="wt-empty-title">暂无可分组的列</div>'
                + '<div class="wt-empty-sub">请到「⚙ 列设置」添加 multi 或 select 列。</div>'
                + '</div>';
            return;
        }

        // 默认维度:第一个 multi(通常是「标签」),否则第一个 select,再否则 status
        if (!_curDim || !dims.find(function(d) { return d.key === _curDim; })) {
            var firstMulti = dims.find(function(d) { return d.type === 'multi'; });
            _curDim = firstMulti ? firstMulti.key : dims[0].key;
        }
        var col = dims.find(function(d) { return d.key === _curDim; });

        var rows = Work.applyTimeTabFilter(Work.rows());
        var groups = _groupByDim(rows, col);
        var totalTasks = rows.length;
        var realGroups = groups.filter(function(g) { return !g.isUnmarked; });
        var unmarkedG = groups.find(function(g) { return g.isUnmarked; });
        var unmarkedN = unmarkedG ? unmarkedG.tasks.length : 0;
        var isMulti = (col.type === 'multi');

        var dimSegHtml = dims.map(function(d) {
            var active = d.key === _curDim ? ' active' : '';
            return '<button class="wt-dim-btn' + active + '" '
                +    'onclick="WorkDistribution.setDim(\'' + _esc(d.key) + '\')">'
                +    _esc(d.name) + ' <span class="wt-dim-typ">' + d.type + '</span>'
                + '</button>';
        }).join('');

        host.innerHTML =
            '<div class="wt-dim-bar">'
          +   '<span class="wt-dim-lbl">按以下维度看分布:</span>'
          +   '<div class="wt-dim-seg">' + dimSegHtml + '</div>'
          +   '<span class="wt-dim-spacer"></span>'
          +   '<span class="wt-dim-stats">'
          +     '共 <strong>' + totalTasks + '</strong> 任务 · '
          +     '跨 <strong>' + realGroups.length + '</strong> 个 ' + _esc(col.name) + ' · '
          +     '<strong>' + unmarkedN + '</strong> 条未标记'
          +   '</span>'
          + '</div>'
          + '<div class="wt-section-label">分布概览(点气泡跳转到对应卡片)</div>'
          + '<div class="wt-bubble-wrap">'
          +   '<div class="wt-bubble-row" id="wt-dist-bubble-row"></div>'
          +   '<div class="wt-bubble-legend">'
          +     '<span><span class="wt-bubble-dot" style="background:#E11D48"></span>含逾期</span>'
          +     '<span><span class="wt-bubble-dot" style="background:#F59E0B"></span>含 P0</span>'
          +     '<span><span class="wt-bubble-dot" style="background:var(--primary-color)"></span>正常</span>'
          +     '<span><span class="wt-bubble-dot" style="background:#D4D0E0"></span>未标记 / 仅低优</span>'
          +     '<span class="wt-bubble-legend-tail">气泡大小 = 任务数</span>'
          +   '</div>'
          + '</div>'
          + (isMulti
              ? '<div class="wt-multi-note">💡 <strong>multi 字段</strong>:一个任务可有多个' + _esc(col.name) + ',会在每个对应卡片里都出现一次。任务行右上「+N」表示该任务还有 N 个其它' + _esc(col.name) + '。</div>'
              : '')
          + '<div class="wt-section-label">各' + _esc(col.name) + '详情(默认按任务数降序;未标记永远置末)</div>'
          + '<div class="wt-tag-grid" id="wt-dist-tag-grid"></div>';

        _renderBubbles(groups);
        _renderCards(groups, col);
    }

    function _renderBubbles(groups) {
        var row = document.getElementById('wt-dist-bubble-row');
        if (!row) return;
        row.innerHTML = '';
        groups.forEach(function(g) {
            var size = _bubbleSize(g.tasks.length);
            var overdue = _overdueCount(g.tasks);
            var b = document.createElement('div');
            b.className = 'wt-bubble ' + _bubbleClass(g);
            b.style.width = size + 'px';
            b.style.height = size + 'px';
            b.dataset.tag = g.name;
            b.innerHTML = '<div class="wt-b-name">' + _esc(g.name) + '</div>'
                + '<div class="wt-b-count">' + g.tasks.length + '</div>'
                + (overdue ? '<div class="wt-b-badge">⏰ ' + overdue + '</div>' : '');
            b.onclick = function() { _scrollToCard(g.name); };
            row.appendChild(b);
        });
    }

    function _renderCards(groups, col) {
        var grid = document.getElementById('wt-dist-tag-grid');
        if (!grid) return;
        grid.innerHTML = '';
        groups.forEach(function(g) {
            grid.appendChild(_buildCard(g, col));
        });
        _bindCardInteractions();
    }

    function _buildCard(g, col) {
        var card = document.createElement('div');
        card.className = 'wt-tag-card' + (g.isUnmarked ? ' wt-tag-card-unmarked' : '');
        card.dataset.tag = g.name;
        var overdue = _overdueCount(g.tasks);
        var p0 = _p0Count(g.tasks);
        var chipCls = g.isUnmarked ? ' wt-tag-chip-empty'
                    : overdue     ? ' wt-tag-chip-danger'
                    : p0          ? ' wt-tag-chip-warn'   : '';
        var chipPrefix = g.isUnmarked ? '🏷 ' : '#';

        var statsHtml =
            '<span class="wt-tag-stats-total">' + g.tasks.length + ' 任务</span>'
          + (overdue ? '<span class="wt-tag-stats-overdue">⏰ ' + overdue + ' 逾期</span>' : '')
          + (p0      ? '<span class="wt-tag-stats-p0">⚡ ' + p0 + ' P0</span>' : '');

        var taskListHtml = g.tasks.map(function(it) {
            var t = it.task;
            var due = _dueLabel(t);
            var p0Pill = (t.priority === 'high') ? '<span class="wt-person-pill wt-person-pill-p0">P0</span>' : '';
            var doneCls = (t.status === 'done') ? ' wt-person-task-done' : '';
            var extraTags = (it.extra > 0)
                ? '<span class="wt-tag-extra">+' + it.extra + '</span>'
                : '';
            var avatarBg = Work.colorOf(t.assignee || '?');
            var initial = (t.assignee && t.assignee.trim()) ? t.assignee.slice(0, 1) : '?';
            return '<div class="wt-person-task' + doneCls + '" data-tid="' + t.id + '">'
                +    '<div class="wt-person-check" data-tid="' + t.id + '"></div>'
                +    '<div class="wt-person-task-title" data-tid="' + t.id + '">' + _esc(t.title || '(无标题)') + '</div>'
                +    '<div class="wt-person-task-meta">'
                +      '<span class="wt-tag-avatar" style="background:' + avatarBg + '" title="' + _esc(t.assignee || '未指派') + '">' + _esc(initial) + '</span>'
                +      p0Pill
                +      '<span class="wt-person-pill ' + due.cls + '">' + _esc(due.text) + '</span>'
                +      extraTags
                +    '</div>'
                +  '</div>';
        }).join('');

        card.innerHTML =
            '<div class="wt-tag-head">'
          +   '<span class="wt-tag-chip' + chipCls + '">' + chipPrefix + _esc(g.name) + '</span>'
          +   '<div class="wt-tag-stats">' + statsHtml + '</div>'
          + '</div>'
          + '<div class="wt-person-task-list">' + taskListHtml + '</div>';
        return card;
    }

    function _bindCardInteractions() {
        var host = document.getElementById('wt-distribution-view');
        if (!host) return;

        // 事件代理:点 ☐ → 进度弹窗;点标题/任意空白 → 打开抽屉
        host.onclick = function(ev) {
            var check = ev.target.closest('.wt-person-check');
            if (check) {
                ev.stopPropagation();
                if (typeof WorkTable !== 'undefined') {
                    WorkTable.editProgress(+check.dataset.tid, 'progress');
                }
                return;
            }
            var taskRow = ev.target.closest('.wt-person-task');
            if (taskRow) {
                ev.stopPropagation();
                var tid = +taskRow.dataset.tid;
                if (typeof WorkDetail !== 'undefined') {
                    // 取当前过滤后的 id 列表给抽屉做上下导航
                    var ids = Work.applyTimeTabFilter(Work.rows()).map(function(r) { return r.id; });
                    WorkDetail.openDetail(tid, ids);
                } else if (typeof WorkTable !== 'undefined') {
                    WorkTable.editText(tid, 'title');
                }
                return;
            }
        };
    }

    function _scrollToCard(name) {
        // querySelector 对含特殊字符的 attr 要转义;CSS.escape 兼容性好
        var sel = '';
        try {
            sel = '.wt-tag-card[data-tag="' + CSS.escape(name) + '"]';
        } catch (_) {
            sel = '.wt-tag-card[data-tag="' + name.replace(/"/g, '\\"') + '"]';
        }
        var card = document.querySelector(sel);
        if (!card) return;
        card.scrollIntoView({ behavior: 'smooth', block: 'center' });
        card.classList.add('wt-person-highlight');
        setTimeout(function() { card.classList.remove('wt-person-highlight'); }, 1400);
    }

    function setDim(key) {
        _curDim = key;
        render();
    }
    function currentDim() { return _curDim; }

    return {
        render: render,
        setDim: setDim,
        currentDim: currentDim,
    };
})();
