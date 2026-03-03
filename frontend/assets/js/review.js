// ========== 例行审视模块 ==========

var allReviews = [];
var reviewModalMode = null; // 'create' | 'edit'
var editingReviewId = null;
var selectedFrequency = 'daily';
var currentReviewFilter = 'daily';
var _jellyReviewFilter = null;

function _initReviewJelly() {
    if (_jellyReviewFilter || typeof JellyIndicator === 'undefined') return;
    _jellyReviewFilter = JellyIndicator.create({
        container: '#review-filter-buttons',
        itemSelector: '.review-filter-btn',
        activeClass: 'active',
        axis: 'x',
        padding: 0,
        pillClass: 'jelly-pill-review'
    });
    var activeBtn = document.querySelector('.review-filter-btn.active');
    if (activeBtn) _jellyReviewFilter.setPositionImmediate(activeBtn);
}

function loadReviews() {
    _initReviewJelly();
    // Re-sync jelly position when tab becomes visible (may have drifted while hidden)
    if (_jellyReviewFilter) {
        var activeBtn = document.querySelector('.review-filter-btn.active');
        if (activeBtn) _jellyReviewFilter.setPositionImmediate(activeBtn);
    }
    API.getReviews()
        .then(function(data) {
            allReviews = data.items || [];
            renderReviews();
        })
        .catch(function(err) {
            console.error('[loadReviews] error:', err);
            allReviews = [];
            renderReviews();
        });
}

function setReviewFilter(value) {
    currentReviewFilter = value;
    var activeBtn = null;
    document.querySelectorAll('.review-filter-btn').forEach(function(btn) {
        var isActive = btn.dataset.filter === value;
        btn.classList.toggle('active', isActive);
        if (isActive) activeBtn = btn;
    });
    if (_jellyReviewFilter && activeBtn) _jellyReviewFilter.moveTo(activeBtn);
    renderReviews();
}

function renderReviews() {
    var filterValue = currentReviewFilter || 'all';

    // Filter items
    var items = allReviews;
    if (['daily', 'weekly', 'monthly', 'yearly'].indexOf(filterValue) !== -1) {
        items = items.filter(function(i) { return i.frequency === filterValue; });
    }

    // Due section: overdue + due_today + due_soon
    var dueItems = items.filter(function(i) {
        return i.due_status === 'overdue' || i.due_status === 'duetoday' || i.due_status === 'duesoon';
    });
    var dueSection = document.getElementById('review-due-section');
    var dueList = document.getElementById('review-due-list');
    if (dueItems.length > 0) {
        dueSection.style.display = '';
        dueList.innerHTML = dueItems.map(function(item) {
            return renderReviewItem(item, true);
        }).join('');
    } else {
        dueSection.style.display = 'none';
    }

    // Group by frequency
    var groups = { daily: [], weekly: [], monthly: [], yearly: [] };
    items.forEach(function(item) {
        if (groups[item.frequency]) {
            groups[item.frequency].push(item);
        }
    });

    ['daily', 'weekly', 'monthly', 'yearly'].forEach(function(freq) {
        var groupEl = document.getElementById('review-group-' + freq);
        var listEl = document.getElementById('review-list-' + freq);
        if (!groupEl || !listEl) return;

        if (groups[freq].length === 0) {
            groupEl.style.display = 'none';
        } else {
            groupEl.style.display = '';
            listEl.innerHTML = groups[freq].map(function(item) {
                return renderReviewItem(item, false);
            }).join('');
        }
    });

    // 长按操作菜单 (SPEC-047)
    if (typeof ActionSheet !== 'undefined') {
        var reviewContainer = document.getElementById('review-view');
        if (reviewContainer) {
            ActionSheet.bindAll(reviewContainer, '.review-item:not(.completed):not(.paused)', function(el) {
                var id = el.dataset.id;
                return [
                    { icon: '📤', label: '分享给好友', action: function() { Friends.openShareModal('review', id); } },
                    { icon: '✏️', label: '编辑', action: function() { openReviewModal('edit', id); } },
                    { icon: '🗑️', label: '删除', action: function() { deleteReviewItem(id); }, danger: true }
                ];
            });
        }
    }
}

