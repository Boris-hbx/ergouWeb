// ========== InsightDialog — 洞察模块共享 dialog (T-106 P2) ==========
//
// 4 个 dialog 都用同一份 modal helper:
//   1. showRevise(insightId, onSubmit)    — 让 CC 改一版(revision_note + 附带 annotations 复选框)
//   2. showPublish(insightId, report, onConfirm)  — 发布(预览 + show_notes 复选框)
//   3. showRetract(onConfirm)             — 撤回(简单二次确认)
//   4. showForkConfirm(version, onOk)     — 已发布版本编辑前的"创建新版本"确认
//
// spec § 7.1 全局禁 window.prompt;统一用居中 modal,Esc 关闭,点遮罩关闭。
// 内嵌 modal helper(_open / _close)避免引入新依赖。

var InsightDialog = (function() {

    function _esc(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }

    // ── 通用 modal helper ──
    // opts: { title, body(HTML), okText, cancelText, danger, onOk(getValues), onCancel, afterRender(overlay) }
    // 返回 close 函数,允许外部主动关
    function _open(opts) {
        opts = opts || {};
        var ov = document.createElement('div');
        ov.className = 'ins-modal-overlay';
        ov.innerHTML = ''
          + '<div class="ins-modal">'
          +   '<div class="ins-modal-head">'
          +     '<h3>' + _esc(opts.title || '提示') + '</h3>'
          +     '<button class="ins-modal-x" aria-label="关闭">✕</button>'
          +   '</div>'
          +   '<div class="ins-modal-body">' + (opts.body || '') + '</div>'
          +   '<div class="ins-modal-foot">'
          +     '<button class="ins-btn ins-btn-ghost ins-modal-cancel">' + _esc(opts.cancelText || '取消') + '</button>'
          +     '<button class="ins-btn ' + (opts.danger ? 'ins-btn-danger' : 'ins-btn-primary') + ' ins-modal-ok">'
          +       _esc(opts.okText || '确定')
          +     '</button>'
          +   '</div>'
          + '</div>';
        document.body.appendChild(ov);

        var closed = false;
        function close() {
            if (closed) return;
            closed = true;
            document.removeEventListener('keydown', escHandler);
            ov.classList.add('ins-modal-closing');
            setTimeout(function() { ov.remove(); }, 180);
        }
        function escHandler(ev) {
            if (ev.key === 'Escape') { close(); if (opts.onCancel) opts.onCancel(); }
        }
        document.addEventListener('keydown', escHandler);

        ov.querySelector('.ins-modal-x').onclick = function() { close(); if (opts.onCancel) opts.onCancel(); };
        ov.querySelector('.ins-modal-cancel').onclick = function() { close(); if (opts.onCancel) opts.onCancel(); };
        ov.querySelector('.ins-modal-ok').onclick = async function() {
            if (opts.onOk) {
                // onOk 可以是 sync 或 async;返回 true / undefined → 关闭,返回 false → 不关闭(校验失败时)
                var keep = false;
                try {
                    var r = opts.onOk(ov);
                    if (r && typeof r.then === 'function') r = await r;
                    if (r === false) keep = true;
                } catch (e) {
                    console.error('[InsightDialog] onOk err', e);
                    keep = true;
                }
                if (!keep) close();
            } else {
                close();
            }
        };
        ov.onclick = function(ev) {
            if (ev.target === ov) { close(); if (opts.onCancel) opts.onCancel(); }
        };

        if (opts.afterRender) opts.afterRender(ov);
        return close;
    }

    // ============ 1. 让 CC 改一版 ============
    function showRevise(insightId, onSubmitted) {
        _open({
            title: '让 Claude Code 改一版',
            body: ''
              + '<div class="ins-dialog-hint">写一句话告诉 CC 要改什么。可以是"再加一个反例"、"第 3 节太长压缩一半"、"思辨章节再尖锐一点"等。</div>'
              + '<label class="ins-dialog-label">修订指示 <span class="ins-dialog-req">*</span></label>'
              + '<textarea id="ins-rev-note" class="ins-dialog-textarea" rows="5" placeholder="想让 CC 怎么改?(必填,会写入 pending_revision_note 让 CC 在修订模式读到)"></textarea>'
              + '<label class="ins-dialog-checkbox">'
              +   '<input type="checkbox" id="ins-rev-anns" checked> 附带当前版本的 open annotations 给 CC 作为输入'
              + '</label>'
              + '<div class="ins-dialog-foot-hint">确认后 status 切到 ready,在 Claude Code 里说: <code>处理 Insight #' + insightId + '</code></div>',
            okText: '✎ 让 CC 改',
            afterRender: function(ov) {
                var ta = ov.querySelector('#ins-rev-note');
                if (ta) ta.focus();
            },
            onOk: async function(ov) {
                var ta = ov.querySelector('#ins-rev-note');
                var note = (ta && ta.value || '').trim();
                if (!note) {
                    if (typeof showToast === 'function') showToast('修订指示不能为空', 'warning');
                    if (ta) ta.focus();
                    return false;
                }
                try {
                    var resp = await API.insightRegenerate(insightId, { revisionNote: note });
                    if (resp && resp.success) {
                        if (typeof showToast === 'function') showToast('已标记修订请求,在 Claude Code 里说: 处理 Insight #' + insightId, 'success');
                        if (onSubmitted) onSubmitted();
                    } else {
                        if (typeof showToast === 'function') showToast(resp && resp.error || '提交失败', 'error');
                        return false;
                    }
                } catch (e) {
                    console.error('[InsightDialog] regenerate err', e);
                    if (typeof showToast === 'function') showToast('提交失败', 'error');
                    return false;
                }
            },
        });
    }

    // ============ 2. 发布 ============
    function showPublish(insightId, report, onPublished) {
        var previewMd = (report && report.contentMd) || '';
        // 取前 300 字符作为预览
        var preview = previewMd.slice(0, 300);
        if (previewMd.length > 300) preview += '...';
        _open({
            title: '发布 v' + ((report && report.version) || '?') + '?',
            body: ''
              + '<div class="ins-dialog-hint">发布后会 mint 一个公开链接,任何人凭链接可只读访问当前版本。<strong>同一时刻最多 1 个 active 分享</strong>(会自动撤回上一个)。</div>'
              + '<div class="ins-dialog-preview-label">预览(前 300 字符)</div>'
              + '<pre class="ins-dialog-preview">' + _esc(preview || '(空)') + '</pre>'
              + '<label class="ins-dialog-checkbox">'
              +   '<input type="checkbox" id="ins-pub-show-notes"> 显示 source.note 私货备注(默认不显示)'
              + '</label>',
            okText: '📤 发布',
            onOk: async function(ov) {
                var sn = ov.querySelector('#ins-pub-show-notes');
                var showNotes = !!(sn && sn.checked);
                try {
                    var resp = await API.insightPublish(insightId, { showNotes: showNotes });
                    if (resp && resp.success) {
                        if (typeof showToast === 'function') showToast('已发布', 'success');
                        if (onPublished) onPublished(resp.item);
                    } else {
                        if (typeof showToast === 'function') showToast(resp && resp.error || '发布失败', 'error');
                        return false;
                    }
                } catch (e) {
                    console.error('[InsightDialog] publish err', e);
                    if (typeof showToast === 'function') showToast('发布失败:' + (e && e.message || ''), 'error');
                    return false;
                }
            },
        });
    }

    // ============ 3. 撤回 ============
    function showRetract(insightId, onRetracted) {
        _open({
            title: '撤回分享?',
            body: ''
              + '<div class="ins-dialog-hint">'
              +   '撤回后:<br>'
              +   '• 当前分享链接<strong>立即失效</strong>(返回 410 Gone)<br>'
              +   '• 同事手里的链接打不开了<br>'
              +   '• 状态回到 editing,可继续修改<br>'
              +   '• 想再发?必须先创建新版本(手改一字就能 fork);同版本不可重复发<br>'
              + '</div>',
            okText: '⤺ 撤回',
            danger: true,
            onOk: async function() {
                try {
                    var resp = await API.insightRetract(insightId);
                    if (resp && resp.success) {
                        if (typeof showToast === 'function') showToast('已撤回,旧链接失效', 'info');
                        if (onRetracted) onRetracted();
                    } else {
                        if (typeof showToast === 'function') showToast(resp && resp.error || '撤回失败', 'error');
                        return false;
                    }
                } catch (e) {
                    console.error('[InsightDialog] retract err', e);
                    if (typeof showToast === 'function') showToast('撤回失败', 'error');
                    return false;
                }
            },
        });
    }

    // ============ 4. 已发布版本编辑 → 创建新版本 fork 确认 ============
    function showForkConfirm(currentVersion, onOk) {
        _open({
            title: '这个版本已发布过,编辑会创建新版本',
            body: ''
              + '<div class="ins-dialog-hint">'
              +   '当前 v' + currentVersion + ' 是已发布过的版本(可能现在还在分享中),为了保护"已发出去的快照"不被改写,'
              +   '系统会自动 fork 一份新版本 v' + (currentVersion + 1) + ' 供你编辑。<br><br>'
              +   '原 v' + currentVersion + ' 保持凝固,不会被改;新版本是 boris-inline 类型,后续可再次发布。'
              + '</div>',
            okText: '✎ 创建 v' + (currentVersion + 1) + ' 并编辑',
            onOk: function() {
                if (onOk) onOk();
            },
        });
    }

    return {
        showRevise: showRevise,
        showPublish: showPublish,
        showRetract: showRetract,
        showForkConfirm: showForkConfirm,
    };
})();
