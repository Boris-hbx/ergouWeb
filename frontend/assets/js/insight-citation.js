// ========== InsightCitation — `[^N]` 引用 popover (T-106 P3 / spec § 5.6) ==========
//
// 4 处共用:editing / published / 历史版本 / 分享页 /r/:token。
//
// 工作流:
//   marked GFM 把 `[^N]` 渲染成 <sup class="footnote-ref"><a href="#fn:N">N</a></sup>;
//   attach(container, citations, sources) 扫这些元素挂 popover 触发:
//     - hover 200ms 延迟出现(避免误触);离开 hover 消失
//     - 点击 = 钉住;再点 = 关闭;点 popover 外 = 关闭;Esc = 关闭
//   popover 内容:[N] · source kind/author · "quote 30-100 字" · [查看原文 ↗] [跳到 Sources]
//   接近右/下边界自动翻向另一侧。
//
// 旧 report(无 citations)或 CC 没输出 citations:popover 降级显示 source 信息(无 quote)。
// 同时找不到 source(理论不该发生)→ 显示"来源数据缺失"占位。

// 自带样式注入(让分享页 share.js 动态加载本模块时也有样式;主端 insight.css 不必重复)
(function _injectCiteStyles() {
    if (typeof document === 'undefined') return;
    if (document.getElementById('ins-cite-styles')) return;
    var s = document.createElement('style');
    s.id = 'ins-cite-styles';
    s.textContent = [
        '.ins-cite-ref { cursor: pointer; padding: 0 1px; border-radius: 3px; transition: background .15s; }',
        '.ins-cite-ref:hover { background: rgba(124, 77, 255, 0.15); }',
        '.ins-cite-popover {',
        '  position: fixed; z-index: 9999;',
        '  background: #fff;',
        '  border: 1px solid #7C4DFF;',
        '  border-radius: 6px;',
        '  padding: 10px 12px;',
        '  width: 380px; max-width: 92vw;',
        '  box-shadow: 0 8px 28px rgba(15, 15, 30, 0.18);',
        '  font: 13px/1.55 -apple-system, BlinkMacSystemFont, "PingFang SC", "Microsoft YaHei", sans-serif;',
        '  color: #1F2937;',
        '  animation: ins-cite-fadein .15s ease-out;',
        '}',
        '@keyframes ins-cite-fadein { from { opacity: 0; transform: translateY(-3px); } to { opacity: 1; transform: translateY(0); } }',
        '.ins-cite-popover.ins-cite-pinned { box-shadow: 0 8px 28px rgba(124, 77, 255, 0.32); }',
        '.ins-cite-head {',
        '  display: flex; align-items: center; gap: 8px;',
        '  margin-bottom: 8px; padding-bottom: 6px;',
        '  border-bottom: 1px dashed #E5E3EE;',
        '  font-size: 12px;',
        '}',
        '.ins-cite-ref { background: #7C4DFF; color: #fff; padding: 1px 7px; border-radius: 10px; font-weight: 600; font-size: 11px; }',
        '.ins-cite-popover .ins-cite-ref { cursor: default; }',  // popover 里的 [N] 不可点
        '.ins-cite-src { flex: 1; color: #4B5563; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }',
        '.ins-cite-author { color: #9CA3AF; }',
        '.ins-cite-close { background: transparent; border: 0; color: #6B7280; cursor: pointer; font-size: 13px; padding: 0 4px; border-radius: 3px; }',
        '.ins-cite-close:hover { background: #F5F2FB; color: #1F2937; }',
        '.ins-cite-quote {',
        '  background: rgba(124, 77, 255, 0.06);',
        '  border-left: 3px solid #7C4DFF;',
        '  padding: 7px 11px;',
        '  border-radius: 0 4px 4px 0;',
        '  font-size: 12.5px;',
        '  line-height: 1.6;',
        '  color: #1F2937;',
        '  max-height: 200px; overflow-y: auto;',
        '  white-space: pre-wrap;',
        '}',
        '.ins-cite-noquote {',
        '  background: #F5F2FB;',
        '  padding: 7px 11px; border-radius: 4px;',
        '  font-size: 11.5px; color: #6B7280; font-style: italic;',
        '}',
        '.ins-cite-foot {',
        '  display: flex; gap: 6px; margin-top: 8px; flex-wrap: wrap;',
        '}',
        '.ins-cite-action {',
        '  padding: 4px 9px;',
        '  background: #fff; border: 1px solid #E5E3EE;',
        '  border-radius: 5px; cursor: pointer;',
        '  font-size: 11.5px; color: #4B5563;',
        '  text-decoration: none;',
        '  transition: background .15s, border-color .15s;',
        '}',
        '.ins-cite-action:hover { background: #F5F2FB; border-color: #7C4DFF; color: #7C4DFF; }',
        '.ins-cite-action-btn { font-family: inherit; }',
        '.ins-cite-action-disabled { color: #9CA3AF; cursor: not-allowed; }',
        '.ins-cite-action-disabled:hover { background: #fff; border-color: #E5E3EE; color: #9CA3AF; }',
        '.ins-cite-err {',
        '  font-size: 12px; color: #E11D48;',
        '  background: rgba(225, 29, 72, 0.08);',
        '  padding: 4px 8px; border-radius: 4px;',
        '}',
        // jump 高亮(在 source 列表行上闪一下)
        '.ins-cite-jumped {',
        '  animation: ins-cite-flash 1.4s ease-out;',
        '}',
        '@keyframes ins-cite-flash {',
        '  0%, 100% { background: transparent; }',
        '  20% { background: rgba(124, 77, 255, 0.22); box-shadow: 0 0 0 3px rgba(124, 77, 255, 0.18); }',
        '  60% { background: rgba(124, 77, 255, 0.08); box-shadow: 0 0 0 3px rgba(124, 77, 255, 0.05); }',
        '}',
    ].join('\n');
    document.head.appendChild(s);
})();

