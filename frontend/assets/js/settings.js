// ========== 设置页面逻辑 ==========

// 加载用户信息到设置页
async function loadSettingsData() {
    try {
        var data = await API.getMe();
        if (data.success && data.user) {
            document.getElementById('settings-username').textContent =
                data.user.username || '--';
            document.getElementById('settings-display-name').textContent =
                data.user.display_name || '--';
        }
    } catch(e) {
        console.error('[settings] loadSettingsData:', e);
    }
    // 初始化头像选择器
    highlightSelectedPreset();
    applyAvatar();
    // 更新推送通知状态
    if (typeof Notifications !== 'undefined' && Notifications.updatePushStatus) {
        Notifications.updatePushStatus();
    }
    // Guest mode: hide password change and friends sections
    if (window._userStatus === 'guest') {
        var pwdBtn = document.querySelector('#settings-account-section .settings-change-pwd-btn');
        if (pwdBtn) pwdBtn.style.display = 'none';
        var friendsSection = document.getElementById('settings-friends-section');
        if (friendsSection) {
            friendsSection.innerHTML = '<h4>好友</h4><div class="friends-empty">注册后可管理好友</div>';
        }
    }
    // Load timezone preference
    loadTimezone();
    // Init patrol toggle
    initPatrolToggle();
}

// 密码 Modal
var _pwdEscHandler = null;

function openPwdModal() {
    var overlay = document.getElementById('pwd-modal-overlay');
    overlay.style.display = 'flex';
    document.getElementById('settings-old-password').value = '';
    document.getElementById('settings-new-password').value = '';
    document.getElementById('settings-confirm-password').value = '';
    setTimeout(function() {
        document.getElementById('settings-old-password').focus();
    }, 50);
    _pwdEscHandler = function(e) {
        if (e.key === 'Escape') closePwdModal();
    };
    document.addEventListener('keydown', _pwdEscHandler);
}

function closePwdModal() {
    document.getElementById('pwd-modal-overlay').style.display = 'none';
    document.getElementById('settings-old-password').value = '';
    document.getElementById('settings-new-password').value = '';
    document.getElementById('settings-confirm-password').value = '';
    if (_pwdEscHandler) {
        document.removeEventListener('keydown', _pwdEscHandler);
        _pwdEscHandler = null;
    }
}

// 修改密码
async function changePassword() {
    var oldPwd = document.getElementById('settings-old-password').value;
    var newPwd = document.getElementById('settings-new-password').value;
    var confirmPwd = document.getElementById('settings-confirm-password').value;

    if (!oldPwd) {
        showToast('请输入当前密码', 'error');
        return;
    }
    if (!newPwd || newPwd.length < 8) {
        showToast('新密码至少需要 8 个字符', 'error');
        return;
    }
    if (newPwd !== confirmPwd) {
        showToast('两次输入的新密码不一致', 'error');
        return;
    }

    try {
        var data = await API.changePassword(oldPwd, newPwd);
        if (data.success) {
            showToast('密码修改成功', 'success');
            closePwdModal();
        } else {
            showToast(data.message || '密码修改失败', 'error');
        }
    } catch(e) {
        showToast('密码修改失败', 'error');
    }
}

// 退出登录
async function doLogout() {
    try { await API.logout(); } catch(e) {
        console.error('[settings] logout:', e);
    }
    window.location.href = '/login.html';
}

// AI 模型已固定为 Claude，无需用户选择

// ========== 二狗值班开关 ==========

function initPatrolToggle() {
    var toggle = document.getElementById('patrol-toggle');
    if (!toggle) return;
    toggle.checked = localStorage.getItem('patrol-enabled') !== '0';
}

function togglePatrol(enabled) {
    localStorage.setItem('patrol-enabled', enabled ? '1' : '0');
    if (typeof Patrol !== 'undefined') {
        if (enabled) {
            Patrol.init();
        } else {
            Patrol.destroy();
        }
    }
}

// ========== 二狗时区设置 ==========

var _currentTimezone = 'America/Toronto';

