// ========== 统一任务弹窗 ==========

var modalMode = 'view';  // 'view', 'edit', 'create'
var modalTaskId = null;
var modalTaskItem = null;

function shareCurrentTask() {
    if (!modalTaskId) return;
    if (typeof Friends !== 'undefined') {
        Friends.openShareModal('todo', modalTaskId);
    }
}

function handleModalUpgradeToWork() {
    if (!modalTaskId) return;
    if (modalTaskItem && modalTaskItem.upgradedToWork) {
        openWorkTaskFromTodo(modalTaskId);
    } else {
        upgradeTodoToWork(modalTaskId);
    }
}

function showAddModal() {
    openTaskModal('create', null, currentTab, 'important-urgent');
}

function showAddModalForQuadrant(quadrant) {
    openTaskModal('create', null, currentTab, quadrant);
}

function showTaskCard(taskId, element) {
    var item = allItems.find(function(i) { return i.id === taskId; });
    if (!item) return;
    openTaskModal('view', item);
}

function openTaskModal(mode, item, tab, quadrant) {
    modalMode = mode;
    modalTaskId = item ? item.id : null;
    modalTaskItem = item;

    var overlay = document.getElementById('task-modal-overlay');
    var titleInput = document.getElementById('modal-title');
    var contentInput = document.getElementById('modal-content');
    var progressInput = document.getElementById('modal-progress');
    var progressValue = document.getElementById('modal-progress-value');
    var dueDateInput = document.getElementById('modal-due-date');
    var assigneeInput = document.getElementById('modal-assignee');

    contentInput.style.height = '';

    if (item) {
        titleInput.value = item.text || '';
        contentInput.value = item.content || '';
        progressInput.value = item.progress || 0;
        progressValue.textContent = (item.progress || 0) + '%';
        progressInput.style.setProperty('--val', (item.progress || 0) + '%');
        dueDateInput.value = item.due_date || '';
        assigneeInput.value = item.assignee || '';
        setModalTab(item.tab || 'today');
        setModalQuadrant(item.quadrant || 'important-urgent');
    } else {
        titleInput.value = '';
        contentInput.value = '';
        progressInput.value = 0;
        progressValue.textContent = '0%';
        progressInput.style.setProperty('--val', '0%');
        dueDateInput.value = '';
        assigneeInput.value = '';
        setModalTab(tab || currentTab);
        setModalQuadrant(quadrant || 'important-urgent');
    }

    var createdAtEl = document.getElementById('modal-created-at');
    var completedAtEl = document.getElementById('modal-completed-at');
    if (item && item.created_at) {
        createdAtEl.querySelector('.meta-text').textContent = '创建于 ' + formatDateTime(item.created_at);
        createdAtEl.style.display = 'flex';
    } else {
        createdAtEl.style.display = mode === 'create' ? 'none' : 'flex';
        createdAtEl.querySelector('.meta-text').textContent = '创建于 --';
    }
    if (item && item.completed && item.completed_at) {
        completedAtEl.querySelector('.meta-text').textContent = '完成于 ' + formatDateTime(item.completed_at);
        completedAtEl.style.display = 'flex';
    } else {
        completedAtEl.style.display = 'none';
    }

    var changelogList = document.getElementById('modal-changelog');
    var changelog = (item && item.changelog) || [];
    if (changelog.length > 0) {
        var html = '';
        changelog.slice(-10).reverse().forEach(function(log) {
            html += '<div class="changelog-item">' +
                '<span class="changelog-time">' + formatShortTime(log.time) + '</span>' +
                '<span class="changelog-text">' + escapeHtml(log.label) + '</span>' +
            '</div>';
        });
        changelogList.innerHTML = html;
    } else {
        changelogList.innerHTML = '<div class="changelog-empty">暂无变更记录</div>';
    }
    changelogList.classList.remove('expanded');
    document.getElementById('changelog-toggle').classList.remove('expanded');

    document.getElementById('modal-meta-section').style.display = mode === 'create' ? 'none' : 'block';
    document.getElementById('modal-changelog-section').style.display = mode === 'create' ? 'none' : 'block';

    // Reminder row
    var reminderRow = document.getElementById('modal-reminder-row');
    var reminderPicker = document.getElementById('modal-reminder-picker');
    if (reminderPicker) reminderPicker.style.display = 'none';
    if (reminderRow && item) {
        reminderRow.style.display = 'flex';
        var reminderText = document.getElementById('modal-reminder-text');
        var reminderBtn = document.getElementById('modal-set-reminder-btn');
        if (item.next_reminder) {
            var r = item.next_reminder;
            var timeStr = r.remind_at;
            try {
                var d = new Date(r.remind_at);
                timeStr = d.toLocaleString('zh-CN', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' });
            } catch(e) {
                console.error('[modal] parse remind_at:', e);
            }
            var statusLabel = r.status === 'triggered' ? ' (已触发)' : '';
            reminderText.textContent = timeStr + statusLabel;
            reminderBtn.textContent = '修改';
        } else {
            reminderText.textContent = '无提醒';
            reminderBtn.textContent = '设置提醒';
        }
    } else if (reminderRow) {
        reminderRow.style.display = 'none';
    }

    setModalMode(mode);

    if (typeof syncDatePicker === 'function') syncDatePicker();

    // Load collaborators (SPEC-041)
    var collabSection = document.getElementById('modal-collab-section');
    if (collabSection) {
        if (item && item.id) {
            collabSection.style.display = 'block';
            if (typeof loadModalCollaborators === 'function') loadModalCollaborators(item.id);
        } else {
            collabSection.style.display = 'none';
        }
    }

        overlay.style.display = 'flex';

    if (mode === 'create' || mode === 'edit') {
        setTimeout(function() { titleInput.focus(); }, 100);
    }
}

function toggleChangelog() {
    var btn = document.getElementById('changelog-toggle');
    var list = document.getElementById('modal-changelog');
    btn.classList.toggle('expanded');
    list.classList.toggle('expanded');
}

function setModalMode(mode) {
    modalMode = mode;
    var isEditable = (mode === 'edit' || mode === 'create');

    var contentView = document.getElementById('modal-content-view');
    var contentEdit = document.getElementById('modal-content');

    if (mode === 'view') {
        contentView.innerHTML = AppUtils.escapeHtmlMultiline(contentEdit.value) || '<span class="placeholder">无详细描述</span>';
        contentView.style.display = 'block';
        contentEdit.style.display = 'none';
    } else {
        contentView.style.display = 'none';
        contentEdit.style.display = 'block';
    }

    document.getElementById('modal-title').readOnly = (mode === 'view');
    document.getElementById('modal-progress').disabled = false;
    document.getElementById('modal-assignee').readOnly = (mode === 'view');

    // Date picker: always clickable, view mode triggers auto-edit
    var datePicker = document.getElementById('smart-date-picker');
    if (datePicker) {
        datePicker.style.pointerEvents = 'auto';
        datePicker.style.opacity = isEditable ? '1' : '0.85';
    }

    document.querySelectorAll('#modal-tab-buttons .tab-btn').forEach(function(btn) {
        btn.disabled = false;
    });

    document.querySelectorAll('.q-option').forEach(function(opt) {
        opt.classList.toggle('disabled', mode === 'view');
        opt.style.pointerEvents = 'auto';
    });

    document.getElementById('header-edit-btn').style.display = (mode === 'view') ? 'inline-block' : 'none';
    document.getElementById('modal-footer-edit').style.display = (mode !== 'view') ? 'flex' : 'none';
    var shareBtn = document.getElementById('header-share-btn');
    if (shareBtn) shareBtn.style.display = (mode === 'view' && modalTaskId) ? 'inline-block' : 'none';
    var upgradeBtn = document.getElementById('header-upgrade-work-btn');
    if (upgradeBtn) {
        var showUpgrade = mode === 'view' && modalTaskId;
        upgradeBtn.style.display = showUpgrade ? 'inline-block' : 'none';
        if (modalTaskItem && modalTaskItem.upgradedToWork) {
            upgradeBtn.title = '打开工作任务';
            upgradeBtn.textContent = '已升级';
            upgradeBtn.classList.add('is-upgraded');
        } else {
            upgradeBtn.title = '升级到工作任务';
            upgradeBtn.textContent = '↗';
            upgradeBtn.classList.remove('is-upgraded');
        }
    }

    var saveBtn = document.getElementById('modal-save-btn');
    saveBtn.textContent = (mode === 'create') ? '创建' : '保存';
}

function setModalQuadrant(quadrant) {
    document.querySelectorAll('.q-option').forEach(function(opt) {
        opt.classList.remove('selected');
        if (opt.dataset.q === quadrant) {
            opt.classList.add('selected');
            opt.querySelector('input').checked = true;
        }
    });
}

function setModalTab(tab) {
    document.querySelectorAll('#modal-tab-buttons .tab-btn').forEach(function(btn) {
        btn.classList.toggle('selected', btn.dataset.tab === tab);
    });
}

function getModalTab() {
    var selectedBtn = document.querySelector('#modal-tab-buttons .tab-btn.selected');
    return selectedBtn ? selectedBtn.dataset.tab : 'today';
}

function switchToEditMode() {
    setModalMode('edit');
    document.getElementById('modal-title').focus();
}

// ========== Content View: click-to-edit vs select-to-pending ==========
var _pendingPopup = null;

function _removePendingPopup() {
    if (_pendingPopup && _pendingPopup.parentNode) {
        _pendingPopup.parentNode.removeChild(_pendingPopup);
    }
    _pendingPopup = null;
}

function _addSelectionToPending(text) {
    _removePendingPopup();
    var trimmed = text.length <= 80 ? text : text.substring(0, 80) + '...';
    if (typeof addPendingItemDirect === 'function') {
        addPendingItemDirect(trimmed);
    }
    window.getSelection().removeAllRanges();
}

function _showPendingPopup(text, x, y) {
    _removePendingPopup();
    _pendingPopup = document.createElement('div');
    _pendingPopup.className = 'selection-pending-popup';
    _pendingPopup.innerHTML = '<button class="selection-pending-btn">📋 收入待处理</button>';
    _pendingPopup.style.left = x + 'px';
    _pendingPopup.style.top = (y - 40) + 'px';
    document.body.appendChild(_pendingPopup);

    _pendingPopup.querySelector('button').addEventListener('click', function(ev) {
        ev.stopPropagation();
        _addSelectionToPending(text);
    });
}

document.addEventListener('DOMContentLoaded', function() {
    var contentView = document.getElementById('modal-content-view');
    var contentEdit = document.getElementById('modal-content');

    // View mode: click-to-edit or select-to-pending
    if (contentView) {
        contentView.addEventListener('mouseup', function(e) {
            if (modalMode !== 'view') return;

            var text = window.getSelection().toString().trim();
            if (text) {
                _showPendingPopup(text, e.clientX, e.clientY);
            } else {
                switchToEditMode();
                contentEdit.focus();
            }
        });

        contentView.addEventListener('dblclick', function() {
            if (modalMode === 'view') {
                _removePendingPopup();
                window.getSelection().removeAllRanges();
                switchToEditMode();
                contentEdit.focus();
            }
        });
    }

    // Edit mode: select-to-pending in textarea
    if (contentEdit) {
        contentEdit.addEventListener('mouseup', function(e) {
            if (modalMode !== 'edit' && modalMode !== 'create') return;
            var start = contentEdit.selectionStart;
            var end = contentEdit.selectionEnd;
            if (start === end) { _removePendingPopup(); return; }
            var text = contentEdit.value.substring(start, end).trim();
            if (text) {
                _showPendingPopup(text, e.clientX, e.clientY);
            }
        });
    }
});

// Close popup on outside click
document.addEventListener('mousedown', function(e) {
    if (_pendingPopup && !_pendingPopup.contains(e.target)) {
        _removePendingPopup();
    }
});

function closeTaskModal() {
    document.getElementById('task-modal-overlay').style.display = 'none';
    modalMode = 'view';
    modalTaskId = null;
    modalTaskItem = null;
}

function saveTask() {
    var title = document.getElementById('modal-title').value.trim();
    var content = document.getElementById('modal-content').value.trim();
    var progress = parseInt(document.getElementById('modal-progress').value) || 0;
    var dueDate = document.getElementById('modal-due-date').value || null;
    var assignee = document.getElementById('modal-assignee').value.trim();
    var tab = getModalTab();
    var quadrant = document.querySelector('.q-option.selected input').value;

    if (!title) {
        showToast('请输入任务标题', 'error');
        return;
    }

    var data = {
        text: title,
        tab: tab,
        quadrant: quadrant
    };
    if (content) data.content = content;
    if (progress > 0) data.progress = progress;
    if (dueDate) data.due_date = dueDate;
    if (assignee) data.assignee = assignee;

    if (modalMode === 'create') {
        API.createTodo(data)
            .then(result => {
                if (result.success) {
                    allItems.push(result.item);
                    updateCounts();
                    renderItems();
                    closeTaskModal();
                    showToast('任务已创建', 'success');
                } else {
                    showToast('创建失败: ' + (result.message || ''), 'error');
                }
            })
            .catch(err => {
                console.error('创建任务失败:', err);
                showToast('创建失败: ' + err.message, 'error');
            });
    } else {
        API.updateTodo(modalTaskId, data)
            .then(result => {
                if (result.success) {
                    allItems = allItems.map(function(i) {
                        return i.id === modalTaskId ? result.item : i;
                    });
                    updateCounts();
                    renderItems();
                    closeTaskModal();
                    showToast('已保存', 'success');
                } else {
                    showToast('保存失败', 'error');
                }
            });
    }
}

// 兼容旧函数
function closeTaskCard() {
    closeTaskModal();
}

function hideAddModal(e) {
    closeTaskModal();
}

function editTaskFromCard() {
    if (modalTaskItem) {
        switchToEditMode();
    }
}

// DOM 事件绑定
document.addEventListener('keydown', function(e) {
    if (e.key === 'Escape') {
        closeTaskModal();
    }
});

// Click-to-edit: title, assignee, progress
document.getElementById('modal-title').addEventListener('click', function() {
    if (modalMode === 'view') {
        switchToEditMode();
        this.focus();
    }
});

document.getElementById('modal-assignee').addEventListener('click', function() {
    if (modalMode === 'view') {
        switchToEditMode();
        this.focus();
    }
});

document.getElementById('modal-progress').addEventListener('click', function() {
    if (modalMode === 'view') {
        switchToEditMode();
    }
});

document.getElementById('modal-progress').addEventListener('input', function(e) {
    var val = e.target.value;
    document.getElementById('modal-progress-value').textContent = val + '%';
    e.target.style.setProperty('--val', val + '%');
});

document.querySelectorAll('.q-option').forEach(function(opt) {
    opt.addEventListener('click', function() {
        if (modalMode === 'view') switchToEditMode();
        document.querySelectorAll('.q-option').forEach(function(o) {
            o.classList.remove('selected');
        });
        opt.classList.add('selected');
        opt.querySelector('input').checked = true;
    });
});

document.querySelectorAll('#modal-tab-buttons .tab-btn').forEach(function(btn) {
    btn.addEventListener('click', function() {
        if (modalMode === 'view') switchToEditMode();
        document.querySelectorAll('#modal-tab-buttons .tab-btn').forEach(function(b) {
            b.classList.remove('selected');
        });
        btn.classList.add('selected');
    });
});

// Quadrant selector (grid style)
document.querySelectorAll('.quadrant-cell').forEach(function(opt) {
    opt.addEventListener('click', function() {
        document.querySelectorAll('.quadrant-cell').forEach(function(o) {
            o.classList.remove('selected');
        });
        opt.classList.add('selected');
        opt.querySelector('input').checked = true;
    });
});


// ========== Collaborator selector (SPEC-041) ==========
var _modalCollaborators = [];

function loadModalCollaborators(todoId) {
    if (!todoId || typeof _collabAPI === "undefined") return;
    _collabAPI.listCollaborators(todoId).then(function(data) {
        if (data.success) { _modalCollaborators = data.items || []; renderModalCollaborators(_modalCollaborators); }
    }).catch(function(e) {
        console.error('[modal] loadCollaborators:', e);
        renderModalCollaborators([]);
    });
}

function renderModalCollaborators(collabs) {
    var section = document.getElementById("modal-collab-section");
    if (!section) return;
    var html = "";
    if (collabs.length === 0) {
        html = "<div class=\"collab-empty\">点击下方按钮添加协作者</div><button class=\"collab-add-btn\" onclick=\"showAddCollaboratorDialog()\">+ 添加协作者</button>";
    } else {
        collabs.forEach(function(c) {
            var rl = c.role === "owner" ? "所有者" : "协作者";
            html += "<div class=\"collab-person\"><span class=\"collab-person-name\">" + escapeHtml(c.display_name) + "</span>";
            html += "<span class=\"collab-person-role\">" + rl + "</span></div>";
        });
        if (modalTaskItem && modalTaskItem.my_role === "owner") {
            html += "<button class=\"collab-add-btn\" onclick=\"showAddCollaboratorDialog()\">+ 添加协作者</button>";
        }
    }
    section.innerHTML = html;
}
function showAddCollaboratorDialog() {
    if (!modalTaskId) { showToast("请先保存任务", "error"); return; }
    API.getFriends().then(function(data) {
        if (!data.success || !data.items || data.items.length === 0) { showToast("暂无好友", "info"); return; }
        var existing = _modalCollaborators.map(function(c) { return c.user_id; });
        var avail = data.items.filter(function(f) { return existing.indexOf(f.id) === -1; });
        if (avail.length === 0) { showToast("所有好友已是协作者", "info"); return; }
        var ov = document.createElement("div");
        ov.className = "collab-picker-overlay";
        var lh = "";
        avail.forEach(function(f) {
            lh += "<div class=\"collab-picker-item\" data-fid=\"" + f.id + "\">" + escapeHtml(f.display_name || f.username) + "</div>";
        });
        ov.innerHTML = "<div class=\"collab-picker-dialog\"><div class=\"collab-picker-title\">选择协作者</div>" + lh + "<button class=\"collab-picker-close\">取消</button></div>";
        document.body.appendChild(ov);
        ov.addEventListener("click", function(e) {
            if (e.target === ov || e.target.classList.contains("collab-picker-close")) { ov.remove(); return; }
            var el = e.target.closest(".collab-picker-item");
            if (el) { pickCollaborator(el.dataset.fid, ov); }
        });
    });
}

function pickCollaborator(friendId, overlay) {
    _collabAPI.setCollaborator(modalTaskId, friendId).then(function(data) {
        if (data.success) {
            showToast(data.message || "已添加", "success");
            loadModalCollaborators(modalTaskId);
            if (overlay) overlay.remove();
            if (typeof loadItems === "function") loadItems();
        } else { showToast(data.message || "失败", "error"); }
    });
}

function removeCollabFromModal(userId) {
    _collabAPI.removeCollaborator(modalTaskId, userId).then(function(data) {
        if (data.success) {
            showToast(data.message || "已移除", "success");
            loadModalCollaborators(modalTaskId);
            if (typeof loadItems === "function") loadItems();
        } else { showToast(data.message || "失败", "error"); }
    });
}

// ========== 任务详情 - 设置提醒 ==========
function openSetReminderUI() {
    var picker = document.getElementById('modal-reminder-picker');
    if (!picker) return;
    picker.style.display = 'block';
    // Pre-fill with a reasonable default: next hour
    var input = document.getElementById('reminder-datetime-input');
    var now = new Date();
    now.setHours(now.getHours() + 1, 0, 0, 0);
    var local = new Date(now.getTime() - now.getTimezoneOffset() * 60000);
    input.value = local.toISOString().slice(0, 16);
    document.getElementById('reminder-repeat-select').value = '';
}

function cancelReminderPicker() {
    var picker = document.getElementById('modal-reminder-picker');
    if (picker) picker.style.display = 'none';
}

async function confirmSetReminder() {
    if (!modalTaskId) return;
    var input = document.getElementById('reminder-datetime-input');
    var repeatSelect = document.getElementById('reminder-repeat-select');
    if (!input.value) {
        showToast('请选择提醒时间', 'error');
        return;
    }
    // Convert local datetime to ISO with timezone offset
    var dt = new Date(input.value);
    if (dt <= new Date()) {
        showToast('提醒时间必须在未来', 'error');
        return;
    }
    var tzOffset = -dt.getTimezoneOffset();
    var sign = tzOffset >= 0 ? '+' : '-';
    var absOff = Math.abs(tzOffset);
    var offH = String(Math.floor(absOff / 60)).padStart(2, '0');
    var offM = String(absOff % 60).padStart(2, '0');
    var isoStr = dt.getFullYear() + '-' +
        String(dt.getMonth() + 1).padStart(2, '0') + '-' +
        String(dt.getDate()).padStart(2, '0') + 'T' +
        String(dt.getHours()).padStart(2, '0') + ':' +
        String(dt.getMinutes()).padStart(2, '0') + ':00' +
        sign + offH + ':' + offM;

    var data = {
        text: modalTaskItem ? modalTaskItem.text : '任务提醒',
        remind_at: isoStr,
        related_todo_id: modalTaskId,
    };
    var repeat = repeatSelect.value;
    if (repeat) data.repeat = repeat;

    try {
        var result = await API.createReminder(data);
        if (result && result.success) {
            cancelReminderPicker();
            showToast('提醒已设置', 'success');
            // Update the reminder display
            var reminderText = document.getElementById('modal-reminder-text');
            if (reminderText) {
                reminderText.textContent = dt.toLocaleString('zh-CN', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' });
            }
            var reminderBtn = document.getElementById('modal-set-reminder-btn');
            if (reminderBtn) reminderBtn.textContent = '修改';
            // Refresh tasks to show reminder badge
            if (typeof loadItems === 'function') loadItems();
        } else {
            showToast(result.message || '设置失败', 'error');
        }
    } catch(e) {
        showToast('设置提醒失败', 'error');
    }
}

