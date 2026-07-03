// ========== Praxis module (T-283 / T-291 视角+双tab+派生只读+同心弧改版) ==========
var Praxis = (function() {
    var _loaded = false;
    var _contacts = [];
    var _perspectives = [];
    var _activePerspectiveId = null;
    var _selectedId = null;
    var _activeBoard = 'rel';
    var _sideTab = 'info';   // 右侧面板当前 tab：info(人物信息) / logs(交流信息)
    var _healthScore = 58;

    var BOARDS = [
        { key: 'strategy', icon: '🎯', name: '战略定位', score: 62, idea: '想清楚要成为什么样的人，再决定做什么事。' },
        { key: 'skill', icon: '🛠️', name: '能力产品', score: 78, idea: '把能力打磨成可被识别、可被交付的产品。' },
        { key: 'ops', icon: '⏱️', name: '运营管理', score: 55, idea: '用系统和习惯托住状态，别靠意志硬扛。' },
        { key: 'health', icon: '🩺', name: '健康', score: 58, idea: '身体是经营自己的基础资本；先行指标(动/吃/睡)决定滞后的体能与体征。' },
        { key: 'brand', icon: '📣', name: '品牌市场', score: 40, idea: '让对的人知道你能提供什么价值。' },
        { key: 'rel', icon: '🤝', name: '关键关系', score: 58, idea: '核心层每周深聊、重点层每月联系；拒绝无效社交，学会说不。' },
        { key: 'cog', icon: '🔭', name: '竞争认知', score: 66, idea: '看清自己在牌桌上的位置和手里的牌。' },
        { key: 'risk', icon: '🛡️', name: '风险预案', score: 48, idea: '给最坏情况留后手，别让单点故障掀翻全局。' }
    ];

    // 同心弧几何 + 配色（T-295 §11.8，严格对照 docs/praxis-arc-prototype.html）。
    // 正圆扇形：三层同一圆心（在「我」CX,CY），只在右前方张开 SPAN° 扇形，半径依次变大。
    // 人散落在各层「分区」内（沿角度 + 沿半径 2D 铺开），彩色弧只是每层外边界线。
    // 中性代号：显示名 圈一/圈二/圈三（后端 layer 键仍 core/important/normal 不变，仅前端映射）。
    // 周期语义（圈一约7天/圈二约30天/圈三不强制）移到 pill tooltip，不进弧标签。
    var CX = 58, CY = 248, SPAN = 70;
    var A0 = -SPAN / 2, A1 = SPAN / 2;   // 扇形起止角（0°=正右，负=上，正=下）
    // rBound=该层外边界半径；cap=容量软上限；rows=分区内「同心排」(r=半径, k=该排满员人数)。
    // rows 满员数 = cap，原型 v4 已验证满员时圆点+名字不重叠。
    var LAYERS = {
        core:      { display: '圈一', cap: 5,  cycle: 7,    col: '#5B34D6', rBound: 150,
                     cycleHint: '圈一 · 建议约 7 天联系一次', rows: [{ r: 82, k: 2 }, { r: 128, k: 3 }] },
        important: { display: '圈二', cap: 15, cycle: 30,   col: '#8257F5', rBound: 278,
                     cycleHint: '圈二 · 建议约 30 天联系一次', rows: [{ r: 180, k: 4 }, { r: 222, k: 5 }, { r: 260, k: 6 }] },
        normal:    { display: '圈三', cap: 30, cycle: null, col: '#B49BF0', rBound: 415,
                     cycleHint: '圈三 · 不强制联系周期', rows: [{ r: 300, k: 9 }, { r: 340, k: 10 }, { r: 380, k: 11 }] }
    };
    var LAYER_ORDER = ['core', 'important', 'normal'];

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
            loadPerspectives();
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

    // ===== 视角（T-291 §11.1）=====
    async function loadPerspectives() {
        try {
            var res = await API.praxisPerspectiveList();
            if (!res || res.success === false) throw new Error((res && res.error) || '加载失败');
            _perspectives = res.items || [];
            if (!_activePerspectiveId || !_perspectives.some(function(p) { return p.id === _activePerspectiveId; })) {
                _activePerspectiveId = _perspectives.length ? _perspectives[0].id : null;
            }
            await loadContacts();
        } catch (err) {
            console.error('[Praxis] load perspectives failed', err);
            if (typeof showToast === 'function') showToast('Praxis 视角加载失败', 'error');
        }
    }

    async function loadContacts() {
        try {
            var res = await API.praxisContactList(_activePerspectiveId);
            if (!res || res.success === false) throw new Error((res && res.error) || '加载失败');
            _contacts = res.items || [];
            // 后端可能回落默认视角，以响应回显的 perspectiveId 为准。
            if (res.perspectiveId) _activePerspectiveId = res.perspectiveId;
            render();
        } catch (err) {
            console.error('[Praxis] load contacts failed', err);
            if (typeof showToast === 'function') showToast('Praxis 关系人加载失败', 'error');
        }
    }

    function switchPerspective(id) {
        if (id === _activePerspectiveId) return;
        _activePerspectiveId = id;
        _selectedId = null;
        _sideTab = 'info';
        loadContacts();
    }

    async function createPerspective() {
        var name = window.prompt('新建视角名称（如 工作 / 人生建议 / 日常，1–30 字）', '');
        if (name === null) return;
        name = name.trim();
        if (!name) return;
        try {
            var res = await API.praxisPerspectiveCreate({ name: name });
            if (!res || res.success === false) throw new Error((res && res.error) || '创建失败');
            await loadPerspectivesKeep(res.item ? res.item.id : null);
            if (typeof showToast === 'function') showToast('已新建视角', 'success');
        } catch (err) {
            console.error('[Praxis] create perspective failed', err);
            if (typeof showToast === 'function') showToast(err.message || '创建视角失败', 'error');
        }
    }

    // 重新拉视角列表，并切到指定视角（新建后切到新视角）。
    async function loadPerspectivesKeep(preferId) {
        var res = await API.praxisPerspectiveList();
        if (!res || res.success === false) throw new Error((res && res.error) || '加载失败');
        _perspectives = res.items || [];
        if (preferId && _perspectives.some(function(p) { return p.id === preferId; })) {
            _activePerspectiveId = preferId;
        } else if (!_perspectives.some(function(p) { return p.id === _activePerspectiveId; })) {
            _activePerspectiveId = _perspectives.length ? _perspectives[0].id : null;
        }
        _selectedId = null;
        _sideTab = 'info';
        await loadContacts();
    }

    async function deletePerspective(id) {
        var persp = _perspectives.find(function(p) { return p.id === id; });
        if (!persp) return;
        if (!window.confirm('删除视角「' + persp.name + '」？（视角内不能有关系人）')) return;
        try {
            var res = await API.praxisPerspectiveDelete(id);
            // 删非空视角后端返 409：{success:false,error:"视角内还有 N 位关系人，请先清空再删除"}。
            if (!res || res.success === false) {
                if (typeof showToast === 'function') showToast((res && res.error) || '该视角非空，请先清空再删除', 'warning');
                return;
            }
            await loadPerspectivesKeep(null);
            if (typeof showToast === 'function') showToast('已删除视角', 'success');
        } catch (err) {
            console.error('[Praxis] delete perspective failed', err);
            if (typeof showToast === 'function') showToast(err.message || '删除视角失败', 'error');
        }
    }

    function renderPerspBar() {
        var bar = document.getElementById('praxis-persp-bar');
        if (!bar) return;
        if (_activeBoard !== 'rel') {
            bar.style.display = 'none';
            return;
        }
        bar.style.display = '';
        var canDelete = _perspectives.length > 1;
        var chips = _perspectives.map(function(p) {
            var active = p.id === _activePerspectiveId ? ' active' : '';
            var del = (active && canDelete)
                ? '<span class="pchip-x" title="删除视角" onclick="event.stopPropagation(); Praxis.deletePerspective(' + p.id + ')">×</span>'
                : '';
            return '<span class="pchip' + active + '" onclick="Praxis.switchPerspective(' + p.id + ')">' + esc(p.name) + del + '</span>';
        }).join('');
        bar.innerHTML =
            '<span class="persp-lab">视角</span>' +
            chips +
            '<span class="pchip add" onclick="Praxis.createPerspective()">+ 新建视角</span>' +
            '<button class="persp-addp" onclick="Praxis.startCreate()">+ 新建关系人</button>';
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

    // T-297⑤：健康板块 AI 打分后回填「健康」tab 的评分环（原硬编码 58）。
    function setHealthBoardScore(n) {
        if (n == null) return;
        var b = BOARDS.find(function(x) { return x.key === 'health'; });
        if (!b) return;
        b.score = n;
        renderBoards();
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
        renderPerspBar();
        var relLayout = document.getElementById('praxis-relation-layout');
        var healthView = document.getElementById('praxis-health-view');
        // 健康板块(T-296)：独立靶盘视图，交给 PraxisHealth 模块渲染。
        if (_activeBoard === 'health') {
            if (relLayout) relLayout.style.display = 'none';
            if (healthView) healthView.style.display = '';
            if (window.PraxisHealth) PraxisHealth.render();
            return;
        }
        if (relLayout) relLayout.style.display = '';
        if (healthView) healthView.style.display = 'none';
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

    // 板块内容区头部随当前板块切换：标题 / 理念 / 关系人专属控件（计数·图例）。
    function renderBoardHead() {
        var board = BOARDS.find(function(b) { return b.key === _activeBoard; }) || {};
        var isRel = _activeBoard === 'rel';
        var title = document.getElementById('praxis-board-title');
        var count = document.getElementById('praxis-contact-count');
        var idea = document.getElementById('praxis-board-idea');
        var legend = document.getElementById('praxis-legend');
        if (title) title.textContent = board.name || '';
        if (idea) idea.textContent = board.idea || '';
        if (count) count.style.display = isRel ? '' : 'none';
        if (legend) legend.style.display = isRel ? '' : 'none';
    }

    function renderCount() {
        var count = document.getElementById('praxis-contact-count');
        if (count) count.textContent = '本视角 ' + _contacts.length + ' 人 · 与其它视角隔离';
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

    // ===== 同心弧（T-295 §11.8，严格对照 docs/praxis-arc-prototype.html）=====
    // 极坐标 → 直角（deg：0°=正右，A0/A1 上下张开）。正圆，单一半径。
    function polar(r, deg) {
        var a = deg * Math.PI / 180;
        return { x: CX + r * Math.cos(a), y: CY + r * Math.sin(a) };
    }
    function f(n) { return Math.round(n * 100) / 100; }
    // 单层外边界弧：SVG 圆弧命令 A r r（正圆，废弃椭圆 rx/ry）。
    function arcSeg(r) {
        var s = polar(r, A0), e = polar(r, A1);
        return 'M ' + f(s.x) + ' ' + f(s.y) + ' A ' + r + ' ' + r + ' 0 0 1 ' + f(e.x) + ' ' + f(e.y);
    }
    // 分区扇形（从圆心「我」到外边界弧再回收），用于填充分区底色。
    function wedge(r) {
        var s = polar(r, A0), e = polar(r, A1);
        return 'M ' + CX + ' ' + CY + ' L ' + f(s.x) + ' ' + f(s.y) +
            ' A ' + r + ' ' + r + ' 0 0 1 ' + f(e.x) + ' ' + f(e.y) + ' Z';
    }

    // 在某层分区内为 n 个人算落点：内排先满、末排取余；同心排上均匀铺开、隔排半格错位。
    // n 超容量时余数并入末排（更密但不叠，配合温和提示）。返回 [{r, deg}] 顺序对应传入 items。
    function layerPositions(layer, n) {
        var rows = layer.rows, pad = 4, lo = A0 + pad, hi = A1 - pad;
        var counts = [], left = n;
        for (var i = 0; i < rows.length; i++) {
            var take = (i === rows.length - 1) ? left : Math.min(rows[i].k, left);
            if (take < 0) take = 0;
            counts.push(take);
            left -= take;
            if (left < 0) left = 0;
        }
        var pos = [];
        for (var r = 0; r < rows.length; r++) {
            var c = counts[r];
            if (!c) continue;
            // 每排 c 点落在 c 等分子弧的中点：两端留白、始终在 [lo,hi] 内、绝不重合；
            // 各排 c 不同（2/3、4/5/6、9/10/11）→ 天然错位，无需额外偏移。
            var step = (hi - lo) / c;
            for (var j = 0; j < c; j++) {
                pos.push({ r: rows[r].r, deg: lo + (j + 0.5) * step });
            }
        }
        return pos;
    }

    function renderArc() {
        if (_activeBoard !== 'rel') return;
        var wrap = document.getElementById('praxis-arc-wrap');
        if (!wrap) return;
        var parts = ['<svg class="praxis-arc-svg" viewBox="0 -14 505 514" role="img" aria-label="关系同心弧">'];
        // 分区底色：外→内叠放，同色低透明，越往里叠得越深 → 越里越亲近。
        LAYER_ORDER.slice().reverse().forEach(function(k) {
            parts.push('<path d="' + wedge(LAYERS[k].rBound) + '" fill="rgba(124,77,255,.055)" stroke="none"></path>');
        });
        // 外边界弧（每层不同色）+ 层级标签（弧上端外侧，中性代号 + 容量）。
        LAYER_ORDER.forEach(function(k) {
            var l = LAYERS[k];
            parts.push('<path class="praxis-arc praxis-arc-' + k + '" d="' + arcSeg(l.rBound) + '" stroke="' + l.col + '"></path>');
            var lp = polar(l.rBound + 13, A0);
            parts.push('<text class="praxis-layer-label" x="' + f(lp.x) + '" y="' + f(lp.y) + '" fill="' + l.col + '" text-anchor="middle">' + esc(l.display + ' · ≤' + l.cap) + '</text>');
        });
        parts.push(nodesSvg());
        parts.push('<circle class="praxis-self" cx="' + CX + '" cy="' + CY + '" r="15"></circle>');
        parts.push('<text class="praxis-self-text" x="' + CX + '" y="' + (CY + 4) + '" text-anchor="middle">我</text>');
        parts.push('</svg>');
        // 容量软上限：超额不挤爆，给温和提示（§11.8 ④）。
        var counts = { core: 0, important: 0, normal: 0 };
        _contacts.forEach(function(c) { counts[LAYERS[c.layer] ? c.layer : 'normal']++; });
        var over = LAYER_ORDER.filter(function(k) { return counts[k] > LAYERS[k].cap; })
            .map(function(k) { return LAYERS[k].display + '建议 ≤' + LAYERS[k].cap + ' 人，你已 ' + counts[k] + ' 人'; });
        var hint = over.length ? '<div class="praxis-cap-hint">💡 ' + esc(over.join('；')) + '</div>' : '';
        var empty = _contacts.length ? '' : '<div class="praxis-empty-hint">还没有关系人，点「+ 新建关系人」开始经营你的人脉。</div>';
        wrap.innerHTML = hint + parts.join('') + empty;
    }

    function nodesSvg() {
        var byLayer = { core: [], important: [], normal: [] };
        _contacts.forEach(function(c) {
            (byLayer[c.layer] || byLayer.normal).push(c);
        });
        return LAYER_ORDER.map(function(layerKey) {
            var layer = LAYERS[layerKey];
            // 同层按最近联系排序：最近的排在前 → 落到更内的排（越里越亲近）。
            var items = byLayer[layerKey].sort(function(a, b) {
                return contactTime(b) - contactTime(a);
            });
            var positions = layerPositions(layer, items.length);
            return items.map(function(c, idx) {
                var p = polar(positions[idx].r, positions[idx].deg);
                var x = f(p.x), y = f(p.y);
                var state = nodeState(c);
                var selected = c.id === _selectedId ? ' selected' : '';
                var halo = state === 'hi'
                    ? '<circle class="praxis-node-halo" cx="' + x + '" cy="' + y + '" r="10"></circle>'
                    : '';
                var risk = c.risk
                    ? '<circle class="praxis-node-risk" cx="' + f(p.x + 7) + '" cy="' + f(p.y - 7) + '" r="4"></circle>'
                    : '';
                // 名字随点放在圆点上方居中（原型做法），不串弧上。
                return '<g class="praxis-node ' + state + selected + '" onclick="Praxis.selectContact(' + c.id + ')" tabindex="0">' +
                    halo +
                    '<text x="' + x + '" y="' + f(p.y - 9) + '" text-anchor="middle">' + esc(c.name) + '</text>' +
                    '<circle class="praxis-node-dot" cx="' + x + '" cy="' + y + '" r="5.5"></circle>' +
                    risk +
                    '</g>';
            }).join('');
        }).join('');
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

    function statusText(contact) {
        var map = {
            solid: { t: '● 实心 · 最近深聊', c: '#5B34D6' },
            light: { t: '○ 空心 · 仅浅联系', c: '#7C4DFF' },
            dim:   { t: '● 发暗 · 超维护周期', c: '#9C97A8' },
            hi:    { t: '● 高亮 · 建议本周联系', c: '#7C4DFF' }
        };
        var s = map[nodeState(contact)] || map.dim;
        var risk = contact.risk ? ' <span style="color:#D6455F">· ⚠ 有风险</span>' : '';
        return '<b style="color:' + s.c + '">' + s.t + '</b>' + risk;
    }

    // ===== 关系人 CRUD + 右侧双 tab =====
    function startCreate() {
        _selectedId = null;
        _sideTab = 'info';
        renderEditorNew();
    }

    function selectContact(id) {
        _selectedId = id;
        _sideTab = 'info';
        render();
    }

    function setSideTab(tab) {
        _sideTab = tab;
        var info = document.getElementById('praxis-pane-info');
        var logs = document.getElementById('praxis-pane-logs');
        if (info) info.style.display = tab === 'info' ? '' : 'none';
        if (logs) logs.style.display = tab === 'logs' ? '' : 'none';
        document.querySelectorAll('#praxis-editor .praxis-stab').forEach(function(el) {
            el.classList.toggle('active', el.dataset.t === tab);
        });
    }

    function renderEditorEmpty() {
        var editor = document.getElementById('praxis-editor');
        if (!editor) return;
        editor.innerHTML = '<div class="praxis-editor-empty">选择节点或新建关系人</div>';
    }

    function layerPills(current) {
        return '<div class="pf-pills" data-field="layer">' + LAYER_ORDER.map(function(k) {
            return '<span class="pf-pill' + (k === current ? ' on' : '') + '" data-v="' + k + '" title="' + escAttr(LAYERS[k].cycleHint) + '" onclick="Praxis.pickPill(this)">' + LAYERS[k].display + '</span>';
        }).join('') + '</div>';
    }

    // 新建关系人：§11.3 只填 姓名/层级/备注/风险；不问最近联系（去交流信息记）。
    function renderEditorNew() {
        var editor = document.getElementById('praxis-editor');
        if (!editor) return;
        editor.innerHTML =
            '<div class="praxis-side">' +
            '<div class="praxis-side-title">新建关系人</div>' +
            '<div class="praxis-info">' +
            pfRow('姓名', '<input id="pf-name" maxlength="60" placeholder="必填">') +
            pfRow('层级', layerPills('normal')) +
            pfRow('标记', '<div class="pf-pills pf-marks"><span class="pf-pill" data-mark="risk" onclick="Praxis.toggleMark(this)">⚠ 有风险</span></div>') +
            pfRow('备注', '<input id="pf-note" placeholder="可选">') +
            '<button class="pf-save" onclick="Praxis.createPerson()">创建关系人</button>' +
            '</div></div>';
    }

    // 选中关系人：右侧两 tab（人物信息 / 交流信息）。
    function renderEditor(contact) {
        var editor = document.getElementById('praxis-editor');
        if (!editor) return;
        var recent = contact.lastContactAt
            ? (dateOnly(contact.lastContactAt) + (contact.lastQuality ? ' · ' + qualityLabel(contact.lastQuality) : ''))
            : '从未联系';
        var infoPane =
            '<div class="praxis-pane" id="praxis-pane-info"' + (_sideTab === 'info' ? '' : ' style="display:none"') + '>' +
            '<div class="praxis-info">' +
            pfRow('姓名', '<input id="pf-name" maxlength="60" value="' + escAttr(contact.name || '') + '">') +
            pfRow('层级', layerPills(contact.layer || 'normal')) +
            pfRow('标记', '<div class="pf-pills pf-marks">' +
                '<span class="pf-pill' + (contact.risk ? ' on' : '') + '" data-mark="risk" onclick="Praxis.toggleMark(this)">⚠ 有风险</span>' +
                '<span class="pf-pill' + (contact.cycleOff ? ' on' : '') + '" data-mark="cycleOff" onclick="Praxis.toggleMark(this)">暂停周期</span>' +
                '</div>') +
            pfRow('备注', '<input id="pf-note" value="' + escAttr(contact.note || '') + '">') +
            '<div class="pf-row pf-ro"><label>最近联系</label><div class="pf-rov">' + esc(recent) + '<span class="pf-lock">🔒 只读</span></div></div>' +
            '<div class="pf-row pf-ro"><label>当前状态</label><div class="pf-rov">' + statusText(contact) + '</div></div>' +
            '<div class="pf-hint">「最近联系」「状态」读取自『交流信息』最新一条，此处不可改；要改去交流信息记一条。</div>' +
            '<button class="pf-save" onclick="Praxis.savePerson(' + contact.id + ')">保存人物信息</button>' +
            '<button class="pf-del" onclick="Praxis.deleteSelected()">删除关系人</button>' +
            '</div></div>';
        var logsPane =
            '<div class="praxis-pane" id="praxis-pane-logs"' + (_sideTab === 'logs' ? '' : ' style="display:none"') + '>' +
            logsSectionHtml() +
            '</div>';
        editor.innerHTML =
            '<div class="praxis-side">' +
            '<div class="praxis-sidetabs">' +
            '<button class="praxis-stab' + (_sideTab === 'info' ? ' active' : '') + '" data-t="info" onclick="Praxis.setSideTab(\'info\')">👤 人物信息</button>' +
            '<button class="praxis-stab' + (_sideTab === 'logs' ? ' active' : '') + '" data-t="logs" onclick="Praxis.setSideTab(\'logs\')">💬 交流信息</button>' +
            '</div>' +
            infoPane + logsPane +
            '</div>';
        var logForm = document.getElementById('praxis-log-form');
        if (logForm) {
            logForm.addEventListener('submit', function(ev) {
                ev.preventDefault();
                saveLog(contact.id, logForm);
            });
        }
        loadLogs(contact.id);
    }

    function pfRow(label, control) {
        return '<div class="pf-row"><label>' + label + '</label>' + control + '</div>';
    }

    // 层级 pill 单选。
    function pickPill(el) {
        var box = el.parentElement;
        box.querySelectorAll('.pf-pill').forEach(function(p) { p.classList.toggle('on', p === el); });
    }

    // 标记 pill（风险/暂停周期）多选切换。
    function toggleMark(el) {
        el.classList.toggle('on');
    }

    function readPersonForm() {
        var name = (document.getElementById('pf-name') || {}).value || '';
        var note = (document.getElementById('pf-note') || {}).value || '';
        var layerEl = document.querySelector('#praxis-editor .pf-pills[data-field="layer"] .pf-pill.on');
        var layer = layerEl ? layerEl.dataset.v : 'normal';
        var risk = !!document.querySelector('#praxis-editor .pf-pill[data-mark="risk"].on');
        var cycleOff = !!document.querySelector('#praxis-editor .pf-pill[data-mark="cycleOff"].on');
        // §11.3：不再提交 lastContactAt / lastQuality（后端派生、只读）。
        return { name: name.trim(), layer: layer, risk: risk, cycleOff: cycleOff, note: note };
    }

    async function createPerson() {
        var data = readPersonForm();
        if (!data.name) {
            if (typeof showToast === 'function') showToast('填个姓名', 'info');
            return;
        }
        data.perspectiveId = _activePerspectiveId;
        try {
            var res = await API.praxisContactCreate(data);
            if (!res || res.success === false) throw new Error((res && res.error) || '保存失败');
            _contacts.push(res.item);
            _selectedId = res.item.id;
            render();
            if (typeof showToast === 'function') showToast('已新建关系人', 'success');
        } catch (err) {
            console.error('[Praxis] create contact failed', err);
            if (typeof showToast === 'function') showToast(err.message || '保存失败', 'error');
        }
    }

    async function savePerson(id) {
        var data = readPersonForm();
        if (!data.name) {
            if (typeof showToast === 'function') showToast('填个姓名', 'info');
            return;
        }
        try {
            var res = await API.praxisContactUpdate(id, data);
            if (!res || res.success === false) throw new Error((res && res.error) || '保存失败');
            _contacts = _contacts.map(function(c) { return c.id === id ? res.item : c; });
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

    // ===== 交流记录（T-287 + T-291 改删）=====
    var LOG_METHODS = ['面对面', '电话', '微信', '会议', '邮件', '其他'];
    var QUALITIES = [
        { v: 'shallow', t: '浅' },
        { v: 'effective', t: '有效' },
        { v: 'deep', t: '深聊' }
    ];

    function qualityLabel(q) {
        var m = { shallow: '浅', effective: '有效', deep: '深聊' };
        return m[q] || '';
    }

    function methodOptions(selected) {
        return '<option value="">方式</option>' + LOG_METHODS.map(function(m) {
            return '<option value="' + esc(m) + '"' + (m === selected ? ' selected' : '') + '>' + esc(m) + '</option>';
        }).join('');
    }

    function qualityPills(current, cls) {
        return '<div class="pf-pills ' + (cls || '') + '" data-field="quality">' + QUALITIES.map(function(q) {
            return '<span class="pf-pill' + (q.v === current ? ' on' : '') + '" data-v="' + q.v + '" onclick="Praxis.pickPill(this)">' + q.t + '</span>';
        }).join('') + '</div>';
    }

    function logsSectionHtml() {
        return '<div class="praxis-logs2">' +
            '<div class="pl-head">💬 交流记录 <button type="button" class="pl-add" onclick="Praxis.toggleLogForm()">+ 记录交流</button></div>' +
            '<div class="pl-form-wrap" id="praxis-log-form-wrap" style="display:none;">' +
            '<form class="praxis-log-form" id="praxis-log-form">' +
            '<div class="pl-form-row">' +
            '<input name="at" type="date" value="' + todayStr() + '" required>' +
            '<select name="method">' + methodOptions('') + '</select>' +
            '</div>' +
            '<div class="pf-row"><label>质量</label>' + qualityPills('', 'pl-new-quality') + '</div>' +
            '<textarea name="content" rows="2" placeholder="聊了什么（摘要）"></textarea>' +
            '<textarea name="note" rows="2" placeholder="我的心得（可选）"></textarea>' +
            '<button class="pf-save" type="submit">保存交流</button>' +
            '</form></div>' +
            '<div class="pl-list" id="praxis-logs-timeline"><div class="pl-empty">加载中…</div></div>' +
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
            renderLogs(contactId, res.items || []);
        } catch (err) {
            console.error('[Praxis] load logs failed', err);
            box.innerHTML = '<div class="pl-empty">交流记录加载失败</div>';
        }
    }

    function shortDate(v) {
        var s = dateOnly(v);
        var parts = s.split('-');
        return parts.length === 3 ? (parseInt(parts[1], 10) + '/' + parts[2]) : s;
    }

    function renderLogs(contactId, logs) {
        var box = document.getElementById('praxis-logs-timeline');
        if (!box) return;
        if (!logs.length) {
            box.innerHTML = '<div class="pl-empty">还没有交流记录，点「+ 记录交流」开始。</div>';
            return;
        }
        box.innerHTML = logs.map(function(l) {
            var q = l.quality ? '<span class="pl-q q-' + esc(l.quality) + '">' + esc(qualityLabel(l.quality)) + '</span>' : '';
            var sum = l.content || l.method || '（无摘要）';
            return '<div class="pl-log" data-id="' + l.id + '">' +
                '<div class="pl-row" onclick="Praxis.toggleLogRow(this)">' +
                '<span class="pl-caret">▸</span><span class="pl-dt">' + esc(shortDate(l.at)) + '</span>' + q +
                '<span class="pl-sum">' + esc(sum) + '</span></div>' +
                '<div class="pl-edit">' +
                pfRow('时间', '<input class="ple-at" type="date" value="' + escAttr(dateOnly(l.at)) + '">') +
                pfRow('方式', '<select class="ple-method">' + methodOptions(l.method || '') + '</select>') +
                pfRow('质量', qualityPills(l.quality || '', 'ple-quality')) +
                '<textarea class="ple-content" rows="2" placeholder="聊了什么">' + esc(l.content || '') + '</textarea>' +
                '<textarea class="ple-note" rows="2" placeholder="我的心得">' + esc(l.note || '') + '</textarea>' +
                '<div class="pl-acts">' +
                '<button class="pri" onclick="Praxis.saveLogEdit(' + contactId + ',' + l.id + ',this)">保存</button>' +
                '<button onclick="Praxis.toggleLogRow(this.closest(\'.pl-log\').querySelector(\'.pl-row\'))">取消</button>' +
                '<button class="del" onclick="Praxis.deleteLog(' + contactId + ',' + l.id + ')">删除</button>' +
                '</div></div></div>';
        }).join('');
    }

    function toggleLogRow(rowEl) {
        var log = rowEl.closest ? rowEl.closest('.pl-log') : rowEl.parentElement;
        if (log) log.classList.toggle('open');
    }

    async function saveLog(contactId, form) {
        var qEl = document.querySelector('#praxis-log-form .pl-new-quality .pf-pill.on');
        var data = {
            at: form.at.value || todayStr(),
            method: form.method.value || null,
            quality: qEl ? qEl.dataset.v : null,
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
            await afterLogChange(contactId, res.contactUpdated);
            if (typeof showToast === 'function') showToast('已记录交流', 'success');
        } catch (err) {
            console.error('[Praxis] save log failed', err);
            if (typeof showToast === 'function') showToast(err.message || '保存失败', 'error');
        }
    }

    async function saveLogEdit(contactId, logId, btn) {
        var box = btn.closest('.pl-log');
        if (!box) return;
        var qEl = box.querySelector('.ple-quality .pf-pill.on');
        var data = {
            at: (box.querySelector('.ple-at') || {}).value || todayStr(),
            method: (box.querySelector('.ple-method') || {}).value || null,
            quality: qEl ? qEl.dataset.v : null,
            content: (box.querySelector('.ple-content') || {}).value || '',
            note: (box.querySelector('.ple-note') || {}).value || ''
        };
        try {
            var res = await API.praxisContactLogUpdate(contactId, logId, data);
            if (!res || res.success === false) throw new Error((res && res.error) || '保存失败');
            await afterLogChange(contactId, res.contactUpdated);
            if (typeof showToast === 'function') showToast('已保存', 'success');
        } catch (err) {
            console.error('[Praxis] update log failed', err);
            if (typeof showToast === 'function') showToast(err.message || '保存失败', 'error');
        }
    }

    async function deleteLog(contactId, logId) {
        if (!window.confirm('删除这条交流记录？')) return;
        try {
            var res = await API.praxisContactLogDelete(contactId, logId);
            if (!res || res.success === false) throw new Error((res && res.error) || '删除失败');
            await afterLogChange(contactId, res.contactUpdated);
            if (typeof showToast === 'function') showToast('已删除', 'success');
        } catch (err) {
            console.error('[Praxis] delete log failed', err);
            if (typeof showToast === 'function') showToast(err.message || '删除失败', 'error');
        }
    }

    // 交流增/改/删后：contactUpdated 为真则重拉本视角关系人（刷新派生「最近联系」+ 弧），
    // 否则只重渲当前选中详情（会重新拉 logs）。均停留在交流信息 tab。
    async function afterLogChange(contactId, contactUpdated) {
        _sideTab = 'logs';
        if (contactUpdated) {
            await loadContacts();
        } else {
            render();
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
        setHealthBoardScore: setHealthBoardScore,
        editHealth: editHealth,
        startCreate: startCreate,
        selectContact: selectContact,
        deleteSelected: deleteSelected,
        refreshJournalStatus: refreshJournalStatus,
        // 视角
        switchPerspective: switchPerspective,
        createPerspective: createPerspective,
        deletePerspective: deletePerspective,
        // 右侧双 tab + 人物信息
        setSideTab: setSideTab,
        pickPill: pickPill,
        toggleMark: toggleMark,
        createPerson: createPerson,
        savePerson: savePerson,
        // 交流记录
        toggleLogForm: toggleLogForm,
        toggleLogRow: toggleLogRow,
        saveLogEdit: saveLogEdit,
        deleteLog: deleteLog
    };
})();