async function loadTimezone() {
    if (window._userStatus === 'guest') return;
    try {
        var data = await API.getTimezone();
        if (data.success && data.timezone) {
            _currentTimezone = data.timezone;
            highlightTimezone(data.timezone);
        }
    } catch(e) {
        console.error('[settings] loadTimezone:', e);
    }
}

function highlightTimezone(tz) {
    var container = document.getElementById('timezone-options');
    if (!container) return;
    container.querySelectorAll('.ai-model-btn').forEach(function(btn) {
        btn.classList.toggle('active', btn.dataset.tz === tz);
    });
}

async function selectTimezone(tz) {
    if (window._userStatus === 'guest') return;
    if (tz === _currentTimezone) return;

    var prevTz = _currentTimezone;
    _currentTimezone = tz;
    highlightTimezone(tz);

    try {
        var data = await API.setTimezone(tz);
        if (data.success) {
            var names = { 'America/Toronto': '多伦多', 'America/Vancouver': '温哥华', 'Asia/Shanghai': '北京' };
            showToast('时区已切换到 ' + (names[tz] || tz), 'success');
        } else {
            _currentTimezone = prevTz;
            highlightTimezone(prevTz);
            showToast(data.message || '保存失败', 'error');
        }
    } catch(e) {
        _currentTimezone = prevTz;
        highlightTimezone(prevTz);
        showToast('保存失败', 'error');
    }
}

// ========== 头像网格展开/收起 ==========

function toggleAvatarGrid() {
    var grid = document.getElementById('avatar-preset-grid');
    var link = document.getElementById('avatar-toggle-link');
    if (!grid || !link) return;
    var expanded = grid.classList.toggle('expanded');
    link.textContent = expanded ? '收起' : '更多';
}

// ========== 头像系统 ==========

var AVATAR_PRESETS = {
    'preset:cat': 'assets/images/preset-cat.png',
    'preset:panda': 'assets/images/preset-panda.png',
    'preset:boris': 'assets/images/preset-boris.png',
    'preset:shiba': 'assets/images/preset-shiba.png',
    'preset:catpaw': 'assets/images/preset-catpaw.png',
    'preset:whitecat': 'assets/images/preset-whitecat.png',
    'preset:emoji-ball': 'assets/images/preset-emoji-ball.png',
    'preset:pandaman': 'assets/images/preset-pandaman.png',
    'preset:samoyed': 'assets/images/preset-samoyed.png',
    'preset:cartooncat': 'assets/images/preset-cartooncat.png',
    'preset:pandatea': 'assets/images/preset-pandatea.png',
    'preset:backview': 'assets/images/preset-backview.png',
    'preset:cutecat': 'assets/images/preset-cutecat.png',
    'preset:hamster': 'assets/images/preset-hamster.png',
    'preset:bunny': 'assets/images/preset-bunny.png',
    'preset:shibapair': 'assets/images/preset-shibapair.png',
    'preset:tangping': 'assets/images/preset-tangping.png',
    'preset:doge': 'assets/images/preset-doge.png',
    'preset:shibarest': 'assets/images/preset-shibarest.png',
    'preset:shibaflower': 'assets/images/preset-shibaflower.png',
    'preset:cheems': 'assets/images/preset-cheems.png'
};

var AVATAR_GRADIENTS = {
    'color:blue': 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)',
    'color:green': 'linear-gradient(135deg, #43e97b 0%, #38f9d7 100%)',
    'color:orange': 'linear-gradient(135deg, #f7971e 0%, #ffd200 100%)',
    'color:pink': 'linear-gradient(135deg, #f093fb 0%, #f5576c 100%)'
};

// 选择预置头像
function selectPresetAvatar(el) {
    var value = el.dataset.avatar;
    localStorage.setItem('userAvatar', value);
    highlightSelectedPreset();
    applyAvatar();
    // Sync to server
    API.updateAvatar(value).catch(function(e) {
        console.error('[settings] updateAvatar:', e);
        showToast('头像同步失败，下次刷新可能丢失', 'error');
    });
}

