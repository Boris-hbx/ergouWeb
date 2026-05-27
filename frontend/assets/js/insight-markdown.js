// ========== InsightMd — 洞察模块的 Markdown 渲染 (T-106) ==========
//
// 调研后:项目已通过 CDN 引入 marked@15(见 index.html),复用之即可。
// 没引入 DOMPurify,故自带一个轻量 sanitize(strip <script>/<iframe>/on* 属性/javascript: 链接)。
// 报告体量级在 2k-5k token,渲染开销忽略不计。
//
// citation popover (`[^N]`) — 留 P3 实现。当前 marked GFM footnote 默认渲染会自动加超链接到底部尾注。

var InsightMd = (function() {

    function render(md) {
        if (typeof marked === 'undefined') {
            return _fallbackRender(md || '');
        }
        try {
            var html = marked.parse(md || '', { breaks: true, gfm: true });
            return _sanitize(html);
        } catch (e) {
            console.error('[InsightMd] render error:', e);
            return _fallbackRender(md || '');
        }
    }

    function _sanitize(html) {
        // 删可执行/嵌入元素 + 内联事件 + javascript: 链接;白名单 sanitizer 在没 DOMPurify 时的最简兜底
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
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    }

    function _fallbackRender(md) {
        return '<pre class="ins-md-fallback">' + _esc(md) + '</pre>';
    }

    return {
        render: render,
    };
})();
