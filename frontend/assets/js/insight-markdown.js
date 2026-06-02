// ========== InsightMd — 洞察模块的 Markdown 渲染 + 报告排版 (T-106 / T-129 / T-135) ==========
//
// 调研后:项目已通过 CDN 引入 marked@15(见 index.html),复用之即可。
// 没引入 DOMPurify,故自带一个轻量 sanitize(strip <script>/<iframe>/on* 属性/javascript: 链接)。
//
// T-129 报告视觉呈现:render() 出正文 HTML;cover() 出页头(isPublic 控制徽章/版本显隐);
//   decorate() 在 DOM 插入后把「思辨」节包成 callout。详情页(insight.js)与公开分享页(share.js)共用。
// T-135 重点强调:render 预处理 ==文字==→<mark class="ins-key">(标红加粗);
//   decorate 识别以 🎯 开头的 blockquote → .ins-takeaway 醒目块(与思辨紫块区分)。

var InsightMd = (function() {

    function render(md) {
        if (typeof marked === 'undefined') {
            return _fallbackRender(md || '');
        }
        try {
            var pre = _emphasisMarks(md || '');
            var html = marked.parse(pre, { breaks: true, gfm: true });
            return _sanitize(html);
        } catch (e) {
            console.error('[InsightMd] render error:', e);
            return _fallbackRender(md || '');
        }
    }

    // T-135:行内重点 ==文字== → <mark class="ins-key">(marked 不原生支持)。
    // 先把 fenced ``` 与 inline `code` 抠出占位,避免误伤代码里的 ==;转换后再还原。
    function _emphasisMarks(md) {
        var store = [];
        function stash(m) { store.push(m); return 'MARK' + (store.length - 1) + ''; }
        var s = ('' + md)
            .replace(/```[\s\S]*?```/g, stash)
            .replace(/`[^`\n]*`/g, stash);
        s = s.replace(/==([^=\n]+?)==/g, '<mark class="ins-key">$1</mark>');
        return s.replace(/MARK(\d+)/g, function(_, i) { return store[+i]; });
    }

    function _sanitize(html) {
        return ('' + html)
            .replace(/<script[\s\S]*?<\/script>/gi, '')
            .replace(/<iframe[\s\S]*?<\/iframe>/gi, '')
            .replace(/<style[\s\S]*?<\/style>/gi, '')
            .replace(/<embed[^>]*>/gi, '')
            .replace(/<object[\s\S]*?<\/object>/gi, '')
            .replace(/\son\w+\s*=\s*"[^"]*"/gi, '')
            .replace(/\son\w+\s*=\s*'[^']*'/gi, '')
            .replace(/\son\w+\s*=\s*[^\s>]+/gi, '')
            .replace(/href\s*=\s*"javascript:[^"]*"/gi, 'href="#"')
            .replace(/href\s*=\s*'javascript:[^']*'/gi, "href='#'");
    }

    function _esc(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }

    function _fallbackRender(md) {
        return '<pre class="ins-md-fallback">' + _esc(md) + '</pre>';
    }

    // ---- T-129 报告排版辅助 ----

    function _templateLabel(t) {
        return ({ survey: '综述型', decision: '决策型', watch: '追踪型' })[t] || '';
    }
    function _formatDate(iso) {
        if (!iso) return '';
        try {
            var d = new Date(iso);
            return d.getFullYear() + '-' + String(d.getMonth() + 1).padStart(2, '0') + '-' + String(d.getDate()).padStart(2, '0');
        } catch (_) { return iso; }
    }
    function _badge(t) {
        var lbl = _templateLabel(t);
        if (!lbl) return '';
        return '<span class="ins-badge ' + (t || 'survey') + '">' + lbl + '</span>';
    }

    // 报告页头。meta = { title, template, version, createdAt, modelUsed }
    // isPublic=true(分享页):标题 + 生成时间,**不显示徽章/版本/模型**(对外读者不关心,内部管理信息)。
    // isPublic=false(详情页):紧凑 meta 条(徽章 + 版本 + 时间 + 模型);标题不重复(详情页信息块已有)。
    function cover(meta, isPublic) {
        meta = meta || {};
        var time = _formatDate(meta.createdAt);
        if (isPublic) {
            return '<header class="ins-report-cover">'
                + '<h1>' + _esc(meta.title || '(无标题)') + '</h1>'
                + (time ? '<div class="ins-cover-meta"><time>' + time + '</time></div>' : '')
                + '</header>';
        }
        var bits = [];
        var b = _badge(meta.template); if (b) bits.push(b);
        if (meta.version) bits.push('<span class="ins-cover-ver">v' + meta.version + '</span>');
        if (time) bits.push('<time>' + time + '</time>');
        if (meta.modelUsed) bits.push('<span class="ins-cover-model">' + _esc(meta.modelUsed) + '</span>');
        if (!bits.length) return '';
        return '<div class="ins-report-metabar">' + bits.join('<span class="ins-cover-dot">·</span>') + '</div>';
    }

    // DOM 后处理:① 把「思辨」节(含"思辨"的 h2 到下一个 h2 之前)包成 .ins-sibian callout;
    //            ② T-135:把以 🎯 开头的 blockquote 标成 .ins-takeaway 核心观点块。
    // root = 已插入 DOM 的报告正文容器(.ins-report-body)。
    function decorate(root) {
        if (!root || !root.querySelectorAll) return;
        // ① 思辨
        var hs = root.querySelectorAll('h2');
        for (var i = 0; i < hs.length; i++) {
            var h = hs[i];
            if (h.textContent && h.textContent.indexOf('思辨') >= 0) {
                var sec = document.createElement('section');
                sec.className = 'ins-sibian';
                var head = document.createElement('div');
                head.className = 'ins-sibian-head';
                head.innerHTML = '<span class="ico">🔍</span> 思辨 <span class="tag">对抗性视角</span>';
                var moved = [];
                var n = h.nextSibling;
                while (n && !(n.nodeType === 1 && n.tagName === 'H2')) {
                    moved.push(n);
                    n = n.nextSibling;
                }
                h.parentNode.insertBefore(sec, h);
                h.parentNode.removeChild(h);
                sec.appendChild(head);
                moved.forEach(function(x) { sec.appendChild(x); });
                break;   // 只处理第一处「思辨」
            }
        }
        // ② T-135 核心观点:🎯 开头的 blockquote
        var bqs = root.querySelectorAll('blockquote');
        for (var j = 0; j < bqs.length; j++) {
            var txt = (bqs[j].textContent || '').replace(/^\s+/, '');
            if (txt.indexOf('🎯') === 0) bqs[j].classList.add('ins-takeaway');
        }
    }

    return {
        render: render,
        cover: cover,
        decorate: decorate,
    };
})();