// 上传自定义头像（canvas 压缩到 128x128）
function handleAvatarUpload(event) {
    var file = event.target.files[0];
    if (!file) return;
    var reader = new FileReader();
    reader.onload = function(e) {
        var img = new Image();
        img.onload = function() {
            var canvas = document.createElement('canvas');
            canvas.width = 128;
            canvas.height = 128;
            var ctx = canvas.getContext('2d');
            // 居中裁切为正方形
            var size = Math.min(img.width, img.height);
            var sx = (img.width - size) / 2;
            var sy = (img.height - size) / 2;
            ctx.drawImage(img, sx, sy, size, size, 0, 0, 128, 128);
            var dataURL = canvas.toDataURL('image/jpeg', 0.8);
            try {
                localStorage.setItem('userAvatar', dataURL);
            } catch(e) {
                showToast('图片太大，保存失败，请选择较小的图片', 'error');
                return;
            }
            highlightSelectedPreset();
            applyAvatar();
            // Sync to server
            API.updateAvatar(dataURL).catch(function(e) {
                console.error('[settings] updateAvatar:', e);
                showToast('头像同步失败，下次刷新可能丢失', 'error');
            });
            showToast('头像已更新', 'success');
        };
        img.src = e.target.result;
    };
    reader.readAsDataURL(file);
    // Reset input so same file can be selected again
    event.target.value = '';
}

// 应用头像到所有位置（header + settings preview）
function applyAvatar() {
    var value = localStorage.getItem('userAvatar');
    var initial = window._userInitial || 'B';

    // 收集所有需要更新的头像目标
    var targets = [
        {
            text: document.getElementById('avatar-text'),
            img: document.getElementById('avatar-img'),
            container: document.getElementById('header-avatar')
        },
        {
            text: document.getElementById('settings-avatar-text'),
            img: document.getElementById('settings-avatar-img'),
            container: document.getElementById('settings-avatar-preview')
        }
    ];

    targets.forEach(function(t) {
        if (!t.container) return;

        if (value && AVATAR_PRESETS[value]) {
            // 预置图片头像
            if (t.img) {
                t.img.src = AVATAR_PRESETS[value];
                t.img.style.display = 'block';
            }
            if (t.text) t.text.style.display = 'none';
        } else if (value && AVATAR_GRADIENTS[value]) {
            // 渐变色 + 首字母
            if (t.img) t.img.style.display = 'none';
            if (t.text) {
                t.text.style.display = '';
                t.text.textContent = initial;
            }
            t.container.style.background = AVATAR_GRADIENTS[value];
        } else if (value && value.startsWith('data:image/')) {
            // 用户上传的自定义头像
            if (t.img) {
                t.img.src = value;
                t.img.style.display = 'block';
            }
            if (t.text) t.text.style.display = 'none';
        } else {
            // 默认：蓝紫渐变 + 首字母
            if (t.img) t.img.style.display = 'none';
            if (t.text) {
                t.text.style.display = '';
                t.text.textContent = initial;
            }
            t.container.style.background = 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)';
        }
    });
}

// 高亮当前选中的预置头像
function highlightSelectedPreset() {
    var value = localStorage.getItem('userAvatar') || '';
    document.querySelectorAll('.avatar-preset').forEach(function(el) {
        el.classList.toggle('selected', el.dataset.avatar === value);
    });
}

// ========== 联系人管理 (Contacts) ==========

