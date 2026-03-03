// ========== 通用工具函数 ==========

// HTML 转义（基础版，用于任务名等单行文本）
function escapeHtml(str) {
    if (!str) return '';
    return str.replace(/&/g, '&amp;')
              .replace(/</g, '&lt;')
              .replace(/>/g, '&gt;')
              .replace(/"/g, '&quot;');
}

// 完整日期时间格式化: [YYYY-]MM-DD HH:mm (同年省略年份)
function formatDateTime(isoString) {
    if (!isoString) return '--';
    var date = new Date(isoString);
    var now = new Date();
    var y = date.getFullYear();
    var m = String(date.getMonth() + 1).padStart(2, '0');
    var d = String(date.getDate()).padStart(2, '0');
    var h = String(date.getHours()).padStart(2, '0');
    var min = String(date.getMinutes()).padStart(2, '0');
    if (y === now.getFullYear()) {
        return m + '-' + d + ' ' + h + ':' + min;
    }
    return y + '-' + m + '-' + d + ' ' + h + ':' + min;
}

// 短时间格式化: MM-DD HH:mm
function formatShortTime(isoString) {
    if (!isoString) return '--';
    var date = new Date(isoString);
    var m = String(date.getMonth() + 1).padStart(2, '0');
    var d = String(date.getDate()).padStart(2, '0');
    var h = String(date.getHours()).padStart(2, '0');
    var min = String(date.getMinutes()).padStart(2, '0');
    return m + '-' + d + ' ' + h + ':' + min;
}

// 象限名称映射
function getQuadrantName(q) {
    var names = {
        'important-urgent': '优先处理',
        'important-not-urgent': '就等你翻牌子了',
        'not-important-urgent': '待分类',
        'not-important-not-urgent': '短平快'
    };
    return names[q] || q;
}

// 象限名称映射（带emoji，用于弹窗）
function getQuadrantNameEmoji(q) {
    var names = {
        'important-urgent': '🔥 优先处理',
        'important-not-urgent': '🎯 就等你翻牌子了',
        'not-important-urgent': '📥 待分类',
        'not-important-not-urgent': '⚡ 短平快'
    };
    return names[q] || q;
}

// Tab 名称映射
function getTabName(tab) {
    var names = { today: 'Today', week: 'This Week', month: 'Next 30 Days' };
    return names[tab] || tab;
}

// 全局鼠标位置跟踪（用于 toast 在鼠标附近显示）
window._mousePos = { x: window.innerWidth / 2, y: window.innerHeight / 2 };
document.addEventListener('mousemove', function(e) {
    window._mousePos = { x: e.clientX, y: e.clientY };
});
document.addEventListener('click', function(e) {
    window._mousePos = { x: e.clientX, y: e.clientY };
});

// AppUtils - 共享工具集
window.AppUtils = {
    escapeHtmlMultiline: function(str) {
        if (!str) return '';
        return str.replace(/&/g, '&amp;')
                  .replace(/</g, '&lt;')
                  .replace(/>/g, '&gt;')
                  .replace(/"/g, '&quot;')
                  .replace(/\n/g, '<br>');
    },
    showToast: function(message, type) {
        type = type || 'success';
        var toast = document.createElement('div');
        toast.className = 'toast toast-' + type + ' toast-at-mouse';
        toast.textContent = message;
        document.body.appendChild(toast);

        var mouseX = window._mousePos.x;
        var mouseY = window._mousePos.y;
        var offsetX = 15;
        var offsetY = -40;

        var rect = toast.getBoundingClientRect();
        var posX = mouseX + offsetX;
        var posY = mouseY + offsetY;

        if (posX + rect.width > window.innerWidth - 10) {
            posX = mouseX - rect.width - offsetX;
        }
        if (posY < 10) {
            posY = mouseY + 20;
        }
        if (posY + rect.height > window.innerHeight - 10) {
            posY = window.innerHeight - rect.height - 10;
        }

        toast.style.left = posX + 'px';
        toast.style.top = posY + 'px';

        setTimeout(function() {
            toast.classList.add('toast-hide');
            setTimeout(function() { toast.remove(); }, 300);
        }, 2000);
    },

    showConfirm: function(message, onConfirm, options) {
        options = options || {};
        var confirmText = options.confirmText || '确定';
        var cancelText = options.cancelText || '取消';
        var danger = options.danger || false;

        var overlay = document.createElement('div');
        overlay.className = 'confirm-overlay';
        overlay.innerHTML = '<div class="confirm-dialog">' +
            '<div class="confirm-body">' + message + '</div>' +
            '<div class="confirm-actions">' +
                '<button class="confirm-btn cancel-btn">' + cancelText + '</button>' +
                '<button class="confirm-btn ok-btn' + (danger ? ' danger' : '') + '">' + confirmText + '</button>' +
            '</div>' +
        '</div>';

        document.body.appendChild(overlay);

        var closeDialog = function(confirmed) {
            overlay.classList.add('confirm-hide');
            setTimeout(function() {
                overlay.remove();
                if (confirmed && onConfirm) {
                    onConfirm();
                }
            }, 200);
        };

        overlay.querySelector('.cancel-btn').onclick = function() { closeDialog(false); };
        overlay.querySelector('.ok-btn').onclick = function() { closeDialog(true); };

        overlay.onclick = function(e) {
            if (e.target === overlay) { closeDialog(false); }
        };

        var escHandler = function(e) {
            if (e.key === 'Escape') {
                closeDialog(false);
                document.removeEventListener('keydown', escHandler);
            }
        };
        document.addEventListener('keydown', escHandler);

        overlay.querySelector('.ok-btn').focus();
    }
};

// showToast 快捷函数
function showToast(message, type) {
    AppUtils.showToast(message, type);
}

// 更新所有 .guest-ai-hint 元素（AI 剩余次数提示）
function updateAllGuestAiHints() {
    var remaining = window._guestAiRemaining;
    if (remaining === undefined) remaining = 0;
    // Update all hint spans
    var hints = document.querySelectorAll('.guest-ai-hint');
    for (var i = 0; i < hints.length; i++) {
        var el = hints[i];
        if (remaining <= 0) {
            el.textContent = '(已用完)';
            el.classList.add('guest-ai-warning');
        } else {
            el.textContent = '(剩余' + remaining + '次)';
            if (remaining <= 3) {
                el.classList.add('guest-ai-warning');
            } else {
                el.classList.remove('guest-ai-warning');
            }
        }
    }
    // Enable/disable AI action elements
    var actionEls = document.querySelectorAll('[data-guest-ai-action]');
    for (var j = 0; j < actionEls.length; j++) {
        var ael = actionEls[j];
        if (remaining <= 0) {
            ael.classList.add('guest-disabled');
            ael.title = 'AI 体验次数已用完 — 注册解锁';
        } else {
            ael.classList.remove('guest-disabled');
            ael.title = '';
        }
    }
}

function isGuestRestricted(featureName) {
    if (window._userStatus === 'guest') {
        showToast('体验模式不支持' + featureName + '，注册账户解锁', 'warning');
        return true;
    }
    return false;
}

/**
 * 图片文件 → { data: base64, mime_type } 对象
 * 自动压缩到 maxPx（默认 1600），JPEG quality 0.85
 * 用于 Claude vision API — expense/trip/health 等模块共用
 *
 * @param {File} file
 * @param {number} [maxPx=1600]
 * @returns {Promise<{data: string, mime_type: string}>}
 */
function imageFileToBase64(file, maxPx) {
    var MAX = maxPx || 1600;
    var QUALITY = 0.85;
    return new Promise(function(resolve, reject) {
        var url = URL.createObjectURL(file);
        var img = new Image();
        img.onload = function() {
            URL.revokeObjectURL(url);
            var w = img.width, h = img.height;
            if (w > MAX || h > MAX) {
                if (w > h) { h = Math.round(h * MAX / w); w = MAX; }
                else { w = Math.round(w * MAX / h); h = MAX; }
            }
            var canvas = document.createElement('canvas');
            canvas.width = w; canvas.height = h;
            canvas.getContext('2d').drawImage(img, 0, 0, w, h);
            var dataUrl = canvas.toDataURL('image/jpeg', QUALITY);
            resolve({ data: dataUrl.split(',')[1], mime_type: 'image/jpeg' });
        };
        img.onerror = reject;
        img.src = url;
    });
}

// ========== PhotoManager — 公共照片处理组件 ==========

/**
 * @param {Object} opts
 * @param {string} opts.container  - 缩略图预览容器的 CSS 选择器
 * @param {Function} [opts.onChange] - 照片列表变化回调 onChange(files)
 */
function PhotoManager(opts) {
    this._files = [];
    this._containerSel = opts.container;
    this._onChange = opts.onChange || function() {};
}

PhotoManager.prototype._getContainer = function() {
    return document.querySelector(this._containerSel);
};

PhotoManager.prototype._notify = function() {
    this._onChange(this._files.slice());
};

PhotoManager.prototype.addFiles = function(fileList) {
    var self = this;
    var files = Array.from(fileList || []);
    var added = 0;
    files.forEach(function(f) {
        if (!f.type || !f.type.startsWith('image/')) {
            showToast('跳过非图片文件: ' + f.name, 'warning');
            return;
        }
        self._files.push(f);
        added++;
    });
    if (added > 0) {
        this._render();
        this._notify();
    }
};

PhotoManager.prototype.remove = function(index) {
    if (index >= 0 && index < this._files.length) {
        this._files.splice(index, 1);
        this._render();
        this._notify();
    }
};

PhotoManager.prototype.clear = function() {
    this._files = [];
    var container = this._getContainer();
    if (container) container.innerHTML = '';
    this._notify();
};

PhotoManager.prototype.getFiles = function() {
    return this._files.slice();
};

PhotoManager.prototype.getBase64 = function() {
    return Promise.all(this._files.map(function(f) {
        return imageFileToBase64(f);
    }));
};

PhotoManager.prototype._render = function() {
    var container = this._getContainer();
    if (!container) return;
    container.innerHTML = '';
    var self = this;

    this._files.forEach(function(file, idx) {
        var thumb = document.createElement('div');
        thumb.className = 'pm-thumb';

        var img = document.createElement('img');
        img.src = URL.createObjectURL(file);
        img.onclick = function() { self._showLightbox(file); };

        var removeBtn = document.createElement('button');
        removeBtn.className = 'pm-remove';
        removeBtn.textContent = '\u00d7';
        removeBtn.onclick = function(e) {
            e.stopPropagation();
            self.remove(idx);
        };

        thumb.appendChild(img);
        thumb.appendChild(removeBtn);
        container.appendChild(thumb);
    });
};

PhotoManager.prototype._showLightbox = function(file) {
    var overlay = document.createElement('div');
    overlay.className = 'pm-lightbox';

    var img = document.createElement('img');
    img.src = URL.createObjectURL(file);

    var closeBtn = document.createElement('button');
    closeBtn.className = 'pm-lightbox-close';
    closeBtn.textContent = '\u00d7';
    closeBtn.onclick = function() { overlay.remove(); };

    overlay.onclick = function(e) {
        if (e.target === overlay) overlay.remove();
    };

    overlay.appendChild(img);
    overlay.appendChild(closeBtn);
    document.body.appendChild(overlay);
};
