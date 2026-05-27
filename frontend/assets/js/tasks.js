
// ========== Collaboration helpers ==========
var _collabAPI = {
    setCollaborator: async function(todoId, friendId) {
        var resp = await fetch("/api/collaborate/todos/" + todoId, {
            method: "POST", headers: { "Content-Type": "application/json" },
            credentials: "same-origin", body: JSON.stringify({ friend_id: friendId })
        });
        return await resp.json();
    },
    removeCollaborator: async function(todoId, friendId) {
        var resp = await fetch("/api/collaborate/todos/" + todoId, {
            method: "DELETE", headers: { "Content-Type": "application/json" },
            credentials: "same-origin", body: JSON.stringify({ friend_id: friendId })
        });
        return await resp.json();
    },
    listCollaborators: async function(todoId) {
        var resp = await fetch("/api/collaborate/todos/" + todoId + "/collaborators", {credentials: "same-origin"});
        return await resp.json();
    },
    getPendingConfirmations: async function() {
        var resp = await fetch("/api/collaborate/confirmations/pending", {credentials: "same-origin"});
        return await resp.json();
    },
    respondConfirmation: async function(confId, response) {
        var resp = await fetch("/api/collaborate/confirmations/" + confId + "/respond", {
            method: "POST", headers: { "Content-Type": "application/json" },
            credentials: "same-origin", body: JSON.stringify({ response: response })
        });
        return await resp.json();
    },
    withdrawConfirmation: async function(confId) {
        var resp = await fetch("/api/collaborate/confirmations/" + confId + "/withdraw", {
            method: "POST", credentials: "same-origin"
        });
        return await resp.json();
    }
};

// ========== 任务渲染、CRUD、象限逻辑 ==========

var _loadingItems = false;

function loadItems() {
    if (_loadingItems) return;
    _loadingItems = true;
    API.getTodos()
        .then(data => {
            allItems = data.items || [];
            updateCounts();
            renderItems();
            loadPendingConfirmations();
            // Moment pool is loaded once per day; no need to refresh on task load
        })
        .catch(err => {
            console.error('[loadItems] error:', err);
            showToast('加载任务失败: ' + err.message, 'error');
        })
        .finally(() => {
            _loadingItems = false;
        });
}

function updateCounts() {
    var counts = { today: 0, week: 0, month: 0 };
    var qcounts = {
        'important-urgent': 0,
        'important-not-urgent': 0,
        'not-important-urgent': 0,
        'not-important-not-urgent': 0
    };
    allItems.forEach(function(item) {
        if (!item.completed && !item.deleted) {
            counts[item.tab] = (counts[item.tab] || 0) + 1;
            if (item.tab === currentTab) {
                if (!currentAssigneeFilter || item.assignee === currentAssigneeFilter) {
                    qcounts[item.quadrant] = (qcounts[item.quadrant] || 0) + 1;
                }
            }
        }
    });
    document.getElementById('count-today').textContent = counts.today;
    document.getElementById('count-week').textContent = counts.week;
    document.getElementById('count-month').textContent = counts.month;
    Object.keys(qcounts).forEach(function(q) {
        var el = document.getElementById('qcount-' + q);
        if (el) el.textContent = '(' + qcounts[q] + ')';
    });
}

function renderItems() {
    if (window.innerWidth <= 768) {
        renderFlatList();
    } else {
        renderMatrix();
    }
}

function renderMatrix() {
    var quadrants = ['important-urgent', 'important-not-urgent', 'not-important-urgent', 'not-important-not-urgent'];

    quadrants.forEach(function(q) {
        document.getElementById('items-' + q).innerHTML = '';
    });

    var completedHtml = '';
    var deletedHtml = '';

    allItems.forEach(function(item) {
        if (item.deleted) return;
        if (item.tab !== currentTab) return;
        if (item.completed) return;
        if (currentAssigneeFilter && item.assignee !== currentAssigneeFilter) return;

        var container = document.getElementById('items-' + item.quadrant);
        if (container) {
            container.innerHTML += createItemHtml(item);
        }
    });

    allItems.forEach(function(item) {
        if (item.deleted) return;
        if (item.completed) {
            completedHtml += createCompletedItemHtml(item);
        }
    });

    allItems.forEach(function(item) {
        if (item.deleted) {
            deletedHtml += createDeletedItemHtml(item);
        }
    });

    document.getElementById('completed-list').innerHTML = completedHtml || '<div class="empty-hint">暂无已完成任务</div>';
    document.getElementById('deleted-list').innerHTML = deletedHtml || '<div class="empty-hint">暂无已删除任务</div>';

    // 桌面端显示象限，隐藏扁平列表
    var flatView = document.getElementById('flat-list-view');
    if (flatView) flatView.style.display = 'none';

    attachTouchHandlers();
    attachTooltipHandlers();
    updateQuadrantScroll();
    updateButtonAnimations();
    renderAssigneeFilter();
    renderPendingItems();

    // 长按操作菜单 — 桌面端右键 (SPEC-047)
    if (typeof ActionSheet !== 'undefined') {
        var quadrants = ['important-urgent', 'important-not-urgent', 'not-important-urgent', 'not-important-not-urgent'];
        quadrants.forEach(function(q) {
            var container = document.getElementById('items-' + q);
            if (container) {
                ActionSheet.bindAll(container, '.task-item:not(.completed)', function(el) {
                    var id = el.dataset.id;
                    return [
                        { icon: '📤', label: '分享给好友', action: function() { Friends.openShareModal('todo', id); } },
                        { icon: '✏️', label: '编辑', action: function() { showTaskCard(id); } },
                        { icon: '🗑️', label: '删除', action: function() { deleteTask(id); }, danger: true }
                    ];
                });
            }
        });
    }
}

