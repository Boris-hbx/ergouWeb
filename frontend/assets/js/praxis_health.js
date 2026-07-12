// ========== Praxis 健康板块 v0.3 (T-296) 靶盘视图 ==========
// 招牌视图「健康靶盘」：3 同心圈(优先级 核心/常规/观察) × 4 象限(类别 动/吃/睡·恢复/体征·信号)。
// 位置 = 优先级×类别(固定)；颜色 = 执行/完成度(会变)。边栏 2×4 分数看板 + 加权总分。
// 严格对照原型 docs/praxis-health-target-preview.html；布点复用「同(象限,圈层)内均分子弧中点」防重叠。
var PraxisHealth = (function() {
    var _board = null;      // { week, dims:[...], total }
    var _sub = 'body';      // body(身体) / finance(占位)
    var _weekOffset = 0;    // 0=本周
    var _selId = null;      // 选中维度 id
    var _metrics = [];      // 选中维度的指标记录
    var _marks = [];        // 选中(习惯/信号)维度近 21 天打卡(趋势时间线, T-297②)
    var _addMode = false;   // 新增维度表单态 (T-297④)
    var _detailFor = null;  // 已加载明细的维度 id(防重拉守卫, T-297②)
    var _busy = false;
    var _loaded = false;

    var CX = 250, CY = 250;
    // 三圈：r=点落半径(带中线)，band=背景带内外半径，w=总分权重。
    var RINGS = {
        core:  { r: 78,  band: [38, 118],  label: '核心', w: 3 },
        mid:   { r: 158, band: [118, 198], label: '常规', w: 2 },
        watch: { r: 222, band: [198, 246], label: '观察', w: 1 }
    };
    // 四象限（0°=右，逆时针为正，y 向上翻转）：动=右上, 吃=左上, 睡恢复=左下, 体征=右下。
    // lx/ly：象限名落在圆环外的四角空白（半径≈280，远离最外圈观察点，避免与节点叠字）。
    var SECTORS = {
        move: { a: [6, 84],    label: '动',       lx: 198,  ly: -198 },
        eat:  { a: [96, 174],  label: '吃',       lx: -198, ly: -198 },
        rest: { a: [186, 264], label: '睡·恢复',  lx: -198, ly: 198 },
        sign: { a: [276, 354], label: '体征·信号', lx: 198,  ly: 198 }
    };
    var RING_ORDER = ['core', 'mid', 'watch'];
    var KIND_LABEL = { habit: '每日打卡', metric: '周期自测', signal: '信号/自评' };

    function color(s) {
        if (s == null) return '#c3c1bd';
        return s < 50 ? '#cc7a72' : (s < 75 ? '#cdaa5c' : '#7fa87f');
    }
    function esc(v) {
        return String(v == null ? '' : v)
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;').replace(/'/g, '&#39;');
    }
    function f(n) { return Math.round(n * 100) / 100; }
    function pos(ang, r) {
        var a = ang * Math.PI / 180;
        return { x: CX + r * Math.cos(a), y: CY - r * Math.sin(a) };
    }
    function fmtDate(d) {
        var p = function(n) { return n < 10 ? '0' + n : '' + n; };
        return d.getFullYear() + '-' + p(d.getMonth() + 1) + '-' + p(d.getDate());
    }
    function todayStr() { return fmtDate(new Date()); }
    // ISO-8601 周键（周一为周首，含首个周四的周为第 1 周）——与后端 chrono 一致。
    function isoWeek(d) {
        var date = new Date(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()));
        var day = date.getUTCDay() || 7;
        date.setUTCDate(date.getUTCDate() + 4 - day);
        var yearStart = new Date(Date.UTC(date.getUTCFullYear(), 0, 1));
        var wk = Math.ceil((((date - yearStart) / 86400000) + 1) / 7);
        return date.getUTCFullYear() + '-W' + (wk < 10 ? '0' : '') + wk;
    }
    function activeWeek() {
        var d = new Date();
        d.setDate(d.getDate() + _weekOffset * 7);
        return isoWeek(d);
    }

    async function render() {
        renderShell();
        if (!_loaded) {
            await load();
        } else {
            paint();
        }
    }

    async function load() {
        var host = document.getElementById('praxis-health-view');
        try {
            var res = await API.praxisHealthBoard(activeWeek());
            if (!res || res.success === false) throw new Error((res && res.error) || '加载失败');
            _board = res.board;
            _loaded = true;
            paint();
        } catch (err) {
            console.error('[PraxisHealth] load board', err);
            if (host) {
                var stage = host.querySelector('#ph-stage');
                if (stage) stage.innerHTML = '<div class="ph-empty">健康数据加载失败，稍后重试。</div>';
            }
            if (typeof showToast === 'function') showToast('健康板块加载失败', 'error');
        }
    }

    // 外框：子 tab + 周选择 + 操作条 + 舞台（靶盘 + 边栏）。
    function renderShell() {
        var host = document.getElementById('praxis-health-view');
        if (!host || host.dataset.built === '1') return;
        host.innerHTML =
            '<div class="ph-head">' +
              '<div class="ph-subtabs">' +
                '<button class="ph-subtab on" data-s="body" onclick="PraxisHealth.setSub(\'body\')">身体健康</button>' +
                '<button class="ph-subtab ghost" data-s="finance" title="后续展开" onclick="PraxisHealth.setSub(\'finance\')">财务健康 · 后续</button>' +
              '</div>' +
              '<div class="ph-weekpick">' +
                '<button onclick="PraxisHealth.shiftWeek(-1)" title="上一周">‹</button>' +
                '<span id="ph-week-label"></span>' +
                '<button onclick="PraxisHealth.shiftWeek(1)" title="下一周">›</button>' +
              '</div>' +
              '<div class="ph-actions">' +
                '<button class="ph-btn" id="ph-derive" onclick="PraxisHealth.runDerive()" title="从每日复盘记录抽取身体信号回填">↺ 从每日复盘派生</button>' +
                '<button class="ph-btn pri" id="ph-score" onclick="PraxisHealth.runScore()" title="让 AI 教练打分(可解释)">✦ AI 打分</button>' +
                '<button class="ph-btn" onclick="PraxisHealth.addDim()" title="自定义追踪维度">+ 维度</button>' +
              '</div>' +
            '</div>' +
            '<div class="ph-stage" id="ph-stage"></div>';
        host.dataset.built = '1';
    }

    function paint() {
        var host = document.getElementById('praxis-health-view');
        if (!host) return;
        var wl = document.getElementById('ph-week-label');
        if (wl) wl.textContent = (_board ? _board.week : activeWeek()).replace('-W', ' · 第 ') + ' 周';
        var stage = document.getElementById('ph-stage');
        if (!stage) return;
        if (_sub === 'finance') {
            stage.innerHTML = '<div class="ph-placeholder">💰 财务健康子板块为 v0.3 占位，后续展开。</div>';
            return;
        }
        var dims = (_board && _board.dims) || [];
        // T-297⑤：把真实总分回填八板块「健康」tab 评分环。
        var total = (_board && _board.total) || {};
        if (window.Praxis && Praxis.setHealthBoardScore) Praxis.setHealthBoardScore(total.score);
        var side = _addMode ? addFormHtml() : (_selId ? editorHtml() : sidebarHtml(dims));
        stage.innerHTML =
            '<div class="ph-board">' + boardSvg(dims) + '</div>' +
            '<div class="ph-side">' + side + '</div>';
        if (_selId && !_addMode) loadDetailIfNeeded();
    }

    // ===== 靶盘 SVG =====
    function boardSvg(dims) {
        var parts = ['<svg class="ph-target" viewBox="0 0 500 500" role="img" aria-label="健康靶盘">'];
        // 背景带（外→内）
        [['watch', '#f6f3ee'], ['mid', '#f1ede7'], ['core', '#e9e4dc']].forEach(function(kc) {
            parts.push('<circle cx="' + CX + '" cy="' + CY + '" r="' + RINGS[kc[0]].band[1] + '" fill="' + kc[1] + '"></circle>');
        });
        parts.push('<circle cx="' + CX + '" cy="' + CY + '" r="' + RINGS.core.band[0] + '" fill="#fff"></circle>');
        // 圈层描边
        RING_ORDER.forEach(function(k) {
            parts.push('<circle cx="' + CX + '" cy="' + CY + '" r="' + RINGS[k].band[1] + '" fill="none" stroke="#e4e0d8" stroke-width="1"></circle>');
        });
        // 十字轴
        parts.push('<line x1="' + CX + '" y1="' + (CY - 246) + '" x2="' + CX + '" y2="' + (CY + 246) + '" stroke="#e0dcd4" stroke-width="1"></line>');
        parts.push('<line x1="' + (CX - 246) + '" y1="' + CY + '" x2="' + (CX + 246) + '" y2="' + CY + '" stroke="#e0dcd4" stroke-width="1"></line>');
        // 圈层名（顶部竖轴外侧）
        RING_ORDER.forEach(function(k) {
            parts.push('<text x="' + (CX + 5) + '" y="' + (CY - RINGS[k].band[1] + 14) + '" font-size="10" fill="#b7b2a8">' + RINGS[k].label + '</text>');
        });
        // 象限名
        Object.keys(SECTORS).forEach(function(sk) {
            var S = SECTORS[sk];
            parts.push('<text x="' + (CX + S.lx) + '" y="' + (CY + S.ly) + '" font-size="12.5" fill="#a49e93" text-anchor="middle" font-weight="600">' + S.label + '</text>');
        });
        // 中心总分
        var total = (_board && _board.total) || {};
        var tScore = total.score == null ? '—' : total.score;
        parts.push('<text x="' + CX + '" y="' + (CY - 4) + '" font-size="20" text-anchor="middle" font-weight="700" fill="#b98d86">' + esc(tScore) + '</text>');
        parts.push('<text x="' + CX + '" y="' + (CY + 12) + '" font-size="9" text-anchor="middle" fill="#b7b2a8">总分</text>');
        // 布点：同(象限,圈层)内均分子弧中点，防重叠（复用关系弧引擎思路）。
        var groups = {};
        dims.forEach(function(d) {
            var key = d.sector + '|' + d.ring;
            (groups[key] = groups[key] || []).push(d);
        });
        Object.keys(groups).forEach(function(key) {
            var arr = groups[key];
            var sk = key.split('|')[0], ring = key.split('|')[1];
            var S = SECTORS[sk], r = (RINGS[ring] || RINGS.mid).r;
            if (!S) return;
            var a0 = S.a[0], a1 = S.a[1], step = (a1 - a0) / arr.length;
            arr.forEach(function(d, i) {
                var ang = a0 + step * (i + 0.5);
                var p = pos(ang, r);
                var sel = d.id === _selId ? ' ph-node-sel' : '';
                var tip = d.name + ' · ' + (RINGS[d.ring] ? RINGS[d.ring].label : '') + ' · ' + (d.score == null ? '待测' : d.score + ' 分') + (d.trend ? ' ' + d.trend : '');
                parts.push('<g class="ph-node' + sel + '" onclick="PraxisHealth.selectDim(' + d.id + ')" tabindex="0"><title>' + esc(tip) + '</title>' +
                    '<text x="' + f(p.x) + '" y="' + f(p.y - 13) + '" font-size="10.5" text-anchor="middle" fill="#6f6a61">' + esc(d.name) + '</text>' +
                    '<circle cx="' + f(p.x) + '" cy="' + f(p.y) + '" r="9" fill="' + color(d.score) + '" stroke="#fff" stroke-width="2"></circle>' +
                    (d.trend && d.trend !== '→' ? '<text x="' + f(p.x) + '" y="' + f(p.y + 3.5) + '" font-size="9" text-anchor="middle" fill="#fff">' + esc(d.trend) + '</text>' : '') +
                    '</g>');
            });
        });
        parts.push('</svg>');
        return parts.join('');
    }

    // ===== 边栏：总分 + 2×4 分数看板 + 图例 =====
    function sidebarHtml(dims) {
        var total = (_board && _board.total) || {};
        var tScore = total.score == null ? '—' : total.score;
        var ai = total.explain || '还没打分——点右上「AI 打分」，让教练按你的留痕给可解释的健康度。';
        var html =
            '<div class="ph-total">' +
              '<div class="ph-total-ring" style="border-color:' + color(total.score) + '"><b>' + esc(tScore) + '</b><span>总分</span></div>' +
              '<div class="ph-total-meta"><h4>本周健康度' + (total.trend ? ' <em>' + esc(total.trend) + '</em>' : '') + '</h4><p>' + esc(ai) + '</p></div>' +
            '</div>' +
            '<div class="ph-scoreboard">';
        Object.keys(SECTORS).forEach(function(sk) {
            html += '<div class="ph-sbcell"><h5>' + SECTORS[sk].label + '</h5>';
            dims.filter(function(d) { return d.sector === sk; })
                .sort(function(a, b) { return RING_ORDER.indexOf(a.ring) - RING_ORDER.indexOf(b.ring); })
                .forEach(function(d) {
                    html += '<div class="ph-row" onclick="PraxisHealth.selectDim(' + d.id + ')" title="' + esc((RINGS[d.ring] ? RINGS[d.ring].label : '') + (d.explain ? ' · ' + d.explain : '')) + '">' +
                        '<span class="ph-dot" style="background:' + color(d.score) + '"></span>' +
                        '<span class="ph-nm">' + esc(d.name) + '</span>' +
                        '<span class="ph-sc">' + (d.score == null ? '—' : d.score) + '</span>' +
                        '<span class="ph-tr">' + esc(d.trend || '') + '</span></div>';
                });
            html += '</div>';
        });
        html += '</div>' +
            '<div class="ph-legend">' +
              '<span><i style="background:#cc7a72"></i>不行</span>' +
              '<span><i style="background:#cdaa5c"></i>预警</span>' +
              '<span><i style="background:#7fa87f"></i>良好</span>' +
              '<span><i style="background:#c3c1bd"></i>待测/未接入</span>' +
            '</div>' +
            '<div class="ph-note"><b>怎么读：</b>位置＝优先级×类别（固定），颜色＝完成度（会变）。最内圈的红点＝对你最重要、且现在最该管。点维度可打卡/录指标。</div>';
        return html;
    }

    // ===== 选中维度编辑器 =====
    function findDim(id) {
        return ((_board && _board.dims) || []).find(function(d) { return d.id === id; });
    }
    function editorHtml() {
        var d = findDim(_selId);
        if (!d) return sidebarHtml((_board && _board.dims) || []);
        var scoreLine = d.score == null
            ? '<span class="ph-badge gray">待测</span>'
            : '<span class="ph-badge" style="background:' + color(d.score) + '">' + d.score + ' 分 ' + esc(d.trend || '') + '</span>';
        var html =
            '<div class="ph-editor">' +
              '<button class="ph-back" onclick="PraxisHealth.back()">← 返回看板</button>' +
              '<div class="ph-ed-head"><h4>' + esc(d.name) + '</h4>' + scoreLine + '</div>' +
              '<div class="ph-ed-meta">' + (RINGS[d.ring] ? RINGS[d.ring].label : '') + ' · ' + esc(SECTORS[d.sector] ? SECTORS[d.sector].label : '') + ' · ' + esc(KIND_LABEL[d.kind] || d.kind) + '</div>';
        if (d.explain) html += '<div class="ph-why">💡 ' + esc(d.explain) + '</div>';
        // T-297②：三类目标都显示（基线 floor / 个人目标 goal），不再只显 floor。
        if (d.targetFloor) html += '<div class="ph-target-line"><b>基线</b>（守）：' + esc(d.targetFloor) + '</div>';
        if (d.targetGoal) html += '<div class="ph-target-line"><b>个人目标</b>（攻）：' + esc(d.targetGoal) + '</div>';

        // T-297②：趋势时间线（习惯/信号=近 21 天打卡点条；指标=历次实测折线）。
        html += timelineHtml(d);

        if (d.kind === 'metric') {
            html += metricFormHtml(d);
        } else {
            html += habitFormHtml(d);
        }
        // 维度设置（改圈层/类别/单位/目标/删除）
        html +=
            '<details class="ph-dim-settings"><summary>维度设置</summary>' +
              '<div class="ph-set-row"><label>优先级</label>' + ringPills(d) + '</div>' +
              '<div class="ph-set-row"><label>类别</label>' + sectorPills(d) + '</div>' +
              '<div class="ph-set-row"><label>单位</label><input id="ph-set-unit" value="' + esc(d.unit || '') + '" placeholder="如 分钟/升/cm"></div>' +
              '<div class="ph-set-row"><label>基线</label><input id="ph-set-floor" value="' + esc(d.targetFloor || '') + '" placeholder="健康基线(守)"></div>' +
              '<div class="ph-set-row"><label>目标</label><input id="ph-set-goal" value="' + esc(d.targetGoal || '') + '" placeholder="个人目标(攻)"></div>' +
              '<div class="ph-set-acts"><button class="ph-btn" onclick="PraxisHealth.saveDim()">保存维度</button>' +
              '<button class="ph-btn danger" onclick="PraxisHealth.deleteDim()">删除维度</button></div>' +
            '</details>';
        html += '</div>';
        return html;
    }

    function habitFormHtml(d) {
        var unit = d.unit ? ('（' + esc(d.unit) + '）') : '';
        return '<div class="ph-checkin">' +
            '<div class="ph-ci-streak">🔥 连续 <b>' + (d.streak || 0) + '</b> 天</div>' +
            '<div class="ph-ci-row">' +
              '<input id="ph-ci-val" type="number" step="any" placeholder="数值' + unit + '(可选)">' +
              '<button class="ph-btn pri" onclick="PraxisHealth.checkIn(' + d.id + ')">今天已做 ✓</button>' +
            '</div>' +
            '<div class="ph-ci-hint">打一次卡即记入连续天数与覆盖度；也可写进「每日复盘」让 AI 抓。</div>' +
            '</div>';
    }

    function metricFormHtml(d) {
        var rows = _metrics.length
            ? _metrics.map(function(m) {
                return '<div class="ph-mrow"><span class="ph-mdt">' + esc((m.measuredAt || '').slice(5)) + '</span>' +
                    '<span class="ph-mval">' + esc(m.textValue || (m.value == null ? '' : m.value)) + ' ' + esc(m.unit || '') + '</span>' +
                    '<span class="ph-msrc">' + esc(m.source || '') + '</span>' +
                    '<button class="ph-mx" title="删除" onclick="PraxisHealth.deleteMetric(' + m.id + ')">×</button></div>';
            }).join('')
            : '<div class="ph-mempty">还没有实测记录。测一次起基线。</div>';
        return '<div class="ph-metricbox">' +
            '<div class="ph-mform">' +
              '<div class="ph-mform-row"><input id="ph-m-date" type="date" value="' + todayStr() + '">' +
              '<input id="ph-m-val" type="number" step="any" placeholder="数值">' +
              '<input id="ph-m-unit" value="' + esc(d.unit || '') + '" placeholder="单位" style="width:64px"></div>' +
              '<div class="ph-mform-row"><select id="ph-m-src"><option value="self">自测</option><option value="exam">体检</option><option value="device">设备</option></select>' +
              '<input id="ph-m-note" placeholder="备注(可选)"></div>' +
              '<button class="ph-btn pri" onclick="PraxisHealth.recordMetric(' + d.id + ')">记录一次实测</button>' +
            '</div>' +
            '<div class="ph-mlist">' + rows + '</div>' +
            '</div>';
    }

    // T-297②：趋势时间线。习惯/信号→近 21 天打卡点条；指标→历次实测值序列。
    function timelineHtml(d) {
        if (d.kind === 'metric') {
            if (!_metrics.length) return '';
            var seq = _metrics.slice().reverse().map(function(m) {   // _metrics 为 desc，转升序
                var v = m.textValue || (m.value == null ? '·' : m.value);
                return '<span class="ph-tl-mv">' + esc((m.measuredAt || '').slice(5)) + ' <b>' + esc(v) + '</b></span>';
            }).join('<span class="ph-tl-arrow">→</span>');
            return '<div class="ph-timeline"><div class="ph-tl-h">趋势（历次实测）</div><div class="ph-tl-metrics">' + seq + '</div></div>';
        }
        var byDate = {};
        _marks.forEach(function(m) { byDate[m.markDate] = m; });
        var cells = '', base = new Date();
        for (var i = 20; i >= 0; i--) {
            var dt = new Date(); dt.setDate(base.getDate() - i);
            var ds = fmtDate(dt), m = byDate[ds];
            var cls = !m ? 'none' : (m.done ? 'done' : 'miss');
            var tip = ds + (m ? ((m.done ? ' ✓' : ' 记录') + (m.value != null ? ' ' + m.value : '') + (m.note ? ' ' + m.note : '')) : ' 未打卡');
            cells += '<span class="ph-tl-cell ' + cls + '" title="' + esc(tip) + '"></span>';
        }
        return '<div class="ph-timeline"><div class="ph-tl-h">近 21 天</div><div class="ph-tl-strip">' + cells + '</div></div>';
    }

    // T-297④：新增维度表单（替代两次 prompt；sector/ring/kind 用下拉）。
    function addFormHtml() {
        var secOpt = Object.keys(SECTORS).map(function(k) { return '<option value="' + k + '">' + SECTORS[k].label + '</option>'; }).join('');
        var ringOpt = RING_ORDER.map(function(k) { return '<option value="' + k + '"' + (k === 'mid' ? ' selected' : '') + '>' + RINGS[k].label + '</option>'; }).join('');
        var kindOpt = Object.keys(KIND_LABEL).map(function(k) { return '<option value="' + k + '">' + KIND_LABEL[k] + '</option>'; }).join('');
        return '<div class="ph-editor">' +
            '<button class="ph-back" onclick="PraxisHealth.cancelAdd()">← 返回看板</button>' +
            '<div class="ph-ed-head"><h4>新增追踪维度</h4></div>' +
            '<div class="ph-set-row"><label>名称</label><input id="ph-add-name" maxlength="40" placeholder="如 冥想 / 泡沫轴放松"></div>' +
            '<div class="ph-set-row"><label>类别</label><select id="ph-add-sector">' + secOpt + '</select></div>' +
            '<div class="ph-set-row"><label>优先级</label><select id="ph-add-ring">' + ringOpt + '</select></div>' +
            '<div class="ph-set-row"><label>采集</label><select id="ph-add-kind">' + kindOpt + '</select></div>' +
            '<div class="ph-set-row"><label>单位</label><input id="ph-add-unit" placeholder="可选，如 分钟"></div>' +
            '<div class="ph-set-acts"><button class="ph-btn pri" id="ph-add-save" onclick="PraxisHealth.createDim()">创建维度</button>' +
            '<button class="ph-btn" onclick="PraxisHealth.cancelAdd()">取消</button></div>' +
            '</div>';
    }

    function ringPills(d) {
        return '<div class="ph-pills" data-field="ring">' + RING_ORDER.map(function(k) {
            return '<span class="ph-pill' + (k === d.ring ? ' on' : '') + '" data-v="' + k + '" onclick="PraxisHealth.pick(this)">' + RINGS[k].label + '</span>';
        }).join('') + '</div>';
    }
    function sectorPills(d) {
        return '<div class="ph-pills" data-field="sector">' + Object.keys(SECTORS).map(function(k) {
            return '<span class="ph-pill' + (k === d.sector ? ' on' : '') + '" data-v="' + k + '" onclick="PraxisHealth.pick(this)">' + SECTORS[k].label + '</span>';
        }).join('') + '</div>';
    }
    function pick(el) {
        var box = el.parentElement;
        box.querySelectorAll('.ph-pill').forEach(function(p) { p.classList.toggle('on', p === el); });
    }

    // ===== 交互 =====
    function setSub(s) {
        _sub = s;
        _selId = null;
        _addMode = false;
        document.querySelectorAll('#praxis-health-view .ph-subtab').forEach(function(el) {
            el.classList.toggle('on', el.dataset.s === s);
        });
        paint();
    }
    function shiftWeek(delta) {
        _weekOffset += delta;
        if (_weekOffset > 0) _weekOffset = 0;   // 不看未来
        _selId = null;
        load();
    }
    function selectDim(id) {
        _selId = id;
        _addMode = false;
        _metrics = [];
        _marks = [];
        _detailFor = null;   // 触发该维度明细(指标/打卡)重新拉取
        paint();
    }
    function back() {
        _selId = null;
        _addMode = false;
        paint();
    }
    // T-297②：加载选中维度明细。metric→实测；habit/signal→近 21 天打卡（供时间线）。
    // `_detailFor` 占位守卫：避免 paint()→拉取→paint() 的无限重拉循环（T-296 遗留）。
    function loadDetailIfNeeded() {
        if (_detailFor === _selId) return;
        var d = findDim(_selId);
        if (!d) return;
        _detailFor = _selId;   // 乐观占位，阻断重复请求与重绘循环
        var done = function() { if (_selId === d.id) paint(); };
        if (d.kind === 'metric') {
            API.praxisHealthMetricList(d.id).then(function(res) {
                if (res && res.success !== false) _metrics = res.items || [];
                done();
            }).catch(function(err) { console.error('[PraxisHealth] metrics', err); _detailFor = null; });
        } else {
            var fromD = new Date();
            fromD.setDate(fromD.getDate() - 20);
            API.praxisHealthMarkList(fmtDate(fromD), todayStr()).then(function(res) {
                if (res && res.success !== false) {
                    _marks = (res.items || []).filter(function(m) { return m.dimId === d.id; });
                }
                done();
            }).catch(function(err) { console.error('[PraxisHealth] marks', err); _detailFor = null; });
        }
    }

    // T-297④：所有写操作加 _busy 防重复提交。
    async function checkIn(id) {
        if (_busy) return;
        _busy = true;
        var btn = document.querySelector('#praxis-health-view .ph-checkin .ph-btn.pri');
        if (btn) btn.disabled = true;
        var valEl = document.getElementById('ph-ci-val');
        var v = valEl && valEl.value !== '' ? parseFloat(valEl.value) : null;
        try {
            var res = await API.praxisHealthMarkUpsert({ dimId: id, done: true, value: v });
            if (!res || res.success === false) throw new Error((res && res.error) || '打卡失败');
            if (typeof showToast === 'function') showToast('已打卡', 'success');
            _detailFor = null;   // 让时间线随新打卡刷新
            await load();
        } catch (err) {
            console.error('[PraxisHealth] checkIn', err);
            if (typeof showToast === 'function') showToast(err.message || '打卡失败', 'error');
        } finally {
            _busy = false;
        }
    }

    async function recordMetric(id) {
        if (_busy) return;
        _busy = true;
        var btn = document.querySelector('#praxis-health-view .ph-metricbox .ph-btn.pri');
        if (btn) btn.disabled = true;
        var date = (document.getElementById('ph-m-date') || {}).value || todayStr();
        var valStr = (document.getElementById('ph-m-val') || {}).value;
        var unit = (document.getElementById('ph-m-unit') || {}).value || '';
        var src = (document.getElementById('ph-m-src') || {}).value || 'self';
        var note = (document.getElementById('ph-m-note') || {}).value || '';
        var data = { dimId: id, measuredAt: date, unit: unit, source: src, note: note };
        if (valStr !== '' && valStr != null) data.value = parseFloat(valStr);
        else data.textValue = note || '已测';
        try {
            var res = await API.praxisHealthMetricCreate(data);
            if (!res || res.success === false) throw new Error((res && res.error) || '记录失败');
            if (typeof showToast === 'function') showToast('已记录实测', 'success');
            _detailFor = null;
            paint();
        } catch (err) {
            console.error('[PraxisHealth] recordMetric', err);
            if (typeof showToast === 'function') showToast(err.message || '记录失败', 'error');
        } finally {
            _busy = false;
        }
    }

    async function deleteMetric(id) {
        if (_busy) return;
        if (!window.confirm('删除这条实测记录？')) return;
        _busy = true;
        try {
            var res = await API.praxisHealthMetricDelete(id);
            if (!res || res.success === false) throw new Error((res && res.error) || '删除失败');
            _detailFor = null;
            paint();
        } catch (err) {
            console.error('[PraxisHealth] deleteMetric', err);
            if (typeof showToast === 'function') showToast('删除失败', 'error');
        } finally {
            _busy = false;
        }
    }

    async function saveDim() {
        if (_busy) return;
        var d = findDim(_selId);
        if (!d) return;
        _busy = true;
        var ringEl = document.querySelector('#praxis-health-view .ph-pills[data-field="ring"] .ph-pill.on');
        var secEl = document.querySelector('#praxis-health-view .ph-pills[data-field="sector"] .ph-pill.on');
        var patch = {
            ring: ringEl ? ringEl.dataset.v : d.ring,
            sector: secEl ? secEl.dataset.v : d.sector,
            unit: (document.getElementById('ph-set-unit') || {}).value || '',
            targetFloor: (document.getElementById('ph-set-floor') || {}).value || '',
            targetGoal: (document.getElementById('ph-set-goal') || {}).value || ''
        };
        try {
            var res = await API.praxisHealthDimUpdate(d.id, patch);
            if (!res || res.success === false) throw new Error((res && res.error) || '保存失败');
            if (typeof showToast === 'function') showToast('已保存维度', 'success');
            await load();
        } catch (err) {
            console.error('[PraxisHealth] saveDim', err);
            if (typeof showToast === 'function') showToast(err.message || '保存失败', 'error');
        } finally {
            _busy = false;
        }
    }

    async function deleteDim() {
        var d = findDim(_selId);
        if (!d) return;
        if (!window.confirm('删除维度「' + d.name + '」？该维度的打卡/指标记录会一并隐藏。')) return;
        try {
            var res = await API.praxisHealthDimDelete(d.id);
            if (!res || res.success === false) throw new Error((res && res.error) || '删除失败');
            _selId = null;
            if (typeof showToast === 'function') showToast('已删除维度', 'success');
            await load();
        } catch (err) {
            console.error('[PraxisHealth] deleteDim', err);
            if (typeof showToast === 'function') showToast(err.message || '删除失败', 'error');
        }
    }

    // T-297④：打开新增维度表单（不再用 prompt）。
    function addDim() {
        _addMode = true;
        _selId = null;
        paint();
    }
    function cancelAdd() {
        _addMode = false;
        paint();
    }
    async function createDim() {
        if (_busy) return;
        var name = ((document.getElementById('ph-add-name') || {}).value || '').trim();
        if (!name) {
            if (typeof showToast === 'function') showToast('填个维度名称', 'info');
            return;
        }
        _busy = true;
        var btn = document.getElementById('ph-add-save');
        if (btn) btn.disabled = true;
        var data = {
            name: name,
            sector: (document.getElementById('ph-add-sector') || {}).value || 'move',
            ring: (document.getElementById('ph-add-ring') || {}).value || 'mid',
            kind: (document.getElementById('ph-add-kind') || {}).value || 'habit',
            unit: (document.getElementById('ph-add-unit') || {}).value || ''
        };
        try {
            var res = await API.praxisHealthDimCreate(data);
            if (!res || res.success === false) throw new Error((res && res.error) || '创建失败');
            if (typeof showToast === 'function') showToast('已新增维度', 'success');
            _addMode = false;
            await load();
        } catch (err) {
            console.error('[PraxisHealth] createDim', err);
            if (typeof showToast === 'function') showToast(err.message || '创建失败', 'error');
        } finally {
            _busy = false;
        }
    }

    async function runScore() {
        if (_busy) return;
        _busy = true;
        var btn = document.getElementById('ph-score');
        if (btn) { btn.disabled = true; btn.textContent = '打分中…'; }
        try {
            var res = await API.praxisHealthScore();
            if (!res || res.success === false) throw new Error((res && res.error) || 'AI 打分失败');
            _board = res.board;
            _selId = null;
            paint();
            if (typeof showToast === 'function') showToast('AI 已给出可解释健康度', 'success');
        } catch (err) {
            console.error('[PraxisHealth] runScore', err);
            if (typeof showToast === 'function') showToast(err.message || 'AI 打分失败，可重试', 'error');
        } finally {
            _busy = false;
            if (btn) { btn.disabled = false; btn.textContent = '✦ AI 打分'; }
        }
    }

    async function runDerive() {
        if (_busy) return;
        _busy = true;
        var btn = document.getElementById('ph-derive');
        if (btn) { btn.disabled = true; btn.textContent = '派生中…'; }
        try {
            var res = await API.praxisHealthDerive();
            if (!res || res.success === false) throw new Error((res && res.error) || '派生失败');
            if (typeof showToast === 'function') showToast('已从每日复盘派生 ' + (res.derived || 0) + ' 条身体信号', 'success');
            await load();
        } catch (err) {
            console.error('[PraxisHealth] runDerive', err);
            if (typeof showToast === 'function') showToast(err.message || '派生失败', 'error');
        } finally {
            _busy = false;
            if (btn) { btn.disabled = false; btn.textContent = '↺ 从每日复盘派生'; }
        }
    }

    return {
        render: render,
        setSub: setSub,
        shiftWeek: shiftWeek,
        selectDim: selectDim,
        back: back,
        pick: pick,
        checkIn: checkIn,
        recordMetric: recordMetric,
        deleteMetric: deleteMetric,
        saveDim: saveDim,
        deleteDim: deleteDim,
        addDim: addDim,
        createDim: createDim,
        cancelAdd: cancelAdd,
        runScore: runScore,
        runDerive: runDerive
    };
})();
