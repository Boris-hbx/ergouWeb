// ========== 例行任务面板 ==========

// About 弹窗
function openAbout() {
    document.getElementById('about-overlay').style.display = 'flex';
    document.body.style.overflow = 'hidden';
}

function closeAbout() {
    document.getElementById('about-overlay').style.display = 'none';
    document.body.style.overflow = '';
}

var versionClickCount = 0;
var versionClickTimer = null;

function copyVersion() {
    versionClickCount++;

    if (versionClickTimer) clearTimeout(versionClickTimer);
    versionClickTimer = setTimeout(function() {
        versionClickCount = 0;
    }, 2000);

    if (versionClickCount >= 5) {
        versionClickCount = 0;
        showToast('You found the secret!');
    } else if (versionClickCount === 1) {
        navigator.clipboard.writeText('Next v1.0.0 (2026.1.11)').then(function() {
            showToast('版本号已复制');
        }).catch(function() {
            showToast('复制失败');
        });
    } else if (versionClickCount >= 3) {
        showToast('再点 ' + (5 - versionClickCount) + ' 次...');
    }
}

// Security 安全声明弹窗
function openSecurity() {
    document.getElementById('security-overlay').style.display = 'flex';
    document.body.style.overflow = 'hidden';
}

function closeSecurity() {
    document.getElementById('security-overlay').style.display = 'none';
    document.body.style.overflow = '';
}

// ESC 关闭弹窗
document.addEventListener('keydown', function(e) {
    if (e.key === 'Escape') {
        if (document.getElementById('security-overlay').style.display !== 'none') {
            closeSecurity();
        } else if (document.getElementById('about-overlay').style.display !== 'none') {
            closeAbout();
        }
    }
});

// 例行任务面板
function toggleRoutinePanel() {
    var overlay = document.getElementById('routine-panel-overlay');
    if (overlay.style.display === 'none') {
        overlay.style.display = 'block';
        var btn = document.querySelector('.btn-routine');
        var panel = document.querySelector('.routine-panel');
        if (btn && panel) {
            var rect = btn.getBoundingClientRect();
            panel.style.top = (rect.bottom + 8) + 'px';
            panel.style.left = rect.left + 'px';
        }
        loadRoutines();
    } else {
        overlay.style.display = 'none';
    }
}

function loadRoutines() {
    API.getRoutines()
        .then(data => {
            routines = data.items || [];
            renderRoutines();
        })
        .catch(() => {
            routines = [];
            renderRoutines();
        });
}

function renderRoutines() {
    var list = document.getElementById('routine-list');
    if (routines.length === 0) {
        list.innerHTML = '<div class="routine-empty">暂无例行任务</div>';
        updateButtonAnimations();
        return;
    }
    var html = '';
    routines.forEach(function(r) {
        var checkedClass = r.completed_today ? 'checked' : '';
        var completedClass = r.completed_today ? 'completed' : '';
        var isCollab = r.is_collaborative;
        var collabIndicator = '';
        if (isCollab) {
            var ownerLabel = r.owner_name ? '与 ' + escapeHtml(r.owner_name) + ' 一起' : '协作';
            collabIndicator = '<span class="routine-collab-chip" style="font-size:10px;color:var(--primary-color,#5b6abf);margin-left:4px;opacity:0.8;">' + ownerLabel + '</span>';
        }
        html += '<div class="routine-item ' + completedClass + '" data-id="' + r.id + '">' +
            '<div class="routine-checkbox ' + checkedClass + '" onclick="toggleRoutine(\'' + r.id + '\')">' +
                (r.completed_today ? '✓' : '') +
            '</div>' +
            '<span class="routine-text">' + escapeHtml(r.text) + collabIndicator + '</span>' +
            '<button class="routine-delete" onclick="deleteRoutine(\'' + r.id + '\')">&times;</button>' +
        '</div>';
    });
    list.innerHTML = html;
    updateButtonAnimations();

    // 长按操作菜单 (SPEC-047)
    if (typeof ActionSheet !== 'undefined') {
        ActionSheet.bindAll(list, '.routine-item:not(.completed)', function(el) {
            var id = el.dataset.id;
            return [
                { icon: '📤', label: '分享给好友', action: function() { Friends.openShareModal('routine', id); } },
                { icon: '🗑️', label: '删除', action: function() { deleteRoutine(id); }, danger: true }
            ];
        });
    }
}

function addRoutine() {
    var input = document.getElementById('routine-input');
    var text = input.value.trim();
    if (!text) return;

    API.createRoutine(text)
        .then(data => {
            if (data.success) {
                input.value = '';
                loadRoutines();
                showToast('例行任务已添加');
            }
        });
}

function toggleRoutine(id) {
    // Optimistic UI: immediately toggle the local state
    var idx = routines.findIndex(function(r) { return r.id === id; });
    if (idx !== -1) {
        routines[idx].completed_today = !routines[idx].completed_today;
        renderRoutines();
    }

    API.toggleRoutine(id)
        .then(data => {
            if (data.success) {
                // Update from server response to ensure consistency
                if (data.item) {
                    var si = routines.findIndex(function(r) { return r.id === id; });
                    if (si !== -1) {
                        routines[si] = data.item;
                        renderRoutines();
                    }
                }
            } else {
                // Revert optimistic update on failure
                loadRoutines();
                showToast(data.message || '操作失败', 'error');
            }
        })
        .catch(err => {
            // Revert optimistic update on error
            loadRoutines();
            console.error('toggleRoutine error:', err);
        });
}

function deleteRoutine(id) {
    if (!confirm('确定删除此例行任务？')) return;
    API.deleteRoutine(id)
        .then(data => {
            if (data.success) {
                loadRoutines();
                showToast('例行任务已删除');
            }
        });
}

// 按钮动效更新
function updateButtonAnimations() {
    var routineBtn = document.querySelector('.btn-routine');
    if (routineBtn) {
        var pendingRoutines = routines.filter(function(r) { return !r.completed_today; }).length;
        var totalRoutines = routines.length;

        if (totalRoutines > 0 && pendingRoutines > 0) {
            routineBtn.classList.add('has-pending');
            var completion = (totalRoutines - pendingRoutines) / totalRoutines;
            routineBtn.classList.toggle('speed-fast', completion < 0.5);
            routineBtn.classList.toggle('speed-slow', completion >= 0.5);
        } else {
            routineBtn.classList.remove('has-pending', 'speed-fast', 'speed-slow');
        }
    }

    var todayBtn = document.querySelector('.matrix-tab[data-tab="today"]');
    if (todayBtn) {
        var todayItems = allItems.filter(function(i) { return i.tab === 'today' && !i.deleted; });
        var pendingToday = todayItems.filter(function(i) { return !i.completed; }).length;
        var totalToday = todayItems.length;

        if (totalToday > 0 && pendingToday > 0) {
            todayBtn.classList.add('has-pending');
            var completion = (totalToday - pendingToday) / totalToday;
            todayBtn.classList.toggle('speed-fast', completion < 0.5);
            todayBtn.classList.toggle('speed-slow', completion >= 0.5);
        } else {
            todayBtn.classList.remove('has-pending', 'speed-fast', 'speed-slow');
        }
    }
}
