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

    // 视图切换 (table / board / cal)
    function setView(v, skipBtnSync) {
        _view = v;
        if (!skipBtnSync) {
            var t = document.getElementById('wt-seg-table');
            var b = document.getElementById('wt-seg-board');
            var c = document.getElementById('wt-seg-cal');
            if (t) t.classList.toggle('active', v === 'table');
            if (b) b.classList.toggle('active', v === 'board');
            if (c) c.classList.toggle('active', v === 'cal');
        }
        var t  = document.getElementById('wt-table-view');
        var bv = document.getElementById('wt-board-view');
        var cv = document.getElementById('wt-cal-view');
        if (t)  t.classList.toggle('wt-hidden',  v !== 'table');
        if (bv) bv.classList.toggle('wt-hidden', v !== 'board');
        if (cv) cv.classList.toggle('wt-hidden', v !== 'cal');
        render();
    }

    // 当前激活视图重渲;给子模块用(列设置改完、单元格改完 → 调一次)
    function render() {
        if (_view === 'table') WorkTable.render();
        else if (_view === 'board') WorkBoard.render();
        else if (_view === 'cal') WorkCalendar.render();
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

    return {
        init: init,
        showHub: showHub,
        openFeature: openFeature,
        setView: setView,
        render: render,
        renderActiveView: renderActiveView,
        reload: reload,

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