function renderReviewItem(item, isDueSection) {
    var isCompleted = item.due_status === 'completed';
    var isPaused = item.due_status === 'paused';
    var checkClass = isCompleted ? 'checked' : '';
    var itemClass = 'review-item';
    if (isCompleted) itemClass += ' completed';
    if (isPaused) itemClass += ' paused';
    if (item.notes) itemClass += ' has-notes';

    var statusClass = '';
    var statusText = item.due_label || '';
    if (item.due_status === 'overdue') statusClass = 'status-overdue';
    else if (item.due_status === 'duetoday') statusClass = 'status-today';
    else if (item.due_status === 'duesoon') statusClass = 'status-soon';
    else if (item.due_status === 'completed') statusClass = 'status-completed';
    else if (item.due_status === 'paused') statusClass = 'status-paused';

    var freqLabel = getFrequencyLabel(item);
    var notesIndicator = item.notes ? '<span class="review-notes-indicator">📝</span>' : '';
    var categoryHtml = item.category ? '<span class="review-category">' + escapeHtml(item.category) + '</span>' : '';
    var notesHtml = item.notes
        ? '<div class="review-notes-body">' + escapeHtml(item.notes) + '</div>'
        : '';

    return '<div class="' + itemClass + '" data-id="' + item.id + '">' +
        '<div class="review-item-row">' +
            '<div class="review-checkbox ' + checkClass + '" onclick="event.stopPropagation(); toggleReviewComplete(\'' + item.id + '\')">' +
                (isCompleted ? '✓' : '') +
            '</div>' +
            '<div class="review-item-content" onclick="toggleReviewNotes(this)">' +
                '<span class="review-item-text">' + escapeHtml(item.text) + '</span>' +
                categoryHtml + notesIndicator +
            '</div>' +
            '<span class="review-freq-label">' + freqLabel + '</span>' +
            '<span class="review-status ' + statusClass + '">' + statusText + '</span>' +
            '<div class="review-item-actions">' +
                '<button class="review-action-btn" onclick="event.stopPropagation(); openReviewModal(\'edit\', \'' + item.id + '\')" title="编辑">✎</button>' +
                '<button class="review-action-btn danger" onclick="event.stopPropagation(); deleteReviewItem(\'' + item.id + '\')" title="删除">×</button>' +
            '</div>' +
        '</div>' +
        notesHtml +
    '</div>';
}

function toggleReviewNotes(contentEl) {
    var item = contentEl.closest('.review-item');
    if (!item || !item.classList.contains('has-notes')) return;
    item.classList.toggle('expanded');
}

function getFrequencyLabel(item) {
    var config = item.frequency_config || {};
    var dayNames = ['', '周一', '周二', '周三', '周四', '周五', '周六', '周日'];
    switch (item.frequency) {
        case 'daily': return '每日';
        case 'weekly': return '每' + (dayNames[config.day_of_week] || '周一');
        case 'monthly': return '每月' + (config.day_of_month || 1) + '号';
        case 'yearly': return (config.month || 1) + '月' + (config.day || 1) + '日';
        default: return '';
    }
}

function toggleReviewComplete(id) {
    var item = allReviews.find(function(i) { return i.id === id; });
    if (!item) return;

    var apiCall = item.due_status === 'completed'
        ? API.uncompleteReview(id)
        : API.completeReview(id);

    apiCall.then(function(data) {
        if (data.success) {
            loadReviews();
            showToast(data.message || '已更新', 'success');
        }
    });
}


// ========== 新建/编辑弹窗 ==========

function openReviewModal(mode, id) {
    reviewModalMode = mode;
    editingReviewId = id || null;

    var overlay = document.getElementById('review-modal-overlay');
    overlay.style.display = 'flex';

    document.getElementById('review-modal-title').textContent =
        mode === 'create' ? '新建例行事项' : '编辑例行事项';

    if (mode === 'edit' && id) {
        var item = allReviews.find(function(i) { return i.id === id; });
        if (item) {
            document.getElementById('review-text').value = item.text;
            document.getElementById('review-category').value = item.category || '';
            document.getElementById('review-notes').value = item.notes || '';
            selectFrequency(item.frequency);
            fillFrequencyConfig(item.frequency_config || {});
        }
    } else {
        document.getElementById('review-text').value = '';
        document.getElementById('review-category').value = '';
        document.getElementById('review-notes').value = '';
        var defaultFreq = (currentReviewFilter && currentReviewFilter !== 'all') ? currentReviewFilter : 'daily';
        selectFrequency(defaultFreq);
    }
}

function closeReviewModal() {
    document.getElementById('review-modal-overlay').style.display = 'none';
    reviewModalMode = null;
    editingReviewId = null;
}

