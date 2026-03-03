// ========== 学习笔记模块 (IIFE) ==========
var English = (function() {
    var scenarios = [];
    var currentScenario = null;
    var initialized = false;
    var currentCategory = null; // null = 全部
    var detailMode = 'view'; // 'view' or 'edit'

    var CATEGORIES = [
        { key: '英语', icon: '🇬🇧', label: '英语' },
        { key: '编程', icon: '💻', label: '编程' },
        { key: '职场', icon: '💼', label: '职场' },
        { key: '生活', icon: '🌱', label: '生活' },
        { key: '其他', icon: '📝', label: '其他' }
    ];
    var _jellyLearnFilter = null;

    function init() {
        if (!initialized) {
            bindEvents();
            initialized = true;
        }
        loadScenarios();
    }

    function bindEvents() {
        // 创建弹窗
        var modalOverlay = document.getElementById('english-modal-overlay');
        if (modalOverlay) modalOverlay.onclick = closeCreateModal;

        var cancelBtn = document.getElementById('english-modal-cancel');
        if (cancelBtn) cancelBtn.onclick = closeCreateModal;

        var submitBtn = document.getElementById('english-modal-submit');
        if (submitBtn) submitBtn.onclick = createEntry;

        // 详情页：返回
        var backBtn = document.getElementById('english-back-btn');
        if (backBtn) backBtn.onclick = function() {
            if (detailMode === 'edit') {
                cancelEdit();
            } else {
                showList();
            }
        };

        // 详情页：阅读模式按钮
        var editBtn = document.getElementById('learn-edit-mode-btn');
        if (editBtn) editBtn.onclick = enterEditMode;

        var aiBtn = document.getElementById('english-ai-btn');
        if (aiBtn) {
            aiBtn.onclick = aiOrganize;
            if (window._userStatus === 'guest') {
                aiBtn.setAttribute('data-guest-ai-action', '1');
                var hintSpan = document.createElement('span');
                hintSpan.className = 'guest-ai-hint';
                aiBtn.appendChild(hintSpan);
                setTimeout(function() {
                    if (typeof updateAllGuestAiHints === 'function') updateAllGuestAiHints();
                }, 600);
            }
        }

        var shareBtn = document.getElementById('english-share-btn');
        if (shareBtn) shareBtn.onclick = shareCurrentScenario;

        // 详情页：编辑模式按钮
        var editCancelBtn = document.getElementById('learn-edit-cancel-btn');
        if (editCancelBtn) editCancelBtn.onclick = cancelEdit;

        var editSaveBtn = document.getElementById('learn-edit-save-btn');
        if (editSaveBtn) editSaveBtn.onclick = saveAndReturn;

        // 删除
        var deleteBtn = document.getElementById('english-delete-btn');
        if (deleteBtn) deleteBtn.onclick = deleteCurrentScenario;
    }

    // ========== 列表 ==========

    async function loadScenarios() {
        try {
            var resp = await API.getScenarios(0, currentCategory);
            if (resp.success) {
                scenarios = resp.items || [];
                renderList();
            }
        } catch (e) {
            console.error('[Learn] load failed:', e);
        }
    }

    function renderList() {
        var container = document.getElementById('english-scenarios');
        var emptyState = document.getElementById('english-empty-state');
        var listView = document.querySelector('.english-list-view');
        var detailView = document.querySelector('.english-detail-view');

        if (listView) listView.style.display = '';
        if (detailView) detailView.style.display = 'none';

        var fab = document.getElementById('learn-fab');
        if (fab) fab.style.display = '';

        if (!container) return;

        renderCategoryFilters();

        if (scenarios.length === 0) {
            container.style.display = 'none';
            if (emptyState) emptyState.style.display = '';
            return;
        }

        container.style.display = '';
        if (emptyState) emptyState.style.display = 'none';

        container.innerHTML = scenarios.map(function(s) {
            var preview = (s.content || '').replace(/[#*`\n]/g, ' ').trim();
            if (preview.length > 60) preview = preview.substring(0, 60) + '...';

            var categoryBadge = '';
            if (s.category) {
                var cat = CATEGORIES.find(function(c) { return c.key === s.category; });
                var catLabel = cat ? cat.icon + ' ' + cat.label : s.category;
                categoryBadge = '<span class="learn-entry-badge">' + catLabel + '</span>';
            }

            var timeStr = formatTimeAgo(s.updated_at);

            return '<div class="learn-entry" data-id="' + s.id + '" onclick="English.openDetail(\'' + s.id + '\')">' +
                '<div class="learn-entry-body">' +
                    '<div class="learn-entry-title">' + escapeHtml(s.title) + '</div>' +
                    (preview ? '<div class="learn-entry-preview">' + escapeHtml(preview) + '</div>' : '') +
                '</div>' +
                '<div class="learn-entry-meta">' +
                    categoryBadge +
                    '<span class="learn-entry-time">' + timeStr + '</span>' +
                '</div>' +
            '</div>';
        }).join('');

        // 长按操作菜单 (SPEC-047)
        if (typeof ActionSheet !== 'undefined') {
            ActionSheet.bindAll(container, '.learn-entry', function(el) {
                var id = el.dataset.id;
                return [
                    { icon: '📤', label: '分享给好友', action: function() { Friends.openShareModal('scenario', id); } },
                    { icon: '✏️', label: '编辑', action: function() { openDetail(id); } },
                    { icon: '🗑️', label: '删除', action: function() { deleteScenarioById(id); }, danger: true }
                ];
            });
        }
    }

    function renderCategoryFilters() {
        var container = document.getElementById('learn-category-filters');
        if (!container) return;

        // Only build DOM on first render; subsequent calls just update active class
        var existing = container.querySelectorAll('.learn-filter-pill');
        if (existing.length === 0) {
            var html = '<button class="learn-filter-pill" data-cat="" onclick="English.filterCategory(null)">全部</button>';
            CATEGORIES.forEach(function(cat) {
                html += '<button class="learn-filter-pill" data-cat="' + cat.key + '" onclick="English.filterCategory(\'' + cat.key + '\')">' + cat.icon + ' ' + cat.label + '</button>';
            });
            container.innerHTML = html;

            // Create jelly indicator once
            if (typeof JellyIndicator !== 'undefined') {
                _jellyLearnFilter = JellyIndicator.create({
                    container: container,
                    itemSelector: '.learn-filter-pill',
                    activeClass: 'active',
                    axis: 'x',
                    padding: 0,
                    pillClass: 'jelly-pill-learn',
                    bounce: false
                });
            }
        }

        // Update active state
        var activeBtn = null;
        container.querySelectorAll('.learn-filter-pill').forEach(function(btn) {
            var isActive = (btn.dataset.cat || '') === (currentCategory || '');
            btn.classList.toggle('active', isActive);
            if (isActive) activeBtn = btn;
        });

        // Position pill
        if (_jellyLearnFilter && activeBtn) {
            if (_jellyLearnFilter._hidden) {
                _jellyLearnFilter.setPositionImmediate(activeBtn);
            }
        }
    }

    function filterCategory(cat) {
        currentCategory = cat;
        // Animate pill to new position
        var container = document.getElementById('learn-category-filters');
        if (container && _jellyLearnFilter) {
            var activeBtn = null;
            container.querySelectorAll('.learn-filter-pill').forEach(function(btn) {
                var isActive = (btn.dataset.cat || '') === (cat || '');
                btn.classList.toggle('active', isActive);
                if (isActive) activeBtn = btn;
            });
            if (activeBtn) _jellyLearnFilter.moveTo(activeBtn);
        }
        loadScenarios();
    }

    // ========== 创建 ==========

    function openCreateModal() {
        var overlay = document.getElementById('english-modal-overlay');
        if (overlay) overlay.style.display = 'flex';
        var titleInput = document.getElementById('english-modal-title');
        if (titleInput) { titleInput.value = ''; titleInput.focus(); }
        var contentInput = document.getElementById('english-modal-content');
        if (contentInput) contentInput.value = '';

        var catContainer = document.getElementById('learn-modal-categories');
        if (catContainer) {
            var html = '';
            CATEGORIES.forEach(function(cat, idx) {
                var isActive = idx === 0;
                html += '<button class="learn-modal-cat-pill' + (isActive ? ' active' : '') + '" data-category="' + cat.key + '" onclick="English.selectModalCategory(this)">' + cat.icon + ' ' + cat.label + '</button>';
            });
            catContainer.innerHTML = html;
        }
    }

    function selectModalCategory(el) {
        var container = el.parentElement;
        if (!container) return;
        container.querySelectorAll('.learn-modal-cat-pill').forEach(function(btn) {
            btn.classList.remove('active');
        });
        el.classList.add('active');
    }

    function closeCreateModal() {
        var overlay = document.getElementById('english-modal-overlay');
        if (overlay) overlay.style.display = 'none';
    }

    function getSelectedModalCategory() {
        var container = document.getElementById('learn-modal-categories');
        if (!container) return '英语';
        var active = container.querySelector('.learn-modal-cat-pill.active');
        return active ? active.dataset.category : '英语';
    }

    async function createEntry() {
        var title = (document.getElementById('english-modal-title').value || '').trim();
        if (!title) {
            showToast('请输入标题', 'error');
            return;
        }
        var content = (document.getElementById('english-modal-content').value || '').trim();
        var category = getSelectedModalCategory();

        closeCreateModal();

        try {
            var resp = await API.createScenario({
                title: title,
                content: content || undefined,
                category: category
            });
            if (!resp.success) {
                showToast(resp.message || '创建失败', 'error');
                return;
            }

            scenarios.unshift(resp.item);
            renderList();
            showToast('笔记已创建', 'success');
        } catch (e) {
            showToast('创建失败', 'error');
        }
    }

    // ========== 详情页：阅读模式 ==========

    async function openDetail(id) {
        var scenario = scenarios.find(function(s) { return s.id === id; });

        // 获取完整数据
        try {
            var resp = await API.getScenario(id);
            if (resp.success && resp.item) {
                scenario = resp.item;
                var idx = scenarios.findIndex(function(s) { return s.id === id; });
                if (idx >= 0) scenarios[idx] = scenario;
                else scenarios.unshift(scenario);
            }
        } catch (e) {
            if (!scenario) {
                showToast('加载失败', 'error');
                return;
            }
        }

        if (!scenario) return;
        currentScenario = scenario;

        // 显示详情页
        var listView = document.querySelector('.english-list-view');
        var detailView = document.querySelector('.english-detail-view');
        if (listView) listView.style.display = 'none';
        if (detailView) detailView.style.display = 'block';

        var fab = document.getElementById('learn-fab');
        if (fab) fab.style.display = 'none';

        // 进入阅读模式
        renderReadingView(scenario);
        setDetailMode('view');
    }

    function renderReadingView(scenario) {
        // 标题
        var titleView = document.getElementById('learn-detail-title-view');
        if (titleView) titleView.textContent = scenario.title || '';

        // 分类标签
        var badgeEl = document.getElementById('learn-category-badge');
        if (badgeEl) {
            if (scenario.category) {
                var cat = CATEGORIES.find(function(c) { return c.key === scenario.category; });
                var label = cat ? cat.icon + ' ' + cat.label : scenario.category;
                badgeEl.innerHTML = '<span class="learn-entry-badge">' + label + '</span>';
            } else {
                badgeEl.innerHTML = '';
            }
        }

        // 时间
        var timeEl = document.getElementById('learn-meta-time');
        if (timeEl) timeEl.textContent = formatTimeAgo(scenario.updated_at);

        // 内容渲染
        var contentEl = document.getElementById('learn-content-rendered');
        if (contentEl) {
            var content = scenario.content || '';
            if (content) {
                if (hasMarkdown(content)) {
                    contentEl.innerHTML = renderMarkdown(content);
                } else {
                    contentEl.innerHTML = '<div class="learn-content-plain">' + escapeHtml(content) + '</div>';
                }
            } else {
                contentEl.innerHTML = '<div class="learn-content-empty">暂无内容，点击 ✏️ 添加</div>';
            }
        }

        // 笔记展示
        var notesDisplay = document.getElementById('learn-notes-display');
        var notesText = document.getElementById('learn-notes-text');
        if (notesDisplay && notesText) {
            if (scenario.notes && scenario.notes.trim()) {
                notesText.textContent = scenario.notes;
                notesDisplay.style.display = '';
            } else {
                notesDisplay.style.display = 'none';
            }
        }
    }

    function hasMarkdown(text) {
        return /^#{1,3} /m.test(text) || text.indexOf('**') >= 0 ||
               /^[-*] /m.test(text) || text.indexOf('```') >= 0;
    }

    // ========== 模式切换 ==========

    function setDetailMode(mode) {
        detailMode = mode;
        var isEditing = mode === 'edit';

        // 标题切换
        var titleView = document.getElementById('learn-detail-title-view');
        var titleEdit = document.getElementById('learn-detail-title-edit');
        if (titleView) titleView.style.display = isEditing ? 'none' : '';
        if (titleEdit) titleEdit.style.display = isEditing ? 'block' : 'none';

        // 按钮组切换
        var actionsView = document.getElementById('learn-actions-view');
        var actionsEdit = document.getElementById('learn-actions-edit');
        if (actionsView) actionsView.style.display = isEditing ? 'none' : 'flex';
        if (actionsEdit) actionsEdit.style.display = isEditing ? 'flex' : 'none';

        // 元信息栏
        var metaBar = document.getElementById('learn-meta-bar');
        if (metaBar) metaBar.style.display = isEditing ? 'none' : '';

        // 阅读/编辑区域切换
        var readingView = document.getElementById('learn-reading-view');
        var editingView = document.getElementById('learn-editing-view');
        if (readingView) readingView.style.display = isEditing ? 'none' : '';
        if (editingView) editingView.style.display = isEditing ? '' : 'none';
    }

    function enterEditMode() {
        if (!currentScenario) return;

        // 填充编辑表单
        var titleEdit = document.getElementById('learn-detail-title-edit');
        if (titleEdit) titleEdit.value = currentScenario.title || '';

        var contentTextarea = document.getElementById('learn-edit-content');
        if (contentTextarea) contentTextarea.value = currentScenario.content || '';

        var notesTextarea = document.getElementById('learn-edit-notes');
        if (notesTextarea) notesTextarea.value = currentScenario.notes || '';

        // 分类选择器
        renderDetailCategoryPills(currentScenario.category);

        setDetailMode('edit');

        // 聚焦内容区
        if (contentTextarea) setTimeout(function() { contentTextarea.focus(); }, 100);
    }

    function renderDetailCategoryPills(activeCategory) {
        var container = document.getElementById('learn-detail-categories');
        if (!container) return;
        var html = '';
        CATEGORIES.forEach(function(cat) {
            var isActive = activeCategory === cat.key;
            html += '<button class="learn-modal-cat-pill' + (isActive ? ' active' : '') + '" data-category="' + cat.key + '" onclick="English.selectDetailCategory(this)">' + cat.icon + ' ' + cat.label + '</button>';
        });
        container.innerHTML = html;
    }

    function selectDetailCategory(el) {
        var container = document.getElementById('learn-detail-categories');
        if (!container) return;
        container.querySelectorAll('.learn-modal-cat-pill').forEach(function(btn) {
            btn.classList.remove('active');
        });
        el.classList.add('active');
    }

    function cancelEdit() {
        setDetailMode('view');
    }

    // ========== 保存 ==========

    async function saveEntry() {
        if (!currentScenario) return false;

        var title = (document.getElementById('learn-detail-title-edit').value || '').trim();
        if (!title) {
            showToast('标题不能为空', 'error');
            return false;
        }
        var content = (document.getElementById('learn-edit-content').value || '').trim();
        var notes = (document.getElementById('learn-edit-notes').value || '').trim();

        var catContainer = document.getElementById('learn-detail-categories');
        var category = currentScenario.category;
        if (catContainer) {
            var active = catContainer.querySelector('.learn-modal-cat-pill.active');
            if (active) category = active.dataset.category;
        }

        var status = content ? 'ready' : 'draft';

        try {
            var resp = await API.updateScenario(currentScenario.id, {
                title: title,
                content: content,
                notes: notes,
                category: category,
                status: status
            });
            if (resp.success && resp.item) {
                currentScenario = resp.item;
                var idx = scenarios.findIndex(function(s) { return s.id === currentScenario.id; });
                if (idx >= 0) scenarios[idx] = currentScenario;
                return true;
            } else {
                showToast(resp.message || '保存失败', 'error');
                return false;
            }
        } catch (e) {
            showToast('保存失败', 'error');
            return false;
        }
    }

    async function saveAndReturn() {
        var ok = await saveEntry();
        if (ok) {
            renderReadingView(currentScenario);
            setDetailMode('view');
            showToast('已保存', 'success');
        }
    }

    // ========== AI 整理 ==========

    async function aiOrganize() {
        if (!currentScenario) return;

        // 如果在编辑模式，先保存
        if (detailMode === 'edit') {
            var ok = await saveEntry();
            if (!ok) return;
        }

        showToast('二狗正在整理内容...', 'info');
        try {
            var resp = await API.generateScenario(currentScenario.id);
            if (resp.success && resp.item) {
                currentScenario = resp.item;
                var idx = scenarios.findIndex(function(s) { return s.id === currentScenario.id; });
                if (idx >= 0) scenarios[idx] = currentScenario;
                renderReadingView(currentScenario);
                setDetailMode('view');
                showToast('内容已整理', 'success');
            } else {
                showToast(resp.message || '整理失败', 'error');
            }
        } catch (e) {
            showToast('整理失败', 'error');
        }
    }

    // ========== 删除 / 分享 ==========

    async function deleteCurrentScenario() {
        if (!currentScenario) return;
        deleteScenarioById(currentScenario.id);
    }

    async function deleteScenarioById(id) {
        if (!confirm('确定删除这条笔记吗？')) return;

        try {
            var resp = await API.deleteScenario(id);
            if (resp.success) {
                scenarios = scenarios.filter(function(s) { return s.id !== id; });
                showList();
                showToast('已删除', 'success');
            }
        } catch (e) {
            showToast('删除失败', 'error');
        }
    }

    function shareCurrentScenario() {
        if (!currentScenario) return;
        if (typeof Friends !== 'undefined' && Friends.openShareModal) {
            Friends.openShareModal('scenario', currentScenario.id);
        } else {
            showToast('好友功能加载中', 'info');
        }
    }

    // ========== 列表返回 ==========

    function showList() {
        currentScenario = null;
        detailMode = 'view';
        var listView = document.querySelector('.english-list-view');
        var detailView = document.querySelector('.english-detail-view');
        if (listView) listView.style.display = '';
        if (detailView) detailView.style.display = 'none';

        var fab = document.getElementById('learn-fab');
        if (fab) fab.style.display = '';

        loadScenarios();
    }

    // ========== 工具函数 ==========

    function escapeHtml(str) {
        var div = document.createElement('div');
        div.textContent = str;
        return div.innerHTML;
    }

    function formatTimeAgo(dateStr) {
        if (!dateStr) return '';
        try {
            var date = new Date(dateStr);
            var now = new Date();
            var diff = Math.floor((now - date) / 1000);
            if (diff < 60) return '刚刚';
            if (diff < 3600) return Math.floor(diff / 60) + '分钟前';
            if (diff < 86400) return Math.floor(diff / 3600) + '小时前';
            if (diff < 2592000) return Math.floor(diff / 86400) + '天前';
            return date.toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' });
        } catch (e) {
            return '';
        }
    }

    function renderMarkdown(md) {
        if (!md) return '';
        var lines = md.split('\n');
        var html = [];
        var inList = false;
        var inCodeBlock = false;
        var codeContent = [];
        var codeLang = '';

        for (var i = 0; i < lines.length; i++) {
            var line = lines[i];

            if (/^```/.test(line)) {
                if (inCodeBlock) {
                    html.push('<pre><code class="lang-' + escapeHtml(codeLang) + '">' + escapeHtml(codeContent.join('\n')) + '</code></pre>');
                    codeContent = [];
                    codeLang = '';
                    inCodeBlock = false;
                } else {
                    if (inList) { html.push('</ul>'); inList = false; }
                    codeLang = line.replace(/^```/, '').trim();
                    inCodeBlock = true;
                }
                continue;
            }

            if (inCodeBlock) { codeContent.push(line); continue; }

            if (/^### (.+)/.test(line)) {
                if (inList) { html.push('</ul>'); inList = false; }
                html.push('<h4>' + formatInline(line.replace(/^### /, '')) + '</h4>');
            } else if (/^## (.+)/.test(line)) {
                if (inList) { html.push('</ul>'); inList = false; }
                html.push('<h3>' + formatInline(line.replace(/^## /, '')) + '</h3>');
            } else if (/^# (.+)/.test(line)) {
                if (inList) { html.push('</ul>'); inList = false; }
                html.push('<h2>' + formatInline(line.replace(/^# /, '')) + '</h2>');
            } else if (/^[-*] (.+)/.test(line)) {
                if (!inList) { html.push('<ul>'); inList = true; }
                html.push('<li>' + formatInline(line.replace(/^[-*] /, '')) + '</li>');
            } else if (/^\d+\. (.+)/.test(line)) {
                if (!inList) { html.push('<ol>'); inList = true; }
                html.push('<li>' + formatInline(line.replace(/^\d+\. /, '')) + '</li>');
            } else if (line.trim() === '') {
                if (inList) { html.push(html[html.length-1] && html[html.length-1].indexOf('<ol>') >= 0 ? '</ol>' : '</ul>'); inList = false; }
            } else {
                if (inList) { html.push('</ul>'); inList = false; }
                html.push('<p>' + formatInline(line) + '</p>');
            }
        }
        if (inList) html.push('</ul>');
        if (inCodeBlock) html.push('<pre><code>' + escapeHtml(codeContent.join('\n')) + '</code></pre>');
        return html.join('\n');
    }

    function formatInline(text) {
        text = escapeHtml(text);
        text = text.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
        text = text.replace(/\*(.+?)\*/g, '<em>$1</em>');
        text = text.replace(/`(.+?)`/g, '<code>$1</code>');
        return text;
    }

    async function retryGenerate(id) {
        var scenario = scenarios.find(function(s) { return s.id === id; });
        if (!scenario) return;
        showToast('正在生成...', 'info');
        try {
            var resp = await API.generateScenario(id);
            if (resp.success && resp.item) {
                var idx = scenarios.findIndex(function(s) { return s.id === id; });
                if (idx >= 0) scenarios[idx] = resp.item;
                renderList();
                showToast('内容生成完成', 'success');
            } else {
                showToast(resp.message || '生成失败', 'error');
            }
        } catch (e) {
            showToast('生成失败', 'error');
        }
    }

    return {
        init: init,
        openDetail: openDetail,
        openCreateModal: openCreateModal,
        showList: showList,
        saveEntry: saveEntry,
        retryGenerate: retryGenerate,
        filterCategory: filterCategory,
        selectModalCategory: selectModalCategory,
        selectDetailCategory: selectDetailCategory,
        getCurrentId: function() { return currentScenario ? currentScenario.id : ''; }
    };
})();

var Learn = English;
