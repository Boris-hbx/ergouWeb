// ========== 通用弹窗组件 (T-095) ==========
//
// 提供两个姊妹组件,统一所有任务模块的弹窗体验:
//   - openProgressDialog({ label, currentProgress, onConfirm, onComplete })
//       进度调节 slider + 100% 时二次确认。
//   - openTextInputDialog({ label, initial, type, placeholder, onConfirm })
//       居中文本/数字输入(替代 window.prompt 的原生弹框)。
//
// 视觉复用 components.css 的 .progress-dialog-* 样式(spec § 7.1 要求,不另写 CSS)。
// 键盘:Esc 关闭(取消),Enter 在输入框上确认。点蒙层关闭(取消)。
//
// 抽象目的:todo 任务列表(tasks.js)和工作任务表(work-table.js)共用同一个弹窗
// 框架,视觉、动画、键盘交互完全一致(spec § 7.1 验收标准)。

var WorkDialogs = (function() {
    function _esc(s) {
        return ('' + (s == null ? '' : s))
            .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }

    /**
     * 进度调节弹窗 —— slider + 100% 时调 onComplete,否则调 onConfirm。
     *
     * @param {Object}   opts
     * @param {string}   opts.label             显示在标题下方的任务名/列名
     * @param {number}   opts.currentProgress   0..100 初始值
     * @param {Function} opts.onConfirm(p)      用户确认非 100% 时回调
     * @param {Function} opts.onComplete(p)     用户确认 100% 时回调
     *                                          (若未提供则降级到 onConfirm)
     */
    function openProgressDialog(opts) {
        opts = opts || {};
        var current = Math.max(0, Math.min(100, parseInt(opts.currentProgress, 10) || 0));
        var label = opts.label || '';

        var overlay = document.createElement('div');
        overlay.className = 'progress-dialog-overlay';
        overlay.innerHTML =
            '<div class="progress-dialog" role="dialog" aria-modal="true">' +
                '<div class="progress-dialog-title">设置完成度</div>' +
                '<div class="progress-dialog-task">' + _esc(label) + '</div>' +
                '<div class="progress-slider-container">' +
                    '<input type="range" class="progress-slider" id="wd-prog-slider" min="0" max="100" value="' + current + '" style="--val:' + current + '%">' +
                    '<div class="progress-value" id="wd-prog-value">' + current + '%</div>' +
                '</div>' +
                '<div class="progress-dialog-buttons">' +
                    '<button class="progress-btn cancel" id="wd-prog-cancel">取消</button>' +
                    '<button class="progress-btn confirm" id="wd-prog-confirm">确定</button>' +
                '</div>' +
            '</div>';
        document.body.appendChild(overlay);

        var slider     = overlay.querySelector('#wd-prog-slider');
        var valueEl    = overlay.querySelector('#wd-prog-value');
        var confirmBtn = overlay.querySelector('#wd-prog-confirm');
        var cancelBtn  = overlay.querySelector('#wd-prog-cancel');

        var lastVal = current;
        slider.addEventListener('input', function() {
            var v = parseInt(slider.value);
            valueEl.textContent = v + '%';
            slider.style.setProperty('--val', v + '%');

            // 沿用 tasks.js 既有的 living-line 联动(若全局存在则触发;不存在自动忽略)
            if (window.syncLineWithProgress) {
                var rect = slider.getBoundingClientRect();
                var x = rect.left + (v / 100) * rect.width;
                window.syncLineWithProgress(x, Math.abs(v - lastVal));
            }
            lastVal = v;

            if (v >= 100) {
                valueEl.classList.add('complete');
                confirmBtn.textContent = '确定完成';
                confirmBtn.classList.add('complete');
            } else {
                valueEl.classList.remove('complete');
                confirmBtn.textContent = '确定';
                confirmBtn.classList.remove('complete');
            }
        });
        slider.addEventListener('mouseup',  function() { if (window.releaseLineProgress) window.releaseLineProgress(); });
        slider.addEventListener('touchend', function() { if (window.releaseLineProgress) window.releaseLineProgress(); });

        function close() {
            if (overlay.parentNode) overlay.parentNode.removeChild(overlay);
            document.removeEventListener('keydown', onKey);
        }
        function onKey(e) {
            if (e.key === 'Escape') { close(); }
            else if (e.key === 'Enter') { confirmBtn.click(); }
        }
        document.addEventListener('keydown', onKey);

        cancelBtn.addEventListener('click', close);
        overlay.addEventListener('click', function(e) { if (e.target === overlay) close(); });
        confirmBtn.addEventListener('click', function() {
            var v = parseInt(slider.value);
            close();
            if (v >= 100) {
                if (typeof opts.onComplete === 'function') opts.onComplete(100);
                else if (typeof opts.onConfirm === 'function') opts.onConfirm(100);
            } else {
                if (typeof opts.onConfirm === 'function') opts.onConfirm(v);
            }
        });
    }

    /**
     * 文本/数字输入弹窗 —— 替代 window.prompt 的统一居中 modal。
     *
     * @param {Object}   opts
     * @param {string}   opts.label        显示标题(列名/动作说明)
     * @param {string}   opts.initial      预填值
     * @param {string}   opts.type         'text' | 'number'(默认 text)
     * @param {string}   opts.placeholder  输入框 placeholder
     * @param {Function} opts.onConfirm(v) 用户确认时回调(text 时是 string;number 时是 number,空输入 → 0)
     */
    function openTextInputDialog(opts) {
        opts = opts || {};
        var label = opts.label || '';
        var type = (opts.type === 'number') ? 'number' : 'text';
        var initial = opts.initial == null ? '' : String(opts.initial);
        var placeholder = opts.placeholder || '';

        var overlay = document.createElement('div');
        overlay.className = 'progress-dialog-overlay';
        // 复用 .progress-dialog 的容器/按钮;输入框用内联样式(spec § 7.1:不再新写 progress-dialog-* CSS)。
        overlay.innerHTML =
            '<div class="progress-dialog" role="dialog" aria-modal="true">' +
                '<div class="progress-dialog-title">' + _esc(label) + '</div>' +
                '<div style="padding:18px 24px 8px;">' +
                    '<input id="wd-txt-input" type="' + type + '" ' +
                        'placeholder="' + _esc(placeholder) + '" ' +
                        'value="' + _esc(initial) + '" ' +
                        'style="width:100%;padding:10px 12px;font-size:0.95rem;' +
                               'border:1px solid var(--border-color);border-radius:9px;' +
                               'background:var(--bg-input);color:var(--text-primary);' +
                               'box-sizing:border-box;outline:none;">' +
                '</div>' +
                '<div class="progress-dialog-buttons">' +
                    '<button class="progress-btn cancel" id="wd-txt-cancel">取消</button>' +
                    '<button class="progress-btn confirm" id="wd-txt-confirm">确定</button>' +
                '</div>' +
            '</div>';
        document.body.appendChild(overlay);

        var input      = overlay.querySelector('#wd-txt-input');
        var confirmBtn = overlay.querySelector('#wd-txt-confirm');
        var cancelBtn  = overlay.querySelector('#wd-txt-cancel');

        // input 聚焦时蓝紫色描边,和 .progress-dialog 主色一致
        input.addEventListener('focus', function() {
            input.style.borderColor = 'var(--primary-color)';
        });
        input.addEventListener('blur', function() {
            input.style.borderColor = 'var(--border-color)';
        });
        // 自动 focus + 全选(便于直接覆盖现有值)
        setTimeout(function() { input.focus(); if (initial) input.select(); }, 10);

        function close() {
            if (overlay.parentNode) overlay.parentNode.removeChild(overlay);
            document.removeEventListener('keydown', onKey);
        }
        function onKey(e) {
            if (e.key === 'Escape') close();
            // Enter 在 input 内时浏览器不会触发表单提交(没 form 包裹),手动处理
            else if (e.key === 'Enter' && document.activeElement === input) confirmBtn.click();
        }
        document.addEventListener('keydown', onKey);

        cancelBtn.addEventListener('click', close);
        overlay.addEventListener('click', function(e) { if (e.target === overlay) close(); });
        confirmBtn.addEventListener('click', function() {
            var raw = input.value;
            close();
            if (typeof opts.onConfirm === 'function') {
                if (type === 'number') {
                    var n = parseFloat(raw);
                    opts.onConfirm(isNaN(n) ? 0 : n);
                } else {
                    opts.onConfirm(raw);
                }
            }
        });
    }

    return {
        openProgressDialog: openProgressDialog,
        openTextInputDialog: openTextInputDialog,
    };
})();

// 暴露成顶层函数,方便 work-table.js / tasks.js 直接调
var openProgressDialog  = WorkDialogs.openProgressDialog;
var openTextInputDialog = WorkDialogs.openTextInputDialog;