var Contacts = (function() {
    var contacts = [];
    var sectionInserted = false;

    // Ensure the contacts area is visible in the DOM
    function ensureSection() {
        if (sectionInserted) return;
        var area = document.getElementById('contacts-area');
        if (!area) return;
        area.style.display = '';
        sectionInserted = true;
    }

    async function loadContacts() {
        ensureSection();
        try {
            var resp = await API.getContacts();
            if (resp.success) {
                contacts = resp.items || [];
                renderContacts(contacts);
            }
        } catch (e) {
            console.error('[Contacts] load failed:', e);
        }
    }

    function renderContacts(items) {
        var container = document.getElementById('contacts-list');
        if (!container) return;
        var area = document.getElementById('contacts-area');

        if (items.length === 0) {
            if (area) area.style.display = 'none';
            container.innerHTML = '<div class="friends-empty">暂无联系人</div>';
            return;
        }
        if (area) area.style.display = '';

        // Split into linked (friends) and self-managed
        var linked = items.filter(function(c) { return c.friendship_id; });
        var selfManaged = items.filter(function(c) { return !c.friendship_id; });

        var html = '';

        if (linked.length > 0) {
            html += '<div class="contacts-group-label">可协作好友</div>';
            html += linked.map(function(c) {
                var displayName = c.linked_display_name || c.linked_username || c.name;
                var initial = displayName.charAt(0).toUpperCase();
                return '<div class="friend-item contact-item">' +
                    '<div class="friend-avatar" style="background:linear-gradient(135deg,#43e97b 0%,#38f9d7 100%)">' + escapeContactHtml(initial) + '</div>' +
                    '<div class="friend-info">' +
                        '<span class="friend-name">' + escapeContactHtml(displayName) + '</span>' +
                        (c.linked_username ? '<span class="friend-username">@' + escapeContactHtml(c.linked_username) + '</span>' : '') +
                        (c.note ? '<span class="contact-note">' + escapeContactHtml(c.note) + '</span>' : '') +
                    '</div>' +
                    '<button class="contact-edit-btn" onclick="Contacts.editContactNote(\'' + c.id + '\')" title="编辑备注">✎</button>' +
                '</div>';
            }).join('');
        }

        if (selfManaged.length > 0) {
            html += '<div class="contacts-group-label">自管理联系人</div>';
            html += selfManaged.map(function(c) {
                var initial = c.name.charAt(0).toUpperCase();
                return '<div class="friend-item contact-item">' +
                    '<div class="friend-avatar" style="background:linear-gradient(135deg,#f7971e 0%,#ffd200 100%)">' + escapeContactHtml(initial) + '</div>' +
                    '<div class="friend-info">' +
                        '<span class="friend-name">' + escapeContactHtml(c.name) + '</span>' +
                        (c.note ? '<span class="contact-note">' + escapeContactHtml(c.note) + '</span>' : '') +
                    '</div>' +
                    '<button class="contact-edit-btn" onclick="Contacts.editContactNote(\'' + c.id + '\')" title="编辑备注">✎</button>' +
                    '<button class="friend-remove-btn" onclick="Contacts.deleteSelfContact(\'' + c.id + '\')" title="删除联系人">&times;</button>' +
                '</div>';
            }).join('');
        }

        container.innerHTML = html;
    }

    function addSelfContact() {
        var name = prompt('联系人名称:');
        if (!name || !name.trim()) return;
        var note = prompt('备注 (可选):') || '';

        API.createContact({ name: name.trim(), note: note || undefined }).then(function(resp) {
            if (resp.success) {
                showToast('联系人已添加', 'success');
                loadContacts();
            } else {
                showToast(resp.message || '添加失败', 'error');
            }
        }).catch(function() {
            showToast('添加失败', 'error');
        });
    }

    function editContactNote(id) {
        var contact = contacts.find(function(c) { return c.id === id; });
        if (!contact) return;

        var newNote = prompt('编辑备注:', contact.note || '');
        if (newNote === null) return; // cancelled

        API.updateContact(id, { note: newNote }).then(function(resp) {
            if (resp.success) {
                showToast('备注已更新', 'success');
                loadContacts();
            } else {
                showToast(resp.message || '更新失败', 'error');
            }
        }).catch(function() {
            showToast('更新失败', 'error');
        });
    }

    function deleteSelfContact(id) {
        if (!confirm('确定删除此联系人吗？')) return;

        API.deleteContact(id).then(function(resp) {
            if (resp.success) {
                showToast('联系人已删除', 'success');
                contacts = contacts.filter(function(c) { return c.id !== id; });
                renderContacts(contacts);
            } else {
                showToast(resp.message || '删除失败', 'error');
            }
        }).catch(function() {
            showToast('删除失败', 'error');
        });
    }

    function escapeContactHtml(str) {
        var div = document.createElement('div');
        div.textContent = str || '';
        return div.innerHTML;
    }

    return {
        loadContacts: loadContacts,
        addSelfContact: addSelfContact,
        editContactNote: editContactNote,
        deleteSelfContact: deleteSelfContact
    };
})();

