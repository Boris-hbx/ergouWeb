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
    var _graphMode = 'topic';
    var _graphFocus = null;
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
        // T-230: accept comma OR newline separated tags; trim, drop empties, dedupe.
        var list;
        if (Array.isArray(v)) list = v.map(function(x) { return ('' + x).trim(); });
        else if (typeof v === 'string' && v.trim()) list = v.split(/[\n\r,，;；]+/).map(function(x) { return x.trim(); });
        else return [];
        var seen = {}, out = [];
        list.forEach(function(x) { if (x && !seen[x]) { seen[x] = 1; out.push(x); } });
        return out;
    }
    // T-230: 「接口事项」→「相关工作标签」, frontend-only alias. Backend column key
    // stays `liaison`; only overrides the default display name (not user renames).
    function _colLabel(col) {
        if (col && col.builtin && col.key === 'liaison' && col.name === '接口事项') return '相关工作标签';
        return (col && (col.name || col.key)) || '';
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
        ['table', 'board', 'distribution', 'graph'].forEach(function(v) {
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
            return '<option value="' + _esc(c.key) + '"' + (c.key === _boardDim ? ' selected' : '') + '>按' + _esc(_colLabel(c)) + '</option>';
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
        else if (_view === 'distribution') renderDistribution(rows);
        else renderGraph(rows);
        refreshDetailIfOpen();
    }
    function _showOnly(view) {
        var tv = document.getElementById('sh-table-view');
        var bv = document.getElementById('sh-board-view');
        var dv = document.getElementById('sh-distribution-view');
        var gv = document.getElementById('sh-graph-view');
        if (tv) tv.classList.toggle('wt-hidden', view !== 'table');
        if (bv) bv.classList.toggle('wt-hidden', view !== 'board');
        if (dv) dv.classList.toggle('wt-hidden', view !== 'distribution');
        if (gv) gv.classList.toggle('wt-hidden', view !== 'graph');
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
                    return '<th>' + _esc(_colLabel(c)) + '</th>';
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
                + _esc(_colLabel(d)) + ' <span class="wt-dim-typ">' + _esc(d.type) + '</span></button>';
        }).join('');
        host.innerHTML = '<div class="wt-dim-bar"><span class="wt-dim-lbl">按以下维度看分布:</span><div class="wt-dim-seg">' + seg + '</div></div>'
            + '<div class="wt-section-label">分布概览</div><div class="wt-bubble-wrap"><div class="wt-bubble-row" id="sh-bubble-row"></div></div>'
            + '<div class="wt-section-label">各' + _esc(_colLabel(col)) + '详情</div><div class="wt-tag-grid" id="sh-tag-grid"></div>';
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

    function setGraphMode(mode) {
        _graphMode = mode || 'topic';
        _graphFocus = null;
        render();
    }

    function renderGraph(rows) {
        var host = document.getElementById('sh-graph-view');
        if (!host) return;
        if (!rows.length) {
            host.innerHTML = '<div class="wt-search-empty">还没有干系人，先新建一条再看关系图</div>';
            return;
        }
        var graph = _buildGraph(rows, _graphMode);
        var seg = [
            { key: 'team', label: '按部门' },
            { key: 'topic', label: '按事项' },
            { key: 'region', label: '按地域' },
        ].map(function(m) {
            return '<button class="wt-dim-btn' + (_graphMode === m.key ? ' active' : '') + '" onclick="Stakeholder.setGraphMode(\'' + m.key + '\')">' + m.label + '</button>';
        }).join('');

        if (!graph.edges.length) {
            host.innerHTML = '<div class="wt-dim-bar"><span class="wt-dim-lbl">关系图模式</span><div class="wt-dim-seg">' + seg + '</div></div>'
                + '<div class="wt-search-empty">' + _graphEmptyMessage(_graphMode) + '</div>';
            return;
        }

        var lines = graph.edges.map(function(e) {
            var a = graph.nodeMap[e.from];
            var b = graph.nodeMap[e.to];
            if (!a || !b) return '';
            return '<path class="sh-graph-edge' + (e.type === 'clue' ? ' is-clue' : '') + '" data-from="' + _attr(e.from) + '" data-to="' + _attr(e.to) + '" d="' + _attr(_graphEdgePath(e, a, b)) + '"></path>';
        }).join('');
        var nodes = graph.nodes.map(function(n) {
            var size = n.kind === 'dim' ? Math.round(54 + Math.min(44, Math.log2(n.count + 1) * 18)) : 46;
            var style = 'left:' + n.x + '%;top:' + n.y + '%;width:' + size + 'px;height:' + size + 'px;';
            var meta = n.kind === 'dim' ? (n.count + ' 人') : (n.meta || '');
            var classes = 'sh-graph-node sh-graph-' + n.kind + (n.isEmpty ? ' is-empty' : '') + (n.hasRhythm ? ' has-rhythm' : '') + (_nodeMatchesSearch(n) ? ' is-match' : '');
            return '<button type="button" class="' + classes + '" style="' + style + '" data-node="' + _attr(n.id) + '" title="' + _attr(n.label) + '">'
                + '<span class="sh-graph-label">' + _esc(n.label) + '</span>'
                + (meta ? '<span class="sh-graph-meta">' + _esc(meta) + '</span>' : '')
                + '</button>';
        }).join('');
        host.innerHTML = '<div class="wt-dim-bar sh-graph-toolbar">'
            + '<span class="wt-dim-lbl">关系图模式</span><div class="wt-dim-seg">' + seg + '</div>'
            + '<span class="wt-dim-spacer"></span><span class="wt-dim-stats">点击人员看详情，点击维度看关联名单</span></div>'
            + '<div class="sh-graph-shell">'
            + '<div class="sh-graph-canvas" id="sh-graph-canvas"><svg class="sh-graph-svg" viewBox="0 0 100 100" preserveAspectRatio="none">' + lines + '</svg>' + nodes + '</div>'
            + '<aside class="sh-graph-side" id="sh-graph-side">' + _graphSideHtml(graph) + '</aside>'
            + '</div>';
        _bindGraph(graph);
    }

    function _buildGraph(rows, mode) {
        var dimMap = Object.create(null);
        var personMap = Object.create(null);
        var edges = [];
        rows.forEach(function(r) {
            var vals = _graphValues(r, mode);
            if (!vals.length) return;
            var personId = 'p:' + r.id;
            if (!personMap[personId]) {
                personMap[personId] = {
                    id: personId,
                    kind: 'person',
                    row: r,
                    label: r.name || '(未命名)',
                    meta: [r.title, r.team || r.region].filter(Boolean).join(' · '),
                    count: 1,
                    hasRhythm: _hasFollowRhythm(r),
                };
            }
            vals.forEach(function(v) {
                var dimId = 'd:' + mode + ':' + v.value;
                if (!dimMap[dimId]) dimMap[dimId] = { id: dimId, kind: 'dim', label: v.label, value: v.value, count: 0, people: [], isEmpty: !!v.isEmpty };
                if (dimMap[dimId].people.indexOf(r.id) < 0) {
                    dimMap[dimId].people.push(r.id);
                    dimMap[dimId].count++;
                }
                edges.push({ from: dimId, to: personId, type: 'main' });
            });
        });
        var dims = Object.keys(dimMap).map(function(k) { return dimMap[k]; }).sort(function(a, b) {
            if (a.isEmpty !== b.isEmpty) return a.isEmpty ? 1 : -1;
            return b.count - a.count || a.label.localeCompare(b.label);
        });
        var people = Object.keys(personMap).map(function(k) { return personMap[k]; }).sort(function(a, b) {
            return a.label.localeCompare(b.label);
        });
        _placeGraphNodes(dims, people);
        var nodes = dims.concat(people);
        var nodeMap = {};
        nodes.forEach(function(n) { nodeMap[n.id] = n; });
        var clueEdges = _graphRelationClues(people);
        return { mode: mode, dims: dims, people: people, nodes: nodes, edges: edges.concat(clueEdges), mainEdges: edges, clueEdges: clueEdges, nodeMap: nodeMap };
    }

    function _graphRelationClues(people) {
        var names = people.map(function(p) {
            return { id: p.id, name: (p.row.name || '').trim() };
        }).filter(function(p) {
            return p.name.length >= 2;
        }).sort(function(a, b) {
            return b.name.length - a.name.length || a.name.localeCompare(b.name);
        });
        var seen = {};
        var edges = [];
        people.forEach(function(p) {
            var text = (p.row.relation || '').trim();
            if (!text) return;
            names.forEach(function(target) {
                if (target.id === p.id || text.indexOf(target.name) < 0) return;
                var key = p.id + '>' + target.id;
                if (seen[key]) return;
                seen[key] = 1;
                edges.push({ from: p.id, to: target.id, type: 'clue', source: p.row.id, target: target.name, text: text });
            });
        });
        return edges;
    }

    function _hasFollowRhythm(row) {
        return _arr(row.method).length > 0 || !!(row.cadence || '').trim();
    }

    function _graphValues(row, mode) {
        if (mode === 'team') {
            var team = (row.team || '').trim();
            return [{ value: team || UNMARKED, label: team || UNMARKED, isEmpty: !team }];
        }
        if (mode === 'region') {
            var region = (row.region || '').trim();
            return [{ value: region || UNMARKED, label: region || UNMARKED, isEmpty: !region }];
        }
        var seen = {};
        var values = _arr(row.duty).concat(_arr(row.liaison)).map(function(v) { return v.trim(); }).filter(function(v) {
            if (!v || seen[v]) return false;
            seen[v] = 1;
            return true;
        }).map(function(v) { return { value: v, label: v }; });
        return values.length ? values : [{ value: UNMARKED, label: UNMARKED, isEmpty: true }];
    }

    function _graphEdgePath(edge, a, b) {
        var sx = a.x, sy = a.y, ex = b.x, ey = b.y;
        var dx = ex - sx, dy = ey - sy;
        var len = Math.sqrt(dx * dx + dy * dy) || 1;
        var sr = _graphNodeRadius(a);
        var er = _graphNodeRadius(b);
        sx += dx / len * sr;
        sy += dy / len * sr;
        ex -= dx / len * er;
        ey -= dy / len * er;
        if (edge.type === 'clue') return _graphCluePath(sx, sy, ex, ey);
        var bend = Math.max(8, Math.min(18, Math.abs(dx) * 0.36));
        var c1x = sx + (dx >= 0 ? bend : -bend);
        var c2x = ex - (dx >= 0 ? bend : -bend);
        var c1y = sy + dy * 0.08;
        var c2y = ey - dy * 0.08;
        return 'M ' + _num(sx) + ' ' + _num(sy) + ' C ' + _num(c1x) + ' ' + _num(c1y) + ', ' + _num(c2x) + ' ' + _num(c2y) + ', ' + _num(ex) + ' ' + _num(ey);
    }

    function _graphCluePath(sx, sy, ex, ey) {
        var dx = ex - sx;
        var dy = ey - sy;
        var side = dx >= 0 ? 1 : -1;
        if (Math.abs(dx) < 6) side = sy <= ey ? 1 : -1;
        var lift = Math.max(5, Math.min(12, Math.abs(dy) * 0.22 + 5));
        var c1x = sx + side * lift;
        var c2x = ex + side * lift;
        var c1y = sy + dy * 0.32;
        var c2y = ey - dy * 0.32;
        return 'M ' + _num(sx) + ' ' + _num(sy) + ' C ' + _num(c1x) + ' ' + _num(c1y) + ', ' + _num(c2x) + ' ' + _num(c2y) + ', ' + _num(ex) + ' ' + _num(ey);
    }

    function _graphNodeRadius(node) {
        if (node.kind === 'dim') return 4.2 + Math.min(2.8, Math.log2((node.count || 1) + 1) * 0.9);
        return 3.6;
    }

    function _num(n) {
        return Math.round(n * 100) / 100;
    }

    function _placeGraphNodes(dims, people) {
        var dimCount = Math.max(1, dims.length);
        var personCount = Math.max(1, people.length);
        dims.forEach(function(n, i) {
            var y = dimCount === 1 ? 50 : 16 + (68 * i / (dimCount - 1));
            n.x = 22 + (i % 2) * 6;
            n.y = Math.round(y);
        });
        people.forEach(function(n, i) {
            var y = personCount === 1 ? 50 : 14 + (72 * i / (personCount - 1));
            n.x = 70 + (i % 3) * 7;
            n.y = Math.round(y);
        });
    }

    function _bindGraph(graph) {
        var canvas = document.getElementById('sh-graph-canvas');
        if (!canvas) return;
        canvas.querySelectorAll('.sh-graph-node').forEach(function(el) {
            el.addEventListener('mouseenter', function() { _highlightGraphNode(el.dataset.node); });
            el.addEventListener('mouseleave', function() { _highlightGraphNode(_graphFocus); });
            el.addEventListener('click', function() { _clickGraphNode(el.dataset.node, graph); });
        });
        _highlightGraphNode(_graphFocus);
    }

    function _clickGraphNode(id, graph) {
        var node = graph.nodeMap[id];
        if (!node) return;
        _graphFocus = _graphFocus === id ? null : id;
        _highlightGraphNode(_graphFocus);
        var side = document.getElementById('sh-graph-side');
        if (side) side.innerHTML = _graphSideHtml(graph);
    }

    function _highlightGraphNode(id) {
        var related = {};
        if (id) {
            related[id] = 1;
            document.querySelectorAll('#sh-graph-canvas .sh-graph-edge').forEach(function(edge) {
                if (edge.dataset.from === id || edge.dataset.to === id) {
                    related[edge.dataset.from] = 1;
                    related[edge.dataset.to] = 1;
                }
            });
        }
        document.querySelectorAll('#sh-graph-canvas .sh-graph-node').forEach(function(node) {
            var on = !id || related[node.dataset.node];
            node.classList.toggle('is-dimmed', !on);
            node.classList.toggle('is-active', !!id && node.dataset.node === id);
            node.classList.toggle('is-related', !!id && on && node.dataset.node !== id);
        });
        document.querySelectorAll('#sh-graph-canvas .sh-graph-edge').forEach(function(edge) {
            var on = !id || edge.dataset.from === id || edge.dataset.to === id;
            edge.classList.toggle('is-dimmed', !on);
            edge.classList.toggle('is-active', !!id && on);
        });
    }

    function _graphSideHtml(graph) {
        var node = _graphFocus && graph.nodeMap[_graphFocus];
        if (!node) {
            return '<div class="sh-graph-side-title">关系图</div>'
                + '<div class="sh-graph-side-sub">' + _graphModeLabel(graph.mode) + ' · ' + graph.dims.length + ' 个维度节点 · ' + graph.people.length + ' 人</div>'
                + '<div class="sh-graph-side-empty">点击部门、事项、地域或人员节点，查看下一步找谁、补谁、盯谁。</div>'
                + (graph.clueEdges.length ? '<div class="sh-graph-clue-note">已从关系说明中识别 ' + graph.clueEdges.length + ' 条可能的触达路径。</div>' : '');
        }
        if (node.kind === 'person') return _graphPersonSideHtml(graph, node);
        var people = node.people.map(rowById).filter(Boolean);
        var hasUnmarked = !!node.isEmpty || people.some(function(r) {
            return _graphValues(r, graph.mode).some(function(v) { return v.isEmpty; });
        });
        var hubs = people.filter(function(r) {
            return _arr(r.duty).concat(_arr(r.liaison)).length > 1;
        });
        var risks = [];
        if (people.length === 1) risks.push('单点联系人风险');
        if (hasUnmarked) risks.push('包含未填信息');
        return '<div class="sh-graph-side-title">' + _esc(node.label) + '</div>'
            + '<div class="sh-graph-side-sub">' + _graphModeLabel(graph.mode) + ' · 行动面板</div>'
            + '<div class="sh-graph-action-grid">'
            + '<div><span>' + people.length + '</span><small>关联人员</small></div>'
            + '<div><span>' + (hasUnmarked ? '有' : '无') + '</span><small>未填情况</small></div>'
            + '<div><span>' + (people.length === 1 ? '有' : '无') + '</span><small>单点风险</small></div>'
            + '<div><span>' + hubs.length + '</span><small>枢纽人员</small></div>'
            + '</div>'
            + (risks.length ? '<div class="sh-graph-risk">' + risks.map(_esc).join(' · ') + '</div>' : '<div class="sh-graph-ok">覆盖较稳，暂未发现单点联系人风险。</div>')
            + (hubs.length ? '<div class="sh-graph-section-title">可作为枢纽</div><div class="sh-graph-chip-row">' + hubs.slice(0, 6).map(function(r) { return '<button type="button" onclick="Stakeholder.focusGraphPerson(' + r.id + ')">' + _esc(r.name || '(未命名)') + '</button>'; }).join('') + '</div>' : '')
            + _graphCluesHtml(graph, people.map(function(r) { return 'p:' + r.id; }))
            + '<div class="sh-graph-section-title">关联人员</div>'
            + '<div class="sh-graph-person-list">' + people.map(function(r) {
                return '<button type="button" class="sh-graph-person-row" onclick="Stakeholder.focusGraphPerson(' + r.id + ')">'
                    + '<span>' + _esc(r.name || '(未命名)') + '</span>'
                    + '<small>' + _esc([r.title, r.team, r.region].filter(Boolean).join(' · ') || '未补充职务/部门') + '</small>'
                    + '</button>';
            }).join('') + '</div>';
    }

    function _graphPersonSideHtml(graph, node) {
        var r = node.row;
        var ids = graph.people.map(function(p) { return p.row.id; });
        return '<div class="sh-graph-side-title">' + _esc(r.name || '(未命名)') + '</div>'
            + '<div class="sh-graph-side-sub">人员行动面板' + (_hasFollowRhythm(r) ? ' · 已有跟进节奏' : ' · 未设置跟进节奏') + '</div>'
            + '<div class="sh-graph-field-grid">'
            + _graphField('职务', r.title)
            + _graphField('部门', r.team)
            + _graphField('地域', r.region)
            + _graphField('负责事项', _arr(r.duty).join('、'))
            + _graphField('相关工作标签', _arr(r.liaison).join('、'))
            + _graphField('管理方式/频率', [_arr(r.method).join('、'), r.cadence].filter(Boolean).join(' · '))
            + _graphField('关系说明', r.relation, true)
            + '</div>'
            + _graphCluesHtml(graph, [node.id])
            + '<button type="button" class="sh-graph-detail-btn" onclick="Stakeholder.openDetail(' + r.id + ', [' + ids.join(',') + '])">打开详情抽屉</button>';
    }

    function _graphField(label, value, long) {
        var text = (value || '').trim();
        return '<div class="sh-graph-field' + (long ? ' is-long' : '') + '"><small>' + _esc(label) + '</small><span>' + _esc(text || '未填') + '</span></div>';
    }

    function _graphCluesHtml(graph, nodeIds) {
        var lookup = {};
        nodeIds.forEach(function(id) { lookup[id] = 1; });
        var clues = graph.clueEdges.filter(function(e) { return lookup[e.from] || lookup[e.to]; });
        if (!clues.length) return '';
        return '<div class="sh-graph-section-title">可能的触达路径/关系线索</div>'
            + '<div class="sh-graph-clue-list">' + clues.map(function(e) {
                var from = graph.nodeMap[e.from];
                var to = graph.nodeMap[e.to];
                return '<div class="sh-graph-clue-item"><span>' + _esc((from && from.label) || '') + ' → ' + _esc((to && to.label) || '') + '</span><small>来源：relation 文本匹配</small></div>';
            }).join('') + '</div>';
    }

    function focusGraphPerson(id) {
        _graphFocus = 'p:' + id;
        render();
    }

    function _graphModeLabel(mode) {
        if (mode === 'team') return '按部门';
        if (mode === 'region') return '按地域';
        return '按事项';
    }
    function _graphEmptyMessage(mode) {
        if (mode === 'team') return '当前没有可连接的部门字段，请回表格补充部门/团队。';
        if (mode === 'region') return '当前没有可连接的地域字段，请回表格补充地域。';
        return '当前没有可连接的负责事项或相关工作标签，请回表格补充事项字段。';
    }
    function _nodeMatchesSearch(node) {
        return !!_search && _hay(node.label, _search);
    }
    function _attr(s) {
        return _esc(s).replace(/`/g, '&#96;');
    }

    function updateRow(id, patch) {
        var row = rowById(id);
        if (row) _applyPatch(row, patch);
        render();
        return API.stakeholderUpdate(id, patch).then(function(resp) {
            // request() resolves with the body even on 4xx, so check success flag.
            if (resp && resp.success === false) {
                if (typeof showToast === 'function') showToast(resp.error || '保存失败', 'error');
                reload();
                return false;
            }
            if (resp && resp.item) {
                var idx = _rows.findIndex(function(r) { return r.id === id; });
                if (idx >= 0) _rows[idx] = resp.item;
                render();
            }
            return true;
        }).catch(function(err) {
            console.error('[stakeholder] update failed', err);
            if (typeof showToast === 'function') showToast('保存失败,正在刷新...', 'warning');
            reload();
            return false;
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
            + '<div class="sh-detail-actions"><button class="eg-btn eg-btn--primary" onclick="Stakeholder.saveDetail()">保存</button></div>';
    }
    function _detailField(row, col) {
        var v = _get(row, col.key);
        var isMulti = col.type === 'multi';
        // multi: one tag per line so 换行/逗号 both work and existing tags are editable.
        var value = isMulti ? _arr(v).join('\n') : (v == null ? '' : '' + v);
        var useTextarea = col.type === 'longtext' || isMulti;
        return '<label class="sh-field"><span>' + _esc(_colLabel(col)) + (col.key === 'name' ? ' *' : '') + '</span>'
            + (useTextarea
                ? '<textarea data-key="' + _esc(col.key) + '"' + (isMulti ? ' rows="2" placeholder="可用逗号或换行分隔多个标签"' : '') + '>' + _esc(value) + '</textarea>'
                : '<input data-key="' + _esc(col.key) + '" value="' + _esc(value) + '">')
            + (isMulti ? '<small class="sh-field-hint">可用逗号或换行分隔多个标签</small>' : '')
            + '</label>';
    }
    function saveDetail() {
        var row = rowById(_detailId);
        var d = document.getElementById('sh-detail-drawer');
        if (!row || !d) return;
        var patch = _collectForm(d);
        if (patch.name != null && !patch.name.trim()) { showToast('姓名不能为空', 'warning'); return; }
        updateRow(row.id, patch).then(function(ok) {
            if (!ok) return; // 保存失败：保留抽屉，updateRow 已提示错误
            if (typeof showToast === 'function') showToast('已保存', 'success');
            closeDetail();   // 保存成功后关闭右侧抽屉
            render();        // 同步刷新表格/看板/分布
        });
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
            + '<div class="sh-create-bar eg-actionbar eg-actionbar--modal"><button class="eg-btn eg-btn--secondary" onclick="Stakeholder.closeCreate()">取消</button>'
            + '<button class="eg-btn eg-btn--primary" onclick="Stakeholder.submitCreate()">创建</button></div></div>';
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
            + '<div class="wt-modal-actions eg-actionbar eg-actionbar--modal"><button class="eg-btn eg-btn--primary" onclick="Stakeholder.closeColumns()">完成</button></div>'
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
                + '<input class="wt-colcfg-name" value="' + _esc(_colLabel(c)) + '" onchange="Stakeholder._renameColumn(' + i + ',this.value)">'
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
        setGraphMode: setGraphMode,
        focusGraphPerson: focusGraphPerson,
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
