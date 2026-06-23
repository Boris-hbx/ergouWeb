// ========== 认证检查 ==========

// 页面加载时检查登录状态
(async function checkAuth() {
    // login.html 不需要检查
    if (window.location.pathname === '/login.html') return;
    try {
        var resp = await fetch('/api/auth/me', { credentials: 'same-origin' });
        if (!resp.ok) {
            window.location.href = '/login.html';
            return;
        }
        var data = await resp.json();
        if (data.success && data.user) {
            // Update avatar with first letter of display name or username
            var name = data.user.display_name || data.user.username || '';
            window._userInitial = name ? name.charAt(0).toUpperCase() : 'B';
            var avatarTextEl = document.getElementById('avatar-text');
            if (avatarTextEl && name) {
                avatarTextEl.textContent = window._userInitial;
            }
            // Sync server avatar to localStorage, then apply
            if (data.user.avatar) {
                localStorage.setItem('userAvatar', data.user.avatar);
            } else {
                localStorage.removeItem('userAvatar');
            }
            if (typeof applyAvatar === 'function') applyAvatar();

            // Store current user info for cross-module use
            window._currentUser = data.user;

            // Show admin nav link for admin/owner roles
            var adminNavLink = document.getElementById('admin-nav-link');
            if (adminNavLink) {
                var userRole = data.user.role || 'user';
                adminNavLink.style.display = (userRole === 'admin' || userRole === 'owner') ? '' : 'none';
            }

            // Handle account status
            window._userStatus = data.user.status || 'active';
            if (window._userStatus === 'pending') {
                // Show pending banner
                var banner = document.getElementById('pending-banner');
                if (banner) banner.style.display = '';
                // Hide FAB and 阿宝 entry
                var fab = document.getElementById('mobile-fab');
                if (fab) fab.style.display = 'none';
                var abaoBtn = document.querySelector('.nav-link[onclick*="abao"], .sidebar-btn[onclick*="toggleAbao"]');
                if (abaoBtn) abaoBtn.style.display = 'none';
                var mobileAbao = document.querySelector('.mobile-nav-item[onclick*="toggleAbao"]');
                if (mobileAbao) mobileAbao.style.display = 'none';
            }

            // Handle guest mode
            if (window._userStatus === 'guest') {
                window._guestAiRemaining = data.user.ai_calls_remaining != null ? data.user.ai_calls_remaining : 21;
                // Show guest banner
                var guestBanner = document.getElementById('guest-banner');
                if (guestBanner) guestBanner.style.display = '';
                var aiCount = document.getElementById('guest-ai-count');
                if (aiCount) aiCount.textContent = window._guestAiRemaining;
                // Hide friend/share related entries
                var inboxBell = document.getElementById('inbox-bell-wrapper');
                if (inboxBell) inboxBell.style.display = 'none';
            }

            // Mobile install banner: show on mobile browsers not in standalone mode
            checkInstallBanner();
        }
    } catch(e) {
        // Network error — stay on page, will work offline with cached data
    }
})();

// ========== 手机安装引导 ==========

function checkInstallBanner() {
    // Only on mobile, not already installed (standalone), and not dismissed
    var isMobile = /Android|iPhone|iPad|iPod/i.test(navigator.userAgent);
    var isStandalone = window.matchMedia('(display-mode: standalone)').matches || window.navigator.standalone;
    if (!isMobile || isStandalone) return;

    // Show settings install section
    var settingsInstall = document.getElementById('settings-install-section');
    if (settingsInstall) settingsInstall.style.display = '';
    var isIOS = /iPad|iPhone|iPod/.test(navigator.userAgent);
    var iosSteps = document.getElementById('settings-install-ios');
    var androidSteps = document.getElementById('settings-install-android');
    if (iosSteps) iosSteps.style.display = isIOS ? '' : 'none';
    if (androidSteps) androidSteps.style.display = isIOS ? 'none' : '';

    // Show banner if not dismissed
    if (!localStorage.getItem('install_banner_dismissed')) {
        var banner = document.getElementById('install-banner');
        if (banner) banner.style.display = '';
    }
}

function dismissInstallBanner() {
    var banner = document.getElementById('install-banner');
    if (banner) banner.style.display = 'none';
    localStorage.setItem('install_banner_dismissed', '1');
}

