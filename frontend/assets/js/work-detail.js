// ========== WorkDetail — 任务详情侧拉抽屉 (T-100 / SPEC § 7.6) ==========
//
// 视觉设计依据(strict 1:1):`frontend/work-detail-preview.html`
// 右滑 540px 抽屉(不是 modal),背景视图仍可见可点切换。
//
// 公共入口:
//   WorkDetail.openDetail(taskId, navIds?)  打开抽屉,navIds 是按当前视图顺序的 id 数组(支持上下翻)
//   WorkDetail.closeDetail()                关闭
//   WorkDetail.isOpen()                     是否打开
//
// 字段编辑:
//   标题 textarea     → blur 时 Work.updateRow({title})
//   简介 textarea     → blur / Cmd+Enter 时 Work.updateRow({desc})
//   责任人/层级/频率/优先级 → 复用 WorkTable.openPick / editText
//   截止日             → 复用 WorkTable.editDate('due')(T-104:datepicker callback 模式)
//   进度               → 复用 WorkTable.editProgress
//
// 键盘:
//   Esc        关闭(在输入框里则先 blur)
//   ↑ / ↓     上一/下一条(在输入框里不响应)
//
// 关闭方式:✕ / Esc / 点蒙层 / 切视图(work.js setView 调 closeDetail)

