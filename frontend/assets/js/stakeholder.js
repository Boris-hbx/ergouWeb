// ========== Stakeholder - Work Hub stakeholder management (T-223) ==========

var Stakeholder = (function() {
    var UNMARKED = '未填';
    var _columns = [];
    var _rows = [];
    var _view = 'table';
    var _loaded = false;
    var _search = '';
    var _searchTimer = null;
    var _boardDim = 'team';
    var _drag = { id: null };
    var _detailId = null;
    var _detailIds = [];
    var _colDragIdx = null;
    var TYPES = [
        { key: 'text', label: '单行文本' },
        { key: 'longtext', label: '长文本' },
        { key: 'select', label: '单选' },
        { key: 'multi', label: '多选' },
        { key: 'number', label: '数字' },
        { key: 'date', label: '日期' },
    ];

    function _esc(s) {
        return WorkGridEngine.esc(s);
    }
    function _arr(v) {
        if (Array.isArray(v)) return v.filter(Boolean).map(String);
        if (typeof v === 'string' && v.trim()) return v.split(/[,，;；]\s*/).map(function(x) { return x.trim(); }).filter(Boolean);
        return [];
    }
    function _get(row, key) {
        if (!row) return '';
        if (Object.prototype.hasOwnProperty.call(row, key)) return row[key];
        return row.customFields && row.customFields[key];
    }
    function _setPatchForColumn(col, value) {
        var key = col.key;
        var patch = {};
        if (col.builtin) patch[key] = value;
        else {
            patch.customFields = {};
            patch.customFields[key] = value;
        }
        return patch;
    }
    function _colByKey(key) {
        for (var i = 0; i < _columns.length; i++) if (_columns[i].key === key) return _columns[i];
        return null;
    }
    function _dimensionColumns() {
        return _columns.filter(function(c) { return c.type === 'select' || c.type === 'multi'; });
    }
    function _selectColumns() {
        return _columns.filter(function(c) { return c.type === 'select'; });
    }

    function openFeature() {
        var hub = document.getElementById('work-hub');
        var table = document.getElementById('work-table-view');
        var ins = document.getElementById('work-insight-view');
        var factory = document.getElementById('work-insight-factory-view');
        var done = document.getElementById('work-done-view');
        var view = document.getElementById('stakeholder-view');
        if (hub) hub.style.display = 'none';
        if (table) table.style.display = 'none';
        if (ins) ins.style.display = 'none';
        if (factory) factory.style.display = 'none';
        if (done) done.style.display = 'none';
        if (view) view.style.display = '';
        localStorage.setItem('work_feature', 'stakeholder');
        _ensureLoaded().then(function() { setView(_view, true); });
    }

    function backToHub() {
        closeDetail();
        closeCreate();
        Work.showHub();
    }

    function _ensureLoaded() {
        if (_loaded) return Promise.resolve();
        return reload();
    }
    function reload() {
        return Promise.all([API.stakeholderListColumns(), API.stakeholderList()])
            .then(function(results) {
                _columns = (results[0] && results[0].items) || [];
                _rows = (results[1] && results[1].items) || [];
                _loaded = true;
                _syncBoardDimOptions();
                render();
            })
            .catch(function(err) {
                console.error('[stakeholder] reload failed', err);
                if (typeof showToast === 'function') showToast('干系人加载失败', 'error');
            });
    }

    function _visibleRows() {
        var rows = _rows.slice();
        if (!_search) return rows;
        return rows.filter(function(r) {
            var q = _search;
            if (_hay(r.name, q) || _hay(r.team, q) || _hay(r.region, q) || _hay(r.title, q) ||
                _hay(r.relation, q) || _hay(r.strategy, q) || _hay(r.notes, q)) return true;
            if (_matchArr(r.duty, q) || _matchArr(r.liaison, q) || _matchArr(r.method, q)) return true;
            var cf = r.customFields || {};
            for (var k in cf) if (Object.prototype.hasOwnProperty.call(cf, k) && _matchVal(cf[k], q)) return true;
            return false;
        });
    }
    function _hay(v, q) { return v != null && ('' + v).toLowerCase().indexOf(q) >= 0; }
    function _matchArr(v, q) { return _arr(v).some(function(x) { return _hay(x, q); }); }
    function _matchVal(v, q) { return Array.isArray(v) ? _matchArr(v, q) : _hay(v, q); }

    function setView(view, skipBtnSync) {
        _view = view;
        if (!skipBtnSync) _syncViewButtons();
        render();
    }
    function _syncViewButtons() {
        ['table', 'board', 'distribution'].forEach(function(v) {
            var el = document.getElementById('sh-seg-' + v);
            if (el) el.classList.toggle('active', _view === v);
        });
    }
    function _syncBoardDimOptions() {
        var sel = document.getElementById('sh-board-dim');
        if (!sel) return;
        var cols = _selectColumns();
        if (!cols.some(function(c) { return c.key === _boardDim; })) _boardDim = cols[0] ? cols[0].key : 'team';
        sel.innerHTML = cols.map(function(c) {
            return '<option value="' + _esc(c.key) + '"' + (c.key === _boardDim ? ' selected' : '') + '>按' + _esc(c.name) + '</option>';
        }).join('');
        sel.style.display = _view === 'board' ? '' : 'none';
    }
    function setBoardDim(key) {
        _boardDim = key || 'team';
        render();
    }

    function render() {
        _syncViewButtons();
        _syncBoardDimOptions();
        var rows = _visibleRows();
        var total = document.getElementById('sh-total');
        if (total) total.textContent = _search ? (rows.length + ' 人命中 / 共 ' + _rows.length + ' 人') : ('共 ' + rows.length + ' 人');
        var clear = document.getElementById('sh-search-clear');
        if (clear) clear.style.display = _search ? '' : 'none';

        _showOnly(_view);
        if (_view === 'table') renderTable(rows);
        else if (_view === 'board') renderBoard(rows);
        else renderDistribution(rows);
        refreshDetailIfOpen();
    }
    function _showOnly(view) {
        var tv = document.getElementById('sh-table-view');
        var bv = document.getElementById('sh-board-view');
        var dv = document.getElementById('sh-distribution-view');
        if (tv) tv.classList.toggle('wt-hidden', view !== 'table');
        if (bv) bv.classList.toggle('wt-hidden', view !== 'board');
        if (dv) dv.classList.toggle('wt-hidden', view !== 'distribution');
    }

    function renderTable(rows) {
        var host = document.getElementById('sh-table-view');
        if (!host) return;
        if (!_columns.length) {
            host.innerHTML = '<div class="wt-search-empty">暂无列配置</div>';
            return;
        }
        if (!rows.length) {
            host.innerHTML = '<div class="wt-search-empty">还没有干系人</div>';
            return;
        }
        WorkGridEngine.renderTable({
            host: host,
            columns: _columns,
            rows: rows,
            serialWidth: 54,
            defaultWidth: 130,
            headerHtml: function(cols) {
                return '<thead><tr><th class="wt-num-th">#</th>' + cols.map(function(c) {
                    return '<th>' + _esc(c.name) + '</th>';
                }).join('') + '</tr></thead>';
            },
            rowHtml: function(row, num) { return _rowHtml(row, num); },
            addRowHtml: function(span) {
                return '<tr class="wt-add-row" onclick="Stakeholder.openCreate()"><td colspan="' + span + '">+ 新建干系人</td></tr>';
            },
        });
    }
    function _rowHtml(row, num) {
        var tds = '<td class="wt-num" onclick="Stakeholder.openDetail(' + row.id + ')" title="打开详情">' + num + '</td>';
        _columns.forEach(function(c) { tds += _cellHtml(row, c); });
        return '<tr data-id="' + row.id + '">' + tds + '</tr>';
    }
    function _cellHtml(row, col) {
        var v = _get(row, col.key);
        if (col.type === 'multi') {
            var chips = _arr(v).map(function(x) { return '<span class="wt-tagchip">' + _esc(x) + '</span>'; }).join('');
            return '<td class="wt-editable" onclick="Stakeholder.openDetail(' + row.id + ')">' + (chips || '<span class="wt-empty">—</span>') + '</td>';
        }
        if (col.type === 'longtext') {
            var text = v ? ('' + v).replace(/\s+/g, ' ').trim() : '';
            return '<td class="wt-editable" onclick="Stakeholder.openDetail(' + row.id + ')">' + (text ? _esc(text) : '<span class="wt-empty">—</span>') + '</td>';
        }
        var cls = col.key === 'name' ? ' wt-cell-title' : '';
        return '<td class="wt-editable' + cls + '" onclick="Stakeholder.openDetail(' + row.id + ')">'
            + (v ? _esc(v) : '<span class="wt-empty">—</span>') + '</td>';
    }

    function renderBoard(rows) {
        var host = document.getElementById('sh-board-view');
        if (!host) return;
        var col = _colByKey(_boardDim) || _selectColumns()[0];
        if (!col) {
            host.innerHTML = '<div class="wt-search-empty">暂无可分组的 select 列</div>';
            return;
        }
        var values = _uniqueValues(rows, col);
        if (!values.length) values = [UNMARKED];
        WorkGridEngine.renderBoard({
            host: host,
            columns: values.map(function(v) { return { key: v, label: v }; }),
            rows: rows,
            dragState: _drag,
            rowsForColumn: function(list, boardCol) {
                return list.filter(function(r) { return (_singleValue(r, col) || UNMARKED) === boardCol.key; });
            },
            columnHeaderHtml: function(boardCol, items) {
                return '<div class="wt-col-head"><span>' + _esc(boardCol.label) + '</span><span class="wt-count">' + items.length + '</span></div>';
            },
            cardHtml: _boardCard,
            onDrop: function(id, value) {
                if (value === UNMARKED) value = '';
                updateRow(id, _setPatchForColumn(col, value));
            },
        });
    }
    function _uniqueValues(rows, col) {
        var seen = {}, out = [];
        rows.forEach(function(r) {
            var v = _singleValue(r, col);
            if (!v) return;
            if (!seen[v]) { seen[v] = 1; out.push(v); }
        });
        out.sort();
        if (rows.some(function(r) { return !_singleValue(r, col); })) out.push(UNMARKED);
        return out;
    }
    function _singleValue(row, col) {
        var v = _get(row, col.key);
        return v == null ? '' : ('' + v).trim();
    }
    function _boardCard(row) {
        var methods = _arr(row.method).slice(0, 2).join('、');
        return '<div class="wt-card sh-card" draggable="true" data-id="' + row.id + '" onclick="Stakeholder.openDetail(' + row.id + ')">'
            + '<div class="wt-card-title">' + _esc(row.name || '(未命名)') + '</div>'
            + '<div class="wt-card-meta">'
            + (row.title ? '<span class="wt-pill wt-lv">' + _esc(row.title) + '</span>' : '')
            + (row.region ? '<span class="wt-pill wt-area">' + _esc(row.region) + '</span>' : '')
            + (methods ? '<span class="wt-pill wt-fq">' + _esc(methods) + '</span>' : '')
            + '</div></div>';
    }

    function renderDistribution(rows) {
        var host = document.getElementById('sh-distribution-view');
        if (!host) return;
        var dims = _dimensionColumns();
        if (!dims.length) {
            host.innerHTML = '<div class="wt-search-empty">暂无可分布的 select / multi 列</div>';
            return;
        }
        var cur = host.dataset.dim;
        if (!cur || !dims.some(function(d) { return d.key === cur; })) cur = dims[0].key;
        host.dataset.dim = cur;
        var col = _colByKey(cur);
        var groups = WorkGridEngine.groupByDimension(rows, col, _valuesForDim, function(c, v) { return v; }, UNMARKED);
        var seg = dims.map(function(d) {
            return '<button class="wt-dim-btn' + (d.key === cur ? ' active' : '') + '" onclick="Stakeholder.setDistributionDim(\'' + _escAttr(d.key) + '\')">'
                + _esc(d.name) + ' <span class="wt-dim-typ">' + _esc(d.type) + '</span></button>';
        }).join('');
        host.innerHTML = '<div class="wt-dim-bar"><span class="wt-dim-lbl">按以下维度看分布:</span><div class="wt-dim-seg">' + seg + '</div></div>'
            + '<div class="wt-section-label">分布概览</div><div class="wt-bubble-wrap"><div class="wt-bubble-row" id="sh-bubble-row"></div></div>'
            + '<div class="wt-section-label">各' + _esc(col.name) + '详情</div><div class="wt-tag-grid" id="sh-tag-grid"></div>';
        WorkGridEngine.renderBubbles({
            host: document.getElementById('sh-bubble-row'),
            groups: groups,
            size: function(g) { return Math.round(58 + Math.min(1, Math.log2(g.tasks.length + 1) / 4) * 46); },
            className: function(g) { return 'wt-bubble sh-bubble' + (g.isUnmarked ? ' sh-bubble-empty' : ''); },
            html: function(g) { return '<div class="wt-b-name">' + _esc(g.name) + '</div><div class="wt-b-count">' + g.tasks.length + '</div>'; },
            onClick: function(g) { _toggleDistributionCard(g.name); },
        });
        WorkGridEngine.renderCards({
            host: document.getElementById('sh-tag-grid'),
            groups: groups,
            card: _distributionCard,
        });
    }
    function setDistributionDim(key) {
        var host = document.getElementById('sh-distribution-view');
        if (host) host.dataset.dim = key;
        render();
    }
    function _valuesForDim(row, col) {
        var v = _get(row, col.key);
        if (col.type === 'multi') return _arr(v);
        var s = v == null ? '' : ('' + v).trim();
        return s ? [s] : [];
    }
    function _distributionCard(g) {
        var card = document.createElement('div');
        card.className = 'wt-tag-card' + (g.isUnmarked ? ' wt-tag-card-unmarked' : '');
        card.dataset.tag = g.name;
        var list = g.tasks.map(function(it) {
            var r = it.row || it.task;
            return '<div class="wt-person-task" data-tid="' + r.id + '" onclick="Stakeholder.openDetail(' + r.id + ')">'
                + '<div class="wt-person-task-title">' + _esc(r.name || '(未命名)') + '</div>'
                + '<div class="wt-person-task-meta">'
                + (r.team ? '<span class="wt-person-pill">' + _esc(r.team) + '</span>' : '')
                + (r.region ? '<span class="wt-person-pill">' + _esc(r.region) + '</span>' : '')
                + (it.extra > 0 ? '<span class="wt-tag-extra">+' + it.extra + '</span>' : '')
                + '</div></div>';
        }).join('');
        card.innerHTML = '<div class="wt-tag-head"><span class="wt-tag-chip' + (g.isUnmarked ? ' wt-tag-chip-empty' : '') + '">#' + _esc(g.name) + '</span>'
            + '<div class="wt-tag-stats"><span class="wt-tag-stats-total">' + g.tasks.length + ' 人</span></div></div>'
            + '<div class="wt-person-task-list">' + list + '</div>';
        return card;
    }
    function _toggleDistributionCard(name) {
        var cards = document.querySelectorAll('#sh-tag-grid .wt-tag-card');
        for (var i = 0; i < cards.length; i++) {
            if (cards[i].dataset.tag === name) {
                cards[i].classList.toggle('wt-person-active');
                cards[i].scrollIntoView({ behavior: 'smooth', block: 'center' });
                break;
            }
        }
    }

    function updateRow(id, patch) {
        var row = rowById(id);
        if (row) _applyPatch(row, patch);
        render();
        return API.stakeholderUpdate(id, patch).then(function(resp) {
            if (resp && resp.item) {
                var idx = _rows.findIndex(function(r) { return r.id === id; });
                if (idx >= 0) _rows[idx] = resp.item;
                render();
            }
        }).catch(function(err) {
            console.error('[stakeholder] update failed', err);
            if (typeof showToast === 'function') showToast('保存失败,正在刷新...', 'warning');
            reload();
        });
    }
    function _applyPatch(row, patch) {
        Object.keys(patch).forEach(function(k) {
            if (k === 'customFields') {
                row.customFields = row.customFields || {};
                Object.keys(patch.customFields).forEach(function(ck) { row.customFields[ck] = patch.customFields[ck]; });
            } else {
                row[k] = patch[k];
            }
        });
    }
    function rowById(id) {
        for (var i = 0; i < _rows.length; i++) if (_rows[i].id === +id) return _rows[i];
        return null;
    }

    function openDetail(id, ids) {
        _detailId = +id;
        _detailIds = ids || _visibleRows().map(function(r) { return r.id; });
        _ensureDetailShell();
        _renderDetail();
        var d = document.getElementById('sh-detail-drawer');
        if (d) d.classList.add('open');
    }
    function closeDetail() {
        var d = document.getElementById('sh-detail-drawer');
        if (d) d.classList.remove('open');
        _detailId = null;
    }
    function refreshDetailIfOpen() {
        var d = document.getElementById('sh-detail-drawer');
        if (d && d.classList.contains('open')) _renderDetail();
    }
    function _ensureDetailShell() {
        if (document.getElementById('sh-detail-drawer')) return;
        var d = document.createElement('div');
        d.id = 'sh-detail-drawer';
        d.className = 'wt-detail-drawer sh-detail-drawer';
        document.body.appendChild(d);
    }
    function _renderDetail() {
        var row = rowById(_detailId);
        var d = document.getElementById('sh-detail-drawer');
        if (!row || !d) return;
        var idx = _detailIds.indexOf(row.id);
        d.innerHTML = '<div class="sh-detail-head">'
            + '<button class="sh-icon-btn" onclick="Stakeholder.navDetail(-1)" ' + (idx <= 0 ? 'disabled' : '') + '>↑</button>'
            + '<button class="sh-icon-btn" onclick="Stakeholder.navDetail(1)" ' + (idx < 0 || idx >= _detailIds.length - 1 ? 'disabled' : '') + '>↓</button>'
            + '<button class="sh-icon-btn danger" onclick="Stakeholder.deleteCurrent()" title="删除">🗑</button>'
            + '<button class="sh-icon-btn" onclick="Stakeholder.closeDetail()">✕</button>'
            + '</div>'
            + '<div class="sh-detail-title">' + _esc(row.name || '未命名干系人') + '</div>'
            + '<div class="sh-detail-grid">' + _columns.map(function(c) { return _detailField(row, c); }).join('') + '</div>'
            + '<div class="sh-detail-actions"><button class="wt-newbtn" onclick="Stakeholder.saveDetail()">保存</button></div>';
    }
    function _detailField(row, col) {
        var v = _get(row, col.key);
        var value = col.type === 'multi' ? _arr(v).join('，') : (v == null ? '' : '' + v);
        var tag = col.type === 'longtext' ? 'textarea' : 'input';
        return '<label class="sh-field"><span>' + _esc(col.name) + (col.key === 'name' ? ' *' : '') + '</span>'
            + (tag === 'textarea'
                ? '<textarea data-key="' + _esc(col.key) + '">' + _esc(value) + '</textarea>'
                : '<input data-key="' + _esc(col.key) + '" value="' + _esc(value) + '">')
            + '</label>';
    }
    function saveDetail() {
        var row = rowById(_detailId);
        var d = document.getElementById('sh-detail-drawer');
        if (!row || !d) return;
        var patch = _collectForm(d);
        if (patch.name != null && !patch.name.trim()) { showToast('姓名不能为空', 'warning'); return; }
        updateRow(row.id, patch).then(function() { if (typeof showToast === 'function') showToast('已保存', 'success'); });
    }
    function navDetail(delta) {
        var idx = _detailIds.indexOf(_detailId);
        var next = _detailIds[idx + delta];
        if (next != null) openDetail(next, _detailIds);
    }
    function deleteCurrent() {
        var row = rowById(_detailId);
        if (!row) return;
        if (!confirm('删除干系人「' + (row.name || '未命名') + '」？')) return;
        API.stakeholderDelete(row.id).then(function() {
            _rows = _rows.filter(function(r) { return r.id !== row.id; });
            closeDetail();
            render();
            if (typeof showToast === 'function') showToast('已删除', 'success');
        }).catch(function(err) {
            console.error('[stakeholder] delete failed', err);
            if (typeof showToast === 'function') showToast('删除失败', 'error');
        });
    }

    function openCreate() {
        _ensureCreateShell();
        var ov = document.getElementById('sh-create-ov');
        var body = document.getElementById('sh-create-body');
        if (!ov || !body) return;
        body.innerHTML = _columns.map(function(c) { return _detailField({}, c); }).join('');
        ov.classList.add('open');
        var first = body.querySelector('input');
        if (first) first.focus();
    }
    function closeCreate() {
        var ov = document.getElementById('sh-create-ov');
        if (ov) ov.classList.remove('open');
    }
    function _ensureCreateShell() {
        if (document.getElementById('sh-create-ov')) return;
        var ov = document.createElement('div');
        ov.id = 'sh-create-ov';
        ov.className = 'sh-create-ov';
        ov.innerHTML = '<div class="sh-create-box"><div class="sh-create-hd">新建干系人</div><div class="sh-create-bd" id="sh-create-body"></div>'
            + '<div class="sh-create-bar"><button class="wt-create-btn wt-create-cancel" onclick="Stakeholder.closeCreate()">取消</button>'
            + '<button class="wt-create-btn wt-create-ok" onclick="Stakeholder.submitCreate()">创建</button></div></div>';
        document.body.appendChild(ov);
    }
    function submitCreate() {
        var body = document.getElementById('sh-create-body');
        if (!body) return;
        var payload = _collectForm(body);
        if (!payload.name || !payload.name.trim()) { showToast('姓名不能为空', 'warning'); return; }
        API.stakeholderCreate(payload).then(function(resp) {
            if (resp && resp.item) _rows.push(resp.item);
            closeCreate();
            render();
            if (typeof showToast === 'function') showToast('已创建', 'success');
        }).catch(function(err) {
            console.error('[stakeholder] create failed', err);
            if (typeof showToast === 'function') showToast('创建失败', 'error');
        });
    }
    function _collectForm(root) {
        var patch = {};
        var customFields = {};
        root.querySelectorAll('[data-key]').forEach(function(el) {
            var key = el.dataset.key;
            var col = _colByKey(key);
            if (!col) return;
            var raw = el.value || '';
            var val = col.type === 'multi' ? _arr(raw) : raw.trim();
            if (col.builtin) patch[key] = val;
            else customFields[key] = val;
        });
        if (Object.keys(customFields).length) patch.customFields = customFields;
        return patch;
    }

    function onSearchInput(v) {
        if (_searchTimer) clearTimeout(_searchTimer);
        _searchTimer = setTimeout(function() {
            _search = (v || '').trim().toLowerCase();
            render();
        }, 120);
    }
    function onSearchKey(ev) {
        if (ev && ev.key === 'Escape') clearSearch();
    }
    function clearSearch() {
        if (_searchTimer) clearTimeout(_searchTimer);
        _search = '';
        var input = document.getElementById('sh-search');
        if (input) input.value = '';
        render();
    }

    function openColumns() {
        _ensureColumnShell();
        _renderColumnList();
        var m = document.getElementById('sh-col-modal');
        if (m) m.classList.add('open');
    }
    function closeColumns() {
        var m = document.getElementById('sh-col-modal');
        if (m) m.classList.remove('open');
    }
    function _ensureColumnShell() {
        if (document.getElementById('sh-col-modal')) return;
        var m = document.createElement('div');
        m.id = 'sh-col-modal';
        m.className = 'wt-modal-overlay';
        m.onclick = function(ev) { if (ev.target === m) closeColumns(); };
        m.innerHTML = '<div class="wt-modal" style="max-width:640px">'
            + '<div class="wt-modal-title">干系人列设置</div>'
            + '<div class="wt-modal-sub">拖动 ≡ 调整列顺序 · 改列名 / 类型 / 选项 / 宽度 · 底部可新增自定义列</div>'
            + '<div id="sh-col-list"></div>'
            + '<div class="wt-modal-actions"><button class="wt-mbtn primary" onclick="Stakeholder.closeColumns()">完成</button></div>'
            + '</div>';
        document.body.appendChild(m);
    }
    function _renderColumnList() {
        var host = document.getElementById('sh-col-list');
        if (!host) return;
        var html = '';
        _columns.forEach(function(c, i) {
            html += '<div class="wt-colcfg" data-idx="' + i + '"'
                + ' ondragover="Stakeholder._colDragOver(event,' + i + ')"'
                + ' ondragleave="Stakeholder._colDragLeave(' + i + ')"'
                + ' ondrop="Stakeholder._colDrop(event,' + i + ')">'
                + '<div class="wt-colcfg-top">'
                + '<span class="wt-colcfg-handle" draggable="true"'
                + ' ondragstart="Stakeholder._colDragStart(event,' + i + ')"'
                + ' ondragend="Stakeholder._colDragEnd()" title="按住拖动调整列顺序">≡</span>'
                + '<input class="wt-colcfg-name" value="' + _esc(c.name) + '" onchange="Stakeholder._renameColumn(' + i + ',this.value)">'
                + '<select class="wt-colcfg-type" onchange="Stakeholder._setColumnType(' + i + ',this.value)">';
            TYPES.forEach(function(tp) {
                html += '<option value="' + tp.key + '"' + (tp.key === c.type ? ' selected' : '') + '>' + tp.label + '</option>';
            });
            html += '</select>'
                + '<input class="wt-colcfg-type" style="width:76px" type="number" min="72" max="320" value="' + (parseInt(c.width, 10) || 130) + '" onchange="Stakeholder._setColumnWidth(' + i + ',this.value)" title="列宽">';
            if (c.builtin) {
                html += '<span class="wt-colcfg-lock" title="内置列不可删除">🔒</span>';
            } else {
                html += '<span class="wt-colcfg-del" title="删除此列" onclick="Stakeholder._deleteColumn(' + i + ')">🗑</span>';
            }
            html += '</div>';
            if (c.type === 'select' || c.type === 'multi') {
                html += '<div class="wt-colcfg-opts">';
                (c.options || []).forEach(function(o, oi) {
                    html += '<span class="wt-opt-chip">'
                        + '<span class="wt-opt-label" title="点击重命名" onclick="Stakeholder._renameOption(' + i + ',' + oi + ')">' + _esc(o) + '</span>'
                        + '<span class="wt-opt-x" title="删除" onclick="Stakeholder._deleteOption(' + i + ',' + oi + ')">×</span>'
                        + '</span>';
                });
                html += '<span class="wt-opt-add" onclick="Stakeholder._addOption(' + i + ')">+ 选项</span></div>';
            } else {
                html += '<div class="wt-colcfg-hint">字段值在详情抽屉和新建表单中编辑。</div>';
            }
            html += '</div>';
        });
        html += '<button class="wt-addcol" onclick="Stakeholder._addColumn()">+ 新增列</button>';
        host.innerHTML = html;
    }
    function _saveColumnPatches(patches) {
        return API.stakeholderSaveColumns(patches).then(function(resp) {
            if (resp && resp.items) _columns = resp.items;
            _syncBoardDimOptions();
            render();
            _renderColumnList();
        }).catch(function(err) {
            console.error('[stakeholder] save columns failed', err);
            if (typeof showToast === 'function') showToast('列设置保存失败', 'warning');
            reload();
        });
    }
    function _renameColumn(i, v) {
        var c = _columns[i];
        if (!c || !v || !v.trim()) return;
        c.name = v.trim();
        _saveColumnPatches([{ key: c.key, name: c.name }]);
    }
    function _setColumnType(i, type) {
        var c = _columns[i];
        if (!c) return;
        c.type = type;
        if ((type === 'select' || type === 'multi') && !Array.isArray(c.options)) c.options = [];
        _saveColumnPatches([{ key: c.key, type: c.type, options: c.options || [] }]);
    }
    function _setColumnWidth(i, width) {
        var c = _columns[i];
        if (!c) return;
        var w = Math.max(72, Math.min(320, parseInt(width, 10) || 130));
        c.width = w;
        _saveColumnPatches([{ key: c.key, width: w }]);
    }
    function _addOption(i) {
        var c = _columns[i];
        if (!c) return;
        var v = prompt('新选项名称:');
        if (!v || !v.trim()) return;
        c.options = c.options || [];
        c.options.push(v.trim());
        _saveColumnPatches([{ key: c.key, options: c.options }]);
    }
    function _deleteOption(i, oi) {
        var c = _columns[i];
        if (!c || !c.options) return;
        c.options.splice(oi, 1);
        _saveColumnPatches([{ key: c.key, options: c.options }]);
    }
    function _renameOption(i, oi) {
        var c = _columns[i];
        if (!c || !c.options) return;
        var old = c.options[oi];
        var v = prompt('重命名选项:', old);
        if (!v || !v.trim()) return;
        c.options[oi] = v.trim();
        _saveColumnPatches([{ key: c.key, options: c.options }]);
    }
    function _addColumn() {
        var name = prompt('新列名称:');
        if (!name || !name.trim()) return;
        API.stakeholderAddColumn({ name: name.trim(), type: 'text' }).then(function(resp) {
            if (resp && resp.items) _columns = resp.items;
            else return reload();
            _syncBoardDimOptions();
            render();
            _renderColumnList();
        }).catch(function(err) {
            console.error('[stakeholder] add column failed', err);
            if (typeof showToast === 'function') showToast('新增列失败', 'error');
        });
    }
    function _deleteColumn(i) {
        var c = _columns[i];
        if (!c || c.builtin) return;
        if (!confirm('删除列「' + c.name + '」？该列数据会一并移除。')) return;
        API.stakeholderDeleteColumn(c.key).then(function(resp) {
            _rows.forEach(function(r) { if (r.customFields) delete r.customFields[c.key]; });
            if (resp && resp.items) _columns = resp.items;
            else _columns.splice(i, 1);
            _syncBoardDimOptions();
            render();
            _renderColumnList();
        }).catch(function(err) {
            console.error('[stakeholder] delete column failed', err);
            if (typeof showToast === 'function') showToast('删除列失败', 'error');
        });
    }
    function _colDragStart(e, i) {
        _colDragIdx = i;
        e.dataTransfer.effectAllowed = 'move';
        try { e.dataTransfer.setData('text/plain', '' + i); } catch (_) {}
        var rows = document.querySelectorAll('#sh-col-list .wt-colcfg');
        if (rows[i]) rows[i].classList.add('dragging');
    }
    function _colDragEnd() {
        document.querySelectorAll('#sh-col-list .wt-colcfg').forEach(function(r) {
            r.classList.remove('dragging', 'dragover');
        });
        _colDragIdx = null;
    }
    function _colDragOver(e, i) {
        if (_colDragIdx == null || _colDragIdx === i) return;
        e.preventDefault();
        var rows = document.querySelectorAll('#sh-col-list .wt-colcfg');
        if (rows[i]) rows[i].classList.add('dragover');
    }
    function _colDragLeave(i) {
        var rows = document.querySelectorAll('#sh-col-list .wt-colcfg');
        if (rows[i]) rows[i].classList.remove('dragover');
    }
    function _colDrop(e, i) {
        e.preventDefault();
        if (_colDragIdx == null || _colDragIdx === i) { _colDragEnd(); return; }
        var moved = _columns.splice(_colDragIdx, 1)[0];
        var target = (_colDragIdx < i) ? i - 1 : i;
        _columns.splice(target, 0, moved);
        var patches = _columns.map(function(c, idx) { c.position = idx; return { key: c.key, position: idx }; });
        _colDragEnd();
        _saveColumnPatches(patches);
    }
    function _escAttr(s) { return ('' + s).replace(/\\/g, '\\\\').replace(/'/g, "\\'"); }

    return {
        openFeature: openFeature,
        backToHub: backToHub,
        reload: reload,
        setView: setView,
        setBoardDim: setBoardDim,
        setDistributionDim: setDistributionDim,
        onSearchInput: onSearchInput,
        onSearchKey: onSearchKey,
        clearSearch: clearSearch,
        openColumns: openColumns,
        closeColumns: closeColumns,
        openDetail: openDetail,
        closeDetail: closeDetail,
        navDetail: navDetail,
        saveDetail: saveDetail,
        deleteCurrent: deleteCurrent,
        openCreate: openCreate,
        closeCreate: closeCreate,
        submitCreate: submitCreate,
        _renameColumn: _renameColumn,
        _setColumnType: _setColumnType,
        _setColumnWidth: _setColumnWidth,
        _addOption: _addOption,
        _deleteOption: _deleteOption,
        _renameOption: _renameOption,
        _addColumn: _addColumn,
        _deleteColumn: _deleteColumn,
        _colDragStart: _colDragStart,
        _colDragEnd: _colDragEnd,
        _colDragOver: _colDragOver,
        _colDragLeave: _colDragLeave,
        _colDrop: _colDrop,
    };
})();
