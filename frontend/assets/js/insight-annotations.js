// ========== InsightAnnotations — 报告段落备注 (T-106 P2 / spec § 5.3.3) ==========
//
// 简化版 gutter:annotation 卡片显示在所对应段落下方,而非右侧 540px gutter。
// (理由:工作 Hub 内已分两栏,右栏宽度有限;放下方语义清晰,实现简单。)
//
// 锚点策略(MVP):
//   - 只支持 paragraph 锚点(`{kind:'paragraph', index:N}`)
//   - 渲染时给 .ins-report-md > * 加 data-idx
//   - 重新生成版本后 index 对不上 → 标 orphaned,展示在"未锚定"区,annotation 仍可被 CC 读到
//
// 交互:
//   - hover 段落 → 段右侧浮出「+ 备注」按钮
//   - 点 + → 段下方展开编辑器(kind 单选 + body textarea + 保存/取消)
//   - 已有 annotation 显示为段下小卡:body + kind 标 + ✓ resolved / 🗑 删除
//
// 数据流:
//   InsightAnnotations.attach(insightId, reportId, container) ← detail.js 渲染完报告后调
//     → GET /api/insights/:id/annotations?report_id=...&status=open
//     → 给每段挂 + 按钮 + 已有卡片
//
// 写操作:POST / PATCH / DELETE 走 API 已有包装。