var _flatListShowCompleted = false;

function renderFlatList() {
    var flatView = document.getElementById('flat-list-view');
    if (!flatView) return;
    flatView.style.display = 'block';

    var QUADRANT_BADGES = {
        'important-urgent': { icon: '🔥', cls: 'flat-badge-q1' },
        'important-not-urgent': { icon: '🎯', cls: 'flat-badge-q2' },
        'not-important-urgent': { icon: '📥', cls: 'flat-badge-q3' },
        'not-important-not-urgent': { icon: '⚡', cls: 'flat-badge-q4' }
    };

    var pending = [];
    var done = [];

    allItems.forEach(function(item) {
        if (item.deleted) return;
        if (item.tab !== currentTab) return;
        if (currentAssigneeFilter && item.assignee !== currentAssigneeFilter) return;
        if (item.completed) {
            done.push(item);
        } else {
            pending.push(item);
        }
    });

    var html = '';

    if (pending.length === 0 && done.length === 0) {
        html = '<div class="flat-empty">暂无任务</div>';
    } else {
        pending.forEach(function(item) {
            var badge = QUADRANT_BADGES[item.quadrant] || { icon: '○', cls: 'flat-badge-q4' };
            var progress = item.progress || 0;
            var dueDateHtml = (item.due_date && typeof formatRelativeDate === 'function')
                ? '<span class="task-due ' + getDueDateClass(item.due_date) + '">' + formatRelativeDate(item.due_date) + '</span>'
                : '';
            var flatReminderHtml = getReminderBadgeHtml(item);
            var flatTriggeredClass = (item.next_reminder && item.next_reminder.status === 'triggered') ? ' task-item-triggered' : '';
            html += '<div class="flat-task-item' + flatTriggeredClass + '" data-id="' + item.id + '" onclick="showTaskCard(\'' + item.id + '\')">' +
                '<div class="task-checkbox" onclick="event.stopPropagation(); toggleComplete(\'' + item.id + '\')"></div>' +
                '<div class="flat-task-content">' +
                    '<div class="task-text">' + escapeHtml(item.text) + '</div>' +
                    ((dueDateHtml || flatReminderHtml) ? '<div class="flat-task-meta">' + flatReminderHtml + dueDateHtml + '</div>' : '') +
                '</div>' +
                '<span class="flat-task-badge ' + badge.cls + '">' + badge.icon + '</span>' +
            '</div>';
        });

        if (done.length > 0) {
            html += '<div class="flat-task-done-section">' +
                '<div class="flat-task-done-toggle" onclick="toggleFlatCompleted()">' +
                    '已完成 (' + done.length + ') ' + (_flatListShowCompleted ? '▲' : '▼') +
                '</div>';
            if (_flatListShowCompleted) {
                done.forEach(function(item) {
                    html += '<div class="flat-task-item completed" data-id="' + item.id + '" onclick="showTaskCard(\'' + item.id + '\')">' +
                        '<div class="task-checkbox checked" onclick="event.stopPropagation(); toggleComplete(\'' + item.id + '\')">✓</div>' +
                        '<div class="flat-task-content">' +
                            '<div class="task-text">' + escapeHtml(item.text) + '</div>' +
                        '</div>' +
                    '</div>';
                });
            }
            html += '</div>';
        }
    }

    flatView.innerHTML = html;

    // 长按操作菜单 (SPEC-047)
    if (typeof ActionSheet !== 'undefined') {
        ActionSheet.bindAll(flatView, '.flat-task-item:not(.completed)', function(el) {
            var id = el.dataset.id;
            return [
                { icon: '📤', label: '分享给好友', action: function() { Friends.openShareModal('todo', id); } },
                { icon: '✏️', label: '编辑', action: function() { showTaskCard(id); } },
                { icon: '🗑️', label: '删除', action: function() { deleteTask(id); }, danger: true }
            ];
        });
    }

    // 同步右侧边栏数据
    var completedHtml = '';
    var deletedHtml = '';
    allItems.forEach(function(item) {
        if (item.deleted) return;
        if (item.completed) completedHtml += createCompletedItemHtml(item);
    });
    allItems.forEach(function(item) {
        if (item.deleted) deletedHtml += createDeletedItemHtml(item);
    });
    document.getElementById('completed-list').innerHTML = completedHtml || '<div class="empty-hint">暂无已完成任务</div>';
    document.getElementById('deleted-list').innerHTML = deletedHtml || '<div class="empty-hint">暂无已删除任务</div>';

    updateButtonAnimations();
    renderAssigneeFilter();
    renderPendingItems();
}

