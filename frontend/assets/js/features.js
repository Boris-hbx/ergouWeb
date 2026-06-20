// ========== 工具提示、手势、键盘快捷键等 ==========

// 毛玻璃 Tooltip
var glassTooltip = null;
var tooltipTimer = null;
var TOOLTIP_DELAY = 500;

function initTooltip() {
    if (!glassTooltip) {
        glassTooltip = document.createElement('div');
        glassTooltip.className = 'glass-tooltip';
        document.body.appendChild(glassTooltip);
    }
}

function showTooltip(text, x, y) {
    if (!glassTooltip) initTooltip();
    glassTooltip.textContent = text;

    var tooltipRect = glassTooltip.getBoundingClientRect();
    var maxX = window.innerWidth - 320;
    var maxY = window.innerHeight - 100;

    glassTooltip.style.left = Math.min(x, maxX) + 'px';
    glassTooltip.style.top = Math.min(y + 10, maxY) + 'px';
    glassTooltip.classList.add('visible');
}

function hideTooltip() {
    if (glassTooltip) {
        glassTooltip.classList.remove('visible');
    }
    if (tooltipTimer) {
        clearTimeout(tooltipTimer);
        tooltipTimer = null;
    }
}

function attachTooltipHandlers() {
    document.querySelectorAll('.task-text').forEach(function(textEl) {
        textEl.addEventListener('mouseenter', function(e) {
            var el = e.target;
            if (el.scrollWidth > el.clientWidth) {
                var rect = el.getBoundingClientRect();
                tooltipTimer = setTimeout(function() {
                    showTooltip(el.textContent, rect.left, rect.bottom);
                }, TOOLTIP_DELAY);
            }
        });

        textEl.addEventListener('mouseleave', hideTooltip);
        textEl.addEventListener('mousedown', hideTooltip);
    });
}

// 手势导航（Tab 间滑动切换）
var gestureStartX = 0;
var gestureStartY = 0;
var gestureHint = document.getElementById('gesture-hint');

document.querySelector('.eisenhower-matrix').addEventListener('touchstart', function(e) {
    if (e.target.closest('.task-item')) return;
    gestureStartX = e.touches[0].clientX;
    gestureStartY = e.touches[0].clientY;
}, { passive: true });

document.querySelector('.eisenhower-matrix').addEventListener('touchend', function(e) {
    if (e.target.closest('.task-item')) return;
    var dx = e.changedTouches[0].clientX - gestureStartX;
    var dy = e.changedTouches[0].clientY - gestureStartY;

    if (Math.abs(dx) > 80 && Math.abs(dx) > Math.abs(dy) * 2) {
        var tabs = ['today', 'week', 'month'];
        var currentIndex = tabs.indexOf(currentTab);

        if (dx < 0 && currentIndex < tabs.length - 1) {
            switchTab(tabs[currentIndex + 1]);
            showGestureHint('→ ' + getTabName(tabs[currentIndex + 1]));
        } else if (dx > 0 && currentIndex > 0) {
            switchTab(tabs[currentIndex - 1]);
            showGestureHint('← ' + getTabName(tabs[currentIndex - 1]));
        }
    }
}, { passive: true });

function showGestureHint(text) {
    if (!gestureHint) return;
    gestureHint.textContent = text;
    gestureHint.classList.add('visible');
    setTimeout(function() {
        gestureHint.classList.remove('visible');
    }, 1000);
}

// 版本管理浮动面板
function toggleVersionPanel() {
    var panel = document.getElementById('version-panel');
    var overlay = document.getElementById('version-overlay');
    var trigger = document.getElementById('version-trigger');

    if (panel.classList.contains('visible')) {
        closeVersionPanel();
    } else {
        panel.classList.add('visible');
        overlay.classList.add('visible');
        trigger.classList.add('expanded');
    }
}

function closeVersionPanel() {
    var panel = document.getElementById('version-panel');
    var overlay = document.getElementById('version-overlay');
    var trigger = document.getElementById('version-trigger');

    panel.classList.remove('visible');
    overlay.classList.remove('visible');
    trigger.classList.remove('expanded');
}

document.addEventListener('keydown', function(e) {
    if (e.key === 'Escape') {
        closeVersionPanel();
    }
});

