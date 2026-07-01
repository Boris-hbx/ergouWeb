// ========== Praxis 今日经营记录 (T-285 / SPEC praxis §5) ==========
// 主输入口：轻量自然语言记录 → AI 结构化归八板块 + 摘要 + 明日调整 → 今日经营卡。
var PraxisJournal = (function() {
    var _date = null;       // 当前查看的 entry_date (YYYY-MM-DD)
    var _entry = null;      // 当前已加载/保存的记录对象，null = 未记录
    var _tags = {};         // 快速信号标记
    var _busy = false;

    var BOARD_NAMES = {
        strategy: '战略定位', skill: '能力产品', ops: '运营管理', fin: '财务健康',
        brand: '品牌市场', rel: '关键关系', cog: '竞争认知', risk: '风险预案'
    };
    var VALUE_LABELS = {
        build: '主动建设', respond: '被动响应', firefight: '救火',
        maintain: '维护', rest: '休整'
    };

    // 快速信号标记分组（spec §5.1.5 / 设计文档 §5.1.2.4）。
    var TAG_GROUPS = [
        { key: 'energy', label: '精力', multi: false, opts: [['high', '高'], ['mid', '中'], ['low', '低']] },
        { key: 'mood', label: '情绪', multi: false, opts: [['calm', '平稳'], ['excited', '兴奋'], ['anxious', '焦虑'], ['tired', '疲惫'], ['down', '低落']] },
        { key: 'type', label: '今天类型', multi: false, opts: [['build', '主动建设'], ['respond', '被动响应'], ['firefight', '救火'], ['maintain', '维护'], ['rest', '休整']] },
        { key: 'timeQuality', label: '时间质量', multi: false, opts: [['efficient', '高效'], ['normal', '一般'], ['fragmented', '碎片化'], ['outofcontrol', '失控']] },
        { key: 'relEvent', label: '关系事件', multi: false, opts: [['none', '无'], ['shallow', '浅联系'], ['effective', '有效联系'], ['deep', '深度交流']] },
        { key: 'risk', label: '风险信号', multi: true, opts: [['health', '健康'], ['finance', '财务'], ['career', '职业'], ['social', '人际'], ['emotion', '情绪']] }
    ];

    var PROMPTS = [
        '我今天做了什么', '我今天学到了什么', '我今天被什么消耗',
        '我今天和谁有重要交流', '我明天要调整什么'
    ];

    function pad(n) { return n < 10 ? '0' + n : '' + n; }
    function todayStr() {
        var d = new Date();
        return d.getFullYear() + '-' + pad(d.getMonth() + 1) + '-' + pad(d.getDate());
    }

    function open(date) {
        toggleView(true);
        _date = date || todayStr();
        var di = document.getElementById('pj-date');
        if (di) di.value = _date;
        renderTags();
        renderPrompts();
        loadDay();
    }

    function close() {
        toggleView(false);
        if (window.Praxis && typeof Praxis.refreshJournalStatus === 'function') {
            Praxis.refreshJournalStatus();
        }
    }

    function toggleView(showJournal) {
        ['praxis-cockpit', 'praxis-today-entry', 'praxis-review-entry'].forEach(function(id) {
            var el = document.getElementById(id);
            if (el) el.style.display = showJournal ? 'none' : '';
        });
        var rview = document.getElementById('praxis-review-view');
        if (rview) rview.style.display = 'none';
        var jview = document.getElementById('praxis-journal-view');
        if (jview) jview.style.display = showJournal ? '' : 'none';
    }

    function changeDate(value) {
        if (!value) return;
        _date = value;
        loadDay();
    }

    async function loadDay() {
        _entry = null;
        _tags = {};
        try {
            var res = await API.praxisJournalList({ date: _date, limit: 1 });
            if (!res || res.success === false) throw new Error((res && res.error) || '加载失败');
            _entry = (res.items && res.items[0]) || null;
        } catch (err) {
            console.error('[PraxisJournal] load day failed', err);
            if (typeof showToast === 'function') showToast('今日经营加载失败', 'error');
        }
        var raw = document.getElementById('pj-raw');
        if (raw) raw.value = _entry ? (_entry.rawText || '') : '';
        _tags = (_entry && _entry.tags && typeof _entry.tags === 'object') ? _entry.tags : {};
        renderTags();
        renderStatus();
        renderLeft();
        if (_entry && _entry.structured && typeof _entry.structured === 'object') {
            renderAi(_entry.structured);
        } else {
            renderAiEmpty();
        }
    }

    function statusOf(entry) {
        if (!entry) return '未记录';
        if (entry.structured && typeof entry.structured === 'object') return '已分析';
        return '已记录';
    }

    function renderStatus() {
        var pill = document.getElementById('pj-status-pill');
        if (!pill) return;
        var s = statusOf(_entry);
        var backfill = _date && _date !== todayStr() ? ' · 补记' : '';
        pill.textContent = s + backfill;
        pill.className = 'pj-status-pill pj-status-' + (s === '已分析' ? 'analyzed' : s === '已记录' ? 'saved' : 'empty');
    }

    function renderPrompts() {
        var wrap = document.getElementById('pj-prompts');
        if (!wrap) return;
        wrap.innerHTML = PROMPTS.map(function(p) {
            return '<button type="button" class="pj-prompt" onclick="PraxisJournal.insertPrompt(\'' + p + '\')">' + esc(p) + '</button>';
        }).join('');
    }

    function insertPrompt(text) {
        var raw = document.getElementById('pj-raw');
        if (!raw) return;
        var prefix = raw.value && !raw.value.endsWith('\n') ? '\n' : '';
        raw.value += prefix + text + '：';
        raw.focus();
    }

    function renderTags() {
        var wrap = document.getElementById('pj-tags');
        if (!wrap) return;
        wrap.innerHTML = TAG_GROUPS.map(function(g) {
            var chips = g.opts.map(function(o) {
                var val = o[0], label = o[1];
                var on = g.multi
                    ? (Array.isArray(_tags[g.key]) && _tags[g.key].indexOf(val) !== -1)
                    : (_tags[g.key] === val);
                return '<button type="button" class="pj-chip' + (on ? ' on' : '') +
                    '" onclick="PraxisJournal.toggleTag(\'' + g.key + '\',\'' + val + '\',' + g.multi + ')">' +
                    esc(label) + '</button>';
            }).join('');
            return '<div class="pj-tag-group"><span class="pj-tag-label">' + esc(g.label) + '</span>' + chips + '</div>';
        }).join('');
    }

    function toggleTag(key, value, multi) {
        if (multi) {
            var arr = Array.isArray(_tags[key]) ? _tags[key].slice() : [];
            var i = arr.indexOf(value);
            if (i === -1) arr.push(value); else arr.splice(i, 1);
            if (arr.length) _tags[key] = arr; else delete _tags[key];
        } else {
            if (_tags[key] === value) delete _tags[key]; else _tags[key] = value;
        }
        renderTags();
        renderLeft();
    }

    function collectData() {
        var raw = document.getElementById('pj-raw');
        return {
            entryDate: _date,
            rawText: raw ? raw.value : '',
            tags: _tags
        };
    }

    // 「仅保存」——始终可用，不依赖 AI（守 CLAUDE.md 按钮模式规范）。
    async function saveOnly() {
        if (_busy) return null;
        var data = collectData();
        if (!data.rawText.trim() && !Object.keys(_tags).length) {
            if (typeof showToast === 'function') showToast('先写点什么再保存', 'info');
            return null;
        }
        try {
            var res = _entry && _entry.id
                ? await API.praxisJournalUpdate(_entry.id, { rawText: data.rawText, tags: data.tags })
                : await API.praxisJournalCreate(data);
            if (!res || res.success === false) throw new Error((res && res.error) || '保存失败');
            _entry = res.item;
            renderStatus();
            renderLeft();
            if (typeof showToast === 'function') showToast('已保存', 'success');
            return _entry;
        } catch (err) {
            console.error('[PraxisJournal] save failed', err);
            if (typeof showToast === 'function') showToast(err.message || '保存失败', 'error');
            return null;
        }
    }

    // 「保存并分析」——先保存（原文不会因分析失败丢失），再调 AI 结构化。
    async function saveAndAnalyze() {
        if (_busy) return;
        var saved = await saveOnly();
        if (!saved || !saved.id) return;
        _busy = true;
        var btn = document.getElementById('pj-analyze-btn');
        if (btn) { btn.disabled = true; btn.textContent = '分析中…'; }
        try {
            var res = await API.praxisJournalAnalyze(saved.id);
            if (!res || res.success === false) {
                throw new Error((res && res.error) || '分析失败，可重试');
            }
            _entry = res.item;
            renderStatus();
            renderLeft();
            if (_entry.structured && typeof _entry.structured === 'object') {
                renderAi(_entry.structured);
            }
            if (typeof showToast === 'function') showToast('已生成今日经营卡', 'success');
        } catch (err) {
            console.error('[PraxisJournal] analyze failed', err);
            // 原文已保存，分析失败可重试，不阻塞（spec §5.3）。
            if (typeof showToast === 'function') showToast(err.message || '分析失败，可重试', 'error');
        } finally {
            _busy = false;
            if (btn) { btn.disabled = false; btn.textContent = '保存并分析'; }
        }
    }

    function renderAiEmpty() {
        var right = document.getElementById('pj-right');
        if (right) right.innerHTML = '<div class="pj-ai-empty">写完点「保存并分析」，这里会出现今日观察。</div>';
    }

    function renderAi(s) {
        var right = document.getElementById('pj-right');
        if (!right) return;
        var boards = Array.isArray(s.boards) ? s.boards : [];
        var events = Array.isArray(s.events) ? s.events.slice(0, 3) : [];
        var risks = Array.isArray(s.risks) ? s.risks : [];
        var html = '<div class="pj-card pj-fade">';
        html += '<div class="pj-card-head"><span class="pj-card-title">今日经营卡</span>' +
            '<span class="pj-card-date">' + esc(_entry ? _entry.entryDate : _date) + '</span></div>';
        if (s.value && VALUE_LABELS[s.value]) {
            html += '<div class="pj-card-type pj-type-' + esc(s.value) + '">' + esc(VALUE_LABELS[s.value]) + '</div>';
        }
        if (s.summary) html += '<p class="pj-card-summary">' + esc(s.summary) + '</p>';
        if (boards.length) {
            html += '<div class="pj-card-boards">' + boards.map(function(b) {
                return '<span class="pj-board-tag">' + esc(BOARD_NAMES[b] || b) + '</span>';
            }).join('') + '</div>';
        }
        if (events.length) {
            html += '<div class="pj-card-sec"><h5>发现的信号</h5><ul>' +
                events.map(function(e) { return '<li>' + esc(e) + '</li>'; }).join('') + '</ul></div>';
        }
        if (risks.length) {
            html += '<div class="pj-card-sec pj-card-risks"><h5>风险</h5><ul>' +
                risks.map(function(r) { return '<li>' + esc(r) + '</li>'; }).join('') + '</ul></div>';
        }
        if (s.tomorrow) {
            html += '<div class="pj-card-tomorrow"><h5>明日调整</h5><p>' + esc(s.tomorrow) + '</p></div>';
        }
        html += '<button type="button" class="pj-reanalyze" onclick="PraxisJournal.saveAndAnalyze()">重新分析</button>';
        html += '</div>';
        right.innerHTML = html;
    }

    function renderLeft() {
        var left = document.getElementById('pj-left');
        if (!left) return;
        var s = (_entry && _entry.structured && typeof _entry.structured === 'object') ? _entry.structured : {};
        var boards = Array.isArray(s.boards) ? s.boards : [];
        var relMap = { none: '无', shallow: '浅联系', effective: '有效联系', deep: '深度交流' };
        var riskMap = { health: '健康', finance: '财务', career: '职业', social: '人际', emotion: '情绪' };
        var rel = _tags.relEvent ? (relMap[_tags.relEvent] || _tags.relEvent) : '—';
        var risks = Array.isArray(_tags.risk) ? _tags.risk.map(function(r) { return riskMap[r] || r; }) : [];
        var rows = [
            ['日期', esc(_date) + (_date !== todayStr() ? ' <em>补记</em>' : '')],
            ['记录状态', esc(statusOf(_entry))],
            ['涉及板块', boards.length ? boards.map(function(b) { return esc(BOARD_NAMES[b] || b); }).join('、') : '—'],
            ['关系事件', esc(rel)],
            ['风险信号', risks.length ? risks.map(esc).join('、') : '—']
        ];
        left.innerHTML = '<h4 class="pj-left-title">今日状态</h4>' + rows.map(function(r) {
            return '<div class="pj-left-row"><span>' + r[0] + '</span><b>' + r[1] + '</b></div>';
        }).join('');
    }

    function esc(value) {
        return String(value == null ? '' : value)
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;').replace(/'/g, '&#39;');
    }

    return {
        open: open,
        close: close,
        changeDate: changeDate,
        insertPrompt: insertPrompt,
        toggleTag: toggleTag,
        saveOnly: saveOnly,
        saveAndAnalyze: saveAndAnalyze
    };
})();