function toggleFlatCompleted() {
    _flatListShowCompleted = !_flatListShowCompleted;
    renderFlatList();
}

function updateQuadrantScroll() {
    var quadrants = ['important-urgent', 'important-not-urgent', 'not-important-urgent', 'not-important-not-urgent'];
    quadrants.forEach(function(q) {
        var container = document.getElementById('items-' + q);
        if (container) {
            var taskCount = container.querySelectorAll('.task-item').length;
            if (taskCount > 15) {
                container.classList.add('has-scroll');
            } else {
                container.classList.remove('has-scroll');
            }
        }
    });
}

// Stored assignee counts for collapse callback
var _assigneeCounts = {};
var _chipResizeTimer = null;
var _chipResizeObserver = null;

// Use ResizeObserver to re-collapse chips whenever their container resizes.
// This handles: window resize, sidebar toggle, right panel expand/collapse, screen change.
function setupChipResizeObserver() {
    if (_chipResizeObserver) return;
    var filterEl = document.getElementById('assignee-filter');
    if (!filterEl || typeof ResizeObserver === 'undefined') return;
    _chipResizeObserver = new ResizeObserver(function() {
        clearTimeout(_chipResizeTimer);
        _chipResizeTimer = setTimeout(function() {
            if (Object.keys(_assigneeCounts).length >= 2) {
                renderAssigneeFilter();
            }
        }, 100);
    });
    _chipResizeObserver.observe(filterEl);
}

function renderAssigneeFilter() {
    var assignees = {};
    allItems.forEach(function(item) {
        if (item.deleted || item.completed || item.tab !== currentTab) return;
        if (item.assignee) {
            assignees[item.assignee] = (assignees[item.assignee] || 0) + 1;
        }
    });
    _assigneeCounts = assignees;

    var names = Object.keys(assignees);
    var filterEl = document.getElementById('assignee-filter');
    var chipsEl = document.getElementById('assignee-chips');
    if (!filterEl || !chipsEl) return;

    var tabsEl = document.querySelector('.matrix-tabs');
    if (names.length < 2) {
        filterEl.style.display = 'none';
        if (tabsEl) tabsEl.classList.remove('has-assignee-row');
        currentAssigneeFilter = null;
        return;
    }

    if (currentAssigneeFilter && !assignees[currentAssigneeFilter]) {
        currentAssigneeFilter = null;
    }

    filterEl.style.display = 'flex';
    if (tabsEl) tabsEl.classList.add('has-assignee-row');
    names.sort();

    // Build all chip HTML (all visible initially)
    var html = '<button class="assignee-chip' + (!currentAssigneeFilter ? ' active' : '') +
               '" onclick="filterByAssignee(null)">全部</button>';
    names.forEach(function(name) {
        var isActive = currentAssigneeFilter === name;
        html += '<button class="assignee-chip assignee-chip-auto' + (isActive ? ' active' : '') +
                '" data-name="' + escapeHtml(name) +
                '" onclick="filterByAssignee(this.getAttribute(\'data-name\'))">' +
                '<span class="chip-name"></span> <span class="chip-count">' + assignees[name] + '</span></button>';
    });
    chipsEl.innerHTML = html;
    // Safely set chip text via textContent to avoid XSS from assignee names
    chipsEl.querySelectorAll('.assignee-chip-auto .chip-name').forEach(function(el) {
        el.textContent = el.closest('.assignee-chip-auto').getAttribute('data-name');
    });

    // Collapse chips to fit available width
    collapseAssigneeChips();
    // Observe container resizes (sidebar toggle, window resize, screen change, etc.)
    setupChipResizeObserver();
}