function showInstallGuide() {
    dismissInstallBanner();
    switchPage('settings');
    // Scroll to install section after a tick
    setTimeout(function() {
        var el = document.getElementById('settings-install-section');
        if (el) el.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }, 200);
}

// ========== 全局状态 & 初始化 ==========

var currentTab = 'today';
var allItems = [];
var showCompleted = true;  // 默认显示已完成任务
var draggedItem = null;
var draggedItemQuadrant = null;  // 拖拽任务原本所在的象限
var routines = [];  // 每日例行任务
var currentAssigneeFilter = null;  // null = 全部, 'name' = 指定人
var currentPage = 'todo';  // 'todo' | 'review' | 'english' | 'life' | 'settings' | 'admin'

// ========== Jelly Pill Indicators ==========
var _jellyMobileNav = null;
var _jellySidebar = null;
var _jellyMatrixTab = null;

(function initJellyIndicators() {
    if (typeof JellyIndicator === 'undefined') return;

    // Mobile bottom nav
    _jellyMobileNav = JellyIndicator.create({
        container: '#mobile-bottom-nav',
        itemSelector: '.mobile-nav-item',
        activeClass: 'active',
        axis: 'x',
        skipIndices: [2],
        padding: 4,
        pillClass: 'jelly-pill-mobile'
    });

    // Desktop sidebar nav
    _jellySidebar = JellyIndicator.create({
        container: '.nav-links',
        itemSelector: '.nav-link',
        activeClass: 'active',
        axis: 'y',
        padding: 0,
        pillClass: 'jelly-pill-sidebar'
    });

    // Matrix tabs (Today/Week/Month) — smooth slide, no bounce
    _jellyMatrixTab = JellyIndicator.create({
        container: '.matrix-tabs',
        itemSelector: '.matrix-tab',
        activeClass: 'active',
        axis: 'x',
        padding: 0,
        pillClass: 'jelly-pill-matrix',
        bounce: false
    });

    // Initial positioning (no animation) — only if element is visible
    setTimeout(function() {
        var activeMobileNav = document.querySelector('.mobile-nav-item.active');
        if (activeMobileNav && _jellyMobileNav && activeMobileNav.offsetParent !== null) {
            _jellyMobileNav.setPositionImmediate(activeMobileNav);
        }

        var activeSidebarLink = document.querySelector('.nav-link.active');
        if (activeSidebarLink && _jellySidebar && activeSidebarLink.offsetParent !== null) {
            _jellySidebar.setPositionImmediate(activeSidebarLink);
        }

        var activeMatrixTab = document.querySelector('.matrix-tab.active');
        if (activeMatrixTab && _jellyMatrixTab) _jellyMatrixTab.setPositionImmediate(activeMatrixTab);
    }, 100);
})();

// Tab 切换
function switchTab(tab) {
    currentTab = tab;
    // Reset assignee filter if the selected person doesn't exist in new tab
    if (currentAssigneeFilter) {
        var hasAssignee = allItems.some(function(item) {
            return !item.deleted && !item.completed && item.tab === tab && item.assignee === currentAssigneeFilter;
        });
        if (!hasAssignee) currentAssigneeFilter = null;
    }
    var activeMatrixTab = null;
    document.querySelectorAll('.matrix-tab').forEach(function(t) {
        t.classList.remove('active');
        if (t.dataset.tab === tab) {
            t.classList.add('active');
            activeMatrixTab = t;
        }
    });
    if (_jellyMatrixTab && activeMatrixTab) _jellyMatrixTab.moveTo(activeMatrixTab);
    // 更新 modal 中的时间段按钮
    document.querySelectorAll('#modal-tab-buttons .tab-btn').forEach(function(btn) {
        btn.classList.toggle('selected', btn.dataset.tab === tab);
    });
    updateCounts();
    renderItems();
}

// 象限折叠功能
function toggleQuadrant(header) {
    var quadrant = header.closest('.quadrant');
    quadrant.classList.toggle('collapsed');
    saveQuadrantState();
}

function saveQuadrantState() {
    var states = {};
    document.querySelectorAll('.quadrant').forEach(function(q) {
        states[q.dataset.quadrant] = q.classList.contains('collapsed');
    });
    localStorage.setItem('quadrantStates', JSON.stringify(states));
}