var WorkDetail = (function() {
    var _currentId = null;
    var _navIds = [];  // 当前视图可见 id 顺序(↑↓ 用)
    var _kbBound = false;

    function _esc(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    }

    function isOpen() { return _currentId !== null; }
    function currentId() { return _currentId; }

    // ============ 打开 / 关闭 ============
    function openDetail(taskId, navIds) {
        var t = Work.rowById(taskId);
        if (!t) return;
        _currentId = taskId;
        _navIds = (navIds && navIds.length) ? navIds.slice() : Work.rows().map(function(r) { return r.id; });
        _ensureKbBound();
        var drawer = document.getElementById('wt-detail-drawer');
        var scrim  = document.getElementById('wt-detail-scrim');
        if (drawer) drawer.classList.add('open');
        if (scrim) scrim.classList.add('open');
        _render(t);
        _syncSelectedRow();
    }

    function closeDetail() {
        _currentId = null;
        var drawer = document.getElementById('wt-detail-drawer');
        var scrim  = document.getElementById('wt-detail-scrim');
        if (drawer) drawer.classList.remove('open');
        if (scrim) scrim.classList.remove('open');
        _syncSelectedRow();
    }

    // ============ ↑↓ 切换 ============
    function navigate(delta) {
        if (_currentId === null || !_navIds.length) return;
        var idx = _navIds.indexOf(_currentId);
        if (idx < 0) idx = 0;
        var next = (idx + delta + _navIds.length) % _navIds.length;
        var nextId = _navIds[next];
        openDetail(nextId, _navIds);
    }

    // ============ 渲染 ============
    function _render(t) {
        // 标题
        var titleEl = document.getElementById('wt-d-title');
        if (titleEl) {
            titleEl.value = t.title || '';
            _autoResizeTitle();
        }

        // 状态 / 优先级 pill
        var s = WorkTable._statusBy(t.status);
        var p = WorkTable._prioBy(t.priority);
        var statusPill = document.getElementById('wt-d-status-pill');
        if (statusPill) {
            statusPill.className = 'wt-pill ' + s.cls;
            statusPill.innerHTML = '<span class="wt-pdot" style="background:' + s.dot + '"></span>' + _esc(s.label);
            statusPill.onclick = function(ev) { WorkTable.openPick(t.id, 'status', statusPill); ev.stopPropagation(); };
        }
        var prioPill = document.getElementById('wt-d-prio-pill');
        if (prioPill) {
            prioPill.className = 'wt-pill ' + p.cls;
            prioPill.textContent = p.label;
            prioPill.onclick = function(ev) { WorkTable.openPick(t.id, 'priority', prioPill); ev.stopPropagation(); };
        }
        var duePill = document.getElementById('wt-d-due-pill');
        if (duePill) {
            var dueY = _normalizeDue(t.due);
            var todayY = _todayYMD();
            if (dueY && t.status !== 'done' && dueY < todayY) {
                var n = _daysOverdue(dueY, todayY);
                duePill.style.display = 'inline-block';
                duePill.className = 'wt-pill overdue';
                duePill.textContent = '⏰ 逾期 ' + n + ' 天';
            } else if (dueY === todayY) {
                duePill.style.display = 'inline-block';
                duePill.className = 'wt-pill due-soon';
                duePill.textContent = '📅 今天';
            } else {
                duePill.style.display = 'none';
            }
        }

        // 字段网格
        var avaEl = document.getElementById('wt-d-avatar');
        var name = t.assignee || '?';
        if (avaEl) {
            avaEl.textContent = (t.assignee || '?').charAt(0);
            avaEl.style.background = Work.colorOf(name);
        }
        // T-121:责任人字段合并显示「主 + 协作者头像组」(UI 不分主+协;数据层仍是双字段)
        var collabs = Array.isArray(t.collaborators) ? t.collaborators : [];
        var assigneeEl = document.getElementById('wt-d-assignee');
        if (assigneeEl) {
            var mainName = t.assignee || '未指派';
            var collabStackHtml = '';
            if (collabs.length > 0) {
                var shown = collabs.slice(0, 3);
                var extra = collabs.length - shown.length;
                var allNames = [t.assignee].concat(collabs).filter(Boolean).join('、');
                collabStackHtml = ' <span class="wt-collab-stack" title="' + allNames.replace(/"/g, '&quot;') + '">'
                    + shown.map(function(n) {
                        return '<span class="wt-avatar wt-avatar-xs" style="background:' + Work.colorOf(n) + '">' + (n || '?').slice(0, 1) + '</span>';
                      }).join('')
                    + (extra > 0 ? '<span class="wt-collab-more">+' + extra + '</span>' : '')
                    + '</span>';
            }
            assigneeEl.innerHTML = mainName.replace(/&/g, '&amp;').replace(/</g, '&lt;') + collabStackHtml;
        }

        _setField('wt-d-level', t.level, '—');
        _setField('wt-d-freq', t.freq, '—');
        _setField('wt-d-priority-field', p.label, '—');
        _setField('wt-d-due', t.due, '—');

        var fillEl = document.getElementById('wt-d-progress-fill');
        var pctEl  = document.getElementById('wt-d-progress-pct');
        var pct = Math.max(0, Math.min(100, +t.progress || 0));
        if (fillEl) fillEl.style.width = pct + '%';
        if (pctEl) pctEl.textContent = pct + '%';

        // 字段点击 → 复用 WorkTable 的编辑通道
        // T-121:责任人 + 协作者统一编辑(单 input modal 逗号分隔)
        _bindFieldClick('wt-d-field-assignee', function(ev) { WorkTable.editAssigneeCombined(t.id); });
        _bindFieldClick('wt-d-field-level',    function(ev) { WorkTable.openPick(t.id, 'level',    document.getElementById('wt-d-field-level')); });
        _bindFieldClick('wt-d-field-freq',     function(ev) { WorkTable.openPick(t.id, 'freq',     document.getElementById('wt-d-field-freq')); });
        _bindFieldClick('wt-d-field-priority', function(ev) { WorkTable.openPick(t.id, 'priority', document.getElementById('wt-d-field-priority')); });
        _bindFieldClick('wt-d-field-due',      function(ev) { WorkTable.editDate(ev, t.id, 'due'); });
        _bindFieldClick('wt-d-progress',       function(ev) { WorkTable.editProgress(t.id, 'progress'); });

        // 简介
        var descEl = document.getElementById('wt-d-desc');
        if (descEl) descEl.value = t.desc || '';

        // 活动时间线(MVP:只显示「创建 / 修改」两条)
        var tlEl = document.getElementById('wt-d-timeline');
        if (tlEl) {
            var items = [];
            if (t.updatedAt && t.updatedAt !== t.createdAt) {
                items.push({ txt: '更新', when: _fmtTime(t.updatedAt) });
            }
            if (t.createdAt) {
                items.push({ txt: '创建', when: _fmtTime(t.createdAt) });
            }
            tlEl.innerHTML = items.length
                ? items.map(function(e) {
                    return '<div class="wt-d-tl-item">'
                        +     _esc(e.txt)
                        +     '<span class="wt-d-tl-when">' + _esc(e.when) + '</span>'
                        +  '</div>';
                  }).join('')
                : '<div class="wt-d-tl-empty">暂无活动记录</div>';
        }

        // T-131:周期任务 → 异步拉完成历史,插到时间线顶部("🔁 完成(本期截止 X)")
        if (tlEl && t.freq && t.freq !== '一次性') {
            var forId = t.id;
            API.workTaskCompletions(t.id).then(function(r) {
                if (_currentId !== forId) return;   // 已切到别的任务 → 丢弃
                var el = document.getElementById('wt-d-timeline');
                if (!el || !r || !r.success || !r.items || !r.items.length) return;
                var hist = r.items.map(function(c) {
                    var cyc = c.cycleDueDate ? '(本期截止 ' + _esc(c.cycleDueDate) + ')' : '';
                    return '<div class="wt-d-tl-item">🔁 完成 ' + cyc
                        + '<span class="wt-d-tl-when">' + _esc(_fmtTime(c.completedAt)) + '</span></div>';
                }).join('');
                el.innerHTML = hist + el.innerHTML;
            }).catch(function(e) { console.error('[WorkDetail] completions', e); });
        }

        // 底部元信息
        _setText('wt-d-created', _fmtTime(t.createdAt));
        _setText('wt-d-updated', _fmtTime(t.updatedAt));

        // 上下导航位置
        var idx = _navIds.indexOf(t.id);
        _setText('wt-d-nav-pos',   (idx >= 0 ? idx + 1 : '?'));
        _setText('wt-d-nav-total', _navIds.length || '?');

        // 导航按钮禁用态(只有一条时灰掉)
        var only = _navIds.length <= 1;
        var pBtn = document.getElementById('wt-d-prev');
        var nBtn = document.getElementById('wt-d-next');
        if (pBtn) pBtn.disabled = only;
        if (nBtn) nBtn.disabled = only;
    }

    function _setField(id, val, fallback) {
        var el = document.getElementById(id);
        if (!el) return;
        var has = val != null && ('' + val).trim() !== '';
        el.textContent = has ? ('' + val) : fallback;
        el.classList.toggle('wt-d-field-empty', !has);
    }
    function _setText(id, val) {
        var el = document.getElementById(id);
        if (el) el.textContent = val == null ? '' : ('' + val);
    }
    function _bindFieldClick(id, fn) {
        var el = document.getElementById(id);
        if (!el) return;
        el.onclick = function(ev) {
            ev.stopPropagation();
            fn(ev);
        };
    }

    function _syncSelectedRow() {
        // 在表格视图里给当前选中的 <tr> 加 selected 类(高亮)
        document.querySelectorAll('#wt-table-view tr[data-id]').forEach(function(tr) {
            tr.classList.toggle('wt-detail-selected', _currentId !== null && +tr.dataset.id === _currentId);
        });
    }

    // ============ 标题自适应高度 ============
    function _autoResizeTitle() {
        var ta = document.getElementById('wt-d-title');
        if (!ta) return;
        ta.style.height = 'auto';
        ta.style.height = (ta.scrollHeight + 2) + 'px';
    }

    // ============ 时间格式化 ============
    function _fmtTime(iso) {
        if (!iso) return '—';
        var d = new Date(iso);
        if (isNaN(d.getTime())) return ('' + iso).slice(0, 16);
        var now = new Date();
        var sameDay = d.toDateString() === now.toDateString();
        var pad = function(n) { return ('0' + n).slice(-2); };
        if (sameDay) return '今天 ' + pad(d.getHours()) + ':' + pad(d.getMinutes());
        return d.getFullYear() + '-' + pad(d.getMonth() + 1) + '-' + pad(d.getDate())
             + ' ' + pad(d.getHours()) + ':' + pad(d.getMinutes());
    }
    function _todayYMD() {
        var d = new Date();
        return d.getFullYear() + '-' + ('0' + (d.getMonth() + 1)).slice(-2) + '-' + ('0' + d.getDate()).slice(-2);
    }
    function _normalizeDue(due) {
        if (!due) return '';
        var s = '' + due;
        if (s.length === 10 && s.charAt(4) === '-' && s.charAt(7) === '-') return s;
        if (s.length === 5 && s.charAt(2) === '-') return (new Date()).getFullYear() + '-' + s;
        return '';
    }
    function _daysOverdue(dueY, todayY) {
        var a = new Date(dueY + 'T00:00:00');
        var b = new Date(todayY + 'T00:00:00');
        return Math.max(1, Math.floor((b - a) / 86400000));
    }

    // ============ 键盘 ============
    function _ensureKbBound() {
        if (_kbBound) return;
        _kbBound = true;
        document.addEventListener('keydown', _onKey);
        // 绑标题 / 简介保存
        var titleEl = document.getElementById('wt-d-title');
        if (titleEl) {
            titleEl.addEventListener('input', _autoResizeTitle);
            titleEl.addEventListener('blur', _saveTitleIfChanged);
            titleEl.addEventListener('keydown', function(e) {
                if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); titleEl.blur(); }
            });
        }
        var descEl = document.getElementById('wt-d-desc');
        if (descEl) {
            descEl.addEventListener('blur', _saveDescIfChanged);
            descEl.addEventListener('keydown', function(e) {
                if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); descEl.blur(); }
            });
        }
    }

    function _onKey(e) {
        if (_currentId === null) return;
        // 抽屉关着不响应
        var drawer = document.getElementById('wt-detail-drawer');
        if (!drawer || !drawer.classList.contains('open')) return;
        var ae = document.activeElement;
        var inField = ae && ['INPUT', 'TEXTAREA', 'SELECT'].indexOf(ae.tagName) >= 0;
        if (inField) {
            if (e.key === 'Escape') {
                ae.blur();
                e.preventDefault();
            }
            return;  // 编辑中不响应方向键
        }
        if (e.key === 'Escape')      { closeDetail(); e.preventDefault(); }
        else if (e.key === 'ArrowUp')   { navigate(-1); e.preventDefault(); }
        else if (e.key === 'ArrowDown') { navigate(1);  e.preventDefault(); }
    }

    function _saveTitleIfChanged() {
        if (_currentId === null) return;
        var t = Work.rowById(_currentId);
        if (!t) return;
        var v = (document.getElementById('wt-d-title').value || '').trim();
        if (v !== (t.title || '')) {
            Work.updateRow(_currentId, { title: v });
        }
    }
    function _saveDescIfChanged() {
        if (_currentId === null) return;
        var t = Work.rowById(_currentId);
        if (!t) return;
        var v = document.getElementById('wt-d-desc').value || '';
        if (v !== (t.desc || '')) {
            Work.updateRow(_currentId, { desc: v });
        }
    }

    // ============ 从行/卡片/日历项 click 事件中提取 id 并打开 ============
    // 表格行:跳过点在 editable 区域上的 click(让原弹层照常)
    function openFromRowEvent(ev, taskId, navIds) {
        // 落在可编辑单元、药丸、resizer 等之上 — 让原编辑链路接管
        var skipSel = '.wt-editable, .wt-pill, .wt-check, .wt-desc-row, .wt-desc-add, .wt-desc-expand, .wt-resizer, .wt-add-row';
        if (ev && ev.target && ev.target.closest && ev.target.closest(skipSel)) return;
        openDetail(taskId, navIds);
    }

    // 外部刷新触发(Work.render 后,把抽屉里的数据按 id 重渲一次)
    function refreshIfOpen() {
        if (_currentId === null) return;
        var t = Work.rowById(_currentId);
        if (!t) { closeDetail(); return; }
        _render(t);
        _syncSelectedRow();
    }

    return {
        openDetail: openDetail,
        closeDetail: closeDetail,
        navigate: navigate,
        isOpen: isOpen,
        currentId: currentId,
        openFromRowEvent: openFromRowEvent,
        refreshIfOpen: refreshIfOpen,
    };
})();