function collapseAssigneeChips() {
    var chipsEl = document.getElementById('assignee-chips');
    if (!chipsEl) return;

    var allChips = Array.prototype.slice.call(chipsEl.querySelectorAll('.assignee-chip-auto'));
    if (allChips.length === 0) return;

    // Guard: if element not laid out yet, retry next frame
    if (chipsEl.offsetWidth === 0 || chipsEl.offsetParent === null) {
        requestAnimationFrame(function() { collapseAssigneeChips(); });
        return;
    }

    // === Direct width measurement approach ===
    // All chips are currently rendered visible. Measure each chip's width,
    // then determine how many fit in the container alongside a "+N more" button.

    var containerWidth = chipsEl.clientWidth;
    var gap = 5; // matches CSS gap

    // Measure the "全部" button width
    var allBtn = chipsEl.querySelector('.assignee-chip:not(.assignee-chip-auto)');
    var usedWidth = allBtn ? (allBtn.offsetWidth + gap) : 0;

    // Measure each person chip's width
    var chipWidths = [];
    var totalChipsWidth = usedWidth;
    for (var i = 0; i < allChips.length; i++) {
        var w = allChips[i].offsetWidth;
        chipWidths.push(w);
        totalChipsWidth += w + gap;
    }

    // If all chips fit, no collapse needed
    if (totalChipsWidth <= containerWidth + 2) return;

    // Need collapse. Estimate "+N more" button width (measure with temp element)
    var tempBtn = document.createElement('button');
    tempBtn.className = 'assignee-more-btn';
    tempBtn.style.visibility = 'hidden';
    tempBtn.style.position = 'absolute';
    tempBtn.textContent = '+' + allChips.length + ' 更多 ▾';
    chipsEl.appendChild(tempBtn);
    var moreBtnWidth = tempBtn.offsetWidth;
    chipsEl.removeChild(tempBtn);

    // Available width for person chips = container - 全部btn - moreBtn - gaps
    var available = containerWidth - usedWidth - moreBtnWidth - gap;

    // Count how many chips fit
    var visibleCount = 0;
    var runningWidth = 0;
    for (var i = 0; i < allChips.length; i++) {
        var needed = chipWidths[i] + (visibleCount > 0 ? gap : 0);
        if (runningWidth + needed <= available + 2) {
            visibleCount++;
            runningWidth += needed;
        } else {
            break;
        }
    }

    visibleCount = Math.max(1, visibleCount);

    // If all fit after measurement, no collapse
    if (visibleCount >= allChips.length) return;

    // Hide overflow chips
    var hiddenNames = [];
    for (var i = allChips.length - 1; i >= visibleCount; i--) {
        hiddenNames.unshift(allChips[i].getAttribute('data-name'));
        allChips[i].style.display = 'none';
    }

    if (hiddenNames.length === 0) return;

    // Add "+N more" dropdown button
    var assignees = _assigneeCounts;
    var hiddenActive = hiddenNames.indexOf(currentAssigneeFilter) !== -1;
    var moreBtn = document.createElement('button');
    moreBtn.className = 'assignee-more-btn' + (hiddenActive ? ' active' : '');
    moreBtn.onclick = function(e) { toggleAssigneeDropdown(e); };
    moreBtn.textContent = '+' + hiddenNames.length + ' 更多 ▾';
    chipsEl.appendChild(moreBtn);

    // Add dropdown panel (append to assignee-filter, not chips, to avoid overflow:hidden clipping)
    var filterEl = document.getElementById('assignee-filter');
    var dropdown = document.createElement('div');
    dropdown.className = 'assignee-dropdown';
    dropdown.id = 'assignee-dropdown';
    dropdown.style.display = 'none';
    hiddenNames.forEach(function(name) {
        var isActive = currentAssigneeFilter === name;
        var item = document.createElement('button');
        item.className = 'assignee-dropdown-item' + (isActive ? ' active' : '');
        item.textContent = name + ' ' + (assignees[name] || '');
        item.onclick = function() { filterByAssignee(name); closeAssigneeDropdown(); };
        dropdown.appendChild(item);
    });
    (filterEl || chipsEl).appendChild(dropdown);
}

function toggleAssigneeDropdown(e) {
    e.stopPropagation();
    var dropdown = document.getElementById('assignee-dropdown');
    if (!dropdown) return;
    var isVisible = dropdown.style.display !== 'none';
    dropdown.style.display = isVisible ? 'none' : 'block';
    if (!isVisible) {
        // Close on outside click
        setTimeout(function() {
            document.addEventListener('click', closeAssigneeDropdown, { once: true });
        }, 0);
    }
}

function closeAssigneeDropdown() {
    var dropdown = document.getElementById('assignee-dropdown');
    if (dropdown) dropdown.style.display = 'none';
}

function filterByAssignee(name) {
    currentAssigneeFilter = name;
    closeAssigneeDropdown();
    updateCounts();
    renderItems();
}

function getReminderBadgeHtml(item) {
    if (!item.next_reminder) return '';
    var r = item.next_reminder;
    var cls = r.status === 'triggered' ? 'reminder-badge triggered' : 'reminder-badge';
    var timeStr = '';
    try {
        var dt = new Date(r.remind_at);
        var h = dt.getHours();
        var m = dt.getMinutes();
        timeStr = (h < 10 ? '0' : '') + h + ':' + (m < 10 ? '0' : '') + m;
    } catch(e) {
        timeStr = '';
    }
    return '<span class="' + cls + '" title="提醒: ' + timeStr + '">🔔' + (timeStr ? timeStr : '') + '</span>';
}