function selectFrequency(freq) {
    selectedFrequency = freq;
    document.querySelectorAll('.freq-btn').forEach(function(btn) {
        btn.classList.toggle('active', btn.dataset.freq === freq);
    });
    updateFrequencyConfig(freq);
}

function updateFrequencyConfig(freq) {
    var field = document.getElementById('freq-config-field');
    var label = document.getElementById('freq-config-label');
    var content = document.getElementById('freq-config-content');

    if (freq === 'daily') {
        field.style.display = 'none';
        return;
    }

    field.style.display = '';

    if (freq === 'weekly') {
        label.textContent = '每周几';
        var days = ['周一', '周二', '周三', '周四', '周五', '周六', '周日'];
        content.innerHTML = '<div class="dow-selector">' +
            days.map(function(name, i) {
                return '<button class="dow-btn" data-dow="' + (i + 1) + '" onclick="selectDow(' + (i + 1) + ')">' + name + '</button>';
            }).join('') + '</div>';
        selectDow(1);
    } else if (freq === 'monthly') {
        label.textContent = '每月几号';
        content.innerHTML = '<input type="number" id="freq-day-of-month" min="1" max="31" value="1" class="freq-input">';
    } else if (freq === 'yearly') {
        label.textContent = '日期';
        content.innerHTML = '<div class="yearly-config">' +
            '<select id="freq-month" class="freq-input freq-month">' +
            [1,2,3,4,5,6,7,8,9,10,11,12].map(function(m) {
                return '<option value="' + m + '">' + m + '月</option>';
            }).join('') +
            '</select>' +
            '<input type="number" id="freq-day" min="1" max="31" value="1" class="freq-input freq-day">' +
            '<span>日</span>' +
            '</div>';
    }
}

function selectDow(dow) {
    document.querySelectorAll('.dow-btn').forEach(function(btn) {
        btn.classList.toggle('active', parseInt(btn.dataset.dow) === dow);
    });
    // Store on a data attribute
    var selector = document.querySelector('.dow-selector');
    if (selector) selector.dataset.selected = dow;
}

function fillFrequencyConfig(config) {
    if (selectedFrequency === 'weekly' && config.day_of_week) {
        selectDow(config.day_of_week);
    } else if (selectedFrequency === 'monthly' && config.day_of_month) {
        var el = document.getElementById('freq-day-of-month');
        if (el) el.value = config.day_of_month;
    } else if (selectedFrequency === 'yearly') {
        if (config.month) {
            var m = document.getElementById('freq-month');
            if (m) m.value = config.month;
        }
        if (config.day) {
            var d = document.getElementById('freq-day');
            if (d) d.value = config.day;
        }
    }
}

function getFrequencyConfig() {
    var config = {};
    if (selectedFrequency === 'weekly') {
        var selector = document.querySelector('.dow-selector');
        config.day_of_week = parseInt(selector ? selector.dataset.selected : 1);
    } else if (selectedFrequency === 'monthly') {
        var el = document.getElementById('freq-day-of-month');
        config.day_of_month = parseInt(el ? el.value : 1);
    } else if (selectedFrequency === 'yearly') {
        var m = document.getElementById('freq-month');
        var d = document.getElementById('freq-day');
        config.month = parseInt(m ? m.value : 1);
        config.day = parseInt(d ? d.value : 1);
    }
    return config;
}

function saveReview() {
    var text = document.getElementById('review-text').value.trim();
    if (!text) {
        showToast('请输入事项名称', 'error');
        return;
    }

    var data = {
        text: text,
        frequency: selectedFrequency,
        frequency_config: getFrequencyConfig(),
        category: document.getElementById('review-category').value.trim(),
        notes: document.getElementById('review-notes').value.trim()
    };

    if (reviewModalMode === 'edit' && editingReviewId) {
        API.updateReview(editingReviewId, data)
            .then(function(res) {
                if (res.success) {
                    closeReviewModal();
                    loadReviews();
                    showToast('已更新', 'success');
                }
            });
    } else {
        API.createReview(data)
            .then(function(res) {
                if (res.success) {
                    closeReviewModal();
                    loadReviews();
                    showToast('已创建', 'success');
                }
            });
    }
}

function deleteReviewItem(id) {
    window.AppUtils.showConfirm('确定删除此例行事项？', function() {
        API.deleteReview(id)
            .then(function(res) {
                if (res.success) {
                    loadReviews();
                    showToast('已删除', 'success');
                }
            });
    }, { confirmText: '删除', danger: true });
}