function loadQuadrantState() {
    var saved = localStorage.getItem('quadrantStates');
    if (!saved) return;
    var states = JSON.parse(saved);
    document.querySelectorAll('.quadrant').forEach(function(q) {
        var key = q.dataset.quadrant;
        if (states[key] !== undefined) {
            q.classList.toggle('collapsed', states[key]);
        }
    });
}

// 页面切换 (Todo ↔ 例行审视 ↔ 收件箱 ↔ 设置)
function returnCurrentModuleHome(page) {
    if (typeof Observability !== 'undefined') Observability.addBreadcrumb('nav.home', page);

    switch (page) {
        case 'todo':
            if (typeof loadItems === 'function') loadItems();
            if (typeof loadRoutines === 'function') loadRoutines();
            break;
        case 'review':
            if (typeof loadReviews === 'function') loadReviews();
            break;
        case 'english':
            if (typeof English !== 'undefined') {
                if (English.showList) English.showList();
                else if (English.init) English.init();
            }
            break;
        case 'life':
            if (typeof Life !== 'undefined' && Life.showHub) Life.showHub();
            break;
        case 'work':
            if (typeof Work !== 'undefined' && Work.showHub) Work.showHub();
            break;
        case 'settings':
            if (typeof loadSettingsData === 'function') loadSettingsData();
            if (typeof Friends !== 'undefined' && Friends.loadFriendsData) Friends.loadFriendsData();
            break;
        case 'admin':
            if (typeof AdminPanel !== 'undefined') {
                if (AdminPanel.showSection) AdminPanel.showSection('overview');
                else if (AdminPanel.init) AdminPanel.init();
            }
            break;
    }
}

function switchPage(page) {
    if (currentPage === page) {
        returnCurrentModuleHome(page);
        return;
    }
    currentPage = page;
    // Breadcrumb: navigation
    if (typeof Observability !== 'undefined') Observability.addBreadcrumb('nav', page);

    // 更新桌面端 nav-link
    var activeSidebarLink = null;
    document.querySelectorAll('.nav-link').forEach(function(el) {
        var isActive = el.dataset.page === page;
        el.classList.toggle('active', isActive);
        if (isActive) activeSidebarLink = el;
    });
    if (_jellySidebar) {
        if (page === 'settings') {
            _jellySidebar.moveTo(null); // settings not in sidebar
        } else if (activeSidebarLink) {
            _jellySidebar.moveTo(activeSidebarLink);
        }
    }

    // 切换视图显示
    document.getElementById('todo-view').style.display = page === 'todo' ? '' : 'none';
    document.getElementById('review-view').style.display = page === 'review' ? '' : 'none';
    document.getElementById('english-view').style.display = page === 'english' ? '' : 'none';
    var lifeView = document.getElementById('life-view');
    if (lifeView) lifeView.style.display = page === 'life' ? '' : 'none';
    var workView = document.getElementById('work-view');
    if (workView) workView.style.display = page === 'work' ? '' : 'none';
    // T-100:离开工作模块时自动关闭任务详情抽屉(否则 fixed 抽屉会盖到其它模块)
    if (page !== 'work' && typeof WorkDetail !== 'undefined' && WorkDetail.isOpen && WorkDetail.isOpen()) {
        WorkDetail.closeDetail();
    }
    if (page !== 'work' && window.location.pathname.indexOf('/insight-factory') === 0) {
        window.history.pushState(null, '', '/');
    }
    document.getElementById('settings-view').style.display = page === 'settings' ? '' : 'none';
    var adminView = document.getElementById('admin-view');
    if (adminView) adminView.style.display = page === 'admin' ? '' : 'none';

    // Mobile FAB: show only on todo page
    var fab = document.getElementById('mobile-fab');
    if (fab) fab.classList.toggle('hidden', page !== 'todo');

    // Learn FAB: show only on english page
    var learnFab = document.getElementById('learn-fab');
    if (learnFab) learnFab.style.display = page === 'english' ? '' : 'none';

    // 通过 body class 控制设置页面
    document.body.classList.toggle('page-settings', page === 'settings');

    if (page === 'todo') {
        if (typeof loadItems === 'function') loadItems();
    }
    if (page === 'review' && typeof loadReviews === 'function') {
        loadReviews();
    }
    if (page === 'english' && typeof English !== 'undefined') {
        English.init();
    }
    if (page === 'life') {
        if (typeof Life !== 'undefined') Life.init();
    }
    if (page === 'work') {
        if (typeof Work !== 'undefined') Work.init();
    }
    if (page === 'settings') {
        if (typeof loadSettingsData === 'function') loadSettingsData();
        if (typeof Friends !== 'undefined' && Friends.loadFriendsData) Friends.loadFriendsData();
        activateMobileNav(null);
    }
    if (page === 'admin' && typeof AdminPanel !== 'undefined') {
        AdminPanel.init();
    }

    // Load shared inbox when switching to content pages
    if (page !== 'settings' && typeof Friends !== 'undefined') {
        Friends.loadSharedInbox();
    }

    // Notify patrol system of page switch (Phase 2 terrain awareness)
    document.dispatchEvent(new CustomEvent('patrol:pageSwitch', { detail: { page: page } }));
}

