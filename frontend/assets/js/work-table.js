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

    // ============ 头像配色(4 色低饱和)============
    var PAL = ['#7C4DFF', '#14B8A6', '#E0A23B', '#3B82F6'];
    var _avatarColors = {};
    function _avatarColor(name) {
        if (!_avatarColors[name]) {
            _avatarColors[name] = PAL[Object.keys(_avatarColors).length % PAL.length];
        }
        return _avatarColors[name];
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
        var f = _filterValue();
        var rows = Work.rows();
        if (!f) return rows.slice();
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
            var name = v || '—';
            return '<td class="wt-editable" onclick="WorkTable.editText(' + t.id + ',\'assignee\')">'
                +    '<span class="wt-assignee">' + _avatarHTML(v || '?')
                +    '<span class="wt-aname">' + _esc(name) + '</span></span></td>';
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
                return '<td class="wt-editable" onclick="WorkTable.editNumber(' + t.id + ',\'' + _escAttr(k) + '\')">'
                    + '<div class="wt-progress"><span class="wt-track"><span class="wt-fill" style="width:' + p + '%"></span></span>'
                    + '<span class="wt-pct">' + p + '%</span></div></td>';
            }
            case 'date': {
                var today = _todayMD();
                var overdue = (k === 'due' && t.status !== 'done' && v && v < today) ? ' overdue' : '';
                return '<td class="wt-editable wt-due' + overdue + '" onclick="WorkTable.editText(' + t.id + ',\'' + _escAttr(k) + '\')">'
                    + (v ? _esc(v) : '<span class="wt-empty">—</span>') + '</td>';
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
        // MM-DD,用于 overdue 判断;前端只比较字符串大小,所以 MM-DD < MM-DD 有效。
        var d = new Date();
        var m = ('0' + (d.getMonth() + 1)).slice(-2);
        var dd = ('0' + d.getDate()).slice(-2);
        return m + '-' + dd;
    }

    // ============ 行渲染 ============
    function _rowHTML(t, num) {
        var tds = '<td class="wt-num">' + num + '</td>';
        var cols = Work.columns();
        for (var i = 0; i < cols.length; i++) tds += _cell(t, cols[i]);
        return '<tr>' + tds + '</tr>';
    }

    // ============ 主渲染 ============
    function render() {
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
        function block(list) { return list.map(function(t) { n++; return _rowHTML(t, n); }).join(''); }

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

    // ============ 单元格编辑函数 ============
    function editText(id, field) {
        var t = Work.rowById(id); if (!t) return;
        var c = Work.colByKey(field);
        var cur = (field === 'due') ? (t.due || '') : (t[field] != null ? t[field] : (t.customFields && t.customFields[field]) || '');
        var v = prompt('修改「' + (c ? c.name : field) + '」:', cur);
        if (v === null) return;
        _saveField(id, c, field, v.trim());
    }

    function editNumber(id, field) {
        var t = Work.rowById(id); if (!t) return;
        var c = Work.colByKey(field);
        var cur = t[field] != null ? t[field] : (t.customFields && t.customFields[field]);
        var v = prompt('修改「' + (c ? c.name : field) + '」(数字):', cur == null ? '' : cur);
        if (v === null) return;
        var n = parseFloat(v);
        if (isNaN(n)) n = 0;
        _saveField(id, c, field, n);
    }

    function toggleCheck(id, field) {
        var t = Work.rowById(id); if (!t) return;
        var c = Work.colByKey(field);
        var cur = c && c.builtin ? t[field] : (t.customFields && t.customFields[field]);
        _saveField(id, c, field, !cur);
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

    // ============ 加行 ============
    function addRow() {
        var name = prompt('新任务标题:');
        if (!name || !name.trim()) return;
        Work.createRow({
            title: name.trim(),
            assignee: '我',
            level: '个人',
            freq: '一次性',
            status: 'todo',
            priority: 'mid',
            progress: 0,
        });
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

    return {
        render: render,
        addRow: addRow,
        editText: editText,
        editNumber: editNumber,
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
    };
})();
