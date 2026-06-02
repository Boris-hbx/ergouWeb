// ========== WorkPerson — 工作任务表第 4 视图:责任人视图 (T-099) ==========
//
// 上图(气泡总览)+ 下卡(责任人卡片墙)。
// 视觉设计依据:`frontend/work-person-preview.html`(PM 单文件设计稿,Boris 已认可,1:1 还原)。
// spec § 6.4。
//
// 数据流:
//   Work.rows() → 时间 Tab 过滤(applyTimeTabFilter) → 按 assignee 分组 → 渲染气泡 + 卡片
//   assignee 空字符串 → 「未指派」卡,永远末尾
//
// 交互:
//   点气泡 → 平滑滚动到对应卡片 + 紫框高亮 1.4 秒
//   点 ☐ → openProgressDialog(T-095 抽出的共享组件)
//   点标题 → openTextInputDialog(T-095 抽出的)
//   任务拖到另一张卡 → confirm → PATCH assignee → 重渲(复用 Work.updateRow 乐观更新)
//   拖到自己卡上 → 无效
//
// 配色 / 类名前缀一律 wt-,避免污染其它模块。

var WorkPerson = (function() {
    var UNASSIGNED = '未指派';
    var COLLAPSE_N = 5;   // T-138:每卡默认最多显示的任务数

    function _esc(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    }

    // ── 数据:按 assignee + collaborators 分组(T-119),未指派永远末尾 ────
    //   主责任人卡:含该人 assignee 的所有任务(isCollab=false)
    //   协作者卡:含 collaborators 中含该人的所有任务(isCollab=true,显示「协作」灰标)
    //   同一任务可能同时出现在主卡和协作者卡,这是期望行为
    function _groupRows(rows) {
        var map = Object.create(null);
        function _push(name, task, isCollab) {
            if (!map[name]) map[name] = [];
            map[name].push({ task: task, isCollab: !!isCollab });
        }
        rows.forEach(function(t) {
            var name = (t.assignee && ('' + t.assignee).trim()) ? t.assignee : UNASSIGNED;
            _push(name, t, false);
            // T-119:协作者也按各自姓名 push,标记 isCollab=true
            if (Array.isArray(t.collaborators)) {
                t.collaborators.forEach(function(c) {
                    if (c && c !== name) _push(c, t, true);
                });
            }
        });
        // 维持入场顺序;未指派单独放末尾
        var names = Object.keys(map).filter(function(n) { return n !== UNASSIGNED; });
        if (map[UNASSIGNED]) names.push(UNASSIGNED);
        return names.map(function(n) { return { name: n, tasks: map[n] }; });
    }

    // ── 截止日 pill:对生产数据(YYYY-MM-DD)做语义化转字符串 ──
    // 返回 { text, cls } 其中 cls ∈ ''|'wt-person-due-soon'|'wt-person-due-overdue'
    function _dueLabel(t) {
        if (!t.due) return { text: '—', cls: '' };
        var due = _normalizeDue(t.due);
        var today = _todayYMD();
        if (!due) return { text: ('' + t.due), cls: '' };
        var tmr = _addDaysYMD(today, 1);
        // 逾期(只对未完成的算)
        if (t.status !== 'done' && due < today) {
            var n = _daysBetween(due, today);
            return { text: '逾期' + n + '天', cls: 'wt-person-due-overdue' };
        }
        if (due === today) return { text: '今天', cls: 'wt-person-due-soon' };
        if (due === tmr) return { text: '明天', cls: 'wt-person-due-soon' };
        // 本周内
        var weekEnd = _weekEnd(today);
        if (due <= weekEnd) return { text: '本周', cls: '' };
        // 默认 MM-DD
        return { text: due.slice(5), cls: '' };
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
    function _weekEnd(today) {
        var d = new Date(today + 'T00:00:00');
        var day = d.getDay();
        var toSun = (day === 0) ? 0 : 7 - day;  // 周日 = 0;周一 = 6 天到周日
        return _addDaysYMD(today, toSun);
    }
    function _daysBetween(fromY, toY) {
        var a = new Date(fromY + 'T00:00:00');
        var b = new Date(toY + 'T00:00:00');
        return Math.max(1, Math.floor((b - a) / 86400000));
    }

    // ── 气泡颜色分类(spec § 6.4 优先级:逾期 > P0 > 正常 > 仅低优)
    //   T-119:tasks 现在是 [{task, isCollab}];颜色**只看主责任人那部分**(isCollab=false),
    //         避免协作者卡因为别人那条逾期任务而显红
    function _bubbleClass(group) {
        if (group.name === UNASSIGNED) return 'wt-bubble-low';
        var primary = group.tasks.filter(function(it) { return !it.isCollab; });
        // 协作者卡 primary 为空时,降级看协作任务的颜色
        var pool = primary.length > 0 ? primary : group.tasks;
        var hasOverdue = pool.some(function(it) {
            var t = it.task;
            var due = _normalizeDue(t.due);
            return t.status !== 'done' && due && due < _todayYMD();
        });
        if (hasOverdue) return 'wt-bubble-overdue';
        var hasP0 = pool.some(function(it) { return it.task.priority === 'high'; });
        if (hasP0) return 'wt-bubble-p0';
        var onlyLow = pool.every(function(it) { return it.task.priority === 'low'; });
        if (onlyLow) return 'wt-bubble-low';
        return 'wt-bubble-normal';
    }

    // ── 对数缩放气泡大小(spec § 6.4) ──
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

    // T-119:协作任务数(用于统计行"协作 N 件")
    function _collabCount(items) {
        return items.filter(function(it) { return it.isCollab; }).length;
    }

    // ============ 主渲染 ============
    function render() {
        var host = document.getElementById('wt-person-view');
        if (!host) return;
        var rows = Work.applyTimeTabFilter(Work.rows());
        var groups = _groupRows(rows);

        host.innerHTML =
            '<div class="wt-section-label">人员负载总览(点气泡跳转到对应卡片)</div>'
          + '<div class="wt-bubble-wrap">'
          +   '<div class="wt-bubble-row" id="wt-bubble-row"></div>'
          +   '<div class="wt-bubble-legend">'
          +     '<span><span class="wt-bubble-dot" style="background:#E11D48"></span>含逾期</span>'
          +     '<span><span class="wt-bubble-dot" style="background:#F59E0B"></span>含 P0</span>'
          +     '<span><span class="wt-bubble-dot" style="background:var(--primary-color)"></span>正常</span>'
          +     '<span><span class="wt-bubble-dot" style="background:#D4D0E0"></span>仅低优</span>'
          +     '<span class="wt-bubble-legend-tail">气泡大小 = 任务数</span>'
          +   '</div>'
          + '</div>'
          + '<div class="wt-section-label">分人详情(拖任务卡到另一人 = 转交)</div>'
          + '<div class="wt-person-grid" id="wt-person-grid"></div>';

        _renderBubbles(groups);
        _renderCards(groups);
    }

    function _renderBubbles(groups) {
        var row = document.getElementById('wt-bubble-row');
        if (!row) return;
        row.innerHTML = '';
        groups.forEach(function(g) {
            var size = _bubbleSize(g.tasks.length);
            var overdue = _overdueCount(g.tasks);
            var b = document.createElement('div');
            b.className = 'wt-bubble ' + _bubbleClass(g);
            b.style.width = size + 'px';
            b.style.height = size + 'px';
            b.dataset.person = g.name;
            b.innerHTML = '<div class="wt-b-name">' + _esc(g.name) + '</div>'
                + '<div class="wt-b-count">' + g.tasks.length + '</div>'
                + (overdue ? '<div class="wt-b-badge">⏰ ' + overdue + '</div>' : '');
            b.onclick = function() { _toggleCard(g.name, true); };
            row.appendChild(b);
        });
    }

    function _renderCards(groups) {
        var grid = document.getElementById('wt-person-grid');
        if (!grid) return;
        grid.innerHTML = '';
        groups.forEach(function(g) {
            grid.appendChild(_buildCard(g));
        });
        _bindCardInteractions();
    }

    function _buildCard(g) {
        var card = document.createElement('div');
        card.className = 'wt-person-card';
        card.dataset.person = g.name;
        var overdue = _overdueCount(g.tasks);
        var p0 = _p0Count(g.tasks);
        // T-121:不再单独统计"协作 N",视觉与主任务平等
        var statsHtml =
            '<span class="wt-person-stats-total">' + g.tasks.length + ' 任务</span>'
          + (overdue ? '<span class="wt-person-stats-overdue">⏰ ' + overdue + ' 逾期</span>' : '')
          + (p0      ? '<span class="wt-person-stats-p0">⚡ ' + p0 + ' P0</span>' : '');
        var avatarBg = Work.colorOf(g.name);
        var initial = _initialOf(g.name);

        // T-121:取消"协作"区分 UI(memory single-user-simplicity);
        //   tasks 仍是 [{task, isCollab}],但任务行视觉不再区分主+协,所有卡看着都平等
        //   保留:_groupRows 双 push(主+协卡都出现)+ 拖拽限制(协作任务不允许从协作者卡拖走)
        // T-138:卡内排序(逾期 > P0 > 截止近 > 其余),默认只显前 5,余者折叠
        var sorted = _sortTasks(g.tasks);
        var taskListHtml = sorted.map(function(it, idx) {
            var t = it.task;
            var due = _dueLabel(t);
            var p0Pill = (t.priority === 'high') ? '<span class="wt-person-pill wt-person-pill-p0">P0</span>' : '';
            var doneCls = (t.status === 'done') ? ' wt-person-task-done' : '';
            var extraCls = (idx >= COLLAPSE_N) ? ' wt-person-task-extra' : '';
            return '<div class="wt-person-task' + doneCls + extraCls + '" '
                +    (it.isCollab ? '' : 'draggable="true" ')   // 协作任务不允许从此卡拖走(转交语义只走主责任人卡)
                +    'data-tid="' + t.id + '" data-from="' + _esc(g.name) + '">'
                +    '<div class="wt-person-check" data-tid="' + t.id + '"></div>'
                +    '<div class="wt-person-task-title" data-tid="' + t.id + '">' + _esc(t.title || '(无标题)') + '</div>'
                +    '<div class="wt-person-task-meta">'
                +      p0Pill
                +      '<span class="wt-person-pill ' + due.cls + '">' + _esc(due.text) + '</span>'
                +    '</div>'
                +  '</div>';
        }).join('');

        var expandHtml = (sorted.length > COLLAPSE_N)
            ? '<button type="button" class="wt-person-expand" data-person="' + _esc(g.name) + '" data-count="' + sorted.length + '">展开全部 ' + sorted.length + ' 条 ▾</button>'
            : '';

        card.innerHTML =
            '<div class="wt-person-head">'
          +   '<div class="wt-person-avatar" style="background:' + avatarBg + '">' + _esc(initial) + '</div>'
          +   '<div class="wt-person-name">' + _esc(g.name) + '</div>'
          + '</div>'
          + '<div class="wt-person-stats">' + statsHtml + '</div>'
          + '<div class="wt-person-task-list">' + taskListHtml + '</div>'
          + expandHtml
          + '<div class="wt-person-foot" data-add="' + _esc(g.name) + '">+ 添加任务</div>';
        return card;
    }

    function _initialOf(name) {
        if (name === UNASSIGNED) return '?';
        if (name === '自己' || name === '我') return '我';
        return ('' + name).slice(0, 1);
    }

    // ── 交互 ────
    var _dragTaskId = null;
    var _dragFrom = null;
    function _bindCardInteractions() {
        var host = document.getElementById('wt-person-view');
        if (!host) return;

        // 复选框 / 标题 / 拖动 都用事件代理(避免大量监听器)
        host.onclick = function(ev) {
            var check = ev.target.closest('.wt-person-check');
            if (check) {
                ev.stopPropagation();
                _onCheckClick(+check.dataset.tid);
                return;
            }
            var title = ev.target.closest('.wt-person-task-title');
            if (title) {
                ev.stopPropagation();
                // T-100:标题改为打开任务详情抽屉(原 editText 仍可在抽屉里改标题)
                if (typeof WorkDetail !== 'undefined') {
                    var ids = Work.applyTimeTabFilter(Work.rows()).map(function(r) { return r.id; });
                    WorkDetail.openDetail(+title.dataset.tid, ids);
                } else {
                    WorkTable.editText(+title.dataset.tid, 'title');
                }
                return;
            }
            // T-138:展开/收起按钮(与气泡 toggle 双向同步)
            var exp = ev.target.closest('.wt-person-expand');
            if (exp) {
                ev.stopPropagation();
                _toggleCard(exp.dataset.person, false);
                return;
            }
            var foot = ev.target.closest('.wt-person-foot');
            if (foot) {
                // T-138:「+ 添加任务」走 T-124 完整新建弹窗,预填该卡责任人(未指派卡不预填)
                var who = foot.dataset.add;
                if (who && who !== UNASSIGNED) WorkTable.openCreateDialog({ assignee: who });
                else WorkTable.openCreateDialog();
                return;
            }
        };

        host.querySelectorAll('.wt-person-task').forEach(function(item) {
            item.addEventListener('dragstart', function(ev) {
                _dragTaskId = +item.dataset.tid;
                _dragFrom = item.dataset.from;
                item.classList.add('wt-person-task-dragging');
                item.classList.add('wt-dragging');   // T-103 B.4:物理感共用类
                ev.dataTransfer.effectAllowed = 'move';
                try { ev.dataTransfer.setData('text/plain', '' + _dragTaskId); } catch(_) {}
            });
            item.addEventListener('dragend', function() {
                item.classList.remove('wt-person-task-dragging');
                item.classList.remove('wt-dragging');
                host.querySelectorAll('.wt-person-card').forEach(function(c) {
                    c.classList.remove('wt-person-dragover');
                });
                _dragTaskId = null;
                _dragFrom = null;
            });
        });

        host.querySelectorAll('.wt-person-card').forEach(function(card) {
            card.addEventListener('dragover', function(ev) {
                if (_dragTaskId == null) return;
                // 拖到自己卡上 → 无效,不高亮
                if (_dragFrom === card.dataset.person) return;
                ev.preventDefault();
                card.classList.add('wt-person-dragover');
            });
            card.addEventListener('dragleave', function() {
                card.classList.remove('wt-person-dragover');
            });
            card.addEventListener('drop', function(ev) {
                ev.preventDefault();
                card.classList.remove('wt-person-dragover');
                if (_dragTaskId == null) return;
                var to = card.dataset.person;
                if (_dragFrom === to) return;
                var t = Work.rowById(_dragTaskId);
                if (!t) return;
                if (confirm('将「' + (t.title || '(无标题)') + '」从「' + _dragFrom + '」转交给「' + to + '」吗？')) {
                    // 「未指派」目标 → assignee = '';否则用 to
                    var newAssignee = (to === UNASSIGNED) ? '' : to;
                    Work.updateRow(t.id, { assignee: newAssignee });
                    if (typeof showToast === 'function') showToast('✓ 已转交:' + t.title + ' → ' + to, 'success');
                }
            });
        });
    }

    function _onCheckClick(id) {
        var t = Work.rowById(id);
        if (!t) return;
        // 点 ☐ → 走 T-095 的进度弹窗(spec § 6.4 § 6 要求复用)
        WorkTable.editProgress(id, 'progress');
    }

    // ── T-138:卡内排序(逾期 > P0 > 截止近 > 其余),稳定排序 ──
    function _sortRank(it) {
        var t = it.task;
        var due = _normalizeDue(t.due);
        var today = _todayYMD();
        if (t.status !== 'done' && due && due < today) return 0;   // 逾期
        if (t.priority === 'high') return 1;                        // P0
        if (due) { var we = _weekEnd(today); if (due <= we) return 2; }  // 截止近(本周内)
        return 3;
    }
    function _sortTasks(items) {
        return items.map(function(it, i) { return { it: it, i: i }; })
            .sort(function(a, b) { var r = _sortRank(a.it) - _sortRank(b.it); return r !== 0 ? r : a.i - b.i; })
            .map(function(x) { return x.it; });
    }

    // ── T-138:按 data-person 找卡 / 找气泡(避免 querySelector 选择器注入)──
    function _findByPerson(sel, name) {
        var nodes = document.querySelectorAll(sel);
        for (var i = 0; i < nodes.length; i++) if (nodes[i].dataset.person === name) return nodes[i];
        return null;
    }

    // ── T-138:展开/折叠 + 高亮(持久,非闪)。气泡与卡自带按钮共用此 toggle,天然双向同步。──
    //   active=true:展开全部 + 紫框高亮 + 气泡激活;false:折叠回 5 + 取消高亮。多卡可各自独立。
    function _setCardActive(card, name, active) {
        card.classList.toggle('wt-person-active', active);
        var btn = card.querySelector('.wt-person-expand');
        if (btn) btn.textContent = active ? '收起 ▴' : ('展开全部 ' + btn.dataset.count + ' 条 ▾');
        var bubble = _findByPerson('.wt-bubble', name);
        if (bubble) bubble.classList.toggle('wt-bubble-active', active);
    }
    function _toggleCard(name, scroll) {
        var card = _findByPerson('.wt-person-card', name);
        if (!card) return;
        var active = !card.classList.contains('wt-person-active');
        _setCardActive(card, name, active);
        if (scroll) card.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }

    return {
        render: render,
    };
})();