function createItemHtml(item) {
    var progress = item.progress || 0;
    var progressRing = '<div class="progress-ring" style="--progress:' + progress + '" onclick="event.stopPropagation(); showProgressPopup(\'' + item.id + '\', this)" onmousedown="event.stopPropagation()" title="点击更新进度">' +
        '<span class="progress-ring-text">' + progress + '</span>' +
    '</div>';

    var assigneeHtml = item.assignee
        ? '<span class="task-assignee" title="' + escapeHtml(item.assignee) + '">' + escapeHtml(item.assignee) + '</span>'
        : '';

    var dueDateHtml = (item.due_date && typeof formatRelativeDate === 'function')
        ? '<span class="task-due ' + getDueDateClass(item.due_date) + '">' + formatRelativeDate(item.due_date) + '</span>'
        : '';

    var collabClass = item.is_collaborative ? " collaborative-task" : "";
    var collabMeta = "";
    if (item.is_collaborative) {
        var roleText = item.my_role === 'owner' ? '' : '来自 ';
        var nameText = item.collaborator_name || '协作';
        collabMeta = '<span class="collab-chip" title="协作任务">' + roleText + escapeHtml(nameText) + '</span>';
    }

    var reminderHtml = getReminderBadgeHtml(item);
    var triggeredClass = (item.next_reminder && item.next_reminder.status === 'triggered') ? ' task-item-triggered' : '';

    return '<div class="task-item' + collabClass + triggeredClass + '" data-id="' + item.id + '" onmousedown="startCustomDrag(event)">' +
        '<div class="drag-handle">⋮⋮</div>' +
        '<div class="task-checkbox" onclick="event.stopPropagation(); toggleComplete(\'' + item.id + '\')" onmousedown="event.stopPropagation()"></div>' +
        progressRing +
        '<div class="task-content" onclick="showTaskCard(\'' + item.id + '\', this.closest(\'.task-item\'))">' +
            '<div class="task-text">' + escapeHtml(item.text) + '</div>' +
        '</div>' +
        reminderHtml +
        dueDateHtml +
        collabMeta +
        assigneeHtml +
        '<button class="task-delete" onmousedown="event.stopPropagation()" onclick="event.stopPropagation(); deleteTask(\'' + item.id + '\')" title="删除">&times;</button>' +
    '</div>';
}

function getCompletionStatus(item) {
    if (!item.due_date || !item.completed_at) return '';
    var due = new Date(item.due_date);
    due.setHours(23, 59, 59);
    var completed = new Date(item.completed_at);
    var diff = Math.floor((due - completed) / (1000 * 60 * 60 * 24));
    if (diff > 0) return '<span class="status-early">提前' + diff + '天</span>';
    if (diff < 0) return '<span class="status-late">超期' + Math.abs(diff) + '天</span>';
    return '<span class="status-ontime">按时</span>';
}

function createCompletedItemHtml(item) {
    var assignee = item.assignee ? escapeHtml(item.assignee) : '--';
    var status = getCompletionStatus(item);
    return '<div class="task-item completed" data-id="' + item.id + '" onclick="showTaskCard(\'' + item.id + '\')">' +
        '<div class="task-checkbox checked" onclick="event.stopPropagation(); toggleComplete(\'' + item.id + '\')">✓</div>' +
        '<div class="completed-task-name">' + escapeHtml(item.text) + '</div>' +
        '<div class="completed-task-assignee">' + assignee + '</div>' +
        '<div class="completed-task-status">' + status + '</div>' +
    '</div>';
}

function createDeletedItemHtml(item) {
    return '<div class="task-item deleted" data-id="' + item.id + '">' +
        '<div class="deleted-task-name">' + escapeHtml(item.text) + '</div>' +
        '<div class="deleted-task-actions">' +
            '<button class="btn-restore" onclick="restoreTask(\'' + item.id + '\')" title="恢复">↩</button>' +
            '<button class="btn-permanent-delete" onclick="permanentDeleteTask(\'' + item.id + '\')" title="永久删除">×</button>' +
        '</div>' +
    '</div>';
}

// Snapshot helper for optimistic update rollback
function _cloneItems() {
    return allItems.map(function(i) { return Object.assign({}, i); });
}

// 核心移动函数 — 统一处理象限/Tab 移动（乐观更新 + 失败回滚）
function moveTask(itemId, updates, message) {
    var snapshot = _cloneItems();
    // Optimistic: apply locally first
    allItems = allItems.map(function(i) {
        return i.id === itemId ? Object.assign({}, i, updates) : i;
    });
    updateCounts();
    renderItems();

    API.updateTodo(itemId, updates).then(function(data) {
        if (data.success) {
            if (data.item) {
                allItems = allItems.map(function(i) {
                    return i.id === itemId ? data.item : i;
                });
            }
            showToast(message, 'success');
        } else {
            allItems = snapshot;
            updateCounts();
            renderItems();
            showToast('移动失败', 'error');
        }
    }).catch(function() {
        allItems = snapshot;
        updateCounts();
        renderItems();
        showToast('移动失败，请重试', 'error');
    });
}

