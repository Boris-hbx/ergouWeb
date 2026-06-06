// ========== WorkCenter - owner zhongshu view (T-174) ==========

var WorkCenter = (function() {
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
    function _normDue(t) {
        var due = t && t.due ? '' + t.due : '';
        if (!due) return '';
        if (due.length === 10 && due.charAt(4) === '-' && due.charAt(7) === '-') return due;
        if (due.length === 5 && due.charAt(2) === '-') return (new Date()).getFullYear() + '-' + due;
        return '';
    }
    function _isOverdue(t) {
        var due = _normDue(t);
        return t.status !== 'done' && due && due < _todayYMD();
    }
    function _isNear(t, days) {
        var due = _normDue(t);
        if (!due || t.status === 'done') return false;
        var today = _todayYMD();
        return due >= today && due <= _addDays(today, days);
    }
    function _isRisk(t) {
        return _isOverdue(t) || _isNear(t, 2);
    }
    function _isStaleTrack(t) {
        return t.engage === 'track' && (_isOverdue(t) || (_isNear(t, 3) && t.status !== 'doing'));
    }
    function _isDoAction(t) {
        return t.engage === 'do' && (t.priority === 'high' || _isRisk(t));
    }
    function _sort(a, b) {
        var ao = _isOverdue(a) ? 1 : 0, bo = _isOverdue(b) ? 1 : 0;
        if (ao !== bo) return bo - ao;
        var ap = a.priority === 'high' ? 1 : 0, bp = b.priority === 'high' ? 1 : 0;
        if (ap !== bp) return bp - ap;
        return (_normDue(a) || '9999-99-99').localeCompare(_normDue(b) || '9999-99-99');
    }
    function _role(t) {
        return (typeof WorkTable !== 'undefined' && WorkTable._engageBy) ? WorkTable._engageBy(t.engage) : { label: t.engage || '选择', cls: '' };
    }
    function _riskLabel(t) {
        if (_isOverdue(t)) return '逾期';
        if (_isRisk(t)) return '临期';
        return '';
    }
    function _open(id, ids) {
        if (typeof WorkDetail !== 'undefined') WorkDetail.openDetail(id, ids || [id]);
    }
    function _card(t, ids) {
        var r = _role(t);
        var risk = _riskLabel(t);
        var due = _normDue(t);
        var p0 = t.priority === 'high' ? '<span class="wt-person-pill wt-person-pill-p0">P0</span>' : '';
        return '<div class="wt-center-card" data-id="' + t.id + '" onclick="WorkCenter.open(' + t.id + ',[' + ids.join(',') + '])">'
            + '<div class="wt-center-title">' + _esc(t.title || '(无标题)') + '</div>'
            + '<div class="wt-center-meta">'
            +   '<span class="wt-pill ' + _esc(r.cls || '') + '" onclick="event.stopPropagation();WorkCenter.pickEngage(' + t.id + ',this)">' + _esc(r.label) + '</span>'
            +   (t.area ? '<span class="wt-person-pill">' + _esc(t.area) + '</span>' : '')
            +   p0
            +   (risk ? '<span class="wt-person-pill wt-person-due-overdue">' + risk + '</span>' : '')
            +   (due ? '<span class="wt-person-pill">' + _esc(due.slice(5)) + '</span>' : '')
            + '</div>'
            + '</div>';
    }
    function _section(title, rows, empty, cls) {
        rows = rows.slice().sort(_sort);
        var ids = rows.map(function(t) { return t.id; });
        return '<section class="wt-center-section ' + (cls || '') + '">'
            + '<div class="wt-center-section-head"><span>' + _esc(title) + '</span><b>' + rows.length + '</b></div>'
            + (rows.length ? '<div class="wt-center-list">' + rows.map(function(t) { return _card(t, ids); }).join('') + '</div>'
                           : '<div class="wt-center-empty">' + _esc(empty) + '</div>')
            + '</section>';
    }
    function _actionBlock(groups) {
        var total = groups.decide.length + groups.push.length + groups.do.length;
        return '<section class="wt-center-section wt-center-action">'
            + '<div class="wt-center-section-head"><span>需要我动作</span><b>' + total + '</b></div>'
            + '<div class="wt-center-subsections">'
            + _section('待我决策', groups.decide, '当前无待你决策。', 'wt-center-sub wt-center-decide')
            + _section('等我推动', groups.push, '当前无等你推动。', 'wt-center-sub wt-center-push')
            + _section('我亲自做·临期', groups.do, '没有需你亲自处理的临期或 P0 任务。', 'wt-center-sub wt-center-do')
            + '</div>'
            + '</section>';
    }
    function render() {
        var host = document.getElementById('wt-center-view');
        if (!host) return;
        var rows = Work.rows().filter(function(t) { return t.status !== 'done'; });
        var seenRisk = {};
        var decide = rows.filter(function(t) { return t.engage === 'decide'; });
        var push = rows.filter(function(t) { return t.engage === 'push'; });
        var doRows = rows.filter(_isDoAction);
        var trackVisible = rows.filter(_isStaleTrack);
        decide.concat(push, doRows, trackVisible).forEach(function(t) { seenRisk[t.id] = 1; });
        var risk = rows.filter(function(t) { return _isRisk(t) && !seenRisk[t.id]; });
        var trackFolded = rows.filter(function(t) { return t.engage === 'track' && !_isStaleTrack(t); }).length;
        var informCount = rows.filter(function(t) { return t.engage === 'inform'; }).length;
        var actionCount = decide.length + push.length + doRows.length;

        host.innerHTML = '<div class="wt-center-wrap">'
            + '<div class="wt-center-summary">'
            +   '<button class="wt-primary" onclick="WorkTable.addRow()">+ 新建任务</button>'
            +   '<span>需要动作 <b>' + actionCount + '</b></span>'
            +   '<span>风险 <b>' + risk.length + '</b></span>'
            +   '<span>跟进折叠 <b>' + trackFolded + '</b></span>'
            +   '<span>知会 <b>' + informCount + '</b></span>'
            + '</div>'
            + _actionBlock({ decide: decide, push: push, do: doRows })
            + _section('风险条', risk, '没有额外逾期或临期风险。', 'wt-center-risk')
            + _section('只需我知道', trackVisible, '跟进项已折叠；知会项只计数。', 'wt-center-info')
            + '</div>';
    }
    function pickEngage(id, anchor) {
        var t = Work.rowById(id);
        if (!t || typeof WorkPick === 'undefined') return;
        var opts = (WorkTable._ENGAGE || []).map(function(e) { return { key: e.key, label: e.label }; });
        WorkPick.open({
            anchor: anchor,
            options: opts,
            current: t.engage ? [t.engage] : [],
            isMulti: false,
            onConfirm: function(chosen) { Work.updateRow(id, { engage: chosen[0] || '' }); },
        });
    }
    return { render: render, open: _open, pickEngage: pickEngage };
})();
