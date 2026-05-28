// ========== PatUI — 个人访问令牌管理 (T-116 / spec auth § 12.6) ==========
//
// 设置页「个人访问令牌」区:生成新 token / 列表 / 撤销 / 明文显示 modal(once-only)。
// 调用 API.pat* 包装(api.js 加)。

var PatUI = (function() {
    var _items = [];
    var _shownToken = null;   // 用于"复制到剪贴板"按钮

    function _esc(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }
    function _shortTime(iso) {
        if (!iso) return '—';
        try {
            var d = new Date(iso);
            return d.getFullYear() + '-' + String(d.getMonth() + 1).padStart(2, '0') + '-' + String(d.getDate()).padStart(2, '0')
                + ' ' + String(d.getHours()).padStart(2, '0') + ':' + String(d.getMinutes()).padStart(2, '0');
        } catch (_) { return iso; }
    }

    async function refresh() {
        try {
            var resp = await API.patList();
            _items = (resp && resp.items) || [];
            _render();
        } catch (e) {
            console.error('[PatUI] refresh err', e);
        }
    }

    function _render() {
        var host = document.getElementById('pat-list');
        if (!host) return;
        if (_items.length === 0) {
            host.innerHTML = '<div style="font-size:12.5px;color:#9CA3AF;padding:8px 0">还没有令牌。生成一个用于 Claude Code 或 CLI。</div>';
            return;
        }
        var rows = _items.map(function(t) {
            var revoked = !!t.revokedAt;
            var expired = t.expiresAt && new Date(t.expiresAt) < new Date();
            var statusBadge = revoked
                ? '<span style="color:#E11D48;font-size:11px;background:rgba(225,29,72,.10);padding:1px 6px;border-radius:8px">已撤销</span>'
                : expired
                    ? '<span style="color:#8B5A00;font-size:11px;background:rgba(245,158,11,.12);padding:1px 6px;border-radius:8px">已过期</span>'
                    : '<span style="color:#047857;font-size:11px;background:rgba(16,185,129,.12);padding:1px 6px;border-radius:8px">有效</span>';
            var actionBtn = revoked
                ? ''
                : '<button class="btn" style="padding:3px 10px;font-size:11.5px" onclick="PatUI.confirmRevoke(' + t.id + ', \'' + _esc(t.label) + '\')">撤销</button>';
            return '<tr style="' + (revoked ? 'opacity:0.55' : '') + '">'
                + '<td style="padding:6px 8px;font-size:12.5px"><strong>' + _esc(t.label) + '</strong></td>'
                + '<td style="padding:6px 8px;font-family:ui-monospace,Consolas,monospace;font-size:11.5px;color:#6B7280">' + _esc(t.tokenPrefix) + '...</td>'
                + '<td style="padding:6px 8px;font-size:11.5px;color:#6B7280">' + _shortTime(t.createdAt) + '</td>'
                + '<td style="padding:6px 8px;font-size:11.5px;color:#6B7280">' + _shortTime(t.lastUsedAt) + '</td>'
                + '<td style="padding:6px 8px;font-size:11.5px;color:#6B7280">' + (t.expiresAt ? _shortTime(t.expiresAt) : '永久') + '</td>'
                + '<td style="padding:6px 8px">' + statusBadge + '</td>'
                + '<td style="padding:6px 8px;text-align:right">' + actionBtn + '</td>'
                + '</tr>';
        }).join('');

        host.innerHTML = ''
            + '<table style="width:100%;border-collapse:collapse;background:#fff;border:1px solid #E5E3EE;border-radius:8px;overflow:hidden">'
            + '<thead><tr style="background:#F5F2FB;text-align:left">'
            + '<th style="padding:8px;font-size:11px;font-weight:600;color:#6B7280;text-transform:uppercase;letter-spacing:.04em">名称</th>'
            + '<th style="padding:8px;font-size:11px;font-weight:600;color:#6B7280;text-transform:uppercase;letter-spacing:.04em">前缀</th>'
            + '<th style="padding:8px;font-size:11px;font-weight:600;color:#6B7280;text-transform:uppercase;letter-spacing:.04em">创建</th>'
            + '<th style="padding:8px;font-size:11px;font-weight:600;color:#6B7280;text-transform:uppercase;letter-spacing:.04em">最后使用</th>'
            + '<th style="padding:8px;font-size:11px;font-weight:600;color:#6B7280;text-transform:uppercase;letter-spacing:.04em">过期</th>'
            + '<th style="padding:8px;font-size:11px;font-weight:600;color:#6B7280;text-transform:uppercase;letter-spacing:.04em">状态</th>'
            + '<th></th></tr></thead>'
            + '<tbody>' + rows + '</tbody></table>';
    }

    async function create() {
        var labelEl = document.getElementById('pat-new-label');
        var expEl = document.getElementById('pat-new-expires');
        if (!labelEl) return;
        var label = (labelEl.value || '').trim();
        if (!label) {
            if (typeof showToast === 'function') showToast('请输入令牌名称', 'warning');
            labelEl.focus();
            return;
        }
        var expIso = null;
        if (expEl && expEl.value) {
            // date input YYYY-MM-DD → 转 RFC3339(当地 23:59:59,接近"全天有效")
            try {
                var d = new Date(expEl.value + 'T23:59:59');
                expIso = d.toISOString();
            } catch (_) {}
        }
        try {
            var resp = await API.patCreate({ label: label, expiresAt: expIso });
            if (resp && resp.success && resp.item && resp.item.token) {
                _shownToken = resp.item.token;
                _openShowModal(resp.item.token);
                labelEl.value = '';
                if (expEl) expEl.value = '';
                refresh();
            } else {
                if (typeof showToast === 'function') showToast(resp && resp.message || '创建失败', 'error');
            }
        } catch (e) {
            console.error('[PatUI] create err', e);
            if (typeof showToast === 'function') showToast('创建失败:' + (e && e.message || ''), 'error');
        }
    }

    function _openShowModal(token) {
        var ov = document.getElementById('pat-show-overlay');
        var input = document.getElementById('pat-show-token');
        if (input) input.value = token;
        if (ov) ov.style.display = '';
        setTimeout(function() {
            if (input) { input.focus(); input.select(); }
        }, 50);
    }
    function closeShow() {
        var ov = document.getElementById('pat-show-overlay');
        if (ov) ov.style.display = 'none';
        var input = document.getElementById('pat-show-token');
        if (input) input.value = '';
        _shownToken = null;
    }

    async function copyShown() {
        var input = document.getElementById('pat-show-token');
        if (!input) return;
        try {
            await navigator.clipboard.writeText(input.value);
            if (typeof showToast === 'function') showToast('已复制到剪贴板', 'success');
        } catch (e) {
            // 回退:execCommand
            input.select();
            try { document.execCommand('copy'); } catch (_) {}
            if (typeof showToast === 'function') showToast('已复制', 'success');
        }
    }

    function confirmRevoke(id, label) {
        var msg = '撤销令牌「' + label + '」?<br>'
            + '<small style="color:#6B7280">撤销后立即失效,任何使用该 token 的请求都会 401。无法恢复。</small>';
        if (window.AppUtils && AppUtils.showConfirm) {
            AppUtils.showConfirm(msg, function() { _doRevoke(id); }, { confirmText: '撤销', danger: true });
        } else if (confirm('撤销「' + label + '」?')) {
            _doRevoke(id);
        }
    }
    async function _doRevoke(id) {
        try {
            await API.patRevoke(id);
            if (typeof showToast === 'function') showToast('已撤销', 'info');
            refresh();
        } catch (e) {
            console.error('[PatUI] revoke err', e);
            if (typeof showToast === 'function') showToast('撤销失败', 'error');
        }
    }

    return {
        refresh: refresh,
        create: create,
        closeShow: closeShow,
        copyShown: copyShown,
        confirmRevoke: confirmRevoke,
    };
})();