var InsightAnnotations = (function() {
    var _insightId = null;
    var _reportId = null;
    var _container = null;        // .ins-report-md 元素
    var _annotations = [];        // 当前 report 的 open annotations
    var _composing = null;        // 当前正在编辑的段索引(只允许一个)
    var _editingAnnotationId = null;  // 当前正编辑的 annotation(空白 = 新建)

    function _esc(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }

    function _kindLabel(k) {
        return ({
            question:   '❓ 提问',
            suggestion: '💡 建议',
            factcheck:  '🔍 事实核查',
            other:      '📝 其它',
        })[k] || k;
    }

    async function attach(insightId, reportId, container) {
        _insightId = insightId;
        _reportId  = reportId;
        _container = container;
        _composing = null;
        _editingAnnotationId = null;
        if (!_container || !_reportId || !_insightId) return;

        // 给每段加 data-idx + + 按钮
        _markupParagraphs();

        // 拉 open annotations
        try {
            var resp = await API.insightListAnnotations(_insightId, { reportId: _reportId, status: 'open' });
            _annotations = (resp && resp.items) || [];
            _renderAttached();
        } catch (e) {
            console.error('[InsightAnnotations] list err', e);
            _annotations = [];
        }
    }

    function detach() {
        _insightId = null;
        _reportId  = null;
        _container = null;
        _annotations = [];
        _composing = null;
        _editingAnnotationId = null;
    }

    function _markupParagraphs() {
        if (!_container) return;
        // 给顶层每个 block 元素加 data-idx + + 按钮
        var blocks = _container.children;
        for (var i = 0; i < blocks.length; i++) {
            var el = blocks[i];
            el.dataset.idx = i;
            el.classList.add('ins-annot-block');
            // + 按钮(hover 时显示)
            if (!el.querySelector('.ins-annot-add')) {
                var addBtn = document.createElement('button');
                addBtn.className = 'ins-annot-add';
                addBtn.title = '为这段加备注';
                addBtn.textContent = '+ 备注';
                addBtn.dataset.idx = i;
                addBtn.onclick = function(ev) {
                    ev.stopPropagation();
                    var idx = parseInt(this.dataset.idx, 10);
                    _startCompose(idx);
                };
                el.appendChild(addBtn);
            }
        }
    }

    function _renderAttached() {
        if (!_container) return;
        // 清掉所有现存 annotation 卡和编辑器(避免重叠)
        _container.querySelectorAll('.ins-annot-card-host').forEach(function(el) { el.remove(); });

        // 按 anchor.index 分组
        var byIdx = {};
        var orphaned = [];
        _annotations.forEach(function(a) {
            var anchor = _parseAnchor(a.anchor);
            if (anchor.kind === 'paragraph' && typeof anchor.index === 'number') {
                var target = _container.querySelector('[data-idx="' + anchor.index + '"]');
                if (target) {
                    if (!byIdx[anchor.index]) byIdx[anchor.index] = [];
                    byIdx[anchor.index].push(a);
                    return;
                }
            }
            orphaned.push(a);
        });

        // 在每段下方插卡
        Object.keys(byIdx).forEach(function(idx) {
            var target = _container.querySelector('[data-idx="' + idx + '"]');
            if (!target) return;
            var host = document.createElement('div');
            host.className = 'ins-annot-card-host';
            host.dataset.idx = idx;
            host.innerHTML = byIdx[idx].map(_cardHtml).join('');
            target.parentNode.insertBefore(host, target.nextSibling);
        });

        // composing 编辑器
        if (_composing != null) {
            var t = _container.querySelector('[data-idx="' + _composing + '"]');
            if (t) {
                var ed = document.createElement('div');
                ed.className = 'ins-annot-card-host ins-annot-card-host-composer';
                ed.innerHTML = _composerHtml();
                t.parentNode.insertBefore(ed, t.nextSibling);
                setTimeout(function() {
                    var ta = document.getElementById('ins-annot-body');
                    if (ta) ta.focus();
                }, 30);
            }
        }

        // orphaned 区
        if (orphaned.length) {
            var orphHost = document.createElement('div');
            orphHost.className = 'ins-annot-card-host ins-annot-orphan';
            orphHost.innerHTML = '<div class="ins-annot-orphan-label">📎 未锚定的备注 (' + orphaned.length + ') — 上一版段落结构已变,但 CC 仍能读到</div>'
                + orphaned.map(_cardHtml).join('');
            _container.appendChild(orphHost);
        }
    }

    function _cardHtml(a) {
        var kind = _kindLabel(a.kind);
        var anchor = _parseAnchor(a.anchor);
        var anchorHint = anchor.kind === 'paragraph' && typeof anchor.index === 'number'
            ? '第 ' + (anchor.index + 1) + ' 段'
            : (anchor.kind === 'heading' ? '标题: ' + (anchor.slug || '?') : '未锚定');
        return '<div class="ins-annot-card" data-id="' + a.id + '">'
          +   '<div class="ins-annot-card-head">'
          +     '<span class="ins-annot-kind">' + kind + '</span>'
          +     '<span class="ins-annot-anchor">' + _esc(anchorHint) + '</span>'
          +     '<div class="ins-annot-card-actions">'
          +       '<button class="ins-icon-btn" onclick="InsightAnnotations.resolve(' + a.id + ')" title="标为已处理">✓</button>'
          +       '<button class="ins-icon-btn ins-icon-btn-x" onclick="InsightAnnotations.remove(' + a.id + ')" title="删除">🗑</button>'
          +     '</div>'
          +   '</div>'
          +   '<div class="ins-annot-card-body">' + _esc(a.body) + '</div>'
          + '</div>';
    }

    function _composerHtml() {
        return '<div class="ins-annot-composer">'
          + '<div class="ins-annot-composer-head">📝 新建备注 · 锚定第 ' + (_composing + 1) + ' 段</div>'
          + '<div class="ins-annot-kind-row">'
          +   '<label><input type="radio" name="ins-annot-kind" value="question" checked> ❓ 提问</label>'
          +   '<label><input type="radio" name="ins-annot-kind" value="suggestion"> 💡 建议</label>'
          +   '<label><input type="radio" name="ins-annot-kind" value="factcheck"> 🔍 事实核查</label>'
          +   '<label><input type="radio" name="ins-annot-kind" value="other"> 📝 其它</label>'
          + '</div>'
          + '<textarea id="ins-annot-body" placeholder="写一句给自己 / CC 看的备注(必填)" '
          +   'onkeydown="if(event.key===\'Escape\')InsightAnnotations.cancelCompose();'
          +   'if((event.metaKey||event.ctrlKey)&&event.key===\'Enter\')InsightAnnotations.saveCompose()"></textarea>'
          + '<div class="ins-annot-composer-foot">'
          +   '<button class="ins-btn ins-btn-ghost ins-btn-sm" onclick="InsightAnnotations.cancelCompose()">取消 (Esc)</button>'
          +   '<button class="ins-btn ins-btn-primary ins-btn-sm" onclick="InsightAnnotations.saveCompose()">保存 (Cmd+Enter)</button>'
          + '</div>'
          + '</div>';
    }

    function _parseAnchor(anchorRaw) {
        if (typeof anchorRaw === 'string') {
            try { return JSON.parse(anchorRaw); } catch (_) { return {}; }
        }
        return anchorRaw || {};
    }

    // ============ 操作 ============
    function _startCompose(idx) {
        _composing = idx;
        _editingAnnotationId = null;
        _renderAttached();
    }

    function cancelCompose() {
        _composing = null;
        _editingAnnotationId = null;
        _renderAttached();
    }

    async function saveCompose() {
        var ta = document.getElementById('ins-annot-body');
        if (!ta) return;
        var body = (ta.value || '').trim();
        if (!body) {
            if (typeof showToast === 'function') showToast('备注内容不能为空', 'warning');
            ta.focus();
            return;
        }
        var kindEl = document.querySelector('input[name="ins-annot-kind"]:checked');
        var kind = (kindEl && kindEl.value) || 'other';
        try {
            // 后端 anchor 字段类型是 String,这里 stringify
            var resp = await API.insightCreateAnnotation(_insightId, {
                reportId: _reportId,
                anchor: JSON.stringify({ kind: 'paragraph', index: _composing }),
                body: body,
                kind: kind,
            });
            if (resp && resp.success) {
                _annotations.push(resp.item);
                _composing = null;
                _renderAttached();
            } else {
                if (typeof showToast === 'function') showToast(resp && resp.error || '创建失败', 'error');
            }
        } catch (e) {
            console.error('[InsightAnnotations] create err', e);
            if (typeof showToast === 'function') showToast('创建失败', 'error');
        }
    }

    async function resolve(annotationId) {
        try {
            await API.annotationUpdate(annotationId, { status: 'resolved' });
            // 本地状态:从 open 列表移除
            _annotations = _annotations.filter(function(a) { return a.id !== annotationId; });
            _renderAttached();
            if (typeof showToast === 'function') showToast('已标记已处理', 'info');
        } catch (e) {
            console.error('[InsightAnnotations] resolve err', e);
        }
    }

    async function remove(annotationId) {
        try {
            await API.annotationDelete(annotationId);
            _annotations = _annotations.filter(function(a) { return a.id !== annotationId; });
            _renderAttached();
        } catch (e) {
            console.error('[InsightAnnotations] delete err', e);
        }
    }

    return {
        attach: attach,
        detach: detach,
        cancelCompose: cancelCompose,
        saveCompose: saveCompose,
        resolve: resolve,
        remove: remove,
    };
})();
