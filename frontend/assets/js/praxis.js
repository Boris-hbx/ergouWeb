// ========== Praxis module (T-283) ==========
var Praxis = (function() {
    var _loaded = false;
    var _contacts = [];
    var _selectedId = null;
    var _activeBoard = 'rel';
    var _healthScore = 58;

    var BOARDS = [
        { key: 'strategy', icon: '🎯', name: '战略定位', score: 62, idea: '想清楚要成为什么样的人，再决定做什么事。' },
        { key: 'skill', icon: '🛠️', name: '能力产品', score: 78, idea: '把能力打磨成可被识别、可被交付的产品。' },
        { key: 'ops', icon: '⏱️', name: '运营管理', score: 55, idea: '用系统和习惯托住状态，别靠意志硬扛。' },
        { key: 'fin', icon: '💧', name: '财务健康', score: 70, idea: '现金流健康是自由的底座，别让钱绑架决策。' },
        { key: 'brand', icon: '📣', name: '品牌市场', score: 40, idea: '让对的人知道你能提供什么价值。' },
        { key: 'rel', icon: '🤝', name: '关键关系', score: 58, idea: '核心层每周深聊、重要层每月联系；拒绝无效社交，学会说不。' },
        { key: 'cog', icon: '🔭', name: '竞争认知', score: 66, idea: '看清自己在牌桌上的位置和手里的牌。' },
        { key: 'risk', icon: '🛡️', name: '风险预案', score: 48, idea: '给最坏情况留后手，别让单点故障掀翻全局。' }
    ];

    var LAYERS = {
        core: { label: '核心圈', rx: 300, ry: 120, cycle: 7 },
        important: { label: '重点圈', rx: 560, ry: 178, cycle: 30 },
        normal: { label: '普通圈', rx: 810, ry: 222, cycle: null }
    };

    function open() {
        // 进入驾驶舱时复位到八板块视图（隐藏今日经营/记录回看子页，spec §5.4 入口）。
        var topbar = document.getElementById('praxis-topbar');
        var cockpit = document.getElementById('praxis-cockpit');
        var jview = document.getElementById('praxis-journal-view');
        var rview = document.getElementById('praxis-review-view');
        if (topbar) topbar.style.display = '';
        if (cockpit) cockpit.style.display = '';
        if (jview) jview.style.display = 'none';
        if (rview) rview.style.display = 'none';
        renderHealth();
        renderBoards();
        if (!_loaded) {
            _loaded = true;
            loadContacts();
        } else {
            render();
        }
        refreshJournalStatus();
    }

    // 刷新驾驶舱顶部「今日经营」入口的状态徽标（未记录/已记录/已分析）。
    async function refreshJournalStatus() {
        var pill = document.getElementById('praxis-today-status');
        if (!pill) return;
        try {
            var d = new Date();
            var p = function(n) { return n < 10 ? '0' + n : '' + n; };
            var ds = d.getFullYear() + '-' + p(d.getMonth() + 1) + '-' + p(d.getDate());
            var res = await API.praxisJournalList({ date: ds, limit: 1 });
            var e = res && res.items && res.items[0];
            var s = !e ? '未记录' : (e.structured && typeof e.structured === 'object' ? '已分析' : '已记录');
            pill.textContent = s;
            pill.className = 'praxis-today-status pj-status-' + (s === '已分析' ? 'analyzed' : s === '已记录' ? 'saved' : 'empty');
        } catch (err) {
            console.error('[Praxis] refresh journal status', err);
        }
    }

    async function loadContacts() {
        try {
            var res = await API.praxisContactList();
            if (!res || res.success === false) throw new Error((res && res.error) || '加载失败');
            _contacts = res.items || [];
            render();
        } catch (err) {
            console.error('[Praxis] load contacts failed', err);
            if (typeof showToast === 'function') showToast('Praxis 关系人加载失败', 'error');
        }
    }

    function scoreClass(n) {
        return n >= 75 ? 'ok' : (n >= 50 ? 'mid' : 'low');
    }

    // 八板块 tab 行：名称 + 评分环，hover 弹理念（spec §10）。
    function renderBoards() {
        var grid = document.getElementById('praxis-board-grid');
        if (!grid) return;
        grid.innerHTML = BOARDS.map(function(board) {
            var active = board.key === _activeBoard ? ' active' : '';
            var score = board.key === 'rel' ? _healthScore : board.score;
            return '<button class="praxis-btab' + active + '" onclick="Praxis.setBoard(\'' + board.key + '\')">' +
                '<span class="praxis-btab-name">' + esc(board.name) + '</span>' +
                '<span class="praxis-btab-score sc-' + scoreClass(score) + '">' + score + '</span>' +
                (board.idea ? '<span class="praxis-btab-idea">' + esc(board.idea) + '</span>' : '') +
                '</button>';
        }).join('');
    }

    function setBoard(key) {
        _activeBoard = key;
        renderBoards();
        render();
    }

    function renderHealth() {
        var ring = document.getElementById('praxis-health-ring');
        if (!ring) return;
        ring.textContent = _healthScore;
        ring.className = 'praxis-health-ring s-' + scoreClass(_healthScore);
    }

    function setHealthScore(value) {
        var n = Math.max(0, Math.min(100, parseInt(value, 10) || 0));
        _healthScore = n;
        renderHealth();
        renderBoards();
    }

    // 健康度手动设置（点击圆环，spec §2 手动自设）。
    function editHealth() {
        var v = window.prompt('设置健康度（0–100）', _healthScore);
        if (v === null) return;
        setHealthScore(v);
    }

    function render() {
        renderBoardHead();
        renderCount();
        renderArc();
        if (_activeBoard !== 'rel') {
            renderPlaceholder();
            return;
        }
        if (_selectedId) {
            var selected = findContact(_selectedId);
            if (selected) renderEditor(selected);
            else renderEditorEmpty();
        } else {
            renderEditorEmpty();
        }
    }

    // 板块内容区头部随当前板块切换：标题 / 理念 / 关系人专属控件（计数·新建·图例）。
    function renderBoardHead() {
        var board = BOARDS.find(function(b) { return b.key === _activeBoard; }) || {};
        var isRel = _activeBoard === 'rel';
        var title = document.getElementById('praxis-board-title');
        var count = document.getElementById('praxis-contact-count');
        var add = document.getElementById('praxis-add-contact');
        var idea = document.getElementById('praxis-board-idea');
        var legend = document.getElementById('praxis-legend');
        if (title) title.textContent = board.name || '';
        if (idea) idea.textContent = board.idea || '';
        if (count) count.style.display = isRel ? '' : 'none';
        if (add) add.style.display = isRel ? '' : 'none';
        if (legend) legend.style.display = isRel ? '' : 'none';
    }

    function renderCount() {
        var count = document.getElementById('praxis-contact-count');
        if (count) count.textContent = _contacts.length + ' 人';
    }

    function renderPlaceholder() {
        var wrap = document.getElementById('praxis-arc-wrap');
        var board = BOARDS.find(function(b) { return b.key === _activeBoard; });
        if (!wrap || !board) return;
        wrap.innerHTML = '<div class="praxis-static-board">' +
            '<div class="praxis-static-icon">' + board.icon + '</div>' +
            '<h3>' + esc(board.name) + '</h3>' +
            '<p>v0.1 先保留为占位面板</p>' +
            '</div>';
        renderEditorEmpty();
    }

    function renderArc() {
        if (_activeBoard !== 'rel') return;
        var wrap = document.getElementById('praxis-arc-wrap');
        if (!wrap) return;
        var svg = [
            '<svg class="praxis-arc-svg" viewBox="0 0 1000 470" role="img" aria-label="关系同心弧">',
            '<defs><linearGradient id="praxis-arc-grad" x1="0%" y1="50%" x2="100%" y2="50%"><stop offset="0%" stop-color="#d7dbe6"/><stop offset="100%" stop-color="#7c4dff"/></linearGradient></defs>',
            arcPath('core'), arcPath('important'), arcPath('normal'),
            '<circle class="praxis-self" cx="120" cy="235" r="30"></circle>',
            '<text class="praxis-self-text" x="120" y="240" text-anchor="middle">我</text>',
            labelsSvg(),
            nodesSvg(),
            '</svg>'
        ].join('');
        var empty = _contacts.length ? '' : '<div class="praxis-empty-hint">从新建关系人开始</div>';
        wrap.innerHTML = svg + empty;
    }

    function arcPath(layerKey) {
        var layer = LAYERS[layerKey];
        var sx = 120, cy = 235;
        var d = 'M ' + sx + ' ' + cy +
            ' C ' + sx + ' ' + (cy - layer.ry) + ' ' + (sx + layer.rx) + ' ' + (cy - layer.ry) + ' ' + (sx + layer.rx) + ' ' + cy +
            ' C ' + (sx + layer.rx) + ' ' + (cy + layer.ry) + ' ' + sx + ' ' + (cy + layer.ry) + ' ' + sx + ' ' + cy;
        return '<path class="praxis-arc praxis-arc-' + layerKey + '" d="' + d + '"></path>';
    }

    function labelsSvg() {
        return [
            '<text class="praxis-layer-label" x="402" y="127">核心圈 · 7天</text>',
            '<text class="praxis-layer-label" x="656" y="66">重点圈 · 30天</text>',
            '<text class="praxis-layer-label" x="888" y="24">普通圈</text>'
        ].join('');
    }

    function nodesSvg() {
        var byLayer = { core: [], important: [], normal: [] };
        _contacts.forEach(function(c) {
            byLayer[c.layer || 'normal'].push(c);
        });
        return Object.keys(byLayer).map(function(layerKey) {
            var items = byLayer[layerKey].sort(function(a, b) {
                return contactTime(b) - contactTime(a);
            });
            return items.map(function(c, idx) {
                var pos = nodePosition(layerKey, idx);
                var state = nodeState(c);
                var selected = c.id === _selectedId ? ' selected' : '';
                var risk = c.risk ? '<circle class="praxis-node-risk" cx="' + (pos.x + 16) + '" cy="' + (pos.y - 16) + '" r="5"></circle>' : '';
                return '<g class="praxis-node ' + state + selected + '" onclick="Praxis.selectContact(' + c.id + ')" tabindex="0">' +
                    '<circle cx="' + pos.x + '" cy="' + pos.y + '" r="18"></circle>' +
                    risk +
                    '<text x="' + pos.x + '" y="' + (pos.y + 38) + '" text-anchor="middle">' + esc(c.name) + '</text>' +
                    '</g>';
            }).join('');
        }).join('');
    }

    function nodePosition(layerKey, idx) {
        var layer = LAYERS[layerKey];
        var offset = idx === 0 ? 0 : Math.ceil(idx / 2) * (idx % 2 ? -1 : 1);
        var angle = Math.max(-70, Math.min(70, offset * 18));
        var rad = angle * Math.PI / 180;
        return {
            x: Math.round(120 + layer.rx * Math.cos(rad)),
            y: Math.round(235 + layer.ry * Math.sin(rad))
        };
    }

    function contactTime(contact) {
        if (!contact.lastContactAt) return 0;
        var ts = Date.parse(contact.lastContactAt);
        return Number.isFinite(ts) ? ts : 0;
    }

    function nodeState(contact) {
        var layer = LAYERS[contact.layer] || LAYERS.normal;
        var hasDate = !!contact.lastContactAt;
        var overdue = false;
        if (!hasDate) {
            overdue = true;
        } else if (layer.cycle) {
            var days = (Date.now() - contactTime(contact)) / 86400000;
            overdue = days > layer.cycle;
        }
        if ((contact.layer === 'core' || contact.layer === 'important') && overdue && !contact.cycleOff) return 'hi';
        if (overdue) return 'dim';
        if (contact.lastQuality === 'deep') return 'solid';
        return 'light';
    }

    function startCreate() {
        _selectedId = null;
        renderEditor({
            id: null,
            name: '',
            layer: 'normal',
            lastContactAt: '',
            lastQuality: '',
            risk: false,
            note: '',
            cycleOff: false
        });
    }

    function selectContact(id) {
        _selectedId = id;
        render();
    }

    function renderEditorEmpty() {
        var editor = document.getElementById('praxis-editor');
        if (!editor) return;
        editor.innerHTML = '<div class="praxis-editor-empty">选择节点或新建关系人</div>';
    }

    function renderEditor(contact) {
        var editor = document.getElementById('praxis-editor');
        if (!editor) return;
        var isNew = !contact.id;
        var html = '<form class="praxis-form" id="praxis-contact-form">' +
            '<div class="praxis-form-head"><h3>' + (isNew ? '新建关系人' : '关系详情') + '</h3>' +
            (isNew ? '' : '<button type="button" class="praxis-danger" onclick="Praxis.deleteSelected()">删除</button>') + '</div>' +
            field('姓名', '<input name="name" maxlength="60" required value="' + escAttr(contact.name || '') + '">') +
            field('圈层', '<select name="layer">' + option('core', '核心圈', contact.layer) + option('important', '重点圈', contact.layer) + option('normal', '普通圈', contact.layer) + '</select>') +
            field('最近联系', '<input name="lastContactAt" type="date" value="' + escAttr(dateOnly(contact.lastContactAt)) + '">') +
            field('质量', '<select name="lastQuality">' + option('', '未记录', contact.lastQuality || '') + option('shallow', '浅触达', contact.lastQuality) + option('effective', '有效', contact.lastQuality) + option('deep', '深度', contact.lastQuality) + '</select>') +
            '<label class="praxis-check"><input name="risk" type="checkbox" ' + (contact.risk ? 'checked' : '') + '> 有风险</label>' +
            '<label class="praxis-check"><input name="cycleOff" type="checkbox" ' + (contact.cycleOff ? 'checked' : '') + '> 暂停周期提醒</label>' +
            field('备注', '<textarea name="note" rows="4">' + esc(contact.note || '') + '</textarea>') +
            '<button class="eg-btn eg-btn--primary" type="submit">' + (isNew ? '创建' : '保存') + '</button>' +
            '</form>';
        if (!isNew) html += logsSectionHtml();
        editor.innerHTML = html;
        var form = document.getElementById('praxis-contact-form');
        if (form) {
            form.addEventListener('submit', function(ev) {
                ev.preventDefault();
                saveContact(contact.id, form);
            });
        }
        if (!isNew) {
            var logForm = document.getElementById('praxis-log-form');
            if (logForm) {
                logForm.addEventListener('submit', function(ev) {
                    ev.preventDefault();
                    saveLog(contact.id, logForm);
                });
            }
            loadLogs(contact.id);
        }
    }

    // ===== T-287:关系人交流记录 =====
    var LOG_METHODS = ['面对面', '电话', '微信', '会议', '邮件', '其他'];

    function logsSectionHtml() {
        var methodOpts = '<option value="">方式</option>' + LOG_METHODS.map(function(m) {
            return '<option value="' + esc(m) + '">' + esc(m) + '</option>';
        }).join('');
        var qualityOpts = '<option value="">质量</option>' +
            option('shallow', '浅触达', '') + option('effective', '有效', '') + option('deep', '深度', '');
        return '<div class="praxis-logs">' +
            '<div class="praxis-logs-head"><h4>交流记录</h4>' +
            '<button type="button" class="eg-btn" onclick="Praxis.toggleLogForm()">+ 记录交流</button></div>' +
            '<div class="praxis-log-form-wrap" id="praxis-log-form-wrap" style="display:none;">' +
            '<form class="praxis-log-form" id="praxis-log-form">' +
            '<div class="praxis-log-form-row">' +
            '<input name="at" type="date" value="' + todayStr() + '" required>' +
            '<select name="method">' + methodOpts + '</select>' +
            '<select name="quality">' + qualityOpts + '</select>' +
            '</div>' +
            '<textarea name="content" rows="2" placeholder="聊了什么（摘要）"></textarea>' +
            '<textarea name="note" rows="2" placeholder="我的心得（可选）"></textarea>' +
            '<button class="eg-btn eg-btn--primary" type="submit">保存交流</button>' +
            '</form></div>' +
            '<div class="praxis-logs-timeline" id="praxis-logs-timeline"><div class="praxis-logs-empty">加载中…</div></div>' +
            '</div>';
    }

    function toggleLogForm() {
        var f = document.getElementById('praxis-log-form-wrap');
        if (f) f.style.display = f.style.display === 'none' ? '' : 'none';
    }

    async function loadLogs(contactId) {
        var box = document.getElementById('praxis-logs-timeline');
        if (!box) return;
        try {
            var res = await API.praxisContactLogList(contactId);
            if (!res || res.success === false) throw new Error((res && res.error) || '加载失败');
            renderLogs(res.items || []);
        } catch (err) {
            console.error('[Praxis] load logs failed', err);
            box.innerHTML = '<div class="praxis-logs-empty">交流记录加载失败</div>';
        }
    }

    function qualityLabel(q) {
        return { shallow: '浅触达', effective: '有效', deep: '深度' }[q] || '';
    }

    function renderLogs(logs) {
        var box = document.getElementById('praxis-logs-timeline');
        if (!box) return;
        if (!logs.length) {
            box.innerHTML = '<div class="praxis-logs-empty">还没有交流记录，点「+ 记录交流」开始。</div>';
            return;
        }
        box.innerHTML = logs.map(function(l) {
            var m = l.method ? '<span class="praxis-log-method">' + esc(l.method) + '</span>' : '';
            var q = l.quality ? '<span class="praxis-log-q q-' + esc(l.quality) + '">' + esc(qualityLabel(l.quality)) + '</span>' : '';
            var note = l.note ? '<details class="praxis-log-note"><summary>心得</summary>' + esc(l.note) + '</details>' : '';
            return '<div class="praxis-log-item">' +
                '<div class="praxis-log-head"><b>' + esc(dateOnly(l.at)) + '</b>' + m + q + '</div>' +
                (l.content ? '<p class="praxis-log-content">' + esc(l.content) + '</p>' : '') +
                note + '</div>';
        }).join('');
    }

    async function saveLog(contactId, form) {
        var data = {
            at: form.at.value || todayStr(),
            method: form.method.value || null,
            quality: form.quality.value || null,
            content: form.content.value || '',
            note: form.note.value || ''
        };
        if (!data.at) {
            if (typeof showToast === 'function') showToast('选个交流时间', 'info');
            return;
        }
        try {
            var res = await API.praxisContactLogCreate(contactId, data);
            if (!res || res.success === false) throw new Error((res && res.error) || '保存失败');
            // 回写驱动节点状态：把本地 contact 的最近联系时间/质量同步后重渲弧。
            if (res.contactUpdated) {
                _contacts = _contacts.map(function(c) {
                    return c.id === contactId
                        ? Object.assign({}, c, { lastContactAt: data.at, lastQuality: data.quality })
                        : c;
                });
            }
            render();   // 重渲弧 + 节点状态；重开该 contact 详情会重新拉 logs
            if (typeof showToast === 'function') showToast('已记录交流', 'success');
        } catch (err) {
            console.error('[Praxis] save log failed', err);
            if (typeof showToast === 'function') showToast(err.message || '保存失败', 'error');
        }
    }

    function field(label, control) {
        return '<label class="praxis-field"><span>' + label + '</span>' + control + '</label>';
    }

    function option(value, label, selected) {
        return '<option value="' + escAttr(value) + '"' + (value === (selected || '') ? ' selected' : '') + '>' + esc(label) + '</option>';
    }

    async function saveContact(id, form) {
        var data = {
            name: form.name.value,
            layer: form.layer.value,
            lastContactAt: form.lastContactAt.value || null,
            lastQuality: form.lastQuality.value || null,
            risk: form.risk.checked,
            cycleOff: form.cycleOff.checked,
            note: form.note.value || ''
        };
        try {
            var res = id ? await API.praxisContactUpdate(id, data) : await API.praxisContactCreate(data);
            if (!res || res.success === false) throw new Error((res && res.error) || '保存失败');
            if (id) {
                _contacts = _contacts.map(function(c) { return c.id === id ? res.item : c; });
            } else {
                _contacts.push(res.item);
                _selectedId = res.item.id;
            }
            render();
            if (typeof showToast === 'function') showToast('已保存', 'success');
        } catch (err) {
            console.error('[Praxis] save contact failed', err);
            if (typeof showToast === 'function') showToast(err.message || '保存失败', 'error');
        }
    }

    async function deleteSelected() {
        if (!_selectedId) return;
        if (!window.confirm('删除这个关系人？')) return;
        try {
            var res = await API.praxisContactDelete(_selectedId);
            if (!res || res.success === false) throw new Error((res && res.error) || '删除失败');
            _contacts = _contacts.filter(function(c) { return c.id !== _selectedId; });
            _selectedId = null;
            render();
            if (typeof showToast === 'function') showToast('已删除', 'success');
        } catch (err) {
            console.error('[Praxis] delete contact failed', err);
            if (typeof showToast === 'function') showToast('删除失败', 'error');
        }
    }

    function findContact(id) {
        return _contacts.find(function(c) { return c.id === id; });
    }

    function dateOnly(value) {
        return value ? String(value).slice(0, 10) : '';
    }

    function todayStr() {
        var d = new Date();
        var p = function(n) { return n < 10 ? '0' + n : '' + n; };
        return d.getFullYear() + '-' + p(d.getMonth() + 1) + '-' + p(d.getDate());
    }

    function esc(value) {
        return String(value == null ? '' : value)
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;')
            .replace(/'/g, '&#39;');
    }

    function escAttr(value) {
        return esc(value);
    }

    return {
        open: open,
        setBoard: setBoard,
        setHealthScore: setHealthScore,
        editHealth: editHealth,
        startCreate: startCreate,
        selectContact: selectContact,
        deleteSelected: deleteSelected,
        refreshJournalStatus: refreshJournalStatus,
        toggleLogForm: toggleLogForm
    };
})();
