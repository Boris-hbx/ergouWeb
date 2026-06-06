// ========== WorkAttention - area pressure heat table (T-174) ==========

var WorkAttention = (function() {
    var ACTION = { decide: 1, push: 1, do: 1 };
    var UNSET = '未标记';

    function _esc(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;').replace(/'/g, '&#39;');
    }
    function _todayYMD() {
        var d = new Date();
        return d.getFullYear() + '-' + ('0' + (d.getMonth() + 1)).slice(-2) + '-' + ('0' + d.getDate()).slice(-2);
    }
    function _addDays(ymd, n) {
        var d = new Date(ymd + 'T00:00:00');
        d.setDate(d.getDate() + n);
        return d.getFullYear() + '-' + ('0' + (d.getMonth() + 1)).slice(-2) + '-' + ('0' + d.getDate()).slice(-2);
    }
    function _due(t) {
        var due = t && t.due ? '' + t.due : '';
        if (!due) return '';
        if (due.length === 10 && due.charAt(4) === '-' && due.charAt(7) === '-') return due;
        if (due.length === 5 && due.charAt(2) === '-') return (new Date()).getFullYear() + '-' + due;
        return '';
    }
    function _overdue(t) {
        var d = _due(t);
        return t.status !== 'done' && d && d < _todayYMD();
    }
    function _near(t) {
        var d = _due(t);
        if (!d || t.status === 'done') return false;
        var today = _todayYMD();
        return d >= today && d <= _addDays(today, 3);
    }
    function _bucket(n) {
        if (n <= 0) return 'zero';
        if (n <= 2) return 'low';
        if (n <= 5) return 'mid';
        return 'high';
    }
    function _groups() {
        var map = {};
        Work.rows().forEach(function(t) {
            if (t.status === 'done') return;
            var area = (t.area || '').trim() || UNSET;
            if (!map[area]) map[area] = { area: area, total: 0, action: 0, overdue: 0, p0: 0, stale: 0 };
            var g = map[area];
            g.total++;
            if (ACTION[t.engage]) g.action++;
            if (_overdue(t)) g.overdue++;
            if (t.priority === 'high') g.p0++;
            if (t.engage === 'track' && (_overdue(t) || _near(t))) g.stale++;
        });
        return Object.keys(map).map(function(k) { return map[k]; }).sort(function(a, b) {
            var ap = a.action + a.overdue * 2 + a.p0 + a.stale;
            var bp = b.action + b.overdue * 2 + b.p0 + b.stale;
            if (ap !== bp) return bp - ap;
            return a.area.localeCompare(b.area);
        });
    }
    function _cell(area, metric, n) {
        return '<td class="wt-att-cell wt-att-' + _bucket(n) + '" onclick="WorkAttention.drill(\'' + _esc(area) + '\',\'' + metric + '\')">' + n + '</td>';
    }
    function render() {
        var host = document.getElementById('wt-attention-view');
        if (!host) return;
        var groups = _groups();
        var rows = groups.map(function(g) {
            return '<tr>'
                + '<th onclick="WorkAttention.drill(\'' + _esc(g.area) + '\',\'all\')">' + _esc(g.area) + '</th>'
                + _cell(g.area, 'action', g.action)
                + _cell(g.area, 'overdue', g.overdue)
                + _cell(g.area, 'p0', g.p0)
                + _cell(g.area, 'stale', g.stale)
                + '<td class="wt-att-total">' + g.total + '</td>'
                + '</tr>';
        }).join('');
        host.innerHTML = '<div class="wt-att-wrap">'
            + '<table class="wt-att-table">'
            + '<thead><tr><th>领域</th><th>待我动作</th><th>逾期</th><th>P0</th><th>跟进飘了</th><th>总数</th></tr></thead>'
            + '<tbody>' + (rows || '<tr><td colspan="6" class="wt-center-empty">暂无工作任务。</td></tr>') + '</tbody>'
            + '</table>'
            + '</div>';
    }
    function drill(area, metric) {
        if (typeof WorkTable === 'undefined') return;
        WorkTable.clearAllFilters();
        if (area && area !== UNSET) WorkTable.setColumnFilter('area', [area]);
        else WorkTable.setColumnFilter('area', ['']);
        if (metric === 'action') WorkTable.setColumnFilter('engage', ['decide', 'push', 'do']);
        else if (metric === 'p0') WorkTable.setColumnFilter('priority', ['high']);
        else if (metric === 'stale') WorkTable.setColumnFilter('engage', ['track']);
        Work.setTimeTab(metric === 'overdue' ? 'today' : 'all');
        Work.setView('table');
    }
    return { render: render, drill: drill };
})();