// Sync Status Indicator
(function() {
    var syncStatus = document.getElementById('sync-status');
    var syncText = syncStatus.querySelector('.sync-text');
    var hideTimeout = null;

    window.showSyncStatus = function(status, message) {
        syncStatus.className = 'sync-status visible ' + status;
        syncText.textContent = message || (status === 'syncing' ? '保存中...' : status === 'synced' ? '已保存' : '保存失败');

        if (hideTimeout) clearTimeout(hideTimeout);
        if (status !== 'syncing') {
            hideTimeout = setTimeout(function() {
                syncStatus.classList.remove('visible');
            }, 2000);
        }
    };
})();

// Focus Mode
window.toggleFocusMode = function() {
    document.body.classList.toggle('focus-mode');
};

// Keyboard Shortcuts
(function() {
    var shortcutsModal = document.getElementById('shortcuts-modal');

    function isInputFocused() {
        var active = document.activeElement;
        return active && (active.tagName === 'INPUT' || active.tagName === 'TEXTAREA' || active.isContentEditable);
    }

    document.addEventListener('keydown', function(e) {
        if (isInputFocused() && e.key !== 'Escape') return;

        switch(e.key.toLowerCase()) {
            case 'n':
                e.preventDefault();
                if (typeof showAddModal === 'function') showAddModal();
                break;
            /* 's' shortcut removed — #search-input does not exist */
            case '1':
                e.preventDefault();
                if (typeof switchTab === 'function') switchTab('today');
                break;
            case '2':
                e.preventDefault();
                if (typeof switchTab === 'function') switchTab('week');
                break;
            case '3':
                e.preventDefault();
                if (typeof switchTab === 'function') switchTab('month');
                break;
            case 'f':
                e.preventDefault();
                if (typeof toggleFocusMode === 'function') toggleFocusMode();
                break;
            case '?':
                e.preventDefault();
                shortcutsModal.classList.toggle('visible');
                break;
            case 'escape':
                shortcutsModal.classList.remove('visible');
                document.body.classList.remove('focus-mode');
                var addModal = document.getElementById('add-modal');
                if (addModal) addModal.style.display = 'none';
                break;
        }
    });

    shortcutsModal.addEventListener('click', function(e) {
        if (e.target === shortcutsModal) {
            shortcutsModal.classList.remove('visible');
        }
    });
})();

// Sidebar / Pet Animation
(function() {
    var sidebar = document.getElementById('sidebar');
    var mainContent = document.getElementById('main-content');
    var pet = document.getElementById('pet');
    var navLinks = document.querySelector('.nav-links');
    var sidebarHint = document.getElementById('sidebar-hint');

    var isCollapsed = localStorage.getItem('sidebarCollapsed') === 'true';

    var targetY = window.innerHeight / 2;
    var currentY = targetY;
    var petHeight = 80;
    var sidebarWidth = 190;
    var isFollowing = false;
    var returnTimer = null;
    var isReturning = false;

    var closeDistance = 20;
    var rushEasing = 0.3;
    var gentleEasing = 0.1;

    function getAvoidZones() {
        var zones = [];
        if (navLinks && !isCollapsed) {
            var rect = navLinks.getBoundingClientRect();
            zones.push({ top: rect.top - 10, bottom: rect.bottom + 10 });
        }
        return zones;
    }

    function isInAvoidZone(y) {
        var zones = getAvoidZones();
        var petTop = y - petHeight / 2;
        var petBottom = y + petHeight / 2;
        for (var i = 0; i < zones.length; i++) {
            if (petBottom > zones[i].top && petTop < zones[i].bottom) {
                return true;
            }
        }
        return false;
    }

    function collapseSidebar() {
        isCollapsed = true;
        sidebar.classList.add('collapsed');
        mainContent.classList.add('expanded');
        pet.classList.add('at-edge');
        pet.classList.remove('at-sidebar');
        pet.classList.remove('slim');
        sidebarHint.classList.add('visible');
        localStorage.setItem('sidebarCollapsed', 'true');
        var quickAddBar = document.querySelector('.quick-add-bar');
        if (quickAddBar) quickAddBar.classList.add('expanded');
    }

    function expandSidebar() {
        isCollapsed = false;
        sidebar.classList.remove('collapsed');
        mainContent.classList.remove('expanded');
        pet.classList.remove('at-edge');
        pet.classList.add('at-sidebar');
        sidebarHint.classList.remove('visible');
        localStorage.setItem('sidebarCollapsed', 'false');
        var quickAddBar = document.querySelector('.quick-add-bar');
        if (quickAddBar) quickAddBar.classList.remove('expanded');
    }

    if (isCollapsed) {
        collapseSidebar();
    } else {
        pet.classList.add('at-sidebar');
    }
    document.documentElement.classList.remove('sidebar-will-collapse');

    pet.addEventListener('click', function() {
        if (isCollapsed) {
            expandSidebar();
        } else {
            collapseSidebar();
        }
    });

    document.addEventListener('mousemove', function(e) {
        var petX = isCollapsed ? 0 : sidebarWidth;
        var activeZone = petX + 100;

        if (e.clientX < activeZone) {
            isFollowing = true;
            isReturning = false;
            targetY = e.clientY;
            pet.classList.add('active');

            if (returnTimer) {
                clearTimeout(returnTimer);
                returnTimer = null;
            }
        } else if (isFollowing) {
            isFollowing = false;
            pet.classList.remove('active');
            pet.classList.remove('excited');

            returnTimer = setTimeout(function() {
                isReturning = true;
                targetY = window.innerHeight / 2;
            }, 600);
        }
    });

    function animatePet() {
        var distance = targetY - currentY;
        var absDistance = Math.abs(distance);
        var moveAmount = 0;

        if (isFollowing) {
            if (absDistance > closeDistance) {
                moveAmount = distance * rushEasing;
                pet.classList.add('excited');
            } else {
                moveAmount = distance * gentleEasing;
                pet.classList.remove('excited');
            }
        } else if (isReturning) {
            moveAmount = distance * 0.04;
            pet.classList.remove('excited');
        }

        currentY += moveAmount;

        var minY = petHeight / 2 + 48 + 10;
        var maxY = window.innerHeight - petHeight / 2 - 20;
        currentY = Math.max(minY, Math.min(maxY, currentY));

        if (isInAvoidZone(currentY)) {
            pet.classList.add('slim');
        } else {
            pet.classList.remove('slim');
        }

        pet.style.top = currentY + 'px';
        requestAnimationFrame(animatePet);
    }

    animatePet();

    window.addEventListener('resize', function() {
        if (!isFollowing) {
            targetY = window.innerHeight / 2;
        }
    });
})();