function moveToQuadrant(itemId, newQuadrant) {
    var item = allItems.find(function(i) { return i.id === itemId; });
    if (!item || item.quadrant === newQuadrant) return;
    moveTask(itemId, { quadrant: newQuadrant }, '已移动到' + getQuadrantName(newQuadrant));
}

function moveToTab(itemId, newTab) {
    if (newTab === currentTab) return;
    moveTask(itemId, { tab: newTab }, '已移动到 ' + getTabName(newTab));
}

function moveToTabWithDefaultQuadrant(itemId, newTab) {
    if (newTab === currentTab) return;
    moveTask(itemId, { tab: newTab, quadrant: 'not-important-urgent' }, '已移动到 ' + getTabName(newTab) + ' - 待分类');
}

function moveToTabAndQuadrant(itemId, newTab, newQuadrant) {
    var item = allItems.find(function(i) { return i.id === itemId; });
    if (!item) return;
    if (item.tab === newTab && item.quadrant === newQuadrant) return;
    moveTask(itemId, { tab: newTab, quadrant: newQuadrant }, '已移动到 ' + getTabName(newTab) + ' - ' + getQuadrantName(newQuadrant));
}

function dropToTab(e, targetTab) {
    e.preventDefault();
    var itemId = e.dataTransfer.getData('text/plain');
    if (!itemId || targetTab === currentTab) return;
    moveTask(itemId, { tab: targetTab }, '已移动到 ' + getTabName(targetTab));
}

// 任务完成/进度
function toggleComplete(itemId) {
    var item = allItems.find(function(i) { return i.id === itemId; });
    if (!item) return;

    if (item.completed) {
        var snapshot = _cloneItems();
        // Optimistic: mark uncompleted locally
        allItems = allItems.map(function(i) {
            return i.id === itemId ? Object.assign({}, i, { completed: false, progress: 0 }) : i;
        });
        updateCounts();
        renderItems();

        API.updateTodo(itemId, { completed: false, progress: 0 })
            .then(data => {
                if (data.success) {
                    if (data.item) {
                        allItems = allItems.map(function(i) {
                            return i.id === itemId ? data.item : i;
                        });
                    }
                    showToast('已恢复', 'success');
                } else {
                    allItems = snapshot;
                    updateCounts();
                    renderItems();
                    showToast('恢复失败', 'error');
                }
            })
            .catch(function() {
                allItems = snapshot;
                updateCounts();
                renderItems();
                showToast('恢复失败，请重试', 'error');
            });
        return;
    }

    showProgressDialog(item);
}

function showProgressPopup(itemId, element) {
    var item = allItems.find(function(i) { return i.id === itemId; });
    if (item) {
        showProgressDialog(item);
    }
}

function showProgressDialog(item) {
    // T-095:此处原本是 ~90 行内联实现的 progress-dialog;
    // 现抽成通用组件 openProgressDialog (assets/js/dialogs.js),
    // tasks(本文件)和 work-table 共用同一弹窗框架,视觉/动画/键盘交互完全一致。
    // todo 的额外行为(living-line 联动、完成确认 + saveProgress(true))在回调里保留。
    openProgressDialog({
        label: item.text,
        currentProgress: item.progress || 0,
        onConfirm: function(p) {
            saveProgress(item.id, p, false);
        },
        onComplete: function() {
            window.AppUtils.showConfirm(
                '确定要将此任务标记为已完成吗？\n完成后任务将移至已完成列表。',
                function() {
                    saveProgress(item.id, 100, true);
                },
                { confirmText: '确定完成' }
            );
        },
    });
}

function saveProgress(itemId, progress, completed) {
    var snapshot = _cloneItems();

    // Capture card position BEFORE optimistic render (for patrol interaction)
    var patrolDetail = null;
    if (completed) {
        var item = allItems.find(function(i) { return i.id === itemId; });
        if (item) {
            var cardEl = document.querySelector('.task-item[data-id="' + itemId + '"]');
            var cardRect = cardEl ? cardEl.getBoundingClientRect() : null;
            var quadrant = item.quadrant;
            // Check if this is the last incomplete task in the quadrant
            var othersInQuadrant = allItems.filter(function(i) {
                return i.id !== itemId && i.quadrant === quadrant && i.tab === item.tab &&
                       !i.completed && !i.deleted;
            });
            patrolDetail = {
                itemId: itemId,
                quadrant: quadrant,
                isLastInQuadrant: othersInQuadrant.length === 0,
                cardRect: cardRect,
                cardEl: cardEl
            };
        }
    }

    // Optimistic: update progress/completed locally
    allItems = allItems.map(function(i) {
        return i.id === itemId ? Object.assign({}, i, { progress: progress, completed: completed }) : i;
    });
    updateCounts();
    renderItems();

    API.updateTodo(itemId, { progress: progress, completed: completed })
        .then(data => {
            if (data.success) {
                if (data.item) {
                    allItems = allItems.map(function(i) {
                        return i.id === itemId ? data.item : i;
                    });
                }
                showToast(completed ? '已完成' : '进度已更新', 'success');
                if (window.triggerLinePulse) {
                    window.triggerLinePulse('success');
                }
                // Notify patrol system of task completion
                if (patrolDetail) {
                    document.dispatchEvent(new CustomEvent('patrol:taskComplete', { detail: patrolDetail }));
                }
            } else {
                allItems = snapshot;
                updateCounts();
                renderItems();
                showToast('操作失败', 'error');
                if (window.triggerLinePulse) {
                    window.triggerLinePulse('error');
                }
            }
        })
        .catch(function() {
            allItems = snapshot;
            updateCounts();
            renderItems();
            showToast('操作失败，请重试', 'error');
            if (window.triggerLinePulse) {
                window.triggerLinePulse('error');
            }
        });
}