// ─── Memory management ───

// ===== 记忆管理 UI (T-081) =====
var MemoryUI = (function() {
    var _cat = '';          // current category filter
    var _searchQ = '';      // current search keyword
    var _total = 0;         // total count for current view
    var _memories = [];     // currently displayed memories
    var _debounceTimer = null;
    var _editStarVal = 3;   // star value in modal
    var _clearStep = 0;     // 0=idle, 1=first confirm shown, 2=input shown

    var CAT_LABELS = { fact: '事实', habit: '习惯', personality: '性格', intent: '意图' };

    function relativeTime(iso) {
        if (!iso) return '';
        var diff = (Date.now() - new Date(iso).getTime()) / 1000;
        if (diff < 60) return '刚刚';
        if (diff < 3600) return Math.floor(diff / 60) + ' 分钟前';
        if (diff < 86400) return Math.floor(diff / 3600) + ' 小时前';
        if (diff < 604800) return Math.floor(diff / 86400) + ' 天前';
        if (diff < 2592000) return Math.floor(diff / 604800) + ' 周前';
        return Math.floor(diff / 2592000) + ' 个月前';
    }

    function stars(n) {
        return '⭐'.repeat(Math.max(1, Math.min(5, n || 3)));
    }

    function escapeHtml(text) {
        var d = document.createElement('div');
        d.textContent = text;
        return d.innerHTML;
    }

    async function load() {
        var container = document.getElementById('memory-list');
        if (!container) return;
        container.innerHTML = '<div class="memory-loading">加载中...</div>';

        try {
            var res;
            if (_searchQ) {
                res = await API.searchMemories(_searchQ, _cat || undefined);
                _memories = res.memories || [];
                _total = res.count || _memories.length;
            } else {
                res = await API.getMemories({ category: _cat || undefined, limit: 200 });
                _memories = res.memories || [];
                _total = res.total != null ? res.total : _memories.length;
            }
            if (!res.success && !res.memories) {
                container.innerHTML = '<div class="memory-empty">加载失败 <button class="memory-retry-btn" onclick="MemoryUI.load()">重试</button></div>';
                return;
            }
            render();
        } catch(e) {
            console.error('[MemoryUI]', e);
            container.innerHTML = '<div class="memory-empty">网络异常，记忆暂时无法查看 <button class="memory-retry-btn" onclick="MemoryUI.load()">重试</button></div>';
        }
    }

    function render() {
        var container = document.getElementById('memory-list');
        var statsEl = document.getElementById('memory-stats');
        if (!container) return;

        // Stats
        if (statsEl) {
            var catName = _cat ? (CAT_LABELS[_cat] || _cat) : '';
            statsEl.textContent = _searchQ
                ? '搜索到 ' + _memories.length + ' 条'
                : (catName ? catName + '共 ' : '共 ') + _total + ' 条记忆';
        }

        if (!_memories || _memories.length === 0) {
            container.innerHTML = '<div class="memory-empty">' +
                (_searchQ ? '没有匹配的记忆' : '还没有记忆，和二狗多聊几句吧') + '</div>';
            return;
        }

        var html = _memories.map(function(m) {
            var label = CAT_LABELS[m.category] || m.category;
            var accessInfo = (m.importance >= 4 && m.access_count) ? ' · 已用 ' + m.access_count + ' 次' : '';
            var content = escapeHtml(m.content);
            // Highlight search keyword
            if (_searchQ) {
                var re = new RegExp('(' + _searchQ.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + ')', 'gi');
                content = content.replace(re, '<mark>$1</mark>');
            }
            return '<div class="memory-item" data-id="' + m.id + '">' +
                '<div class="memory-content">' +
                    '<div class="memory-content-text">' + content + '</div>' +
                    '<div class="memory-meta">' +
                        '<span class="memory-category memory-cat-' + m.category + '">' + label + '</span>' +
                        '<span class="memory-importance">' + stars(m.importance) + '</span>' +
                        '<span class="memory-time">' + relativeTime(m.created_at) + '</span>' +
                        accessInfo +
                    '</div>' +
                '</div>' +
                '<div class="memory-item-actions">' +
                    '<button class="memory-edit-btn" onclick="MemoryUI.showEditModal(\'' + m.id + '\')" title="编辑">改</button>' +
                    '<button class="memory-delete-btn" onclick="MemoryUI.deleteOne(\'' + m.id + '\')" title="删除">删</button>' +
                '</div>' +
            '</div>';
        }).join('');

        container.innerHTML = html;
    }

    // ─── Tab ───
    function switchTab(cat) {
        _cat = cat;
        _searchQ = '';
        var input = document.getElementById('memory-search-input');
        if (input) input.value = '';
        // Update active tab
        document.querySelectorAll('.memory-tab').forEach(function(t) {
            t.classList.toggle('active', t.getAttribute('data-category') === cat);
        });
        load();
    }

    // ─── Search ───
    function onSearchInput(val) {
        clearTimeout(_debounceTimer);
        _debounceTimer = setTimeout(function() {
            _searchQ = val.trim();
            load();
        }, 300);
    }

    // ─── Delete single ───
    async function deleteOne(id) {
        if (!confirm('确定删除这条记忆？删除后无法恢复')) return;
        try {
            var res = await API.deleteMemory(id);
            if (res.success) {
                showToast('已删除', 'success');
                load();
            } else {
                showToast(res.error || '删除失败', 'error');
            }
        } catch(e) {
            showToast('删除失败', 'error');
        }
    }

    // ─── Clear all (two-step) ───
    function startClearAll() {
        if (_total === 0) { showToast('没有记忆可清空', 'info'); return; }
        _clearStep = 1;
        var overlay = document.getElementById('memory-clear-overlay');
        var msg = document.getElementById('memory-clear-msg');
        var inputGroup = document.getElementById('memory-clear-input-group');
        var confirmInput = document.getElementById('memory-clear-confirm-input');
        if (msg) msg.textContent = '确定清空全部 ' + _total + ' 条记忆？此操作不可恢复。';
        if (inputGroup) inputGroup.style.display = 'none';
        if (confirmInput) confirmInput.value = '';
        if (overlay) overlay.style.display = 'flex';
    }

    async function confirmClear() {
        if (_clearStep === 1) {
            // Show second step with input
            _clearStep = 2;
            var msg = document.getElementById('memory-clear-msg');
            var inputGroup = document.getElementById('memory-clear-input-group');
            if (msg) msg.textContent = '再次确认：请在下方输入"清空"以执行操作。';
            if (inputGroup) inputGroup.style.display = '';
            document.getElementById('memory-clear-confirm-input')?.focus();
            return;
        }
        // Step 2: validate input
        var input = document.getElementById('memory-clear-confirm-input');
        if (!input || input.value.trim() !== '清空') {
            showToast('请输入"清空"以确认', 'warning');
            return;
        }
        try {
            var res = await API.deleteAllMemories();
            if (res.success) {
                showToast('已清空 ' + _total + ' 条记忆', 'success');
                closeClearModal();
                load();
            } else {
                showToast(res.error || '清空失败', 'error');
            }
        } catch(e) {
            showToast('清空失败', 'error');
        }
    }

    function closeClearModal() {
        _clearStep = 0;
        var overlay = document.getElementById('memory-clear-overlay');
        if (overlay) overlay.style.display = 'none';
    }

    // ─── Edit / Add modal ───
    function showEditModal(id) {
        var m = _memories.find(function(x) { return x.id === id; });
        if (!m) return;
        document.getElementById('memory-modal-title').textContent = '编辑记忆';
        document.getElementById('memory-edit-id').value = id;
        document.getElementById('memory-edit-content').value = m.content;
        updateCharCount();
        var radios = document.querySelectorAll('#memory-edit-category input[name="mem-cat"]');
        radios.forEach(function(r) { r.checked = (r.value === m.category); });
        pickStar(m.importance || 3);
        document.getElementById('memory-modal-overlay').style.display = 'flex';
    }

    function showAddModal() {
        document.getElementById('memory-modal-title').textContent = '添加记忆';
        document.getElementById('memory-edit-id').value = '';
        document.getElementById('memory-edit-content').value = '';
        updateCharCount();
        var radios = document.querySelectorAll('#memory-edit-category input[name="mem-cat"]');
        radios.forEach(function(r) { r.checked = (r.value === 'fact'); });
        pickStar(3);
        document.getElementById('memory-modal-overlay').style.display = 'flex';
        document.getElementById('memory-edit-content').focus();
    }

    function closeModal() {
        document.getElementById('memory-modal-overlay').style.display = 'none';
    }

    function pickStar(n) {
        _editStarVal = n;
        document.querySelectorAll('#memory-edit-importance .memory-star').forEach(function(s) {
            var v = parseInt(s.getAttribute('data-val'));
            s.style.opacity = v <= n ? '1' : '0.3';
        });
    }

    function updateCharCount() {
        var ta = document.getElementById('memory-edit-content');
        var counter = document.getElementById('memory-char-count');
        if (ta && counter) counter.textContent = (ta.value || '').length + '/500';
    }

    async function saveModal() {
        var id = document.getElementById('memory-edit-id').value;
        var content = (document.getElementById('memory-edit-content').value || '').trim();
        if (!content) { showToast('内容不能为空', 'warning'); return; }
        if (content.length > 500) { showToast('内容不能超过 500 字', 'warning'); return; }

        var catRadio = document.querySelector('#memory-edit-category input[name="mem-cat"]:checked');
        var category = catRadio ? catRadio.value : 'fact';

        var data = { content: content, category: category, importance: _editStarVal };

        try {
            var res;
            if (id) {
                res = await API.updateMemory(id, data);
            } else {
                res = await API.createMemory(data);
            }
            if (res.success || res.id) {
                showToast(id ? '已保存' : '已添加', 'success');
                closeModal();
                load();
            } else if (res.error && res.error.indexOf('duplicate') >= 0) {
                showToast('这条记忆已存在', 'warning');
            } else {
                showToast(res.error || '保存失败', 'error');
            }
        } catch(e) {
            showToast('保存失败', 'error');
        }
    }

    // Char counter live update
    document.addEventListener('DOMContentLoaded', function() {
        var ta = document.getElementById('memory-edit-content');
        if (ta) ta.addEventListener('input', updateCharCount);
    });

    return {
        load: load,
        switchTab: switchTab,
        onSearchInput: onSearchInput,
        deleteOne: deleteOne,
        startClearAll: startClearAll,
        confirmClear: confirmClear,
        closeClearModal: closeClearModal,
        showEditModal: showEditModal,
        showAddModal: showAddModal,
        closeModal: closeModal,
        pickStar: pickStar,
        saveModal: saveModal
    };
})();

