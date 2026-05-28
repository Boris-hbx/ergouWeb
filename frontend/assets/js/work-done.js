// ========== WorkDone — 工作 Hub 第 3 张卡「已完成档案」(T-111) ==========
//
// 复盘归档区,按月分组的已完成任务列表;独立于任务表,只读语义(要改回到任务表改 status)。
// 数据源:`Work.rows()`(共享 Work 模块的数据集),前端过滤 `status === 'done' || progress === 100`。
// 完成日期 MVP 用 `updated_at + status='done'` 近似(Phase 2 加 completed_at 字段或事件溯源)。
//
// spec § 附录 C.1 + § B.-1 动效铁律:
//   大动作(进入页面 / 月份折叠展开 / 切时间窗口)→ 允许 spring 过渡 + stagger(克制)
//   小动作(筛选下拉变化)→ 静默更新(无动效)
//
// 与全局时间镜头 Tab 解耦:有自己的时间窗口(本月/上月/近3月/今年/全部)。
// 是未来周月报模块的数据源——查询 + 聚合逻辑写得清楚,周报令复用此处。

var WorkDone = (function() {
    var _loaded = false;
    var _filter = { window: 'month', assignee: '', tag: '' };
    // monthKey('2026-05') → 'open' | 'closed';未定义 = 用默认(最新月展开,其它折叠)
    var _monthStates = {};
    // T-111 + B.-1:仅在切窗口/首次加载/折叠展开时触发 stagger;切责任人/标签下拉静默
    var _renderStagger = false;

    function _esc(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }
    function _todayYMD() {
        var d = new Date();
        return d.getFullYear() + '-' + String(d.getMonth() + 1).padStart(2, '0') + '-' + String(d.getDate()).padStart(2, '0');
    }
    function _ymd(d) {
        return d.getFullYear() + '-' + String(d.getMonth() + 1).padStart(2, '0') + '-' + String(d.getDate()).padStart(2, '0');
    }
    function _firstOfMonth(y, m) {
        return y + '-' + String(m + 1).padStart(2, '0') + '-01';
    }
    function _lastOfMonth(y, m) {
        return _ymd(new Date(y, m + 1, 0));
    }
    function _monthLabel(key) {
        var parts = key.split('-');
        return parts[0] + ' 年 ' + parseInt(parts[1], 10) + ' 月';
    }

    // ============ 路由切换 ============
    function openFeature() {
        var hub = document.getElementById('work-hub');
        var tableView = document.getElementById('work-table-view');
        var insView = document.getElementById('work-insight-view');
        var doneView = document.getElementById('work-done-view');
        if (hub) hub.style.display = 'none';
        if (tableView) tableView.style.display = 'none';
        if (insView) insView.style.display = 'none';
        if (doneView) doneView.style.display = '';
        _ensureLoaded().then(function() {
            _renderStagger = true;  // 进入页面 = 大动作
            render();
            _renderStagger = false;
        });
    }
    function back() {
        var hub = document.getElementById('work-hub');
        var doneView = document.getElementById('work-done-view');
        if (doneView) doneView.style.display = 'none';
        if (hub) hub.style.display = '';
        // 关抽屉(如果开着)
        if (typeof WorkDetail !== 'undefined' && WorkDetail.isOpen && WorkDetail.isOpen()) {
            WorkDetail.closeDetail();
        }
    }

    async function _ensureLoaded() {
        // 共享 Work 模块的数据集,避免重复网络请求
        if (typeof Work === 'undefined') return;
        // 如果 Work 还没加载过,触发一次 reload
        if (!Work.rows() || Work.rows().length === 0) {
            try { await Work.reload(); } catch (_) {}
        }
        _loaded = true;
    }

    // 外部刷新触发(在工作任务表改了 status 后回到归档区时调)
    function refresh() {
        render();
    }

    // ============ 过滤 + 分组 ============
    function _filterRows() {
        if (typeof Work === 'undefined' || !Work.rows) return [];
        var rows = Work.rows().filter(function(t) {
            return t.status === 'done' || t.progress === 100;
        });
        var range = _windowRange();
        if (range) {
            rows = rows.filter(function(t) {
                var d = (t.updatedAt || '').slice(0, 10);
                return d >= range.start && d <= range.end;
            });
        }
        if (_filter.assignee) {
            rows = rows.filter(function(t) { return t.assignee === _filter.assignee; });
        }
        if (_filter.tag) {
            rows = rows.filter(function(t) {
                return Array.isArray(t.tags) && t.tags.indexOf(_filter.tag) >= 0;
            });
        }
        return rows;
    }

    function _windowRange() {
        var today = _todayYMD();
        var d = new Date(today + 'T00:00:00');
        var y = d.getFullYear(), m = d.getMonth();
        switch (_filter.window) {
            case 'month':
                return { start: _firstOfMonth(y, m), end: _lastOfMonth(y, m) };
            case 'prevMonth': {
                var pm = (m === 0) ? { y: y - 1, m: 11 } : { y: y, m: m - 1 };
                return { start: _firstOfMonth(pm.y, pm.m), end: _lastOfMonth(pm.y, pm.m) };
            }
            case 'last3m': {
                var startD = new Date(y, m - 2, 1);
                return { start: _ymd(startD), end: _lastOfMonth(y, m) };
            }
            case 'year':
                return { start: y + '-01-01', end: y + '-12-31' };
            case 'all':
            default:
                return null;
        }
    }

    function _groupByMonth(rows) {
        var byMonth = {};
        rows.forEach(function(t) {
            var key = (t.updatedAt || '').slice(0, 7);
            if (!key) return;
            (byMonth[key] = byMonth[key] || []).push(t);
        });
        // 月内按 updated_at 降序(完成日期新→旧)
        Object.keys(byMonth).forEach(function(k) {
            byMonth[k].sort(function(a, b) {
                return (b.updatedAt || '').localeCompare(a.updatedAt || '');
            });
        });
        return Object.keys(byMonth).sort().reverse().map(function(k) {
            return { month: k, tasks: byMonth[k] };
        });
    }

    function _isCollapsed(month, isTop) {
        var state = _monthStates[month];
        if (state === 'open') return false;
        if (state === 'closed') return true;
        return !isTop;  // 默认:首月展开,其它折叠
    }

    function _uniqueAssignees() {
        if (typeof Work === 'undefined' || !Work.rows) return [];
        var s = {};
        Work.rows().forEach(function(t) {
            if ((t.status === 'done' || t.progress === 100) && t.assignee) s[t.assignee] = 1;
        });
        return Object.keys(s).sort();
    }
    function _uniqueTags() {
        var s = {};
        if (typeof Work === 'undefined' || !Work.rows) return [];
        Work.rows().forEach(function(t) {
            if ((t.status === 'done' || t.progress === 100) && Array.isArray(t.tags)) {
                t.tags.forEach(function(tag) { if (tag) s[tag] = 1; });
            }
        });
        return Object.keys(s).sort();
    }

    // ============ 渲染 ============
    function render() {
        var host = document.getElementById('work-done-view');
        if (!host) return;
        var rows = _filterRows();
        var groups = _groupByMonth(rows);
        var allAssignees = _uniqueAssignees();
        var allTags = _uniqueTags();

        host.innerHTML = ''
          + '<button class="wt-back-btn" onclick="WorkDone.back()">&larr; 工作</button>'
          + '<div class="wd-header">'
          +   '<h2>📦 已完成档案</h2>'
          +   '<span class="wd-total">共 <strong>' + rows.length + '</strong> 件</span>'
          +   '<span class="wd-spacer"></span>'
          +   '<span class="wd-hint">只读 · 要改回到任务表</span>'
          + '</div>'
          + '<div class="wd-filters">'
          +   _windowSegHtml()
          +   '<span class="wd-fil-spacer"></span>'
          +   '<select class="wd-select" onchange="WorkDone.setAssignee(this.value)">'
          +     '<option value="">全部责任人 (' + allAssignees.length + ')</option>'
          +     allAssignees.map(function(a) {
                  var sel = a === _filter.assignee ? ' selected' : '';
                  return '<option value="' + _esc(a) + '"' + sel + '>' + _esc(a) + '</option>';
                }).join('')
          +   '</select>'
          +   '<select class="wd-select" onchange="WorkDone.setTag(this.value)">'
          +     '<option value="">全部标签 (' + allTags.length + ')</option>'
          +     allTags.map(function(tag) {
                  var sel = tag === _filter.tag ? ' selected' : '';
                  return '<option value="' + _esc(tag) + '"' + sel + '>' + _esc(tag) + '</option>';
                }).join('')
          +   '</select>'
          + '</div>'
          + _renderGroups(groups);
    }

    function _windowSegHtml() {
        var opts = [
            ['month',     '本月'],
            ['prevMonth', '上月'],
            ['last3m',    '近 3 月'],
            ['year',      '今年'],
            ['all',       '全部'],
        ];
        return '<div class="wd-window-seg">'
          + opts.map(function(o) {
              var active = _filter.window === o[0] ? ' active' : '';
              return '<button class="wd-win-btn' + active + '" onclick="WorkDone.setWindow(\'' + o[0] + '\')">' + o[1] + '</button>';
            }).join('')
          + '</div>';
    }

    function _renderGroups(groups) {
        if (groups.length === 0) {
            return '<div class="wd-empty">'
              + '<div class="wd-empty-icon">📭</div>'
              + '<div class="wd-empty-title">这个时间窗口没有完成的任务</div>'
              + '<div class="wd-empty-sub">换个时间窗口或去任务表完成几件再来</div>'
              + '</div>';
        }
        var withStagger = _renderStagger;
        return groups.map(function(g, i) {
            var collapsed = _isCollapsed(g.month, i === 0);
            var label = _monthLabel(g.month);
            var listHtml = collapsed
                ? ''
                : '<div class="wd-task-list">'
                    + g.tasks.map(function(t, n) { return _taskHtml(t, n, withStagger); }).join('')
                  + '</div>';
            return '<div class="wd-month' + (collapsed ? ' collapsed' : '') + '">'
              + '<div class="wd-month-head" onclick="WorkDone.toggleMonth(\'' + g.month + '\')">'
              +   '<span class="wd-month-caret">' + (collapsed ? '▶' : '▼') + '</span>'
              +   '<span class="wd-month-label">' + label + '</span>'
              +   '<span class="wd-month-dot">·</span>'
              +   '<span class="wd-month-count">完成 ' + g.tasks.length + ' 件</span>'
              + '</div>'
              + listHtml
              + '</div>';
        }).join('');
    }

    function _taskHtml(t, idx, withStagger) {
        var prioCls = (t.priority === 'high') ? 'wd-p-p0' : (t.priority === 'low' ? 'wd-p-low' : 'wd-p-mid');
        var prioLabel = (t.priority === 'high') ? 'P0' : (t.priority === 'low' ? '低' : '中');
        var doneDate = (t.updatedAt || '').slice(0, 10);
        var displayDate = doneDate ? doneDate.slice(5) : '?';
        var assigneeAvatar = '';
        if (t.assignee && typeof Work !== 'undefined' && Work.colorOf) {
            var initial = (t.assignee === '自己' || t.assignee === '我') ? '我' : t.assignee.slice(0, 1);
            assigneeAvatar = '<span class="wd-avatar" style="background:' + Work.colorOf(t.assignee) + '" title="' + _esc(t.assignee) + '">' + _esc(initial) + '</span>';
        } else if (!t.assignee) {
            assigneeAvatar = '<span class="wd-avatar wd-avatar-empty" title="未指派">?</span>';
        }
        var entering = withStagger ? ' wd-task-entering' : '';
        var styleAttr = withStagger
            ? ' style="animation-delay:' + (Math.min(idx, 20) * 28) + 'ms"'
            : '';
        // T-119:协作者头像组(<=2,超出 +N)
        var collabs = Array.isArray(t.collaborators) ? t.collaborators : [];
        var collabHtml = '';
        if (collabs.length > 0 && typeof Work !== 'undefined' && Work.colorOf) {
            var shown = collabs.slice(0, 2);
            var extra = collabs.length - shown.length;
            collabHtml = '<span class="wt-collab-stack" title="协作者:' + _esc(collabs.join('、')) + '">'
                + shown.map(function(c) {
                    return '<span class="wt-avatar wt-avatar-xs" style="background:' + Work.colorOf(c) + '">' + _esc(('' + c).slice(0, 1)) + '</span>';
                  }).join('')
                + (extra > 0 ? '<span class="wt-collab-more">+' + extra + '</span>' : '')
                + '</span>';
        }
        return '<div class="wd-task' + entering + '"' + styleAttr + ' onclick="WorkDone.openDetail(' + t.id + ')">'
          +   '<span class="wd-task-title">' + _esc(t.title || '(无标题)') + '</span>'
          +   '<span class="wd-task-meta">'
          +     assigneeAvatar
          +     collabHtml
          +     '<span class="wd-pill ' + prioCls + '">' + prioLabel + '</span>'
          +     (Array.isArray(t.tags) && t.tags.length
              ? '<span class="wd-tags-mini">' + t.tags.slice(0, 2).map(function(tg) { return '#' + _esc(tg); }).join(' ') + (t.tags.length > 2 ? ' +' + (t.tags.length - 2) : '') + '</span>'
              : '')
          +     '<span class="wd-date">' + displayDate + ' 完成</span>'
          +   '</span>'
          + '</div>';
    }

    // ============ 操作 ============
    // 切窗口 = 大动作 → stagger
    function setWindow(w) {
        if (_filter.window === w) return;
        _filter.window = w;
        _renderStagger = true;
        render();
        _renderStagger = false;
    }
    // 切下拉筛选 = 小动作 → 静默
    function setAssignee(a) {
        _filter.assignee = a || '';
        render();   // 无 stagger
    }
    function setTag(t) {
        _filter.tag = t || '';
        render();
    }
    // 折叠展开 = 大动作 → stagger 入场(展开时)
    function toggleMonth(month) {
        var prev = _monthStates[month];
        var isCurrentlyCollapsed;
        // 默认逻辑:首月展开,其它折叠
        var groups = _groupByMonth(_filterRows());
        var topMonth = groups[0] && groups[0].month;
        if (prev === 'open') isCurrentlyCollapsed = false;
        else if (prev === 'closed') isCurrentlyCollapsed = true;
        else isCurrentlyCollapsed = (month !== topMonth);
        _monthStates[month] = isCurrentlyCollapsed ? 'open' : 'closed';
        // 展开时 stagger;折叠不需要(直接消失)
        _renderStagger = (_monthStates[month] === 'open');
        render();
        _renderStagger = false;
    }

    // 点任务条 → 打开 T-100 抽屉(只读语义,但抽屉里 status 还能改 → 自然回流到任务表)
    function openDetail(id) {
        if (typeof WorkDetail === 'undefined') return;
        var rows = _filterRows();
        var ids = rows.map(function(t) { return t.id; });
        WorkDetail.openDetail(id, ids);
    }

    return {
        openFeature: openFeature,
        back: back,
        refresh: refresh,
        setWindow: setWindow,
        setAssignee: setAssignee,
        setTag: setTag,
        toggleMonth: toggleMonth,
        openDetail: openDetail,
    };
})();
