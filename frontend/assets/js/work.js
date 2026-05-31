// ========== Work — 工作模块入口 (T-094 / SPEC work-task-table) ==========
//
// 工作 Hub + 任务表(表格/看板/日历三视图)的状态容器。
// 视图模块(WorkTable / WorkBoard / WorkCalendar / WorkColumnCfg / WorkPick)
// 通过 Work.rows() / Work.columns() / Work.updateRow(...) 等读写数据。
//
// 状态在闭包里:
//   _columns — 列配置数组(从 /api/work/columns 加载)
//   _rows    — 任务数组(从 /api/work/tasks 加载)
//   _view    — 当前视图 'table' | 'board' | 'cal'
//   _feature — 当前 Hub 内的子功能('table' 表示进了任务表;null 表示在 Hub)

var Work = (function() {
    var _columns = [];
    var _rows = [];
    var _view = 'table';
    var _feature = null;
    var _loaded = false;
    var _timeTab = 'all';   // T-098:时间镜头 Tab(all / today / week / month)
    var _dateFilter = null; // T-103 B.1:点心电图柱后单日过滤(YYYY-MM-DD);覆盖 _timeTab
    var _renderFrozen = false; // T-103 B.3:完成动画期间冻结 render,避免动画被打断

    // ============ 生命周期 ============
    function init() {
        // 从 localStorage 恢复最后打开的子功能
        var last = localStorage.getItem('work_feature');
        if (last === 'table') openFeature('table');
        else showHub();
    }

    function showHub() {
        _feature = null;
        localStorage.removeItem('work_feature');
        var hub = document.getElementById('work-hub');
        var tableView = document.getElementById('work-table-view');
        if (hub) hub.style.display = '';
        if (tableView) tableView.style.display = 'none';
        // 回 Hub 时一并收起其它子视图(洞察 / 已完成档案),避免导航往返后残留覆盖
        var insView = document.getElementById('work-insight-view');
        var doneView = document.getElementById('work-done-view');
        if (insView) insView.style.display = 'none';
        if (doneView) doneView.style.display = 'none';
        // T-100:回到 Hub 时关闭详情抽屉(任务表已隐藏,抽屉应一起退场)
        if (typeof WorkDetail !== 'undefined' && WorkDetail.isOpen()) {
            WorkDetail.closeDetail();
        }
    }

    function openFeature(name) {
        var hub = document.getElementById('work-hub');
        var tableView = document.getElementById('work-table-view');
        if (hub) hub.style.display = 'none';
        if (tableView) tableView.style.display = 'none';

        if (name === 'table') {
            _feature = 'table';
            localStorage.setItem('work_feature', 'table');
            if (tableView) tableView.style.display = '';
            _ensureLoaded().then(function() {
                setView(_view, true);
            });
        }
    }

    // 视图切换 (table / board / cal / person / distribution)
    function setView(v, skipBtnSync) {
        // T-100:切视图自动关闭详情抽屉(spec § 7.6 关闭方式之一)
        if (typeof WorkDetail !== 'undefined' && WorkDetail.isOpen() && v !== _view) {
            WorkDetail.closeDetail();
        }
        _view = v;
        if (!skipBtnSync) {
            var t = document.getElementById('wt-seg-table');
            var b = document.getElementById('wt-seg-board');
            var c = document.getElementById('wt-seg-cal');
            var pp = document.getElementById('wt-seg-person');  // T-099
            var dd = document.getElementById('wt-seg-distribution');  // T-108
            if (t)  t.classList.toggle('active', v === 'table');
            if (b)  b.classList.toggle('active', v === 'board');
            if (c)  c.classList.toggle('active', v === 'cal');
            if (pp) pp.classList.toggle('active', v === 'person');
            if (dd) dd.classList.toggle('active', v === 'distribution');
        }
        var t  = document.getElementById('wt-table-view');
        var bv = document.getElementById('wt-board-view');
        var cv = document.getElementById('wt-cal-view');
        var pv = document.getElementById('wt-person-view');  // T-099
        var dv = document.getElementById('wt-distribution-view');  // T-108
        if (t)  t.classList.toggle('wt-hidden',  v !== 'table');
        if (bv) bv.classList.toggle('wt-hidden', v !== 'board');
        if (cv) cv.classList.toggle('wt-hidden', v !== 'cal');
        if (pv) pv.classList.toggle('wt-hidden', v !== 'person');
        if (dv) dv.classList.toggle('wt-hidden', v !== 'distribution');
        // T-098:日历视图下时间镜头 Tab 自动隐藏(日历本身按 due_date 排,叠加筛选语义重复)
        var tabBar = document.getElementById('wt-time-tabs');
        if (tabBar) tabBar.style.display = (v === 'cal') ? 'none' : '';
        // T-109:切视图属于"允许 stagger"路径
        render({ stagger: true });
    }

    // T-098:切换时间镜头 Tab(all / today / week / month)
    // T-103:切 tab 同时清掉单日过滤(语义冲突),并移动 spring 指示条
    function setTimeTab(tab) {
        _timeTab = tab;
        _dateFilter = null;   // T-103
        document.querySelectorAll('.wt-time-tab').forEach(function(el) {
            el.classList.toggle('active', el.dataset.tab === tab);
        });
        _updateTabIndicator();
        _updateDateFilterChip();
        // T-109:切 Tab 属于"允许 stagger"路径
        render({ stagger: true });
    }
    function timeTab() { return _timeTab; }

    // T-103 B.1:点心电图柱后 → 设为单日过滤
    function setDateFilter(ymd) {
        _dateFilter = ymd || null;
        _updateDateFilterChip();
        // 单日过滤时不一定还能匹配当前 tab 语义,把视觉重置成「全部」更直观
        if (_dateFilter) {
            _timeTab = 'all';
            document.querySelectorAll('.wt-time-tab').forEach(function(el) {
                el.classList.toggle('active', el.dataset.tab === 'all');
            });
            _updateTabIndicator();
        }
        // 自动切到表格视图(spec 说"跳到表格视图 + 自动筛选")
        // T-109:setView 自带 stagger;直接 render 时也用 stagger(行集大变,跟切 Tab 同质)
        if (_view !== 'table') {
            setView('table');
        } else {
            render({ stagger: true });
        }
    }
    function clearDateFilter() {
        _dateFilter = null;
        _updateDateFilterChip();
        // T-109:清掉单日过滤,行集回到全量,允许 stagger
        render({ stagger: true });
    }
    function dateFilter() { return _dateFilter; }

    function _updateDateFilterChip() {
        var chip = document.getElementById('wt-date-filter-chip');
        var dateEl = document.getElementById('wt-dfc-date');
        if (!chip) return;
        if (_dateFilter) {
            chip.style.display = '';
            if (dateEl) dateEl.textContent = _dateFilter;
        } else {
            chip.style.display = 'none';
        }
    }

    // T-103 B.2:把 spring 指示条移到当前 active tab 下方
    function _updateTabIndicator() {
        var bar = document.getElementById('wt-tab-indicator');
        var active = document.querySelector('.wt-time-tab.active');
        var container = document.getElementById('wt-time-tabs');
        if (!bar || !active || !container) return;
        // 必须用相对父容器的偏移(active.offsetLeft 已是相对 offsetParent 即 .wt-time-tabs)
        bar.style.left = active.offsetLeft + 'px';
        bar.style.width = active.offsetWidth + 'px';
    }

    // T-098:把任务列表按当前 Tab 过滤;表格视图和看板视图共用同一份逻辑。
    // T-113:统一入口先排除 done(已完成任务归 T-111 已完成档案);所有视图都通过本函数
    //        自动生效,不要在每个视图各自实现 done 过滤。
    //        已完成档案 WorkDone 直接调 Work.rows() 拿原始数据 + 自己 filter done,
    //        不走 applyTimeTabFilter,因此不受本变更影响。
    // T-120:加 opts.includeDone 让**日历视图**显式 opt-out T-113 全局规则(日历是"时序视角"
    //        看历史/现在/未来;其它视图仍是"待办视角")。默认 false 不破坏既有行为。
    // 语义(spec § 6.1 / T-113 修订 / T-120 例外):
    //   all   = 所有未删除 **且 status != 'done'**(opts.includeDone=true 时含 done)
    //   today = (due == today  ∪  due < today 逾期) AND status != 'done'
    //   week  = due in 本周(ISO,周一首日)  ∪  逾期未完成
    //   month = due in 本月  ∪  逾期未完成
    function applyTimeTabFilter(rows, opts) {
        opts = opts || {};
        // T-113:统一排除 done(主任务表所有视图的中心过滤点);
        // T-120:日历视图传 includeDone=true 时跳过
        if (!opts.includeDone) {
            rows = rows.filter(function(t) { return t.status !== 'done'; });
        }
        // T-103 B.1:单日过滤优先(覆盖时间 tab 语义)
        if (_dateFilter) {
            return rows.filter(function(t) {
                return _normalizeDue(t.due) === _dateFilter;
            });
        }
        if (_timeTab === 'all') return rows.slice();
        var today = _todayYMD();
        var range = (_timeTab === 'week')  ? _weekRange(today)
                  : (_timeTab === 'month') ? _monthRange(today) : null;
        return rows.filter(function(t) {
            var due = _normalizeDue(t.due);
            // 逾期未完成永远纳入(强制冒头);done 上面已排除,这里 status !== 'done' 守护
            if (due && due < today && t.status !== 'done') return true;
            if (!due) return false;
            if (_timeTab === 'today') return due === today;
            return due >= range.start && due <= range.end;
        });
    }

    // YYYY-MM-DD,字符串比较安全(lexicographic = chronological)
    function _todayYMD() {
        var d = new Date();
        return d.getFullYear() + '-' + ('0' + (d.getMonth() + 1)).slice(-2) + '-' + ('0' + d.getDate()).slice(-2);
    }
    function _ymd(d) {
        return d.getFullYear() + '-' + ('0' + (d.getMonth() + 1)).slice(-2) + '-' + ('0' + d.getDate()).slice(-2);
    }
    // 把 due 规范化成 YYYY-MM-DD:接受 'YYYY-MM-DD' 原样,'MM-DD' 补当前年,其它原样
    function _normalizeDue(due) {
        if (!due) return '';
        if (due.length === 10 && due.charAt(4) === '-' && due.charAt(7) === '-') return due;
        if (due.length === 5 && due.charAt(2) === '-') return (new Date()).getFullYear() + '-' + due;
        return due;
    }
    function _weekRange(today) {
        var d = new Date(today + 'T00:00:00');
        var day = d.getDay();              // 0=日, 1=一, ..., 6=六
        var fromMon = (day === 0) ? 6 : day - 1;
        var mon = new Date(d);
        mon.setDate(d.getDate() - fromMon);
        var sun = new Date(mon);
        sun.setDate(mon.getDate() + 6);
        return { start: _ymd(mon), end: _ymd(sun) };
    }
    function _monthRange(today) {
        var d = new Date(today + 'T00:00:00');
        var first = new Date(d.getFullYear(), d.getMonth(), 1);
        var last  = new Date(d.getFullYear(), d.getMonth() + 1, 0);
        return { start: _ymd(first), end: _ymd(last) };
    }

    // T-098:每次 render 时刷新 4 个 Tab 的实时计数 + 逾期 ⚠ 徽章
    // T-113:计数也排除 done(与 applyTimeTabFilter 一致;done 任务归 T-111 已完成档案)
    function _updateTabCounts() {
        var today = _todayYMD();
        var wk = _weekRange(today);
        var mo = _monthRange(today);
        var c = { all: 0, today: 0, week: 0, month: 0, overdue: 0 };
        _rows.forEach(function(t) {
            if (t.status === 'done') return;   // T-113
            c.all++;
            var due = _normalizeDue(t.due);
            var isOverdue = (due && due < today);
            if (isOverdue) c.overdue++;
            if (isOverdue || due === today) c.today++;
            if (isOverdue || (due && due >= wk.start && due <= wk.end)) c.week++;
            if (isOverdue || (due && due >= mo.start && due <= mo.end)) c.month++;
        });
        var set = function(id, v) { var el = document.getElementById(id); if (el) el.textContent = v; };
        set('wt-tab-count-all',   c.all);
        set('wt-tab-count-today', c.today);
        set('wt-tab-count-week',  c.week);
        set('wt-tab-count-month', c.month);
        var warnEl = document.getElementById('wt-tab-warn');
        if (warnEl) {
            warnEl.style.display = c.overdue > 0 ? '' : 'none';
            warnEl.title = '含 ' + c.overdue + ' 条逾期';
        }
    }

    // T-118:心电图折叠状态(localStorage 持久化,默认折叠)
    function _heartStripOpen() {
        return localStorage.getItem('workHeartStripOpen') === '1';
    }
    function _applyHeartStripState() {
        var wrap = document.getElementById('wt-heart-strip');
        var btn = document.getElementById('wt-heart-toggle');
        if (!wrap) return;
        var open = _heartStripOpen();
        wrap.classList.toggle('open', open);
        if (btn) btn.classList.toggle('active', open);
    }
    function toggleHeartStrip() {
        var open = !_heartStripOpen();
        localStorage.setItem('workHeartStripOpen', open ? '1' : '0');
        _applyHeartStripState();
    }

    // 当前激活视图重渲;给子模块用(列设置改完、单元格改完 → 调一次)
    // T-103 B.3:完成动画期间 _renderFrozen=true 跳过重渲(避免动画被 redraw 打断)
    // T-109:opts.stagger 透传到表格视图(仅切 Tab / 切视图 / 首次加载触发动效;
    //        编辑 / 拖拽 / 工具创建后的高频重渲不触发)
    function render(opts) {
        if (_renderFrozen) return;
        opts = opts || {};
        _updateTabCounts();   // T-098:每次重渲都刷 Tab 计数(任务新增/编辑/删除自动联动)
        _renderHeartStrip();  // T-103 B.1:心电图也跟着数据刷
        _applyHeartStripState();  // T-118:每次 render 都同步折叠态(首次进入也生效)
        if (_view === 'table') WorkTable.render(opts);
        else if (_view === 'board') WorkBoard.render();
        else if (_view === 'cal') WorkCalendar.render();
        else if (_view === 'person') WorkPerson.render();   // T-099
        else if (_view === 'distribution') WorkDistribution.render();   // T-108
        // T-103 B.2:渲完才能拿到 active tab 的几何位置,延迟一帧再放指示条
        requestAnimationFrame(_updateTabIndicator);
        // T-100:抽屉打开时,数据改完同步刷新抽屉里的字段
        if (typeof WorkDetail !== 'undefined' && WorkDetail.isOpen()) {
            WorkDetail.refreshIfOpen();
        }
    }

    // T-103 B.3:完成动画专用 — 冻结一段时间内的所有 render,期满后解冻并主动重渲
    function freezeRender(ms) {
        _renderFrozen = true;
        setTimeout(function() {
            _renderFrozen = false;
            render();
        }, ms || 1100);
    }

    // T-099:按 assignee 字符串 hash 取头像色(4 色调色板),所有视图共用,同人同色。
    // 替代原 work-table.js 中按"出现顺序分配"的方案(否则跨视图同人异色)。
    var _AVATAR_PAL = ['#7C4DFF', '#14B8A6', '#E0A23B', '#3B82F6'];
    function colorOf(name) {
        var h = 0;
        var s = '' + (name || '');
        for (var i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) >>> 0;
        return _AVATAR_PAL[h % _AVATAR_PAL.length];
    }
    var renderActiveView = render;  // 别名 (语义更明确)

    // ============ 数据加载 ============
    function _ensureLoaded() {
        if (_loaded) return Promise.resolve();
        return reload();
    }
    function reload() {
        return Promise.all([API.workListColumns(), API.workListTasks()])
            .then(function(results) {
                var colsResp = results[0], tasksResp = results[1];
                _columns = (colsResp && colsResp.items) || [];
                _rows    = (tasksResp && tasksResp.items) || [];
                _loaded = true;
                render();
            })
            .catch(function(err) {
                console.error('[Work] reload failed', err);
                if (typeof showToast === 'function') showToast('工作模块加载失败:' + (err && err.message || ''), 'error');
            });
    }

    // ============ getters ============
    function rows()    { return _rows; }
    function columns(){ return _columns; }
    function rowById(id) {
        for (var i = 0; i < _rows.length; i++) if (_rows[i].id === id) return _rows[i];
        return null;
    }
    function colByKey(k) {
        for (var i = 0; i < _columns.length; i++) if (_columns[i].key === k) return _columns[i];
        return null;
    }

    // ============ 数据写入(乐观 UI 更新 → 后端确认 → 失败时 reload) ============
    function updateRow(id, patch) {
        var t = rowById(id);
        if (t) _applyPatchLocal(t, patch);
        render();
        return API.workUpdateTask(id, patch).then(function(resp) {
            // 用后端权威结果回写(避免 status=done 自动 progress=100 等服务端规则不一致)
            if (resp && resp.item) {
                var idx = _rows.findIndex(function(x) { return x.id === id; });
                if (idx >= 0) _rows[idx] = resp.item;
                render();
            }
        }).catch(function(err) {
            console.error('[Work] update failed, reloading', err);
            if (typeof showToast === 'function') showToast('保存失败,正在刷新...', 'warning');
            reload();
        });
    }
    function _applyPatchLocal(t, patch) {
        Object.keys(patch).forEach(function(k) {
            if (k === 'customFields') {
                t.customFields = t.customFields || {};
                Object.keys(patch.customFields).forEach(function(ck) {
                    t.customFields[ck] = patch.customFields[ck];
                });
            } else {
                t[k] = patch[k];
            }
        });
        if (patch.status === 'done' && patch.progress == null) t.progress = 100;
    }
    function createRow(payload) {
        return API.workCreateTask(payload).then(function(resp) {
            if (resp && resp.item) _rows.push(resp.item);
            render();
        }).catch(function(err) {
            console.error('[Work] create failed', err);
            if (typeof showToast === 'function') showToast('新建失败:' + (err && err.message || ''), 'error');
        });
    }
    function deleteRow(id) {
        var idx = _rows.findIndex(function(x) { return x.id === id; });
        var backup = idx >= 0 ? _rows.splice(idx, 1)[0] : null;
        render();
        return API.workDeleteTask(id).catch(function(err) {
            console.error('[Work] delete failed, restoring', err);
            if (backup) _rows.splice(idx, 0, backup);
            render();
            if (typeof showToast === 'function') showToast('删除失败', 'error');
        });
    }

    // 批量保存列(rename / type / options / width / position 都走它)
    function saveColumnPatches(patches) {
        return API.workSaveColumns(patches).then(function(resp) {
            if (resp && resp.items) _columns = resp.items;
        }).catch(function(err) {
            console.error('[Work] saveColumns failed', err);
            if (typeof showToast === 'function') showToast('列设置保存失败', 'warning');
        });
    }
    function addColumn(data) {
        return API.workAddColumn(data).then(function(resp) {
            if (resp && resp.items) _columns = resp.items;
        });
    }
    function removeColumn(key) {
        // 乐观更新:从 _rows 的 customFields 里也去掉这个 key
        _rows.forEach(function(t) {
            if (t.customFields) delete t.customFields[key];
        });
        return API.workDeleteColumn(key).then(function(resp) {
            if (resp && resp.items) _columns = resp.items;
        });
    }

    // ============ T-103 B.1:30 天负载心电图 ============
    // 30 个柱(i=29 = 30 天前,i=0 = 今天),柱高线性映射 8-44px;
    // 普通天 primary-soft,今天 primary + 脉动,含逾期未完成的天 danger-soft。
    // 点柱 → 单日过滤(setDateFilter)。
    function _renderHeartStrip() {
        var wrap = document.getElementById('wt-heart-strip');
        if (!wrap) return;
        var today = _todayYMD();
        var todayDate = new Date(today + 'T00:00:00');

        // 准备 30 个槽,索引 0 = 30 天前,索引 29 = 今天
        var days = [];
        for (var i = 0; i < 30; i++) {
            var d = new Date(todayDate);
            d.setDate(todayDate.getDate() - (29 - i));
            var ymd = d.getFullYear() + '-' + ('0' + (d.getMonth() + 1)).slice(-2) + '-' + ('0' + d.getDate()).slice(-2);
            days.push({ ymd: ymd, count: 0, overdue: 0 });
        }
        // 按 due 日期落入对应槽(超 30 天的不算)
        // T-113:done 任务不在主任务表心电图体现(已完成档案 T-111 是 done 的去处)
        _rows.forEach(function(t) {
            if (t.status === 'done') return;
            var due = _normalizeDue(t.due);
            if (!due) return;
            for (var j = 0; j < 30; j++) {
                if (days[j].ymd === due) {
                    days[j].count++;
                    if (due < today) days[j].overdue++;
                    break;
                }
            }
        });
        var maxCount = days.reduce(function(m, d) { return d.count > m ? d.count : m; }, 1);
        var totalCount = days.reduce(function(s, d) { return s + d.count; }, 0);
        var totalOverdue = days.reduce(function(s, d) { return s + d.overdue; }, 0);

        var barsHtml = days.map(function(d, i) {
            var h = d.count === 0 ? 6 : Math.max(8, Math.round((d.count / maxCount) * 44));
            var cls = (d.ymd === today) ? ' wt-hs-today'
                    : (d.overdue > 0)   ? ' wt-hs-overdue' : '';
            if (_dateFilter === d.ymd) cls += ' wt-hs-active';
            var daysAgo = 29 - i;
            var label = (d.ymd === today) ? '今天' : (daysAgo + ' 天前');
            return '<div class="wt-hs-bar' + cls + '" style="height:' + h + 'px" '
                +    'data-ymd="' + d.ymd + '" '
                +    'title="' + label + ' · ' + d.count + ' 任务" '
                +    'onclick="Work.setDateFilter(\'' + d.ymd + '\')">'
                +    '<span class="wt-hs-tip">' + label + ' · ' + d.count + ' 任务'
                +    (d.overdue ? ' · ⏰ ' + d.overdue + ' 逾期' : '')
                + '</span></div>';
        }).join('');

        wrap.innerHTML = ''
            + '<div class="wt-hs-head">'
            +   '<span class="wt-hs-title">⚡ 30 天负载心电图 · 点柱筛该日</span>'
            +   '<span class="wt-hs-meta"><strong>' + totalCount + '</strong> 任务 · '
            +     '<strong>' + totalOverdue + '</strong> 逾期</span>'
            + '</div>'
            + '<div class="wt-hs-bars">' + barsHtml + '</div>'
            + '<div class="wt-hs-axis"><span>30 天前</span><span>2 周前</span><span>今天</span></div>';
    }

    return {
        init: init,
        showHub: showHub,
        openFeature: openFeature,
        setView: setView,
        // T-098
        setTimeTab: setTimeTab,
        timeTab: timeTab,
        applyTimeTabFilter: applyTimeTabFilter,
        // T-099
        colorOf: colorOf,
        render: render,
        renderActiveView: renderActiveView,
        reload: reload,
        // T-103
        setDateFilter: setDateFilter,
        clearDateFilter: clearDateFilter,
        dateFilter: dateFilter,
        freezeRender: freezeRender,
        // T-118
        toggleHeartStrip: toggleHeartStrip,

        rows: rows,
        columns: columns,
        rowById: rowById,
        colByKey: colByKey,

        updateRow: updateRow,
        createRow: createRow,
        deleteRow: deleteRow,

        saveColumnPatches: saveColumnPatches,
        addColumn: addColumn,
        removeColumn: removeColumn,
    };
})();
