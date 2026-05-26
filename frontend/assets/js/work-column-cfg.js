// ========== WorkColumnCfg — 列设置弹层 (T-094) ==========
//
// 唯一入口:WorkColumnCfg.open()
// 列编辑只走这里(spec 原则 3:低频操作不与高频误触共用热区,表头本身不可点)。
//
// 面板内每一列可:
//   - 改列名(input onchange → 立刻 PATCH 单列)
//   - 改类型(select → 同上)
//   - 单选/多选类型可编辑选项(增/删/改名)
//   - 拖动 ≡ 调整列顺序(spec § 7.4)—— 只有 ≡ 手柄触发,inputs/select 不触发
//   - 删除列(只有非内置可删;内置带 🔒)
// 底部「+ 新增列」(spec § 7.2)
//
// 数据写回:Work.saveColumnPatches / Work.addColumn / Work.removeColumn

var WorkColumnCfg = (function() {
    var TYPES = [
        { key: 'text',     label: '单行文本' },
        { key: 'longtext', label: '长文本' },
        { key: 'select',   label: '单选' },
        { key: 'multi',    label: '多选' },
        { key: 'number',   label: '数字' },
        { key: 'percent',  label: '百分比' },
        { key: 'date',     label: '日期' },
        { key: 'checkbox', label: '勾选' },
        { key: 'status',   label: '状态' },
    ];
    var TYPE_HINT = {
        text: '单元格内点击输入短文字',
        longtext: '折叠成图标 / 预览,点击弹层编辑大段文字',
        number: '纯数字,点击输入',
        percent: '0–100,显示为进度条',
        date: '点击输入日期',
        checkbox: '勾选框,是 / 否',
    };

    function _esc(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    }

    function open() {
        _renderList();
        var m = document.getElementById('wt-col-modal');
        if (m) m.classList.add('open');
    }
    function close() {
        var m = document.getElementById('wt-col-modal');
        if (m) m.classList.remove('open');
    }

    function _renderList() {
        var cols = Work.columns();
        var host = document.getElementById('wt-col-list');
        if (!host) return;
        var html = '';
        cols.forEach(function(c, i) {
            html += '<div class="wt-colcfg" data-idx="' + i + '"'
                +    ' ondragover="WorkColumnCfg._dragOver(event,' + i + ')"'
                +    ' ondragleave="WorkColumnCfg._dragLeave(' + i + ')"'
                +    ' ondrop="WorkColumnCfg._drop(event,' + i + ')">'
                +  '<div class="wt-colcfg-top">'
                +  '<span class="wt-colcfg-handle" draggable="true"'
                +    ' ondragstart="WorkColumnCfg._dragStart(event,' + i + ')"'
                +    ' ondragend="WorkColumnCfg._dragEnd()"'
                +    ' title="按住拖动 ↕ 调整列顺序">≡</span>'
                +  '<input class="wt-colcfg-name" value="' + _esc(c.name) + '"'
                +    ' onchange="WorkColumnCfg._renameCol(' + i + ',this.value)">'
                +  '<select class="wt-colcfg-type" onchange="WorkColumnCfg._setType(' + i + ',this.value)">';
            TYPES.forEach(function(tp) {
                html += '<option value="' + tp.key + '"' + (tp.key === c.type ? ' selected' : '') + '>' + tp.label + '</option>';
            });
            html += '</select>';
            if (c.builtin) {
                html += '<span class="wt-colcfg-lock" title="内置列,不可删除">🔒</span>';
            } else {
                html += '<span class="wt-colcfg-del" title="删除此列" onclick="WorkColumnCfg._delCol(' + i + ')">🗑</span>';
            }
            html += '</div>';
            if (c.sys) {
                html += '<div class="wt-colcfg-hint">系统列 —— 选项语义固定(驱动看板 / 优先级配色)</div>';
            } else if (c.type === 'select' || c.type === 'multi') {
                var opts = c.options || [];
                html += '<div class="wt-colcfg-opts">';
                opts.forEach(function(o, oi) {
                    html += '<span class="wt-opt-chip">'
                        +     '<span class="wt-opt-label" title="点击重命名" onclick="WorkColumnCfg._renOpt(' + i + ',' + oi + ')">' + _esc(o) + '</span>'
                        +     '<span class="wt-opt-x" title="删除" onclick="WorkColumnCfg._delOpt(' + i + ',' + oi + ')">×</span>'
                        +   '</span>';
                });
                html += '<span class="wt-opt-add" onclick="WorkColumnCfg._addOpt(' + i + ')">+ 选项</span>';
                html += '</div>';
            } else {
                html += '<div class="wt-colcfg-hint">' + (TYPE_HINT[c.type] || '') + '</div>';
            }
            html += '</div>';
        });
        html += '<button class="wt-addcol" onclick="WorkColumnCfg._addCol()">+ 新增列</button>';
        host.innerHTML = html;
    }

    // ============ 单列字段编辑 ============
    function _renameCol(i, v) {
        if (!v || !v.trim()) return;
        var c = Work.columns()[i];
        c.name = v.trim();
        Work.saveColumnPatches([{ key: c.key, name: c.name }]);
        Work.renderActiveView();
    }
    function _setType(i, v) {
        var c = Work.columns()[i];
        c.type = v;
        // 切到 select/multi 且没有 opts 时,补一个空数组
        if ((v === 'select' || v === 'multi') && !c.sys && !c.options) c.options = [];
        Work.saveColumnPatches([{ key: c.key, type: v, options: c.options || [] }]);
        _renderList();
        Work.renderActiveView();
    }
    function _addOpt(i) {
        var c = Work.columns()[i];
        var v = prompt('新选项名称:');
        if (!v || !v.trim()) return;
        c.options = c.options || [];
        c.options.push(v.trim());
        Work.saveColumnPatches([{ key: c.key, options: c.options }]);
        _renderList();
        Work.renderActiveView();
    }
    function _delOpt(i, oi) {
        var c = Work.columns()[i];
        c.options.splice(oi, 1);
        Work.saveColumnPatches([{ key: c.key, options: c.options }]);
        _renderList();
        Work.renderActiveView();
    }
    function _renOpt(i, oi) {
        var c = Work.columns()[i];
        var old = c.options[oi];
        var v = prompt('重命名选项:', old);
        if (!v || !v.trim()) return;
        var nv = v.trim();
        c.options[oi] = nv;
        Work.saveColumnPatches([{ key: c.key, options: c.options }]);
        // 同步更新所有行该列引用了旧值的地方(单选改成新值;多选数组中替换)
        Work.rows().forEach(function(t) {
            if (c.builtin) {
                if (Array.isArray(t[c.key])) {
                    t[c.key] = t[c.key].map(function(x) { return x === old ? nv : x; });
                } else if (t[c.key] === old) t[c.key] = nv;
            } else if (t.customFields) {
                var v2 = t.customFields[c.key];
                if (Array.isArray(v2)) t.customFields[c.key] = v2.map(function(x) { return x === old ? nv : x; });
                else if (v2 === old) t.customFields[c.key] = nv;
            }
        });
        _renderList();
        Work.renderActiveView();
    }

    function _addCol() {
        var name = prompt('新列名称:');
        if (!name || !name.trim()) return;
        Work.addColumn({ name: name.trim(), type: 'text' }).then(function() {
            _renderList();
            Work.renderActiveView();
        });
    }
    function _delCol(i) {
        var c = Work.columns()[i];
        if (c.builtin) return;
        if (!confirm('删除列「' + c.name + '」?该列数据会一并移除。')) return;
        Work.removeColumn(c.key).then(function() {
            _renderList();
            Work.renderActiveView();
        });
    }

    // ============ 列顺序拖动 ============
    var _dragIdx = null;
    function _dragStart(e, i) {
        _dragIdx = i;
        e.dataTransfer.effectAllowed = 'move';
        try { e.dataTransfer.setData('text/plain', '' + i); } catch (_) {}
        var rows = document.querySelectorAll('#wt-col-list .wt-colcfg');
        if (rows[i]) rows[i].classList.add('dragging');
    }
    function _dragEnd() {
        document.querySelectorAll('#wt-col-list .wt-colcfg').forEach(function(r) {
            r.classList.remove('dragging', 'dragover');
        });
        _dragIdx = null;
    }
    function _dragOver(e, i) {
        if (_dragIdx == null || _dragIdx === i) return;
        e.preventDefault();
        var rows = document.querySelectorAll('#wt-col-list .wt-colcfg');
        if (rows[i]) rows[i].classList.add('dragover');
    }
    function _dragLeave(i) {
        var rows = document.querySelectorAll('#wt-col-list .wt-colcfg');
        if (rows[i]) rows[i].classList.remove('dragover');
    }
    function _drop(e, i) {
        e.preventDefault();
        if (_dragIdx == null || _dragIdx === i) { _dragEnd(); return; }
        var cols = Work.columns();
        var moved = cols.splice(_dragIdx, 1)[0];
        var target = (_dragIdx < i) ? i - 1 : i;
        cols.splice(target, 0, moved);
        // 重新分配 position,然后整批保存
        var patches = cols.map(function(c, idx) { c.position = idx; return { key: c.key, position: idx }; });
        Work.saveColumnPatches(patches);
        _dragEnd();
        _renderList();
        Work.renderActiveView();
    }

    return {
        open: open,
        close: close,
        // 给 HTML 内联事件用
        _renameCol: _renameCol,
        _setType: _setType,
        _addOpt: _addOpt,
        _delOpt: _delOpt,
        _renOpt: _renOpt,
        _addCol: _addCol,
        _delCol: _delCol,
        _dragStart: _dragStart,
        _dragEnd: _dragEnd,
        _dragOver: _dragOver,
        _dragLeave: _dragLeave,
        _drop: _drop,
    };
})();
