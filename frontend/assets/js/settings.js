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
        var pwdSection = document.getElementById('settings-password-section');
        if (pwdSection) pwdSection.style.display = 'none';
        var friendsSection = document.getElementById('settings-friends-section');
        if (friendsSection) {
            friendsSection.innerHTML = '<h4>好友</h4><div class="friends-empty">注册后可管理好友</div>';
        }
    }
    // Load AI model preference
    loadAiModel();
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

// ========== AI 模型选择 ==========

var _currentAiModel = 'auto';

async function loadAiModel() {
    if (window._userStatus === 'guest') {
        // Guest: disable all buttons, show hint
        var desc = document.getElementById('ai-model-desc');
        if (desc) desc.textContent = '注册后可切换模型';
        document.querySelectorAll('.ai-model-btn').forEach(function(btn) {
            btn.disabled = true;
            btn.classList.add('ai-model-disabled');
        });
        return;
    }
    try {
        var data = await API.getAiModel();
        if (data.success && data.model) {
            _currentAiModel = data.model;
            highlightAiModel(data.model);
        }
    } catch(e) {
        console.error('[settings] loadAiModel:', e);
    }
}

function highlightAiModel(model) {
    document.querySelectorAll('.ai-model-btn').forEach(function(btn) {
        btn.classList.toggle('active', btn.dataset.model === model);
    });
}

async function selectAiModel(model) {
    if (window._userStatus === 'guest') return;
    if (model === _currentAiModel) return;

    var prevModel = _currentAiModel;
    _currentAiModel = model;
    highlightAiModel(model);

    try {
        var data = await API.setAiModel(model);
        if (data.success) {
            var names = { auto: '自动', doubao: '模型 A', claude: '模型 B' };
            showToast('已切换到 ' + (names[model] || model), 'success');
        } else {
            _currentAiModel = prevModel;
            highlightAiModel(prevModel);
            showToast(data.message || '保存失败', 'error');
        }
    } catch(e) {
        _currentAiModel = prevModel;
        highlightAiModel(prevModel);
        showToast('保存失败', 'error');
    }
}

// ========== 二狗巡游开关 ==========

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

    // Ensure the contacts section exists in the DOM
    function ensureSection() {
        if (sectionInserted) return;
        var friendsSection = document.getElementById('add-friend-btn');
        if (!friendsSection) return;
        var parentSection = friendsSection.closest('.settings-section');
        if (!parentSection) return;

        var section = document.createElement('div');
        section.className = 'settings-section';
        section.id = 'contacts-section';
        section.innerHTML =
            '<h4>联系人</h4>' +
            '<div id="contacts-list"><div class="friends-empty">暂无联系人</div></div>' +
            '<button class="btn btn-primary settings-add-friend-btn" id="add-contact-btn" onclick="Contacts.addSelfContact()">+ 添加联系人</button>';

        parentSection.parentNode.insertBefore(section, parentSection.nextSibling);
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

        if (items.length === 0) {
            container.innerHTML = '<div class="friends-empty">暂无联系人</div>';
            return;
        }

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

async function loadMemories() {
    var container = document.getElementById('memory-list');
    var clearBtn = document.getElementById('clear-all-memories-btn');
    if (!container) return;

    try {
        var res = await API.getMemories();
        if (!res.success) {
            container.innerHTML = '<div class="memory-empty">加载失败</div>';
            return;
        }
        renderMemories(res.memories || []);
        if (clearBtn) clearBtn.style.display = (res.memories && res.memories.length > 0) ? '' : 'none';
    } catch(e) {
        container.innerHTML = '<div class="memory-empty">加载失败</div>';
    }
}

function renderMemories(memories) {
    var container = document.getElementById('memory-list');
    if (!container) return;

    if (!memories || memories.length === 0) {
        container.innerHTML = '<div class="memory-empty">二狗还没记住什么，聊天中自然提到的信息会被记住</div>';
        return;
    }

    var categoryLabels = {
        'user_fact': '个人信息',
        'preference': '偏好',
        'behavioral_pattern': '行为模式',
        'conversation_highlight': '对话亮点'
    };

    var html = memories.map(function(m) {
        var label = categoryLabels[m.category] || m.category;
        return '<div class="memory-item" data-id="' + m.id + '">' +
            '<div class="memory-content">' +
                '<span class="memory-category memory-cat-' + m.category + '">' + label + '</span>' +
                '<span class="memory-text">' + escapeHtml(m.content) + '</span>' +
            '</div>' +
            '<button class="memory-delete-btn" onclick="deleteMemory(\'' + m.id + '\')" title="删除">&times;</button>' +
        '</div>';
    }).join('');

    container.innerHTML = html;
}

function escapeHtml(text) {
    var d = document.createElement('div');
    d.textContent = text;
    return d.innerHTML;
}

async function deleteMemory(id) {
    try {
        var res = await API.deleteMemory(id);
        if (res.success) {
            var el = document.querySelector('.memory-item[data-id="' + id + '"]');
            if (el) el.remove();
            // Check if list is now empty
            var container = document.getElementById('memory-list');
            if (container && container.children.length === 0) {
                renderMemories([]);
                var clearBtn = document.getElementById('clear-all-memories-btn');
                if (clearBtn) clearBtn.style.display = 'none';
            }
            showToast('已删除', 'success');
        } else {
            showToast(res.message || '删除失败', 'error');
        }
    } catch(e) {
        showToast('删除失败', 'error');
    }
}

async function deleteAllMemories() {
    if (!confirm('确定要清空二狗的所有记忆吗？清空后二狗将不再记得关于你的任何信息。')) return;

    try {
        var res = await API.deleteAllMemories();
        if (res.success) {
            renderMemories([]);
            var clearBtn = document.getElementById('clear-all-memories-btn');
            if (clearBtn) clearBtn.style.display = 'none';
            showToast('已清空所有记忆', 'success');
        } else {
            showToast(res.message || '清空失败', 'error');
        }
    } catch(e) {
        showToast('清空失败', 'error');
    }
}

// Hook into settings loading: also load contacts and memories when settings are shown
var _origLoadSettingsData = loadSettingsData;
loadSettingsData = async function() {
    await _origLoadSettingsData();
    Contacts.loadContacts();
    loadMemories();
};

// BUG-1 fix: settings.js 加载完成后立即应用头像，避免 checkAuth 竞态
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', function() { applyAvatar(); });
} else {
    applyAvatar();
}
