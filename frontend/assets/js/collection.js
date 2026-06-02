// ========== collection.js — 二狗洞察 公开报告集 /r (T-137) ==========
//
// 外壳(COLLECTION_SHELL_HTML)给静态页头/页脚;本脚本拉 GET /api/public/insight-reports
// 渲染卡片列表,点卡 → /r/{token}。无需登录,独立打包(不依赖业务前端)。

(function () {
    var listEl = document.getElementById('list');
    var countEl = document.getElementById('count');
    if (!listEl) return;

    var LABEL = { survey: '综述型', decision: '决策型', watch: '追踪型' };

    function esc(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }

    function fmtDate(iso) {
        if (!iso) return '';
        try {
            var d = new Date(iso);
            return d.getFullYear() + ' 年 ' + (d.getMonth() + 1) + ' 月 ' + d.getDate() + ' 日';
        } catch (_) { return ''; }
    }

    function card(r) {
        var tpl = LABEL[r.template] ? r.template : 'survey';
        var label = LABEL[tpl];
        return ''
            + '<a class="rcard" href="/r/' + encodeURIComponent(r.token) + '">'
            +   '<div class="rcard-top">'
            +     '<span class="badge ' + tpl + '">' + label + '</span>'
            +     '<span class="rcard-date">' + esc(fmtDate(r.publishedAt)) + '</span>'
            +   '</div>'
            +   '<h2>' + esc(r.title || '(无标题)') + '</h2>'
            +   (r.summary ? '<p class="rcard-sum">' + esc(r.summary) + '</p>' : '')
            +   '<div class="rcard-go">阅读全文 →</div>'
            + '</a>';
    }

    fetch('/api/public/insight-reports')
        .then(function (resp) { return resp.json(); })
        .then(function (data) {
            var items = (data && data.items) || [];
            if (countEl) countEl.textContent = items.length;
            if (!items.length) {
                listEl.innerHTML = '<div class="col-empty">还没有公开的洞察报告。</div>';
                return;
            }
            listEl.innerHTML = items.map(card).join('');
        })
        .catch(function (e) {
            console.error('[collection] fetch err', e);
            listEl.innerHTML = '<div class="col-empty">加载失败,请稍后再试。</div>';
        });
})();
