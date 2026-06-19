// ========== WorkPick — 单选/多选弹候选框 (T-094) ==========
//
// 锚定在某个元素下方弹出候选列表;用户选完点「确认」才回调,点遮罩/取消则丢弃。
// 设计原则(spec § 三 原则 5):多了"确认"这一步,消除了"循环切换"时的误改。
//
// 用法:
//   WorkPick.open({
//     anchor: domElement,            // 锚点(候选框定位在它下方)
//     options: [{key, label}, ...],  // 候选项列表
//     current: ['key1', 'key2'],     // 当前已选(数组)
//     isMulti: false,                // 单选 / 多选
//     onConfirm: function(chosenKeys) { ... },  // 用户点确认时回调
//   });
//
// 状态都在闭包里,同一时刻只允许一个 picker 打开。
//
// DOM 依赖:#wt-pick-bd(背景遮罩)、#wt-pick(浮层本体) —— 由 index.html 提供。

var WorkPick = (function() {
    var _state = null;  // { anchor, options, isMulti, chosen, onConfirm }
    var _fstate = null; // T-132 列筛选模式:{ anchor, options:[{value,label}], checked:Set, search, onConfirm, onClear }

    function open(opts) {
        if (!opts || !opts.anchor) {
            console.error('[WorkPick] open: missing anchor');
            return;
        }
        var options = opts.options || [];
        if (!options.length) {
            if (typeof showToast === 'function') {
                showToast('该列还没有选项 —— 请在「⚙ 列设置」里添加', 'warning');
            } else {
                alert('该列还没有选项 —— 请在「⚙ 列设置」里添加');
            }
            return;
        }
        var current = (opts.current || []).slice();
        _state = {
            anchor: opts.anchor,
            options: options,
            isMulti: !!opts.isMulti,
            chosen: current,
            onConfirm: opts.onConfirm || function() {},
        };
        _render();
        _position(_state.anchor);
        var bd = document.getElementById('wt-pick-bd');
        var pop = document.getElementById('wt-pick');
        if (bd) bd.classList.add('open');
        if (pop) pop.classList.add('open');
    }

    // ============ T-132:Excel 风列筛选浮层 ============
    // opts: { anchor, options:[{value,label}], selected:[value,...], onConfirm:fn(checkedValues[]), onClear:fn }
    // 复用本模块的浮层 DOM(#wt-pick)+ 定位 + 遮罩关闭。
    function openFilter(opts) {
        if (!opts || !opts.anchor) { console.error('[WorkPick] openFilter: missing anchor'); return; }
        _state = null;
        var options = opts.options || [];
        var sel = {};
        (opts.selected || []).forEach(function(v) { sel[v] = 1; });
        var checked = {};
        // selected 缺省(无筛选)= 全选
        options.forEach(function(o) {
            checked[o.value] = (opts.selected && opts.selected.length >= 0) ? !!sel[o.value] : true;
        });
        _fstate = {
            anchor: opts.anchor, options: options, checked: checked, search: '',
            onConfirm: opts.onConfirm || function() {}, onClear: opts.onClear || function() {},
        };
        _renderFilter();
        _position(opts.anchor);
        var bd = document.getElementById('wt-pick-bd');
        var pop = document.getElementById('wt-pick');
        if (bd) bd.classList.add('open');
        if (pop) pop.classList.add('open');
        var si = document.getElementById('wt-pick-search-input');
        if (si) si.focus();
    }

    function _renderFilter() {
        var pop = document.getElementById('wt-pick');
        if (!pop || !_fstate) return;
        var q = _fstate.search.toLowerCase();
        var visible = _fstate.options.filter(function(o) {
            return !q || ('' + o.label).toLowerCase().indexOf(q) >= 0;
        });
        var allChecked = visible.length > 0 && visible.every(function(o) { return _fstate.checked[o.value]; });
        var rows = visible.map(function(o) {
            var on = !!_fstate.checked[o.value];
            return '<div class="wt-pick-opt' + (on ? ' selected' : '') + '" data-val="' + _escAttr(o.value) + '">'
                 +   '<span class="wt-pick-check">' + (on ? '☑' : '☐') + '</span>'
                 +   '<span>' + _escape(o.label) + '</span></div>';
        }).join('') || '<div class="wt-pick-noval">无候选值</div>';
        pop.innerHTML =
            '<div class="wt-pick-search"><input id="wt-pick-search-input" type="text" placeholder="🔍 搜索值…" value="' + _escAttr(_fstate.search) + '"></div>'
          + '<div class="wt-pick-list">'
          +   '<div class="wt-pick-opt wt-pick-all" data-all="1"><span class="wt-pick-check">' + (allChecked ? '☑' : '☐') + '</span><span>(全选)</span></div>'
          +   rows
          + '</div>'
          + '<div class="wt-pick-actions eg-actionbar">'
          +   '<button class="eg-btn eg-btn--secondary eg-btn--sm" data-act="clearcol">清除该列</button>'
          +   '<button class="eg-btn eg-btn--primary eg-btn--sm" data-act="fconfirm">确认</button>'
          + '</div>';
        var si = document.getElementById('wt-pick-search-input');
        if (si) si.oninput = function() { _fstate.search = this.value; _renderFilter(); var n = document.getElementById('wt-pick-search-input'); if (n) { n.focus(); n.setSelectionRange(n.value.length, n.value.length); } };
        pop.onclick = function(ev) {
            if (ev.target.closest('.wt-pick-search')) return;
            var allEl = ev.target.closest('.wt-pick-all');
            if (allEl) {
                var qq = _fstate.search.toLowerCase();
                var vis = _fstate.options.filter(function(o) { return !qq || ('' + o.label).toLowerCase().indexOf(qq) >= 0; });
                var allOn = vis.every(function(o) { return _fstate.checked[o.value]; });
                vis.forEach(function(o) { _fstate.checked[o.value] = !allOn; });
                _renderFilter();
                return;
            }
            var optEl = ev.target.closest('.wt-pick-opt');
            if (optEl && optEl.dataset.val !== undefined) {
                var v = optEl.dataset.val;
                _fstate.checked[v] = !_fstate.checked[v];
                _renderFilter();
                return;
            }
            var btn = ev.target.closest('button[data-act]');
            if (btn) {
                if (btn.dataset.act === 'clearcol') { var oc = _fstate.onClear; close(); try { oc(); } catch (e) { console.error('[WorkPick] onClear', e); } }
                else if (btn.dataset.act === 'fconfirm') {
                    var checkedVals = _fstate.options.filter(function(o) { return _fstate.checked[o.value]; }).map(function(o) { return o.value; });
                    var cb = _fstate.onConfirm; close();
                    try { cb(checkedVals); } catch (e) { console.error('[WorkPick] onConfirm', e); }
                }
            }
        };
    }

    function close() {
        var bd = document.getElementById('wt-pick-bd');
        var pop = document.getElementById('wt-pick');
        if (bd) bd.classList.remove('open');
        if (pop) pop.classList.remove('open');
        _state = null;
        _fstate = null;
    }

    function _toggle(idx) {
        if (!_state) return;
        var key = _state.options[idx].key;
        if (_state.isMulti) {
            var i = _state.chosen.indexOf(key);
            if (i >= 0) _state.chosen.splice(i, 1);
            else _state.chosen.push(key);
        } else {
            _state.chosen = [key];
        }
        _render();
    }

    function _confirm() {
        if (!_state) return;
        var chosen = _state.chosen.slice();
        var cb = _state.onConfirm;
        close();
        try { cb(chosen); }
        catch (err) { console.error('[WorkPick] onConfirm error:', err); }
    }

    function _render() {
        if (!_state) return;
        var html = '<div class="wt-pick-list">';
        _state.options.forEach(function(o, oi) {
            var checked = _state.chosen.indexOf(o.key) >= 0;
            var mark = _state.isMulti
                ? (checked ? '☑' : '☐')
                : (checked ? '●' : '○');
            html += '<div class="wt-pick-opt' + (checked ? ' selected' : '') + '" '
                 +    'data-idx="' + oi + '">'
                 +    '<span class="wt-pick-check">' + mark + '</span>'
                 +    '<span>' + _escape(o.label) + '</span>'
                 +    '</div>';
        });
        html += '</div>';
        html += '<div class="wt-pick-actions eg-actionbar">'
             +    '<button class="eg-btn eg-btn--secondary eg-btn--sm" data-act="cancel">取消</button>'
             +    '<button class="eg-btn eg-btn--primary eg-btn--sm" data-act="confirm">确认</button>'
             +  '</div>';
        var pop = document.getElementById('wt-pick');
        if (!pop) return;
        pop.innerHTML = html;
        // 行内 onclick 容易和 picker 复用时撞;改用事件代理
        pop.onclick = function(ev) {
            var optEl = ev.target.closest('.wt-pick-opt');
            if (optEl) {
                _toggle(+optEl.dataset.idx);
                return;
            }
            var btn = ev.target.closest('button[data-act]');
            if (btn) {
                if (btn.dataset.act === 'cancel') close();
                else if (btn.dataset.act === 'confirm') _confirm();
            }
        };
    }

    function _position(anchor) {
        var pop = document.getElementById('wt-pick');
        if (!pop || !anchor) return;
        var rect = anchor.getBoundingClientRect();
        // 防止溢出右边界:把 left 夹在 [8, viewportWidth - 310] 之间
        var vw = window.innerWidth || document.documentElement.clientWidth;
        var maxLeft = vw - 310;
        var left = Math.max(8, Math.min(maxLeft, rect.left + window.scrollX));
        pop.style.left = left + 'px';
        pop.style.top  = (rect.bottom + window.scrollY + 6) + 'px';
    }

    function _escape(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;');
    }
    function _escAttr(s) {
        return _escape(s).replace(/"/g, '&quot;');
    }

    return {
        open: open,
        openFilter: openFilter,   // T-132
        close: close,
    };
})();
