// ========== WorkCalendar — 工作任务表的月历视图 (T-094 / T-102) ==========
//
// 按 due_date 把任务排到月历格子。MVP 只做当前月(spec § 6.3)。
// 每格按周一开始,状态字符(首字)作前缀。
//
// due 字段允许两种格式:
//   - 完整 'YYYY-MM-DD'(后端规范)
//   - 简写 'MM-DD'(HTML 预览数据风格,自动补当前年)
// 月历内部 cell 数据 attr 一律存全 YMD,drop 写回也用全 YMD。
//
// T-102:任务条 draggable;拖到另一日期格 → PATCH `due` 改截止日。
//   - dim 格(上月末预览)不接收 drop
//   - 拖到过去日期需 confirm
//   - 拖到原位置直接 return,不发请求
//   - 原 click 打开侧拉抽屉(T-100)行为保留(HTML5 drag 默认不触发 click)
//   - drag 模式复用 work-board.js(同一 _drag 状态对象;CSS 同一 hover class 思路)

var WorkCalendar = (function() {
    var _drag = null;   // { id, sourceYmd }

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
        var todayYmd = _todayYmd(now);

        // T-120:月头统计本月任务数(含 done);月头展示「N 件(含 X 已完成)」
        var monthTotal = 0;
        var monthDone = 0;
        rows.forEach(function(t) {
            if (!t.due) return;
            var s = '' + t.due;
            var mmdd = s.length >= 5 ? s.slice(-5) : '';
            var mmStr = ('0' + month).slice(-2);
            if (mmdd.slice(0, 2) === mmStr) {
                monthTotal++;
                if (t.status === 'done') monthDone++;
            }
        });
        var monthLabel = year + ' 年 ' + month + ' 月 · 共 ' + monthTotal + ' 件'
            + (monthDone > 0 ? '(含 ' + monthDone + ' 已完成)' : '')
            + ' · 按截止日(可拖任务条改日期)';

        var dow = ['一', '二', '三', '四', '五', '六', '日'];
        var html = '<div class="wt-cal-month-label">' + monthLabel + '</div>'
                 + '<div class="wt-cal">';
        dow.forEach(function(d) { html += '<div class="wt-cal-dow">' + d + '</div>'; });
        // 上月末填充(dim 格,不接收 drop)
        for (var i = 0; i < lead; i++) html += '<div class="wt-cal-cell dim"></div>';
        for (var d = 1; d <= days; d++) {
            var mm = ('0' + month).slice(-2);
            var dd = ('0' + d).slice(-2);
            var ymd = year + '-' + mm + '-' + dd;
            var key = mm + '-' + dd;
            var todayCls = (key === today) ? ' wt-cal-today' : '';
            // T-102:每个有效日期格挂 data-cell-date(全 YMD)+ drop 事件;dim 格不挂。
            html += '<div class="wt-cal-cell' + todayCls + '" data-cell-date="' + ymd + '" '
                 +    'ondragover="WorkCalendar._onDragOver(event,this)" '
                 +    'ondragleave="WorkCalendar._onDragLeave(event,this)" '
                 +    'ondrop="WorkCalendar._onDrop(event,this)">'
                 +    '<div class="wt-cal-num">' + d + '</div>';
            rows.filter(function(t) { return _matchesDay(t.due, mm, dd); })
                .forEach(function(t) {
                    var s = WorkTable._statusBy(t.status);
                    var isDone = (t.status === 'done');
                    // T-120:进度小环(16×16,复用 .progress-ring;已完成 = 实心主色 + ✓)
                    var progress = Math.max(0, Math.min(100, +t.progress || 0));
                    var ringCls = isDone ? 'progress-ring progress-ring-mini progress-ring-done' : 'progress-ring progress-ring-mini';
                    var ringInner = isDone ? '✓' : '';
                    var ringHtml = '<span class="' + ringCls + '" style="--progress:' + progress + '">'
                        + '<span class="progress-ring-text">' + ringInner + '</span>'
                        + '</span>';
                    // T-119 / T-121:主首字 + 协作者数;UI 不强调主+协区分
                    var collabs = Array.isArray(t.collaborators) ? t.collaborators : [];
                    var aPrefix = t.assignee ? WorkTable._esc(t.assignee.charAt(0)) : '';
                    var collabSuffix = collabs.length > 0 ? '<span style="opacity:0.65">+' + collabs.length + '</span>' : '';
                    var nameTag = (aPrefix || collabs.length > 0)
                        ? '<span style="color:var(--primary-color);font-weight:600;margin-right:3px">' + aPrefix + collabSuffix + '</span>'
                        : '';
                    var titleAttr = t.title
                        + (collabs.length ? ' (' + (t.assignee || '?') + '、' + collabs.join('、') + ')' : '')
                        + (isDone ? ' [已完成]' : (progress > 0 ? ' [' + progress + '%]' : ''));
                    var doneCls = isDone ? ' wt-cal-task-done' : '';
                    // T-100:日历项单击打开详情抽屉
                    // T-102:加 draggable + dragstart/dragend(同一元素同时支持点和拖,HTML5 不冲突)
                    html += '<div class="wt-cal-task' + doneCls + '" data-id="' + t.id + '" '
                         +    'draggable="true" '
                         +    'ondragstart="WorkCalendar._onDragStart(event,' + t.id + ')" '
                         +    'ondragend="WorkCalendar._onDragEnd(event)" '
                         +    'onclick="WorkCalendar._openDetail(' + t.id + ')" '
                         +    'title="' + WorkTable._esc(titleAttr) + '">'
                         +    ringHtml + nameTag + WorkTable._esc(t.title)
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
        // T-120:日历显式走 applyTimeTabFilter + opts.includeDone=true,opt-out T-113 全局过滤 done
        //   日历是"时序视角"(看历史/现在/未来),与表格/看板/人员/分布的"待办视角"语义不同
        //   与 T-111 已完成档案互补(档案=月度复盘列表,日历=某天看那天)
        var rows = Work.applyTimeTabFilter(Work.rows(), { includeDone: true });
        var fEl = document.getElementById('wt-filter');
        var f = fEl ? fEl.value : '';
        if (!f) return rows;
        return rows.filter(function(t) { return t.assignee === f; });
    }

    function _todayMD(now) {
        var d = now || new Date();
        return ('0' + (d.getMonth() + 1)).slice(-2) + '-' + ('0' + d.getDate()).slice(-2);
    }
    function _todayYmd(now) {
        var d = now || new Date();
        return d.getFullYear() + '-' + ('0' + (d.getMonth() + 1)).slice(-2) + '-' + ('0' + d.getDate()).slice(-2);
    }

    // 把 due 规范化成 'YYYY-MM-DD',兼容 'MM-DD' 简写(补当前年)
    function _normalizeYmd(due, year) {
        if (!due) return '';
        var s = '' + due;
        if (s.length === 10 && s.charAt(4) === '-' && s.charAt(7) === '-') return s;
        if (s.length === 5 && s.charAt(2) === '-') return year + '-' + s;
        return s;
    }

    // T-100:从日历项打开详情;导航顺序按当前可见行(同一筛选作用域)
    function _openDetail(id) {
        if (typeof WorkDetail === 'undefined') return;
        var ids = _visibleRows().map(function(r) { return r.id; });
        WorkDetail.openDetail(id, ids);
    }

    // ============ T-102:拖拽改截止日 ============

    function _onDragStart(ev, id) {
        var t = Work.rowById(id);
        if (!t) return;
        var year = (new Date()).getFullYear();
        _drag = { id: id, sourceYmd: _normalizeYmd(t.due, year) };
        ev.dataTransfer.effectAllowed = 'move';
        try { ev.dataTransfer.setData('text/plain', '' + id); } catch(_) {}
        // 给 source 一点视觉反馈(沿用 work-board 的 .dragging 同名)
        if (ev.target && ev.target.classList) {
            ev.target.classList.add('wt-cal-task-dragging');
            ev.target.classList.add('wt-dragging');   // T-103 B.4:物理感共用类
        }
    }

    function _onDragEnd(ev) {
        if (ev.target && ev.target.classList) {
            ev.target.classList.remove('wt-cal-task-dragging');
            ev.target.classList.remove('wt-dragging');
        }
        // 清掉残留 hover
        document.querySelectorAll('.wt-cal-cell.wt-cal-drop-hover').forEach(function(c) {
            c.classList.remove('wt-cal-drop-hover');
        });
        _drag = null;
    }

    function _onDragOver(ev, cell) {
        if (!_drag) return;
        // dim 格(上月末/下月头)没有 data-cell-date → 整个就不绑 dragover;
        // 这里防御:即使绑上,缺 data-cell-date 也 return,不 preventDefault
        if (!cell || !cell.dataset || !cell.dataset.cellDate) return;
        ev.preventDefault();   // 必须,才能触发 drop
        ev.dataTransfer.dropEffect = 'move';
        cell.classList.add('wt-cal-drop-hover');
    }

    function _onDragLeave(ev, cell) {
        if (!cell) return;
        cell.classList.remove('wt-cal-drop-hover');
    }

    function _onDrop(ev, cell) {
        ev.preventDefault();
        if (cell) cell.classList.remove('wt-cal-drop-hover');
        if (!_drag) return;
        if (!cell || !cell.dataset || !cell.dataset.cellDate) { _drag = null; return; }

        var targetYmd = cell.dataset.cellDate;
        var id = _drag.id;
        var sourceYmd = _drag.sourceYmd;
        _drag = null;  // 提前清,避免误判后续事件

        // 1) 同一天 → 忽略(不弹 confirm 不发请求)
        if (sourceYmd === targetYmd) return;

        // 2) 目标在过去 → 二次确认
        var today = _todayYmd();
        if (targetYmd < today) {
            var ok = confirm('该日期已过去,确认将截止日改为 ' + targetYmd + ' 吗?');
            if (!ok) return;
        }

        // 3) 走 Work.updateRow 乐观更新(失败会自动 reload 回滚)
        var t = Work.rowById(id);
        Work.updateRow(id, { due: targetYmd });
        if (typeof showToast === 'function') {
            showToast('✓ 截止日已改为 ' + targetYmd + (t ? ':' + (t.title || '(无标题)') : ''), 'success');
        }
    }

    return {
        render: render,
        _openDetail: _openDetail,
        _onDragStart: _onDragStart,
        _onDragEnd: _onDragEnd,
        _onDragOver: _onDragOver,
        _onDragLeave: _onDragLeave,
        _onDrop: _onDrop,
    };
})();