// ===== 灵魂演进 UI (T-092) =====
var SoulUI = (function() {
    var PARAM_LABELS = {
        classical_ratio: { name: '文白比例', icon: '📜' },
        warmth_level: { name: '温度', icon: '🌡️' },
        verbosity_level: { name: '话量', icon: '💬' },
        proactivity_level: { name: '主动性', icon: '🎯' },
        trust_level: { name: '信任', icon: '🤝' },
    };
    var STAGE_LABELS = {
        stranger: '初识', acquaintance: '相识', familiar: '熟悉',
        close: '亲近', intimate: '至交'
    };

    function switchTab(tab) {
        document.querySelectorAll('.soul-tab').forEach(function(t) {
            t.classList.toggle('active', t.getAttribute('data-tab') === tab);
        });
        document.getElementById('soul-params-panel').style.display = tab === 'params' ? '' : 'none';
        document.getElementById('soul-logs-panel').style.display = tab === 'logs' ? '' : 'none';
        if (tab === 'logs') loadLogs();
    }

    async function loadParams() {
        var el = document.getElementById('soul-params-panel');
        if (!el) return;
        try {
            var res = await API.getSoulState();
            if (!res.success) { el.innerHTML = '<div class="soul-empty">加载失败</div>'; return; }
            var s = res.soul_state;
            var html = '<div class="soul-relationship">' +
                '<span class="soul-stage">' + (STAGE_LABELS[s.relationship_stage] || s.relationship_stage) + '</span>' +
                '<span class="soul-interactions">共 ' + s.total_interactions + ' 次对话</span>' +
                '</div>';
            var params = ['classical_ratio', 'warmth_level', 'verbosity_level', 'proactivity_level', 'trust_level'];
            params.forEach(function(key) {
                var val = s[key] || 0;
                var pct = Math.round(val * 100);
                var info = PARAM_LABELS[key] || { name: key, icon: '' };
                html += '<div class="soul-param">' +
                    '<div class="soul-param-header">' +
                        '<span>' + info.icon + ' ' + info.name + '</span>' +
                        '<span class="soul-param-value">' + pct + '%</span>' +
                    '</div>' +
                    '<div class="soul-bar"><div class="soul-bar-fill" style="width:' + pct + '%"></div></div>' +
                '</div>';
            });
            el.innerHTML = html;
        } catch(e) {
            console.error('[SoulUI]', e);
            el.innerHTML = '<div class="soul-empty">网络异常</div>';
        }
    }

    async function loadLogs() {
        var el = document.getElementById('soul-logs-panel');
        if (!el) return;
        el.innerHTML = '<div class="soul-loading">加载中...</div>';
        try {
            var res = await API.getEvolutionLogs();
            if (!res.success || !res.logs || res.logs.length === 0) {
                el.innerHTML = '<div class="soul-empty">暂无演化记录</div>';
                return;
            }
            var html = res.logs.map(function(log) {
                var info = PARAM_LABELS[log.parameter] || { name: log.parameter, icon: '📊' };
                var oldPct = Math.round(log.old_value * 100);
                var newPct = Math.round(log.new_value * 100);
                var diff = newPct - oldPct;
                var diffStr = (diff > 0 ? '+' : '') + diff + '%';
                var diffClass = diff > 0 ? 'soul-log-up' : (diff < 0 ? 'soul-log-down' : '');
                var time = relativeTime(log.created_at);
                var trigger = log.trigger_type === 'auto' ? '对话触发' : '手动调整';
                return '<div class="soul-log-item">' +
                    '<div class="soul-log-main">' +
                        '<span class="soul-log-param">' + info.icon + ' ' + info.name + '</span>' +
                        '<span class="soul-log-change ' + diffClass + '">' + oldPct + '% → ' + newPct + '% (' + diffStr + ')</span>' +
                    '</div>' +
                    '<div class="soul-log-meta">' + trigger + ' · ' + time + '</div>' +
                '</div>';
            }).join('');
            el.innerHTML = html;
        } catch(e) {
            console.error('[SoulUI]', e);
            el.innerHTML = '<div class="soul-empty">加载失败</div>';
        }
    }

    function relativeTime(iso) {
        if (!iso) return '';
        var diff = (Date.now() - new Date(iso).getTime()) / 1000;
        if (diff < 60) return '刚刚';
        if (diff < 3600) return Math.floor(diff / 60) + ' 分钟前';
        if (diff < 86400) return Math.floor(diff / 3600) + ' 小时前';
        if (diff < 604800) return Math.floor(diff / 86400) + ' 天前';
        return Math.floor(diff / 604800) + ' 周前';
    }

    return { switchTab: switchTab, loadParams: loadParams };
})();

// Hook into settings loading: also load contacts, memories, and soul when settings are shown
var _origLoadSettingsData = loadSettingsData;
loadSettingsData = async function() {
    await _origLoadSettingsData();
    Contacts.loadContacts();
    MemoryUI.load();
    SoulUI.loadParams();
    // T-116:个人访问令牌列表
    if (typeof PatUI !== 'undefined' && PatUI.refresh) PatUI.refresh();
};

// BUG-1 fix: settings.js 加载完成后立即应用头像，避免 checkAuth 竞态
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', function() { applyAvatar(); });
} else {
    applyAvatar();
}
