// ========== share.js — 洞察公开分享页 /r/:token (T-106 P1) ==========
//
// 加载流程:
//   后端 SHARE_SHELL_HTML 返回最小 HTML 外壳(div#share-app[data-token]),
//   本脚本 fetch /r/:token/data 拿 JSON,渲染 markdown + sources。
//   无需登录、不依赖业务前端;独立打包(只引 marked CDN)。
//
// 撤销:GET /r/:token 直接返回 410 HTML(SHARE_410_HTML),本脚本不会运行。
//      但 race 情况下数据接口仍可能 410 → 显示降级提示。

(function() {
    var app = document.getElementById('share-app');
    if (!app) return;
    var token = app.dataset.token;
    if (!token) { app.innerHTML = '<div class="share-error">无效的分享链接</div>'; return; }

    function esc(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }

    function kindIcon(k) {
        switch (k) {
            case 'youtube': return '🎬';
            case 'x':       return '🐦';
            case 'github':  return '🐙';
            case 'pdf':     return '📄';
            case 'text':    return '📝';
            default:        return '🌐';
        }
    }

    function formatDate(iso) {
        if (!iso) return '';
        try {
            var d = new Date(iso);
            return d.getFullYear() + '-' + String(d.getMonth() + 1).padStart(2, '0') + '-' + String(d.getDate()).padStart(2, '0');
        } catch (_) { return iso; }
    }

    function sanitize(html) {
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

    function loadMarked() {
        return new Promise(function(resolve, reject) {
            if (typeof marked !== 'undefined') return resolve();
            var s = document.createElement('script');
            s.src = 'https://cdn.jsdelivr.net/npm/marked@15/marked.min.js';
            s.onload = resolve;
            s.onerror = reject;
            document.head.appendChild(s);
        });
    }

    // P3:动态加载 insight-citation.js 让公开分享页也有 popover
    function loadCitationModule() {
        return new Promise(function(resolve, reject) {
            if (typeof InsightCitation !== 'undefined') return resolve();
            var s = document.createElement('script');
            s.src = '/assets/js/insight-citation.js';
            s.onload = resolve;
            s.onerror = function() {
                console.warn('[share] insight-citation.js load failed (popover disabled)');
                resolve();
            };
            document.head.appendChild(s);
        });
    }

    function renderError(msg) {
        app.innerHTML = '<div class="share-error">' + esc(msg) + '</div>';
    }

    function render(d) {
        var ins = d.insight || {};
        var rep = d.report || {};
        var srcs = d.sources || [];
        var mdHtml = '<pre>' + esc(rep.contentMd || '') + '</pre>';
        if (typeof marked !== 'undefined') {
            try {
                mdHtml = sanitize(marked.parse(rep.contentMd || '', { breaks: true, gfm: true }));
            } catch (e) {
                console.error('[share] markdown render err', e);
            }
        }

        var srcHtml = '';
        if (srcs.length) {
            srcHtml = '<section class="share-sources">'
                + '<h2>引用来源 (' + srcs.length + ')</h2>'
                + '<ol>'
                + srcs.map(function(s) {
                    var link = s.url
                        ? '<a href="' + esc(s.url) + '" target="_blank" rel="noopener">' + esc(s.title || s.url) + '</a>'
                        : esc(s.title || '(无标题)');
                    return '<li data-src-id="' + s.id + '">'
                        + kindIcon(s.kind) + ' ' + link
                        + (s.author ? ' · <span class="share-author">' + esc(s.author) + '</span>' : '')
                        + (s.note ? '<div class="share-src-note">' + esc(s.note) + '</div>' : '')
                        + '</li>';
                }).join('')
                + '</ol>'
                + '</section>';
        }

        app.innerHTML = ''
            + '<article class="share-article">'
            +   '<header class="share-header">'
            +     '<h1>' + esc(ins.title || '(无标题)') + '</h1>'
            +     '<div class="share-meta">'
            +       '<time>' + formatDate(rep.createdAt) + '</time> · '
            +       '<span class="share-version">v' + (rep.version || '?') + '</span>'
            +     '</div>'
            +     (ins.topic ? '<div class="share-topic">' + esc(ins.topic) + '</div>' : '')
            +   '</header>'
            +   '<div class="share-md">' + mdHtml + '</div>'
            +   srcHtml
            +   '<footer class="share-footer">由二狗洞察生成 · 撤销后链接失效</footer>'
            + '</article>';

        // 设置文档标题(便于浏览器 tab 显示 / 收藏)
        if (ins.title) document.title = ins.title + ' · 洞察';

        // P3:挂 citation popover(hover [^N] 看原文片段)
        loadCitationModule().then(function() {
            if (typeof InsightCitation !== 'undefined') {
                var mdEl = document.querySelector('.share-md');
                if (mdEl) InsightCitation.attach(mdEl, rep.citations || [], srcs);
            }
        });
    }

    loadMarked()
        .catch(function(e) { console.error('[share] marked load fail', e); })
        .then(function() { return fetch('/r/' + token + '/data'); })
        .then(function(resp) {
            if (resp.status === 410) {
                app.innerHTML = '<div class="share-revoked">'
                    + '<div class="share-revoked-icon">🔗</div>'
                    + '<h1>这个分享链接已被撤销</h1>'
                    + '<p>洞察作者撤回了此链接。如果你确实需要内容,请直接找作者。</p>'
                    + '</div>';
                return null;
            }
            if (resp.status === 404) {
                renderError('链接不存在(可能已过期或拼写错误)');
                return null;
            }
            return resp.json();
        })
        .then(function(data) {
            if (!data) return;
            if (!data.success) {
                renderError(data.error || '加载失败');
                return;
            }
            render(data);
        })
        .catch(function(e) {
            console.error('[share] fetch err', e);
            renderError('网络错误,请稍后再试');
        });
})();