// 手动刷新当前页面数据
function refreshCurrentPage() {
    var btn = document.getElementById('header-refresh-btn');
    if (btn) {
        btn.classList.add('spinning');
        setTimeout(function() { btn.classList.remove('spinning'); }, 600);
    }
    switch (currentPage) {
        case 'todo':
            if (typeof loadItems === 'function') loadItems();
            if (typeof loadRoutines === 'function') loadRoutines();
            break;
        case 'review':
            if (typeof loadReviews === 'function') loadReviews();
            break;
        case 'english':
            if (typeof English !== 'undefined' && English.init) English.init();
            break;
        case 'life':
            if (typeof Life !== 'undefined' && Life.init) Life.init();
            break;
    }
    // Reload moment pool from server
    if (typeof Moment !== 'undefined' && Moment.reload) Moment.reload();
    if (typeof showToast === 'function') showToast('已刷新');
}

// Mobile bottom nav activation (pass null to deactivate all)
function activateMobileNav(el) {
    // Capture jelly pill position before move (for patrol interaction)
    var prevActive = document.querySelector('.mobile-nav-item.active');
    if (el && prevActive && el !== prevActive && _jellyMobileNav) {
        var fromRect = prevActive.getBoundingClientRect();
        var toRect = el.getBoundingClientRect();
        var fromX = fromRect.left + fromRect.width / 2;
        var toX = toRect.left + toRect.width / 2;
        document.dispatchEvent(new CustomEvent('patrol:jellyMove', {
            detail: { fromX: fromX, toX: toX, direction: toX > fromX ? 'right' : 'left' }
        }));
    }

    document.querySelectorAll('.mobile-nav-item').forEach(function(item) {
        item.classList.remove('active');
    });
    if (el) el.classList.add('active');
    if (_jellyMobileNav) _jellyMobileNav.moveTo(el);
}

// ========== 头像下拉菜单 ==========
function toggleAvatarMenu(e) {
    e.stopPropagation();
    var menu = document.getElementById('avatar-menu');
    menu.classList.toggle('open');
}

function closeAvatarMenu() {
    var menu = document.getElementById('avatar-menu');
    if (menu) menu.classList.remove('open');
}

// 点击页面其他区域关闭菜单
document.addEventListener('click', function(e) {
    var wrapper = document.querySelector('.header-avatar-wrapper');
    if (wrapper && !wrapper.contains(e.target)) {
        closeAvatarMenu();
    }
});

function toggleSidebarSection(sectionId) {
    var section = document.getElementById(sectionId);
    if (section) {
        section.classList.toggle('expanded');
    }
}