var InsightCitation = (function() {
    var _pop = null;            // popover DOM(单例)
    var _pinned = false;        // 钉住状态
    var _hoverTimer = null;     // hover 延迟 timer
    var _leaveTimer = null;     // 离开缓冲 timer
    var _hoverTarget = null;    // 当前指向的 ref anchor
    var _citations = [];
    var _sources = [];

    function _esc(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }

    function _kindIcon(k) {
        switch (k) {
            case 'youtube': return '🎬';
            case 'x':       return '🐦';
            case 'github':  return '🐙';
            case 'pdf':     return '📄';
            case 'text':    return '📝';
            default:        return '🌐';
        }
    }

    function _shortUrl(u) {
        try {
            var url = new URL(u);
            return url.hostname;
        } catch (_) { return u || ''; }
    }

    function attach(container, citations, sources) {
        _citations = citations || [];
        _sources   = sources || [];
        if (!container) return;
        // marked GFM 默认 footnote ref:<sup class="footnote-ref"><a href="#fn:N">N</a></sup>
        // 兼容若干变体:有些版本是 <sup id="fnref-1"><a href="#fn-1">1</a></sup>
        var refs = container.querySelectorAll('sup.footnote-ref a, sup.footnote a, a.footnote-ref, sup > a[href^="#fn"]');
        refs.forEach(function(a) {
            var ref = _extractRef(a);
            if (!ref) return;
            a.classList.add('ins-cite-ref');
            a.dataset.ref = ref;
            // 防重复绑定
            if (a.dataset.citeBound === '1') return;
            a.dataset.citeBound = '1';
            a.addEventListener('mouseenter', _onHover);
            a.addEventListener('mouseleave', _onLeave);
            a.addEventListener('click', _onClick);
        });
    }

    function _extractRef(a) {
        var href = a.getAttribute && a.getAttribute('href');
        if (href) {
            var m = href.match(/#fn[-:_]?(\d+)/i);
            if (m) return m[1];
        }
        var txt = (a.textContent || '').trim();
        // 去除可能的方括号 [1] / "^1"
        var m2 = txt.match(/(\d+)/);
        return m2 ? m2[1] : null;
    }

    function _onHover(ev) {
        var a = ev.currentTarget;
        if (_pinned) return;
        _hoverTarget = a;
        clearTimeout(_leaveTimer);
        clearTimeout(_hoverTimer);
        _hoverTimer = setTimeout(function() { _show(a, false); }, 200);
    }

    function _onLeave(ev) {
        clearTimeout(_hoverTimer);
        if (_pinned) return;
        clearTimeout(_leaveTimer);
        // 缓冲 200ms 给鼠标进入 popover 留余地
        _leaveTimer = setTimeout(function() {
            if (!_pinned) _close();
        }, 200);
    }

    function _onClick(ev) {
        var a = ev.currentTarget;
        ev.preventDefault();   // 不滚到底部 footnotes
        ev.stopPropagation();
        if (_pinned && _hoverTarget === a) {
            // 再次点击锁定的 ref → 关
            _close();
            return;
        }
        _pinned = true;
        _hoverTarget = a;
        clearTimeout(_hoverTimer);
        clearTimeout(_leaveTimer);
        _show(a, true);
    }

    function _show(anchor, pinned) {
        var ref = anchor.dataset.ref;
        var citation = _citations.find(function(c) {
            return String(c.ref) === String(ref);
        });
        var source = citation
            ? _sources.find(function(s) { return s.id === citation.sourceId; })
            : null;
        if (!_pop) {
            _pop = document.createElement('div');
            _pop.className = 'ins-cite-popover';
            document.body.appendChild(_pop);
            // popover 自身 hover 时延长生命周期(允许鼠标进来点链接)
            _pop.addEventListener('mouseenter', function() { clearTimeout(_leaveTimer); });
            _pop.addEventListener('mouseleave', function() {
                if (_pinned) return;
                _leaveTimer = setTimeout(function() { if (!_pinned) _close(); }, 180);
            });
        }
        _pop.innerHTML = _popHtml(ref, citation, source, pinned);
        _pop.classList.toggle('ins-cite-pinned', !!pinned);
        _position(anchor);
        _pop.style.display = '';
        if (pinned) {
            setTimeout(function() {
                document.addEventListener('click', _outsideClick, true);
            }, 0);
            document.addEventListener('keydown', _escClose);
        }
    }

    function _popHtml(ref, citation, source, pinned) {
        if (!citation && !source) {
            return '<div class="ins-cite-err">[' + _esc(ref) + '] 来源数据缺失</div>';
        }
        var srcLabel = source
            ? _kindIcon(source.kind) + ' '
              + _esc(source.title || _shortUrl(source.url || '') || '(无标题)')
              + (source.author ? ' · <span class="ins-cite-author">' + _esc(source.author) + '</span>' : '')
            : '来源 #' + _esc(citation && citation.sourceId);
        var quoteHtml = (citation && citation.quote)
            ? '<div class="ins-cite-quote">"' + _esc(citation.quote) + '"</div>'
            : '<div class="ins-cite-noquote">(本版本无原文片段;CC 在生成时未输出 citations 数据)</div>';
        var hasUrl = source && source.url;
        var openBtn = hasUrl
            ? '<a class="ins-cite-action" href="' + _esc(source.url) + '" target="_blank" rel="noopener">查看完整原文 ↗</a>'
            : '<span class="ins-cite-action ins-cite-action-disabled">无 URL</span>';
        var jumpBtn = source
            ? '<button class="ins-cite-action ins-cite-action-btn" onclick="InsightCitation.jumpToSource(' + source.id + ')">跳到 Sources</button>'
            : '';
        var closeBtn = pinned
            ? '<button class="ins-cite-close" onclick="InsightCitation.close()" title="关闭(Esc)">✕</button>'
            : '';
        return '<div class="ins-cite-head">'
          +   '<span class="ins-cite-ref">[' + _esc(ref) + ']</span>'
          +   '<span class="ins-cite-src">' + srcLabel + '</span>'
          +   closeBtn
          + '</div>'
          + quoteHtml
          + '<div class="ins-cite-foot">' + openBtn + jumpBtn + '</div>';
    }

    function _position(anchor) {
        var rect = anchor.getBoundingClientRect();
        _pop.style.position = 'fixed';
        _pop.style.visibility = 'hidden';
        _pop.style.left = '0px'; _pop.style.top = '0px';
        // 先放出来量尺寸
        var pw = _pop.offsetWidth;
        var ph = _pop.offsetHeight;
        var vw = window.innerWidth;
        var vh = window.innerHeight;
        var margin = 8;
        // 默认 anchor 下方对齐左边
        var left = rect.left;
        var top  = rect.bottom + 4;
        // 右溢出 → 右对齐(翻向左侧)
        if (left + pw > vw - margin) {
            left = Math.max(margin, rect.right - pw);
        }
        if (left < margin) left = margin;
        // 下溢出且上方有空间 → 翻上方
        if (top + ph > vh - margin && rect.top - ph - 4 >= margin) {
            top = rect.top - ph - 4;
        }
        _pop.style.left = left + 'px';
        _pop.style.top  = top + 'px';
        _pop.style.visibility = '';
    }

    function _close() {
        if (_pop) _pop.style.display = 'none';
        _pinned = false;
        _hoverTarget = null;
        clearTimeout(_hoverTimer);
        clearTimeout(_leaveTimer);
        document.removeEventListener('click', _outsideClick, true);
        document.removeEventListener('keydown', _escClose);
    }

    function _outsideClick(ev) {
        if (!_pop) return;
        if (_pop.contains(ev.target)) return;
        if (_hoverTarget && _hoverTarget.contains && _hoverTarget.contains(ev.target)) return;
        _close();
    }

    function _escClose(ev) {
        if (ev.key === 'Escape') _close();
    }

    function jumpToSource(sourceId) {
        // 找 detail.js 的 source 行 / share 页的 source 行
        var sel = '[data-src-id="' + sourceId + '"], .ins-report-sources li[data-src-id="' + sourceId + '"], .share-sources li[data-src-id="' + sourceId + '"]';
        var el = document.querySelector(sel);
        if (el) {
            el.scrollIntoView({ behavior: 'smooth', block: 'center' });
            el.classList.add('ins-cite-jumped');
            setTimeout(function() { el.classList.remove('ins-cite-jumped'); }, 1400);
        } else {
            if (typeof showToast === 'function') showToast('未找到对应来源', 'warning');
        }
        _close();
    }

    return {
        attach: attach,
        close: _close,
        jumpToSource: jumpToSource,
    };
})();