// Tab 按钮呼吸效果
(function() {
    var tabBtns = document.querySelectorAll('.tab-btn');
    tabBtns.forEach(function(btn) {
        btn.addEventListener('mouseenter', function() {
            btn.classList.remove('fade-out');
            btn.classList.add('breathing');
        });
        btn.addEventListener('mouseleave', function() {
            btn.classList.remove('breathing');
            btn.classList.add('fade-out');
            setTimeout(function() {
                btn.classList.remove('fade-out');
            }, 2000);
        });
    });
})();

// 时区更新
(function() {
    var weekdays = ['日', '一', '二', '三', '四', '五', '六'];

    function formatTz(date) {
        var m = date.getMonth() + 1;
        var d = date.getDate();
        var w = weekdays[date.getDay()];
        var h = date.getHours().toString().padStart(2, '0');
        var min = date.getMinutes().toString().padStart(2, '0');
        return {
            date: m + '/' + d + ' 周' + w,
            time: h + ':' + min
        };
    }

    function updateTimezones() {
        var bjDateEl = document.getElementById('tz-beijing-date');
        var bjTimeEl = document.getElementById('tz-beijing-time');
        var wtDateEl = document.getElementById('tz-waterloo-date');
        var wtTimeEl = document.getElementById('tz-waterloo-time');
        var vcDateEl = document.getElementById('tz-vancouver-date');
        var vcTimeEl = document.getElementById('tz-vancouver-time');
        if (!bjDateEl || !bjTimeEl || !wtDateEl || !wtTimeEl || !vcDateEl || !vcTimeEl) return;

        var now = new Date();

        var beijing = new Date(now.toLocaleString('en-US', { timeZone: 'Asia/Shanghai' }));
        var bjFmt = formatTz(beijing);
        bjDateEl.textContent = bjFmt.date;
        bjTimeEl.textContent = bjFmt.time;

        var waterloo = new Date(now.toLocaleString('en-US', { timeZone: 'America/Toronto' }));
        var wtFmt = formatTz(waterloo);
        wtDateEl.textContent = wtFmt.date;
        wtTimeEl.textContent = wtFmt.time;

        var vancouver = new Date(now.toLocaleString('en-US', { timeZone: 'America/Vancouver' }));
        var vcFmt = formatTz(vancouver);
        vcDateEl.textContent = vcFmt.date;
        vcTimeEl.textContent = vcFmt.time;
    }

    updateTimezones();
    setInterval(updateTimezones, 1000);
})();

// 天气状态（全局）
window.currentWeather = {
    type: 'cloudy',
    temp: '--',
    updated: false
};
