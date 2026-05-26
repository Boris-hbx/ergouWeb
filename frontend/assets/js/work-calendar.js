// ========== WorkCalendar — 工作任务表的月历视图 (T-094) ==========
//
// 按 due_date 把任务排到月历格子。MVP 只做当前月(spec § 6.3)。
// 每格按周一开始,状态字符(首字)作前缀。
//
// due 字段允许两种格式:
//   - 完整 'YYYY-MM-DD'(后端规范)
//   - 简写 'MM-DD'(HTML 预览数据风格)
// 月历只比较 MM-DD 部分。

var WorkCalendar = (function() {
    function render() {
        var host = document.getElementById('wt-cal-view');
        if (!host) return;

        var rows = _visibleRows();
        var now = new Date();
        var year = now.getFullYear();
        var month = now.getMonth() + 1;  // 1..12
        var firstDay = new Date(year, month - 1, 1);
        var lastDay  = new Date(year, month, 0);
        var leadJsDay = firstDay.getDay();   // 0=周日, 1=周一, ...
        var lead = (leadJsDay + 6) % 7;       // 周一起算,前面补几个空格
        var days = lastDay.getDate();
        var today = _todayMD(now);

        var dow = ['一', '二', '三', '四', '五', '六', '日'];
        var html = '<div class="wt-cal-month-label">' + year + ' 年 ' + month + ' 月 · 按截止日</div>'
                 + '<div class="wt-cal">';
        dow.forEach(function(d) { html += '<div class="wt-cal-dow">' + d + '</div>'; });
        for (var i = 0; i < lead; i++) html += '<div class="wt-cal-cell dim"></div>';
        for (var d = 1; d <= days; d++) {
            var mm = ('0' + month).slice(-2);
            var dd = ('0' + d).slice(-2);
            var key = mm + '-' + dd;
            var todayCls = (key === today) ? ' wt-cal-today' : '';
            html += '<div class="wt-cal-cell' + todayCls + '">'
                 +    '<div class="wt-cal-num">' + d + '</div>';
            rows.filter(function(t) { return _matchesDay(t.due, mm, dd); })
                .forEach(function(t) {
                    var s = WorkTable._statusBy(t.status);
                    html += '<div class="wt-cal-task" title="' + WorkTable._esc(t.title) + '">'
                         +    WorkTable._esc(s.label.charAt(0)) + ' ' + WorkTable._esc(t.title)
                         +  '</div>';
                });
            html += '</div>';
        }
        html += '</div>';
        host.innerHTML = html;
    }

    function _matchesDay(due, mm, dd) {
        if (!due) return false;
        var s = '' + due;
        // 取末尾 5 位作为 MM-DD,适配 'YYYY-MM-DD' 和 'MM-DD' 两种格式
        var mmdd = s.length >= 5 ? s.slice(-5) : '';
        return mmdd === (mm + '-' + dd);
    }

    function _visibleRows() {
        var fEl = document.getElementById('wt-filter');
        var f = fEl ? fEl.value : '';
        var rows = Work.rows();
        if (!f) return rows;
        return rows.filter(function(t) { return t.assignee === f; });
    }

    function _todayMD(now) {
        var d = now || new Date();
        return ('0' + (d.getMonth() + 1)).slice(-2) + '-' + ('0' + d.getDate()).slice(-2);
    }

    return { render: render };
})();