// 任务删除/恢复
function deleteTask(itemId) {
    window.AppUtils.showConfirm('确定要删除这个任务吗？', function() {
        var snapshot = _cloneItems();
        // Optimistic: mark deleted locally
        var item = allItems.find(function(i) { return i.id === itemId; });
        if (item) {
            item.deleted = true;
            item.deleted_at = new Date().toISOString();
        }
        updateCounts();
        renderItems();

        API.deleteTodo(itemId)
            .then(data => {
                if (data.success) {
                    showToast('已移入回收站', 'success');
                } else {
                    allItems = snapshot;
                    updateCounts();
                    renderItems();
                    showToast('删除失败', 'error');
                }
            })
            .catch(function() {
                allItems = snapshot;
                updateCounts();
                renderItems();
                showToast('删除失败，请重试', 'error');
            });
    }, { confirmText: '删除', danger: true });
}

function restoreTask(itemId) {
    var snapshot = _cloneItems();
    // Optimistic: restore locally
    var item = allItems.find(function(i) { return i.id === itemId; });
    if (item) {
        item.deleted = false;
        delete item.deleted_at;
    }
    updateCounts();
    renderItems();

    API.restoreTodo(itemId)
        .then(data => {
            if (data.success) {
                showToast('已恢复', 'success');
            } else {
                allItems = snapshot;
                updateCounts();
                renderItems();
                showToast('恢复失败', 'error');
            }
        })
        .catch(function() {
            allItems = snapshot;
            updateCounts();
            renderItems();
            showToast('恢复失败，请重试', 'error');
        });
}

function permanentDeleteTask(itemId) {
    window.AppUtils.showConfirm('永久删除后无法恢复，确定吗？', function() {
        API.permanentDeleteTodo(itemId)
            .then(data => {
                if (data.success) {
                    allItems = allItems.filter(function(i) { return i.id !== itemId; });
                    renderItems();
                    showToast('已永久删除', 'success');
                }
            });
    }, { confirmText: '永久删除', danger: true });
}

// ========== 待处理事项收集箱 ==========
function loadPendingItems() {
    try {
        return JSON.parse(localStorage.getItem('pendingItems') || '[]');
    } catch(e) { return []; }
}

function savePendingItems(items) {
    localStorage.setItem('pendingItems', JSON.stringify(items));
}

function addPendingItem() {
    var input = document.getElementById('pending-input');
    var text = input.value.trim();
    if (!text) return;
    addPendingItemDirect(text);
    input.value = '';
}

// Direct add (called from selection popup and input)
function addPendingItemDirect(text) {
    if (!text) return;
    var items = loadPendingItems();
    items.push({ id: Date.now().toString(), text: text, time: new Date().toISOString() });
    savePendingItems(items);
    renderPendingItems();
    showToast('已收入待处理', 'success');
}

function removePendingItem(id) {
    var items = loadPendingItems().filter(function(i) { return i.id !== id; });
    savePendingItems(items);
    renderPendingItems();
}

function promotePendingItem(id) {
    var items = loadPendingItems();
    var item = items.find(function(i) { return i.id === id; });
    if (!item) return;
    items = items.filter(function(i) { return i.id !== id; });
    savePendingItems(items);
    renderPendingItems();
    openTaskModal('create', null, currentTab, 'not-important-urgent');
    setTimeout(function() {
        document.getElementById('modal-title').value = item.text;
    }, 50);
}

function renderPendingItems() {
    var list = document.getElementById('pending-list');
    if (!list) return;
    var items = loadPendingItems();
    var section = document.getElementById('pending-section');
    if (section) {
        if (items.length === 0 && !section.classList.contains('user-toggled')) {
            section.classList.remove('expanded');
        } else if (items.length > 0 && !section.classList.contains('user-toggled')) {
            section.classList.add('expanded');
        }
    }
    if (items.length === 0) {
        list.innerHTML = '<div class="empty-hint">记录灵感，稍后处理</div>';
        return;
    }
    var html = '';
    items.forEach(function(item) {
        html += '<div class="pending-item">' +
            '<span class="pending-text">' + escapeHtml(item.text) + '</span>' +
            '<div class="pending-actions">' +
                '<button class="pending-btn promote" onclick="promotePendingItem(\'' + item.id + '\')" title="创建为任务">→</button>' +
                '<button class="pending-btn remove" onclick="removePendingItem(\'' + item.id + '\')" title="删除">×</button>' +
            '</div>' +
        '</div>';
    });
    list.innerHTML = html;
}

