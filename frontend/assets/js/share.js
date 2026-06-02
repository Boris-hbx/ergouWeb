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

    function _loadScript(src) {
        return new Promise(function(resolve, reject) {
            var s = document.createElement('script');
            s.src = src;
            s.onload = resolve;
            s.onerror = reject;
            document.head.appendChild(s);
        });
    }

    // T-133:本地自托管优先,jsdelivr 仅作兜底。CDN 在国内常加载失败 →
    // 没拿到 marked 就退化成 <pre> 裸 markdown,所以必须本地兜底保证渲染统一。
    function loadMarked() {
        if (typeof marked !== 'undefined') return Promise.resolve();
        return _loadScript('/assets/vendor/marked.min.js?v=20260601a')
            .catch(function() {
                console.warn('[share] 本地 marked 加载失败,回退 CDN');
                return _loadScript('https://cdn.jsdelivr.net/npm/marked@15/marked.min.js');
            });
    }

    // T-129:动态加载 insight-markdown.js,公开页复用 InsightMd 的 .ins-report 排版 + 思辨 callout
    function loadInsightMd() {
        return new Promise(function(resolve) {
            if (typeof InsightMd !== 'undefined') return resolve();
            var s = document.createElement('script');
            s.src = '/assets/js/insight-markdown.js?v=20260602a';
            s.onload = resolve;
            s.onerror = function() { console.warn('[share] insight-markdown.js load failed (fallback to marked)'); resolve(); };
            document.head.appendChild(s);
        });
    }

    function renderError(msg) {
        app.innerHTML = '<div class="share-error">' + esc(msg) + '</div>';
    }

    // v0.3 单层模型:数据 = { task:{title,inputType,createdAt}, report:{version,contentMd,template,modelUsed,createdAt}, includeNotes }
    function templateLabel(t) {
        return ({ survey: '综述型', decision: '决策型', watch: '追踪型' })[t] || '';
    }

    function render(d) {
        var task = d.task || {};
        var rep = d.report || {};
        var bodyHtml;
        if (typeof InsightMd !== 'undefined' && InsightMd.render) {
            bodyHtml = InsightMd.render(rep.contentMd || '');
        } else if (typeof marked !== 'undefined') {
            try { bodyHtml = sanitize(marked.parse(rep.contentMd || '', { breaks: true, gfm: true })); }
            catch (e) { console.error('[share] markdown render err', e); bodyHtml = '<pre>' + esc(rep.contentMd || '') + '</pre>'; }
        } else {
            bodyHtml = '<pre>' + esc(rep.contentMd || '') + '</pre>';
        }
        // 发布版页头:isPublic=true → 只标题 + 时间,不显徽章/版本(spec § 8.5 点 3)
        var cover = (typeof InsightMd !== 'undefined' && InsightMd.cover)
            ? InsightMd.cover({ title: task.title, createdAt: rep.createdAt }, true)
            : '<header class="ins-report-cover"><h1>' + esc(task.title || '(无标题)') + '</h1></header>';

        // T-134:删除「本报告由二狗(AI)生成」页脚;T-137:底部加回集链接(不放署名/联系方式)
        app.innerHTML = ''
            + '<article class="ins-report">'
            +   cover
            +   '<div class="ins-report-body">' + bodyHtml + '</div>'
            +   '<footer class="ins-report-foot"><a class="ins-back-collection" href="/r">📚 查看全部洞察 →</a></footer>'
            + '</article>';

        if (task.title) document.title = task.title + ' · 洞察';
        // T-129:渲染后把「思辨」节包成 callout
        var rb = app.querySelector('.ins-report-body');
        if (rb && typeof InsightMd !== 'undefined' && InsightMd.decorate) InsightMd.decorate(rb);
    }

    Promise.all([
        loadMarked().catch(function(e) { console.error('[share] marked load fail', e); }),
        loadInsightMd()
    ])
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
