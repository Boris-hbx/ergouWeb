// ========== WorkTable — 工作任务表的表格视图 (T-094) ==========
//
// 渲染依据 Work.columns() (列配置) 和 Work.rows() (任务数据);
// 任何编辑都通过 Work.updateRow / Work.createRow,Work 再去调后端 API。
//
// 列宽 Excel 风格(spec § 6 / § 7.3):
//   拖竖条只调整两侧两列的宽度,其余不动;表格总宽 = 各列宽之和,显式写到 inline。
// 加行按钮(spec 原则 4):工具栏 + 表格底部,首行不放(避免误点)。
// 单元格编辑(spec § 7.1):
//   text/date/number → prompt;longtext → 弹层;select/multi/status → WorkPick.
//
// 公共入口:
//   WorkTable.render()  — 全表重渲(filter / group 变更后调一次)
//   WorkTable.addRow()  — 工具栏「+ 新建任务」
//   WorkTable.editText / editNumber / openPick / openText / toggleCheck
//   WorkTable.cycleStatus / cyclePriority — status/priority 系统列的快速循环

var WorkTable = (function() {
    var SERIAL_W = 54;

    // ============ 系统列固定数据(对应 spec WT_STATUS / WT_PRIO) ============
    var STATUS = [
        { key: 'todo',    label: '待办',   cls: 'wt-s-todo',    dot: '#94a3b8' },
        { key: 'doing',   label: '进行中', cls: 'wt-s-doing',   dot: '#fbbf24' },
        { key: 'blocked', label: '阻塞',   cls: 'wt-s-blocked', dot: '#f87171' },
        { key: 'done',    label: '已完成', cls: 'wt-s-done',    dot: '#34d399' },
    ];
    var PRIORITY = [
        { key: 'high', label: '高', cls: 'wt-p-high' },
        { key: 'mid',  label: '中', cls: 'wt-p-mid'  },
        { key: 'low',  label: '低', cls: 'wt-p-low'  },
    ];
    function _statusBy(k) { for (var i = 0; i < STATUS.length; i++) if (STATUS[i].key === k) return STATUS[i]; return STATUS[0]; }
    function _prioBy(k)   { for (var i = 0; i < PRIORITY.length; i++) if (PRIORITY[i].key === k) return PRIORITY[i]; return PRIORITY[1]; }

    // ============ 头像配色 ============
    // T-099:统一改走 Work.colorOf(name) hash 取色,所有视图同人同色。
    // 原本是"按出现顺序分配",会导致表格和人员视图同人异色。
    function _avatarColor(name) {
        return Work.colorOf(name);
    }

    function _esc(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    }

    // ============ filter / group:从工具栏 select 读 ============
    function _filterValue() {
        var el = document.getElementById('wt-filter');
        return el ? el.value : '';
    }
    function _groupValue() {
        var el = document.getElementById('wt-group');
        return el ? el.value : '';
    }
    function _visibleRows() {
        // T-098:先过时间镜头 Tab,再过责任人(两层叠加)
        var rows = Work.applyTimeTabFilter(Work.rows());
        var f = _filterValue();
        if (!f) return rows;
        return rows.filter(function(t) { return t.assignee === f; });
    }

    // ============ 工具栏右侧:责任人筛选下拉 (每次 render 重建) ============
    function _refillFilter() {
        var sel = document.getElementById('wt-filter');
        if (!sel) return;
        var cur = sel.value;
        var names = [];
        Work.rows().forEach(function(t) {
            if (t.assignee && names.indexOf(t.assignee) < 0) names.push(t.assignee);
        });
        sel.innerHTML = '<option value="">全部责任人</option>'
            + names.map(function(n) {
                var safe = _esc(n);
                return '<option value="' + safe + '"' + (n === cur ? ' selected' : '') + '>' + safe + '</option>';
            }).join('');
    }

    // ============ 单元格渲染(按列类型) ============
    function _cell(t, c) {
        var k = c.key;
        var v = (k === 'due') ? t.due : (t[k] !== undefined ? t[k] : (t.customFields && t.customFields[k]));

        // 内置专属
        if (k === 'title') {
            var title = v ? _esc(v) : '<span class="wt-empty">(无标题)</span>';
            return '<td class="wt-cell-title wt-editable" '
                +    'onclick="WorkTable.editText(' + t.id + ',\'title\')">' + title + '</td>';
        }
        if (k === 'assignee') {
            // T-121:UI 不再区分主+协,统一"平等多人";点击 → 单 input modal 逗号分隔
            //   数据层仍是 assignee(主)+ collaborators(协) — 前端拆解 + LLM 仍走双字段
            //   hover 头像区显示完整名单
            var name = v || '—';
            var collabs = Array.isArray(t.collaborators) ? t.collaborators : [];
            var collabHtml = '';
            if (collabs.length > 0) {
                var shown = collabs.slice(0, 3);
                var extra = collabs.length - shown.length;
                var allNames = [v].concat(collabs).filter(Boolean).join('、');
                collabHtml = '<span class="wt-collab-stack" title="' + _esc(allNames) + '">'
                    + shown.map(function(c) {
                        return '<span class="wt-avatar wt-avatar-xs" style="background:' + _avatarColor(c) + '">' + _esc(('' + c).slice(0, 1)) + '</span>';
                      }).join('')
                    + (extra > 0 ? '<span class="wt-collab-more">+' + extra + '</span>' : '')
                    + '</span>';
            }
            return '<td class="wt-editable" onclick="WorkTable.editAssigneeCombined(' + t.id + ')">'
                +    '<span class="wt-assignee">' + _avatarHTML(v || '?')
                +    '<span class="wt-aname">' + _esc(name) + '</span>'
                +    collabHtml
                +    '</span></td>';
        }
        if (k === 'priority') {
            var p = _prioBy(t.priority);
            return '<td><span class="wt-pill ' + p.cls + '" '
                +    'onclick="WorkTable.openPick(' + t.id + ',\'priority\',this)">'
                +    _esc(p.label) + '</span></td>';
        }
        if (c.type === 'status') {
            var s = _statusBy(t.status);
            return '<td><span class="wt-pill ' + s.cls + '" '
                +    'onclick="WorkTable.openPick(' + t.id + ',\'status\',this)">'
                +    '<span class="wt-pdot" style="background:' + s.dot + '"></span>'
                +    _esc(s.label) + '</span></td>';
        }

        switch (c.type) {
            case 'longtext': return '<td>' + _descCell(t, k) + '</td>';
            case 'select': {
                var has = v != null && v !== '';
                var cls = (k === 'freq') ? 'wt-fq' : 'wt-lv';
                return '<td>' + (has
                    ? '<span class="wt-pill ' + cls + '" onclick="WorkTable.openPick(' + t.id + ',\'' + _escAttr(k) + '\',this)">' + _esc(v) + '</span>'
                    : '<span class="wt-pill wt-pill-empty" onclick="WorkTable.openPick(' + t.id + ',\'' + _escAttr(k) + '\',this)">选择</span>')
                  + '</td>';
            }
            case 'multi': {
                var arr = Array.isArray(v) ? v : [];
                var inner = arr.length
                    ? arr.map(function(x) { return '<span class="wt-tagchip">' + _esc(x) + '</span>'; }).join('')
                    : '<span class="wt-empty">—</span>';
                return '<td class="wt-editable" onclick="WorkTable.openPick(' + t.id + ',\'' + _escAttr(k) + '\',this)">'
                    + inner + '</td>';
            }
            case 'number':
                return '<td class="wt-editable" onclick="WorkTable.editNumber(' + t.id + ',\'' + _escAttr(k) + '\')">'
                    + (v != null && v !== '' ? _esc(v) : '<span class="wt-empty">—</span>') + '</td>';
            case 'percent': {
                var p = Math.max(0, Math.min(100, +v || 0));
                // T-095:进度列点击复用 todo 的 slider 弹窗(openProgressDialog)
                // T-117:从横条改为 .progress-ring 环形(复用 components.css 的 todo 同款样式)
                return '<td class="wt-editable wt-cell-progress" onclick="WorkTable.editProgress(' + t.id + ',\'' + _escAttr(k) + '\')">'
                    + '<div class="progress-ring" style="--progress:' + p + '">'
                    +   '<span class="progress-ring-text">' + p + '</span>'
                    + '</div></td>';
            }
            case 'date': {
                // T-098:用 YYYY-MM-DD 比较(lex 顺序 = 时间顺序);兼容 'MM-DD' 简写自动补当前年
                var todayY = _todayYMD();
                var dueY = _normalizeDueLocal(v);
                var isOverdue = (k === 'due' && t.status !== 'done' && dueY && dueY < todayY);
                var overdueClass = isOverdue ? ' overdue' : '';
                var badge = '';
                if (isOverdue) {
                    var n = _daysOverdue(dueY, todayY);
                    badge = ' <span class="wt-overdue-badge" title="逾期 ' + n + ' 天">⏰ 逾期 ' + n + ' 天</span>';
                }
                // T-104:date 单元格点击改用 datepicker(spec § 5 + § 7.1),不再走 text input dialog
                return '<td class="wt-editable wt-due' + overdueClass + '" onclick="WorkTable.editDate(event,' + t.id + ',\'' + _escAttr(k) + '\')">'
                    + (v ? _esc(v) : '<span class="wt-empty">—</span>') + badge + '</td>';
            }
            case 'checkbox':
                return '<td><span class="wt-check" onclick="WorkTable.toggleCheck(' + t.id + ',\'' + _escAttr(k) + '\')">'
                    + (v ? '☑' : '☐') + '</span></td>';
            default: /* text */
                return '<td class="wt-editable" onclick="WorkTable.editText(' + t.id + ',\'' + _escAttr(k) + '\')">'
                    + (v ? _esc(v) : '<span class="wt-empty">—</span>') + '</td>';
        }
    }

    function _avatarHTML(n) {
        return '<span class="wt-avatar" style="background:' + _avatarColor(n) + '">' + _esc(('' + n).slice(0, 1)) + '</span>';
    }

    function _descCell(t, field) {
        var v = (field === 'desc') ? t.desc : (t.customFields && t.customFields[field]);
        var has = v && ('' + v).trim();
        if (!has) {
            return '<span class="wt-desc-add" onclick="WorkTable.openText(' + t.id + ',\'' + _escAttr(field) + '\')">+ 添加简介</span>';
        }
        var preview = ('' + v).replace(/\s+/g, ' ').trim();
        return '<div class="wt-desc-row">'
            + '<span class="wt-desc-text" title="点击展开看全文" '
            +    'onclick="WorkTable.openText(' + t.id + ',\'' + _escAttr(field) + '\')">' + _esc(preview) + '</span>'
            + '<button class="wt-desc-expand" '
            +    'onclick="WorkTable.openText(' + t.id + ',\'' + _escAttr(field) + '\')">展开</button>'
            + '</div>';
    }

    function _escAttr(s) { return ('' + s).replace(/'/g, "\\'"); }

    function _todayMD() {
        // MM-DD,用于看板/日历卡片的 overdue 颜色判断(向后兼容旧 demo 数据)
        var d = new Date();
        return ('0' + (d.getMonth() + 1)).slice(-2) + '-' + ('0' + d.getDate()).slice(-2);
    }
    // T-098:用 YYYY-MM-DD 比较 due。'MM-DD' 简写自动补当前年。
    function _todayYMD() {
        var d = new Date();
        return d.getFullYear() + '-' + ('0' + (d.getMonth() + 1)).slice(-2) + '-' + ('0' + d.getDate()).slice(-2);
    }
    function _normalizeDueLocal(due) {
        if (!due) return '';
        if (due.length === 10 && due.charAt(4) === '-' && due.charAt(7) === '-') return due;
        if (due.length === 5 && due.charAt(2) === '-') return (new Date()).getFullYear() + '-' + due;
        return due;
    }
    function _daysOverdue(dueY, todayY) {
        var d = new Date(dueY + 'T00:00:00');
        var t = new Date(todayY + 'T00:00:00');
        return Math.max(1, Math.floor((t - d) / 86400000));
    }

    // ============ 行渲染 ============
    // T-100:行加 data-id;# 列点击打开详情抽屉,其它单元格仍走原内联编辑。
    // T-103 B.2 + T-109 修正:stagger 仅在"加载/Tab/视图切换"时触发,不在编辑后触发。
    //   withStagger 由调用方(render(opts)→ block → _rowHTML)透传。
    function _rowHTML(t, num, withStagger) {
        var tds = '<td class="wt-num" onclick="WorkTable._openDetail(' + t.id + ')" title="点开打开详情">' + num + '</td>';
        var cols = Work.columns();
        for (var i = 0; i < cols.length; i++) tds += _cell(t, cols[i]);
        if (withStagger) {
            // stagger delay 35ms × row index;超过 20 行后封顶,避免最后一行等太久
            var delay = Math.min(num - 1, 20) * 35;
            return '<tr data-id="' + t.id + '" class="wt-entering" style="animation-delay:' + delay + 'ms">' + tds + '</tr>';
        }
        return '<tr data-id="' + t.id + '">' + tds + '</tr>';
    }

    // T-100:由 # 列调用打开详情抽屉,把当前可见行 id 顺序传过去支持上下翻
    function _openDetail(id) {
        if (typeof WorkDetail === 'undefined') return;
        var ids = _visibleRows().map(function(r) { return r.id; });
        WorkDetail.openDetail(id, ids);
    }

    // ============ 主渲染 ============
    // T-109:opts.stagger 控制行入场动效;
    //   true  → 行带 .wt-entering(用于首次加载 / 切 Tab / 切视图);
    //   false → 静默更新(单元格编辑 / 拖拽 / 阿宝工具创建 / 列设置变更等高频路径,避免动效骚扰)
    function render(opts) {
        opts = opts || {};
        var withStagger = !!opts.stagger;
        var cols = Work.columns();
        if (!cols || !cols.length) return;
        _refillFilter();

        // 总数胶囊
        var rows = _visibleRows();
        var tot = document.getElementById('wt-total');
        if (tot) tot.textContent = '共 ' + rows.length + ' 项';

        // 表格本体(只在 table 视图下渲染)
        var host = document.getElementById('wt-table-view');
        if (!host) return;

        // 计算表格总宽
        var sumW = SERIAL_W;
        for (var i = 0; i < cols.length; i++) sumW += (parseInt(cols[i].width, 10) || 130);

        // colgroup
        var colgroup = '<colgroup><col style="width:' + SERIAL_W + 'px">'
            + cols.map(function(c) {
                var w = (c.width || 130);
                return '<col style="width:' + w + 'px">';
            }).join('')
            + '</colgroup>';

        // thead — 表头不可点(原则 3);只在两列之间出拖动条;最右列右侧不放拖动条
        var thead = '<thead><tr><th class="wt-num-th">#</th>'
            + cols.map(function(c, i) {
                var rz = (i < cols.length - 1)
                    ? '<span class="wt-resizer" title="拖动调整左右两列的列宽" onmousedown="WorkTable._resizeStart(event,' + i + ')"></span>'
                    : '';
                return '<th>' + _esc(c.name) + rz + '</th>';
            }).join('')
            + '</tr></thead>';

        var span = cols.length + 1;
        var group = _groupValue();
        var n = 0, body = '';
        function block(list) { return list.map(function(t) { n++; return _rowHTML(t, n, withStagger); }).join(''); }

        if (!group) {
            body = block(rows);
        } else {
            var keys = [];
            rows.forEach(function(t) {
                var kv = t[group] || '';
                if (keys.indexOf(kv) < 0) keys.push(kv);
            });
            keys.forEach(function(k) {
                var label = (group === 'status') ? _statusBy(k).label : (k || '(空)');
                var items = rows.filter(function(t) { return (t[group] || '') === k; });
                body += '<tr class="wt-group-row"><td colspan="' + span + '">' + _esc(label) + ' · ' + items.length + '</td></tr>';
                body += block(items);
            });
        }
        // 加行按钮:仅底部(首行不放,避免误点)
        body += '<tr class="wt-add-row" onclick="WorkTable.addRow()"><td colspan="' + span + '">+ 新建任务</td></tr>';

        host.innerHTML = '<div class="wt-table-scroll">'
            + '<table class="wt-table" style="width:' + sumW + 'px">'
            + colgroup + thead + '<tbody>' + body + '</tbody></table></div>';
    }

    // ============ 单元格编辑函数(T-095:全部走居中 modal,禁用原生 prompt)============
    function editText(id, field) {
        var t = Work.rowById(id); if (!t) return;
        var c = Work.colByKey(field);
        var cur = (field === 'due') ? (t.due || '') : (t[field] != null ? t[field] : (t.customFields && t.customFields[field]) || '');
        openTextInputDialog({
            label: '修改「' + (c ? c.name : field) + '」',
            initial: cur,
            type: 'text',
            onConfirm: function(v) { _saveField(id, c, field, ('' + v).trim()); },
        });
    }

    // T-104:date 单元格 → 弹 datepicker(复用 todo 同款 popover,callback 模式)
    // 注意:WorkDetail 抽屉里的截止日字段也走这里(同一 editText / editDate 通道)。
    function editDate(ev, id, field) {
        if (ev && ev.stopPropagation) ev.stopPropagation();
        var t = Work.rowById(id); if (!t) return;
        var c = Work.colByKey(field);
        var cur = c && c.builtin ? (t[field] || '') : ((t.customFields && t.customFields[field]) || '');
        // anchor = 被点的单元格本身(<td>);未给 event 时用 body 中心(降级)
        var anchor = (ev && ev.currentTarget) || (ev && ev.target) || document.body;
        if (typeof window.toggleDatePicker !== 'function') {
            // 极端降级:如果 datepicker.js 没加载,回退到 text input
            console.warn('[WorkTable] datepicker not available, fallback to text input');
            return editText(id, field);
        }
        window.toggleDatePicker({
            anchor: anchor,
            initial: cur,
            onSelect: function(ymd) {
                _saveField(id, c, field, ymd);
            },
        });
    }

    function editNumber(id, field) {
        var t = Work.rowById(id); if (!t) return;
        var c = Work.colByKey(field);
        var cur = t[field] != null ? t[field] : (t.customFields && t.customFields[field]);
        openTextInputDialog({
            label: '修改「' + (c ? c.name : field) + '」',
            initial: cur == null ? '' : String(cur),
            type: 'number',
            onConfirm: function(n) { _saveField(id, c, field, isNaN(n) ? 0 : n); },
        });
    }

    function toggleCheck(id, field) {
        var t = Work.rowById(id); if (!t) return;
        var c = Work.colByKey(field);
        var cur = c && c.builtin ? t[field] : (t.customFields && t.customFields[field]);
        _saveField(id, c, field, !cur);
    }

    // T-095 § 7.1 问题 4:进度列复用 todo 的 .progress-ring 同款 slider 弹窗。
    // 与 todo 不同之处:work-table 这里 100% 二次确认后 status 也设为 'done'。
    // T-103 B.3:二次确认通过 → 彩纸 + 行温柔淡出(冻结 render 1100ms 避免被打断)
    function editProgress(id, field) {
        var t = Work.rowById(id); if (!t) return;
        var c = Work.colByKey(field);
        var cur;
        if (c && c.builtin) cur = +t[field] || 0;
        else cur = +(t.customFields && t.customFields[field]) || 0;
        openProgressDialog({
            label: t.title || '(无标题)',
            currentProgress: cur,
            onConfirm: function(p) { _saveField(id, c, field, p); },
            onComplete: function() {
                // 100% 时二次确认:同时把状态切到 done(work-table 业务规则)。
                window.AppUtils.showConfirm(
                    '确定要将此任务标记为已完成吗？\n完成后状态变为已完成（done）。',
                    function() {
                        _doCompletionWithCelebration(id, c, field);
                    },
                    { confirmText: '确定完成' }
                );
            },
        });
    }

    // T-103 B.3:完成动画 — 彩纸 + 行淡出;期间冻结 render 避免被打断
    function _doCompletionWithCelebration(id, c, field) {
        // 1) 立刻发 PATCH(乐观更新);但冻结 render 1100ms,让动画跑完再重渲
        if (Work.freezeRender) Work.freezeRender(1100);

        if (field === 'progress') {
            Work.updateRow(id, { progress: 100, status: 'done' });
        } else if (c && c.builtin) {
            var patch = { status: 'done' };
            patch[field] = 100;
            Work.updateRow(id, patch);
        } else {
            var cf = {}; cf[field] = 100;
            Work.updateRow(id, { customFields: cf, status: 'done' });
        }

        // 2) 找到当前行 + 彩纸锚点(进度单元格;T-117:.wt-progress 横条已改环形 .progress-ring)
        var row = document.querySelector('#wt-table-view tr[data-id="' + id + '"]');
        var anchor = row ? (row.querySelector('.progress-ring') || row.querySelector('.wt-cell-progress') || row.querySelector('.wt-num') || row) : document.body;
        _confettiBurst(anchor);

        // 3) 700ms 后给行加 .wt-removing,触发 translateX -30 opacity 0 缓动
        setTimeout(function() {
            if (row && row.parentNode) row.classList.add('wt-removing');
        }, 700);
        // 4) Work.freezeRender 期满会自动调 render(),replace DOM,自然结束动画
    }

    // T-103 B.3:14 片彩纸喷射 1.4s
    var _CONFETTI_COLORS = ['#7C4DFF', '#14B8A6', '#E0A23B', '#3B82F6', '#E11D48', '#10B981'];
    function _confettiBurst(anchor) {
        if (!anchor || !anchor.getBoundingClientRect) return;
        var rect = anchor.getBoundingClientRect();
        var cx = rect.left + rect.width / 2;
        var cy = rect.top + rect.height / 2;
        for (var i = 0; i < 14; i++) {
            var c = document.createElement('div');
            c.className = 'wt-confetti';
            c.style.left = cx + 'px';
            c.style.top  = cy + 'px';
            c.style.background = _CONFETTI_COLORS[i % _CONFETTI_COLORS.length];
            c.style.setProperty('--r', (Math.random() * 720 - 360) + 'deg');
            c.style.setProperty('--x', (Math.random() * 200 - 100) + 'px');
            c.style.setProperty('--initR', (Math.random() * 360) + 'deg');
            document.body.appendChild(c);
            // 1.5s 后清理(动画 1.4s + 缓冲)
            (function(el) { setTimeout(function() { el.remove(); }, 1500); })(c);
        }
    }

    function _saveField(id, c, field, value) {
        // 内置字段直接放 patch 顶层;自定义列走 customFields。
        var patch;
        if (c && !c.builtin) {
            var cf = {}; cf[field] = value;
            patch = { customFields: cf };
        } else {
            patch = {};
            patch[field] = value;
        }
        Work.updateRow(id, patch);
    }

    // 单选/多选/状态/优先级 → 弹候选
    function openPick(id, field, anchor) {
        var t = Work.rowById(id); if (!t) return;
        var c = Work.colByKey(field);
        var options;
        if (field === 'status') {
            options = STATUS.map(function(s) { return { key: s.key, label: s.label }; });
        } else if (field === 'priority') {
            options = PRIORITY.map(function(p) { return { key: p.key, label: p.label }; });
        } else {
            options = (c && c.options || []).map(function(o) { return { key: o, label: o }; });
        }
        var isMulti = c && c.type === 'multi';
        var current;
        if (isMulti) {
            var raw = (c.builtin ? t[field] : (t.customFields && t.customFields[field])) || [];
            current = Array.isArray(raw) ? raw : [];
        } else {
            var v = (c && c.builtin) ? t[field] : (t.customFields && t.customFields[field]);
            current = (v != null && v !== '') ? [v] : [];
        }
        WorkPick.open({
            anchor: anchor,
            options: options,
            current: current,
            isMulti: isMulti,
            onConfirm: function(chosen) {
                var patch;
                if (isMulti) {
                    if (c.builtin) { patch = {}; patch[field] = chosen; }
                    else { var cf = {}; cf[field] = chosen; patch = { customFields: cf }; }
                } else {
                    var k = chosen[0] != null ? chosen[0] : '';
                    if (field === 'status') {
                        patch = { status: k };
                        // 自动 progress=100(后端也会做,前端先反应一下保持一致)
                    } else if (field === 'priority') {
                        patch = { priority: k };
                    } else if (c.builtin) {
                        patch = {}; patch[field] = k;
                    } else {
                        var cf2 = {}; cf2[field] = k; patch = { customFields: cf2 };
                    }
                }
                Work.updateRow(id, patch);
            },
        });
    }

    // ============ 长文本弹层 ============
    var _textCtx = null;  // { id, field }
    function openText(id, field) {
        var t = Work.rowById(id); if (!t) return;
        var c = Work.colByKey(field);
        var cur = c && c.builtin ? t[field] : (t.customFields && t.customFields[field]);
        _textCtx = { id: id, field: field, builtin: !!(c && c.builtin) };
        var titleEl = document.getElementById('wt-text-title');
        var subEl = document.getElementById('wt-text-sub');
        var ta = document.getElementById('wt-text-area');
        if (titleEl) titleEl.textContent = t.title || '(无标题)';
        if (subEl) subEl.textContent = (c ? c.name : '长文本') + ' · 可输入大段文字、换行、分点';
        if (ta) { ta.value = cur || ''; setTimeout(function() { ta.focus(); }, 0); }
        var modal = document.getElementById('wt-text-modal');
        if (modal) modal.classList.add('open');
    }
    function closeText() {
        var modal = document.getElementById('wt-text-modal');
        if (modal) modal.classList.remove('open');
        _textCtx = null;
    }
    function saveText() {
        if (!_textCtx) { closeText(); return; }
        var ta = document.getElementById('wt-text-area');
        var v = ta ? ta.value : '';
        var patch;
        if (_textCtx.builtin) { patch = {}; patch[_textCtx.field] = v; }
        else { var cf = {}; cf[_textCtx.field] = v; patch = { customFields: cf }; }
        var id = _textCtx.id;
        closeText();
        Work.updateRow(id, patch);
    }

    // ============ 加行:新建任务弹窗(T-128 重做 T-124,配置驱动完整表单)============
    // 工具栏主按钮 + 表格底部尾行两个入口都走 addRow() → openCreateDialog()。
    // 弹窗遍历 Work.columns()(列配置真相来源)按每列 type 渲染控件;新增列自动多字段。
    // 仅标题必填;状态候选不给"已完成"(完成走 §7.1 进度确认链)。
    // 提交以原默认 payload 打底(保证后端必填字段)再叠加用户输入,一次 Work.createRow();
    // createRow 内部走静默 render(不 stagger,防 T-109)。
    function addRow() { openCreateDialog(); }

    var _createEl = null;

    function _ensureCreateStyle() {
        if (document.getElementById('wt-create-style')) return;
        var css = ''
          + '.wt-create-ov{position:fixed;inset:0;background:rgba(0,0,0,.32);display:flex;align-items:center;justify-content:center;z-index:1078;}'
          + '.wt-create-box{background:var(--card-bg,#fff);border-radius:12px;width:min(520px,94vw);max-height:88vh;box-shadow:0 8px 30px rgba(0,0,0,.18);display:flex;flex-direction:column;}'
          + '.wt-create-hd{padding:16px 18px;font-size:16px;font-weight:600;color:var(--text-color,#333);border-bottom:1px solid var(--border-color,#e3e5e8);}'
          + '.wt-create-bd{padding:14px 18px;overflow:auto;display:flex;flex-direction:column;gap:12px;}'
          + '.wt-create-f{display:flex;flex-direction:column;gap:5px;}'
          + '.wt-create-lb{font-size:13px;color:var(--text-muted,#888);}'
          + '.wt-create-in,.wt-create-ta{width:100%;border:1px solid var(--border-color,#e3e5e8);border-radius:8px;padding:9px 11px;font-size:14px;box-sizing:border-box;font-family:inherit;}'
          + '.wt-create-ta{min-height:78px;resize:vertical;}'
          + '.wt-create-in:focus,.wt-create-ta:focus{outline:none;border-color:var(--primary-color,#4c6ef5);}'
          + '.wt-create-pk{width:100%;border:1px solid var(--border-color,#e3e5e8);border-radius:8px;padding:9px 11px;font-size:14px;cursor:pointer;box-sizing:border-box;color:var(--text-color,#333);}'
          + '.wt-create-pk:hover{border-color:var(--primary-color,#4c6ef5);}'
          + '.wt-create-pk.empty{color:var(--text-muted,#aaa);}'
          + '.wt-create-cbx{display:flex;align-items:center;gap:8px;cursor:pointer;font-size:14px;color:var(--text-color,#333);}'
          + '.wt-create-bar{display:flex;justify-content:flex-end;gap:8px;padding:14px 18px;border-top:1px solid var(--border-color,#e3e5e8);}'
          + '.wt-create-btn{padding:8px 18px;border-radius:8px;border:none;cursor:pointer;font-size:14px;}'
          + '.wt-create-ok{background:var(--primary-color,#4c6ef5);color:#fff;}'
          + '.wt-create-ok:disabled{opacity:.5;cursor:not-allowed;}'
          + '.wt-create-cancel{background:var(--hover-bg,#f2f3f5);color:var(--text-color,#333);}';
        var st = document.createElement('style');
        st.id = 'wt-create-style';
        st.textContent = css;
        document.head.appendChild(st);
    }

    // 状态候选:新建不给"已完成"(完成必须走 §7.1 进度弹窗确认链 + 彩纸)
    function _createStatusOpts() {
        return STATUS.filter(function(s) { return s.key !== 'done'; })
                     .map(function(s) { return { key: s.key, label: s.label }; });
    }
    function _createPrioOpts() {
        return PRIORITY.map(function(p) { return { key: p.key, label: p.label }; });
    }

    function openCreateDialog() {
        closeCreate();
        _ensureCreateStyle();
        var cols = Work.columns();
        if (!cols || !cols.length) { showToast('列配置未加载', 'warning'); return; }

        var state = {};

        var ov = document.createElement('div');
        ov.className = 'wt-create-ov';
        ov.addEventListener('click', function(e) { if (e.target === ov) closeCreate(); });

        var box = document.createElement('div');
        box.className = 'wt-create-box';

        var hd = document.createElement('div');
        hd.className = 'wt-create-hd';
        hd.textContent = '新建任务';
        box.appendChild(hd);

        var bd = document.createElement('div');
        bd.className = 'wt-create-bd';

        var okBtn = null;
        function refresh() { if (okBtn) okBtn.disabled = !((state.title || '').trim()); }

        for (var i = 0; i < cols.length; i++) {
            bd.appendChild(_buildCreateField(cols[i], state, refresh));
        }
        box.appendChild(bd);

        var bar = document.createElement('div');
        bar.className = 'wt-create-bar';
        var cancel = document.createElement('button');
        cancel.className = 'wt-create-btn wt-create-cancel';
        cancel.textContent = '取消';
        cancel.addEventListener('click', closeCreate);
        okBtn = document.createElement('button');
        okBtn.className = 'wt-create-btn wt-create-ok';
        okBtn.textContent = '创建';
        okBtn.disabled = true;
        okBtn.addEventListener('click', function() { _submitCreate(state); });
        bar.appendChild(cancel);
        bar.appendChild(okBtn);
        box.appendChild(bar);

        ov.appendChild(box);
        document.body.appendChild(ov);
        _createEl = ov;
        document.addEventListener('keydown', _onCreateKey);

        var first = box.querySelector('input,textarea');
        if (first) first.focus();
    }

    function _onCreateKey(e) { if (e.key === 'Escape') closeCreate(); }

    function closeCreate() {
        if (_createEl && _createEl.parentNode) _createEl.parentNode.removeChild(_createEl);
        _createEl = null;
        document.removeEventListener('keydown', _onCreateKey);
    }

    function _buildCreateField(col, state, onChange) {
        var row = document.createElement('div');
        row.className = 'wt-create-f';
        var isTitle = (col.key === 'title');

        if (col.type === 'checkbox') {
            var clab = document.createElement('label');
            clab.className = 'wt-create-cbx';
            var cb = document.createElement('input');
            cb.type = 'checkbox';
            cb.addEventListener('change', function() { state[col.key] = cb.checked; });
            clab.appendChild(cb);
            clab.appendChild(document.createTextNode(col.name || col.key));
            row.appendChild(clab);
            return row;
        }

        var label = document.createElement('div');
        label.className = 'wt-create-lb';
        label.textContent = (col.name || col.key) + (isTitle ? ' *' : '');
        row.appendChild(label);

        var ctl;
        if (col.key === 'assignee') {
            ctl = _mkCreateInput('text', '逗号分隔,第一个为主负责人;留空=我');
            ctl.addEventListener('input', function() { state[col.key] = ctl.value; });
        } else if (col.type === 'status' || col.key === 'status') {
            ctl = _mkCreatePick(col, state, _createStatusOpts(), false);
        } else if (col.key === 'priority') {
            ctl = _mkCreatePick(col, state, _createPrioOpts(), false);
        } else if (col.type === 'select') {
            ctl = _mkCreatePick(col, state, (col.options || []).map(function(o) { return { key: o, label: o }; }), false);
        } else if (col.type === 'multi') {
            ctl = _mkCreatePick(col, state, (col.options || []).map(function(o) { return { key: o, label: o }; }), true);
        } else if (col.type === 'date') {
            ctl = _mkCreateDate(col, state);
        } else if (col.type === 'number' || col.type === 'percent') {
            ctl = _mkCreateInput('number', col.type === 'percent' ? '0-100;留空=0' : '');
            if (col.type === 'percent') { ctl.min = '0'; ctl.max = '100'; }
            ctl.addEventListener('input', function() { state[col.key] = ctl.value; });
        } else if (col.type === 'longtext') {
            ctl = document.createElement('textarea');
            ctl.className = 'wt-create-ta';
            ctl.placeholder = '可空';
            ctl.addEventListener('input', function() { state[col.key] = ctl.value; });
        } else {
            ctl = _mkCreateInput('text', isTitle ? '任务标题(必填)' : '');
            ctl.addEventListener('input', function() { state[col.key] = ctl.value; if (isTitle) onChange(); });
        }
        row.appendChild(ctl);
        return row;
    }

    function _mkCreateInput(type, ph) {
        var el = document.createElement('input');
        el.type = type; el.className = 'wt-create-in'; el.placeholder = ph || '';
        return el;
    }

    // select/status/priority/multi → 复用 WorkPick 候选 UI
    function _mkCreatePick(col, state, options, isMulti) {
        var btn = document.createElement('div');
        btn.className = 'wt-create-pk empty';
        var EMPTY = isMulti ? '(无)' : '(默认)';
        btn.textContent = EMPTY;
        function labelFor(keys) {
            var labs = options.filter(function(o) { return keys.indexOf(o.key) >= 0; })
                              .map(function(o) { return o.label; });
            return labs.length ? labs.join('、') : EMPTY;
        }
        btn.addEventListener('click', function() {
            var cur = state[col.key];
            var current = isMulti ? (Array.isArray(cur) ? cur : []) : (cur != null && cur !== '' ? [cur] : []);
            WorkPick.open({
                anchor: btn,
                options: options,
                current: current,
                isMulti: !!isMulti,
                onConfirm: function(chosen) {
                    if (isMulti) {
                        state[col.key] = chosen.slice();
                        btn.classList.toggle('empty', chosen.length === 0);
                    } else {
                        state[col.key] = chosen[0] != null ? chosen[0] : '';
                        btn.classList.toggle('empty', !state[col.key]);
                    }
                    btn.textContent = labelFor(isMulti ? chosen : (state[col.key] ? [state[col.key]] : []));
                },
            });
            var pop = document.getElementById('wt-pick');
            var bdg = document.getElementById('wt-pick-bd');
            if (pop) pop.style.zIndex = '1200';
            if (bdg) bdg.style.zIndex = '1199';
        });
        return btn;
    }

    // date → 复用 datepicker(T-104 callback 模式)
    function _mkCreateDate(col, state) {
        var btn = document.createElement('div');
        btn.className = 'wt-create-pk empty';
        btn.textContent = '(无)';
        btn.addEventListener('click', function() {
            if (typeof window.toggleDatePicker !== 'function') { showToast('日期选择器未加载', 'warning'); return; }
            window.toggleDatePicker({
                anchor: btn,
                initial: state[col.key] || '',
                onSelect: function(ymd) {
                    state[col.key] = ymd;
                    btn.textContent = ymd || '(无)';
                    btn.classList.toggle('empty', !ymd);
                },
            });
            var pop = document.getElementById('date-popover');
            if (pop) pop.style.zIndex = '1200';
        });
        return btn;
    }

    // 提交:默认 payload 打底(保证后端必填字段)+ 用户输入覆盖;一次 createRow
    function _submitCreate(state) {
        var title = (state.title || '').trim();
        if (!title) { showToast('标题不能为空', 'warning'); return; }

        var payload = {
            title: title,
            assignee: '我',
            level: '个人',
            freq: '一次性',
            status: 'todo',
            priority: 'mid',
            progress: 0,
        };
        var customFields = {};
        var hasCustom = false;
        var cols = Work.columns();

        for (var i = 0; i < cols.length; i++) {
            var col = cols[i];
            var key = col.key;
            if (key === 'title') continue;
            var raw = state[key];

            if (key === 'assignee') {
                var text = ('' + (raw || '')).trim();
                if (!text) continue;
                var parts = text.split(/[,，;；]\s*/).map(function(s) { return s.trim(); }).filter(Boolean);
                var seen = {}, uniq = [];
                parts.forEach(function(p) { if (!seen[p]) { seen[p] = 1; uniq.push(p); } });
                payload.assignee = uniq[0] || '我';
                if (uniq.length > 1) payload.collaborators = uniq.slice(1);
                continue;
            }
            if (col.type === 'multi') {
                if (Array.isArray(raw) && raw.length) {
                    if (col.builtin) payload[key] = raw.slice();
                    else { customFields[key] = raw.slice(); hasCustom = true; }
                }
                continue;
            }
            if (col.type === 'checkbox') {
                if (raw === true) {
                    if (col.builtin) payload[key] = true;
                    else { customFields[key] = true; hasCustom = true; }
                }
                continue;
            }
            if (raw === undefined || raw === null) continue;
            if (typeof raw === 'string' && raw.trim() === '') continue;

            var val;
            if (col.type === 'number' || col.type === 'percent') {
                var n = parseInt(raw, 10);
                if (isNaN(n)) continue;
                if (col.type === 'percent') { if (n < 0) n = 0; if (n > 100) n = 100; }
                val = n;
            } else {
                val = ('' + raw).trim();
            }
            if (col.builtin) payload[key] = val;
            else { customFields[key] = val; hasCustom = true; }
        }
        if (hasCustom) payload.customFields = customFields;

        closeCreate();
        var p = Work.createRow(payload);   // createRow 内部乐观更新 + 静默 render()
        if (p && typeof p.then === 'function') {
            p.then(function() { showToast('已创建', 'success'); })
             .catch(function(e) { console.error('[work-table]', e); showToast('创建失败', 'error'); });
        } else {
            showToast('已创建', 'success');
        }
    }

    // ============ 列宽拖动:Excel 风格 ============
    // 拖动时只改两列 inline width,不触发整表 re-render(丝滑);
    // mouseup 时把新宽度回写到 Work.columns 并 PUT /api/work/columns。
    var _rs = null;  // { col, nextCol, idx, startX, startW, nextW, handle }
    function _resizeStart(e, idx) {
        e.preventDefault(); e.stopPropagation();
        var table = document.querySelector('#wt-table-view table.wt-table');
        if (!table) return;
        var cols = table.querySelectorAll('colgroup col');
        var ths  = table.querySelectorAll('thead th');
        var col = cols[idx + 1], nextCol = cols[idx + 2];
        var th  = ths[idx + 1],  nextTh  = ths[idx + 2];
        if (!col || !nextCol) return;
        _rs = {
            col: col, nextCol: nextCol, idx: idx, handle: e.target,
            startX: e.clientX,
            startW: th.getBoundingClientRect().width,
            nextW:  nextTh.getBoundingClientRect().width,
        };
        if (e.target && e.target.classList) e.target.classList.add('active');
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';
        document.addEventListener('mousemove', _resizeMove);
        document.addEventListener('mouseup', _resizeEnd);
    }
    function _resizeMove(e) {
        if (!_rs) return;
        var cols = Work.columns();
        var leftMin  = parseInt((cols[_rs.idx]     || {}).minWidth, 10) || 60;
        var rightMin = parseInt((cols[_rs.idx + 1] || {}).minWidth, 10) || 60;
        var delta = e.clientX - _rs.startX;
        var maxDelta =   _rs.nextW  - rightMin;
        var minDelta = -(_rs.startW - leftMin);
        delta = Math.max(minDelta, Math.min(maxDelta, delta));
        _rs.col.style.width     = (_rs.startW + delta) + 'px';
        _rs.nextCol.style.width = (_rs.nextW  - delta) + 'px';
    }
    function _resizeEnd() {
        if (!_rs) return;
        var cols = Work.columns();
        var newLeftW  = parseInt(_rs.col.style.width, 10);
        var newRightW = parseInt(_rs.nextCol.style.width, 10);
        if (cols[_rs.idx])     cols[_rs.idx].width     = newLeftW;
        if (cols[_rs.idx + 1]) cols[_rs.idx + 1].width = newRightW;

        // 持久化(批量保存这两列的新宽度)
        Work.saveColumnPatches([
            { key: cols[_rs.idx].key,     width: newLeftW  },
            { key: cols[_rs.idx + 1].key, width: newRightW },
        ]);

        if (_rs.handle && _rs.handle.classList) _rs.handle.classList.remove('active');
        document.body.style.cursor = '';
        document.body.style.userSelect = '';
        document.removeEventListener('mousemove', _resizeMove);
        document.removeEventListener('mouseup', _resizeEnd);
        _rs = null;
    }

    // T-121:责任人 + 协作者合并编辑(单 input,逗号分隔自动拆解)
    //   按 memory single-user-simplicity:Boris 一人用,UI 不区分主+协,统一"平等多人"
    //   数据层仍走 T-119 的 assignee + collaborators 双字段(后端 + LLM 工具不变)
    function editAssigneeCombined(id) {
        var t = Work.rowById(id);
        if (!t) return;
        var initial = '';
        if (t.assignee) initial = t.assignee;
        if (Array.isArray(t.collaborators) && t.collaborators.length > 0) {
            initial = (initial ? initial + '，' : '') + t.collaborators.join('，');
        }
        openTextInputDialog({
            label: '修改「责任人」',
            initial: initial,
            type: 'text',
            placeholder: '多人用逗号分隔,如:我，张三，李四',
            onConfirm: function(v) {
                var text = ('' + v).trim();
                // 中文 / 英文 逗号 / 分号 都识别
                var parts = text.split(/[,，;；]\s*/).map(function(s) { return s.trim(); }).filter(Boolean);
                // 去重保留首次出现位置
                var seen = {};
                var unique = [];
                parts.forEach(function(p) {
                    if (!seen[p]) { seen[p] = 1; unique.push(p); }
                });
                var assignee = unique[0] || '';
                var collaborators = unique.slice(1);
                Work.updateRow(id, { assignee: assignee, collaborators: collaborators });
            },
        });
    }

    // T-119:协作者多选编辑(候选来自所有现存 assignee+collaborators 去重,剔除当前主责任人)
    // T-121 后:已不在 UI 流程里使用(抽屉协作者字段移除),保留为公共 API 供未来扩展
    function editCollaborators(id) {
        var t = Work.rowById(id);
        if (!t) return;
        var names = {};
        Work.rows().forEach(function(r) {
            if (r.assignee) names[r.assignee] = 1;
            if (Array.isArray(r.collaborators)) {
                r.collaborators.forEach(function(c) { if (c) names[c] = 1; });
            }
        });
        if (t.assignee) delete names[t.assignee];   // 不允许主责任人当协作者
        var options = Object.keys(names).sort().map(function(n) { return { key: n, label: n }; });
        if (options.length === 0) {
            if (typeof showToast === 'function') {
                showToast('还没有候选协作者 — 先去其它任务填责任人', 'info');
            }
            return;
        }
        var anchor = document.getElementById('wt-d-field-collaborators')
                  || document.querySelector('#wt-table-view tr[data-id="' + id + '"] .wt-assignee')
                  || document.body;
        var current = Array.isArray(t.collaborators) ? t.collaborators.slice() : [];
        WorkPick.open({
            anchor: anchor,
            options: options,
            current: current,
            isMulti: true,
            onConfirm: function(chosen) {
                var clean = (chosen || []).filter(function(n) {
                    return n && n !== t.assignee;
                });
                Work.updateRow(id, { collaborators: clean });
            }
        });
    }

    return {
        render: render,
        addRow: addRow,
        openCreateDialog: openCreateDialog,
        editText: editText,
        editDate: editDate,   // T-104
        editNumber: editNumber,
        editProgress: editProgress,
        editCollaborators: editCollaborators,   // T-119(保留 API,T-121 后 UI 不主动调用)
        editAssigneeCombined: editAssigneeCombined,   // T-121
        toggleCheck: toggleCheck,
        openPick: openPick,
        openText: openText,
        closeText: closeText,
        saveText: saveText,
        // 给 thead 行内 onmousedown 用
        _resizeStart: _resizeStart,
        // 给 work-board.js 复用(头像配色 / 状态/优先级元数据)
        _avatar: _avatarHTML,
        _statusBy: _statusBy,
        _prioBy: _prioBy,
        _STATUS: STATUS,
        _PRIORITY: PRIORITY,
        _esc: _esc,
        _todayMD: _todayMD,
        _openDetail: _openDetail,
        _visibleRows: _visibleRows,
    };
})();