// 行内编辑任务标题
function editTask(itemId) {
    var item = allItems.find(function(i) { return i.id === itemId; });
    if (!item) return;

    var taskElement = document.querySelector('.task-item[data-id="' + itemId + '"]');
    if (!taskElement) return;

    var textElement = taskElement.querySelector('.task-text');
    if (!textElement || textElement.classList.contains('editing')) return;

    var originalText = item.text;

    var editInput = document.createElement('input');
    editInput.type = 'text';
    editInput.className = 'task-edit-input';
    editInput.value = originalText;

    textElement.classList.add('editing');
    textElement.innerHTML = '';
    textElement.appendChild(editInput);
    editInput.focus();
    editInput.select();

    function saveEdit() {
        var newText = editInput.value.trim();
        if (!newText || newText === originalText) {
            cancelEdit();
            return;
        }

        API.updateTodo(itemId, { text: newText })
            .then(data => {
                if (data.success) {
                    allItems = allItems.map(function(i) {
                        if (i.id === itemId) {
                            i.text = newText;
                        }
                        return i;
                    });
                    renderItems();
                    showToast('已更新', 'success');
                } else {
                    cancelEdit();
                    showToast('更新失败', 'error');
                }
            })
            .catch(function() {
                cancelEdit();
                showToast('更新失败', 'error');
            });
    }

    function cancelEdit() {
        textElement.classList.remove('editing');
        textElement.innerHTML = escapeHtml(originalText);
    }

    editInput.addEventListener('blur', saveEdit);
    editInput.addEventListener('keydown', function(e) {
        if (e.key === 'Enter') {
            e.preventDefault();
            editInput.blur();
        } else if (e.key === 'Escape') {
            e.preventDefault();
            editInput.removeEventListener('blur', saveEdit);
            cancelEdit();
        }
    });
}

// ========== Confirmation banners ==========
var _pendingConfirmations = [];

function loadPendingConfirmations() {
    _collabAPI.getPendingConfirmations().then(function(data) {
        if (data.success) {
            _pendingConfirmations = data.items || [];
            renderConfirmationBanners();
        }
    }).catch(function(e) { console.error('[tasks] loadConfirmations:', e); });
}

var sq = String.fromCharCode(39); // single quote helper

function renderConfirmationBanners() {
    var container = document.getElementById("confirmation-banners");
    if (!container) {
        var matrix = document.querySelector(".matrix-content") || document.querySelector(".main-content");
        if (!matrix) return;
        container = document.createElement("div");
        container.id = "confirmation-banners";
        matrix.insertBefore(container, matrix.firstChild);
    }
    if (_pendingConfirmations.length === 0) { container.innerHTML = ""; return; }
    var html = "";
    _pendingConfirmations.forEach(function(conf) {
        var actionText = conf.action === "complete" ? "完成" : "删除";
        var iName = conf.initiator_name || conf.initiator_username || "";
        var iText = conf.item_text || "(未知)";
        var meData = window._currentUser || {};
        var isInit = conf.initiated_by === (meData.id || "");
        html += "<div class=\"confirm-banner\">";
        html += "<div class=\"confirm-banner-text\">";
        html += "<strong>" + escapeHtml(iName) + "</strong> 请求" + actionText + ": <em>" + escapeHtml(iText) + "</em>";
        html += "</div><div class=\"confirm-banner-actions\">";
        if (isInit) {
            html += "<button class=\"confirm-reject\" onclick=\"withdrawConfirmation(" + sq + conf.id + sq + ")\">撤回</button>";
        } else {
            html += "<button class=\"confirm-approve\" onclick=\"respondToConfirmation(" + sq + conf.id + sq + "," + sq + "approve" + sq + ")\">同意</button>";
            html += "<button class=\"confirm-reject\" onclick=\"respondToConfirmation(" + sq + conf.id + sq + "," + sq + "reject" + sq + ")\">拒绝</button>";
        }
        html += "</div></div>";
    });
    container.innerHTML = html;
}

function respondToConfirmation(confId, response) {
    _collabAPI.respondConfirmation(confId, response).then(function(data) {
        if (data.success) {
            showToast(data.message || "已回应", "success");
            loadPendingConfirmations(); loadItems();
        } else { showToast(data.message || "失败", "error"); }
    });
}

function withdrawConfirmation(confId) {
    _collabAPI.withdrawConfirmation(confId).then(function(data) {
        if (data.success) {
            showToast(data.message || "已撤回", "success");
            loadPendingConfirmations();
        } else { showToast(data.message || "失败", "error"); }
    });
}