// ========== 此刻 (Moment) — 顶栏一句话（候选池模式） ==========
var Moment = (function() {
    var _pool = [];       // current pool of candidates
    var _poolIndex = 0;   // pointer into shuffled pool
    var _currentText = '';

    function getTimeIcon() {
        var h = new Date().getHours();
        if (h >= 6 && h < 12) return '\u2600\uFE0F';   // ☀️
        if (h >= 12 && h < 18) return '\u26C5';          // ⛅
        if (h >= 18 && h < 22) return '\uD83C\uDF19';    // 🌙
        return '\uD83C\uDF1F';                            // 🌟
    }

    function setIcon() {
        var el = document.getElementById('moment-icon');
        if (el) {
            el.textContent = getTimeIcon();
            el.style.cursor = 'pointer';
            el.onclick = function() { rotate(); };
        }
    }

    // Fisher-Yates shuffle
    function shuffle(arr) {
        for (var i = arr.length - 1; i > 0; i--) {
            var j = Math.floor(Math.random() * (i + 1));
            var tmp = arr[i]; arr[i] = arr[j]; arr[j] = tmp;
        }
        return arr;
    }

    function getTodayStr() {
        var d = new Date();
        return d.getFullYear() + '-' +
            String(d.getMonth() + 1).padStart(2, '0') + '-' +
            String(d.getDate()).padStart(2, '0');
    }

    // Read pool from localStorage
    function loadLocalPool() {
        try {
            var raw = localStorage.getItem('momentPool');
            if (!raw) return null;
            var data = JSON.parse(raw);
            if (data && data.date === getTodayStr() && Array.isArray(data.pool) && data.pool.length > 0) {
                return data.pool;
            }
        } catch(e) {}
        return null;
    }

    // Save pool to localStorage
    function saveLocalPool(pool) {
        try {
            localStorage.setItem('momentPool', JSON.stringify({
                pool: pool,
                date: getTodayStr()
            }));
        } catch(e) {}
    }

    // Show a specific text with fade transition
    function showText(text) {
        var textEl = document.getElementById('moment-text');
        if (!textEl) return;
        _currentText = text;
        // Fade out, swap, fade in
        textEl.classList.remove('visible');
        setTimeout(function() {
            textEl.textContent = text;
            textEl.classList.add('visible');
        }, 200);
    }

    // Pick next from shuffled pool
    function pickNext() {
        if (_pool.length === 0) return '';
        if (_pool.length === 1) return _pool[0];
        // Avoid showing the same text
        var candidate = _pool[_poolIndex % _pool.length];
        _poolIndex++;
        // Re-shuffle when we've gone through all
        if (_poolIndex >= _pool.length) {
            _poolIndex = 0;
            shuffle(_pool);
            // If first after shuffle is same as current, swap with next
            if (_pool.length > 1 && _pool[0] === _currentText) {
                var tmp = _pool[0]; _pool[0] = _pool[1]; _pool[1] = tmp;
            }
        }
        return candidate;
    }

    // Rotate to next item — called on icon click and refresh
    function rotate() {
        if (_pool.length <= 1) return;
        var next = pickNext();
        // Skip if same as current
        if (next === _currentText && _pool.length > 1) {
            next = pickNext();
        }
        showText(next);
    }

    async function load() {
        setIcon();

        // Check local cache first
        var localPool = loadLocalPool();
        if (localPool) {
            _pool = shuffle(localPool.slice());
            _poolIndex = 0;
            showText(pickNext());
            return;
        }

        // No local cache or stale — fetch from server
        var textEl = document.getElementById('moment-text');
        try {
            var data = await API.getMoment();
            if (data.success && Array.isArray(data.pool) && data.pool.length > 0) {
                saveLocalPool(data.pool);
                _pool = shuffle(data.pool.slice());
                _poolIndex = 0;
                showText(pickNext());
            } else if (data.success && data.text) {
                // Compat: server returned old format
                if (textEl) {
                    textEl.textContent = data.text;
                    textEl.classList.add('visible');
                }
            }
        } catch(e) {
            // Fallback
            var h = new Date().getHours();
            var greeting = h < 6 ? '夜深了，早点休息' :
                           h < 10 ? '早上好' :
                           h < 13 ? '上午好' :
                           h < 18 ? '下午好' :
                           h < 23 ? '晚上好' : '夜深了，早点休息';
            if (textEl) {
                textEl.textContent = greeting;
                textEl.classList.add('visible');
            }
        }
    }

    function startAutoRefresh() {
        // Check once when page becomes visible after a long background
        document.addEventListener('visibilitychange', function() {
            if (!document.hidden) {
                // If crossed midnight, reload pool
                var localPool = loadLocalPool();
                if (!localPool) {
                    load();
                }
            }
        });
    }

    // Force reload from server (clears local cache)
    async function reload() {
        localStorage.removeItem('momentPool');
        _pool = [];
        _poolIndex = 0;
        await load();
    }

    return {
        load: load,
        startAutoRefresh: startAutoRefresh,
        rotate: rotate,
        reload: reload,
    };
})();
