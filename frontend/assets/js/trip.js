// ========== Trip Module (差旅) ==========
var Trip = (function() {
    var _trips = [];
    var _currentTripId = null;
    var _currentTrip = null;
    var _view = 'list'; // 'list' | 'detail'
    var _editingItemId = null;
    var _itemPhotoManager = null;

    var TYPE_MAP = {
        flight: { icon: '✈️', label: '机票' },
        train:  { icon: '🚄', label: '火车' },
        hotel:  { icon: '🏨', label: '酒店' },
        taxi:   { icon: '🚕', label: '打车' },
        meal:   { icon: '🍽️', label: '餐饮' },
        meeting:{ icon: '📋', label: '会议' },
        telecom:{ icon: '📱', label: '通讯' },
        misc:   { icon: '📦', label: '其他' }
    };

    var STATUS_MAP = {
        pending:   { label: '待提交', cls: 'trip-status-pending' },
        submitted: { label: '已提交', cls: 'trip-status-submitted' },
        approved:  { label: '已批准', cls: 'trip-status-approved' },
        rejected:  { label: '已拒绝', cls: 'trip-status-rejected' },
        na:        { label: '无需报销', cls: 'trip-status-na' }
    };

    // ─── Init ───
    function init() {
        loadTrips();
    }

    async function loadTrips() {
        try {
            var data = await API.getTrips();
            if (data.success) {
                _trips = data.trips || [];
                if (_currentTripId) {
                    showDetail(_currentTripId);
                } else {
                    showList();
                }
            }
        } catch (e) {
            console.error('[Trip] loadTrips error:', e);
        }
    }

    // ─── List View ───
    function showList() {
        _view = 'list';
        _currentTripId = null;
        _currentTrip = null;
        var listView = document.getElementById('trip-list-view');
        var detailView = document.getElementById('trip-detail-view');
        if (listView) listView.style.display = '';
        if (detailView) detailView.style.display = 'none';
        var fab = document.getElementById('trip-fab');
        if (fab) fab.style.display = '';
        renderList();
    }

    function renderList() {
        var container = document.getElementById('trip-list');
        if (!container) return;

        if (_trips.length === 0) {
            container.innerHTML = '<div class="trip-empty">还没有差旅行程<br><small>点击 + 创建新行程</small></div>';
            return;
        }

        var today = new Date().toISOString().slice(0, 10);
        var upcoming = _trips.filter(function(t) { return t.date_to >= today; });
        var past = _trips.filter(function(t) { return t.date_to < today; });

        var html = '';

        if (upcoming.length > 0) {
            html += '<div class="trip-section-label">进行中 / 即将出发</div>';
            upcoming.forEach(function(t) { html += renderTripCard(t); });
        }
        if (past.length > 0) {
            html += '<div class="trip-section-label">已结束</div>';
            past.forEach(function(t) { html += renderTripCard(t); });
        }

        container.innerHTML = html;

        // 长按操作菜单 (SPEC-047)
        if (typeof ActionSheet !== 'undefined') {
            ActionSheet.bindAll(container, '.trip-card', function(el) {
                var id = el.dataset.id;
                var trip = _trips.find(function(t) { return t.id === id; });
                if (!trip) return [];
                var items = [];
                if (trip.is_owner) {
                    items.push({ icon: '📤', label: '分享给好友', action: function() { _shareTripFromList(id); } });
                    items.push({ icon: '✏️', label: '编辑', action: function() { openTripModal(id); } });
                    items.push({ icon: '🗑️', label: '删除', action: function() { _deleteTripFromList(id); }, danger: true });
                }
                return items;
            });
        }
    }

    function _shareTripFromList(tripId) {
        // Load trip detail first, then open share modal
        _currentTripId = tripId;
        API.getTrip(tripId).then(function(data) {
            if (data.success && data.trip) {
                _currentTrip = data.trip;
                openShareModal();
            } else {
                showToast('加载失败', 'error');
            }
        }).catch(function() { showToast('加载失败', 'error'); });
    }

    function _deleteTripFromList(tripId) {
        if (!confirm('确定删除此行程？所有条目和照片都会被删除。')) return;
        API.deleteTrip(tripId).then(function(data) {
            if (data.success) {
                showToast('行程已删除');
                loadTrips();
            } else {
                showToast(data.message || '删除失败', 'error');
            }
        }).catch(function() { showToast('删除失败', 'error'); });
    }

    function renderTripCard(trip) {
        var rs = trip.reimburse_summary || {};
        var total = rs.total || 0;
        var bar = '';
        if (total > 0) {
            var segments = [
                { count: rs.approved || 0, cls: 'bar-approved' },
                { count: rs.submitted || 0, cls: 'bar-submitted' },
                { count: rs.pending || 0, cls: 'bar-pending' },
                { count: rs.rejected || 0, cls: 'bar-rejected' },
                { count: rs.na || 0, cls: 'bar-na' }
            ];
            bar = '<div class="trip-reimburse-bar">';
            segments.forEach(function(s) {
                if (s.count > 0) {
                    var pct = (s.count / total * 100).toFixed(1);
                    bar += '<div class="trip-bar-seg ' + s.cls + '" style="width:' + pct + '%"></div>';
                }
            });
            bar += '</div>';
        }

        var amount = (trip.total_amount || 0).toFixed(2);
        var currency = trip.currency === 'CNY' ? '¥' : 'CA$';
        var ownerTag = trip.is_owner ? '' : '<span class="trip-shared-tag">共享</span>';

        return '<div class="trip-card" data-id="' + escapeAttr(trip.id) + '" onclick="Trip.openTrip(\'' + escapeAttr(trip.id) + '\')">'
            + '<div class="trip-card-header">'
            + '<div class="trip-card-title">' + escapeHtml(trip.title) + ownerTag + '</div>'
            + '<div class="trip-card-amount">' + currency + amount + '</div>'
            + '</div>'
            + '<div class="trip-card-meta">'
            + (trip.destination ? '<span>' + escapeHtml(trip.destination) + '</span>' : '')
            + '<span>' + trip.date_from + ' ~ ' + trip.date_to + '</span>'
            + '<span>' + (trip.item_count || 0) + ' 项</span>'
            + '</div>'
            + bar
            + '</div>';
    }

    // ─── Detail View ───
    async function showDetail(tripId) {
        _view = 'detail';
        _currentTripId = tripId;
        var listView = document.getElementById('trip-list-view');
        var detailView = document.getElementById('trip-detail-view');
        if (listView) listView.style.display = 'none';
        if (detailView) detailView.style.display = '';
        // FAB becomes add-item in detail
        var fab = document.getElementById('trip-fab');
        if (fab) fab.style.display = '';

        try {
            var data = await API.getTrip(tripId);
            if (data.success && data.trip) {
                _currentTrip = data.trip;
                renderDetail();
            } else {
                showToast(data.message || '加载失败', 'error');
                showList();
            }
        } catch (e) {
            console.error('[Trip] showDetail error:', e);
            showList();
        }
    }

    function renderDetail() {
        var trip = _currentTrip;
        if (!trip) return;

        // Header
        var headerEl = document.getElementById('trip-detail-header');
        if (headerEl) {
            var ownerTag = trip.is_owner ? '' : '<span class="trip-shared-tag">共享</span>';
            headerEl.innerHTML = '<div class="trip-detail-top">'
                + '<button class="trip-back-btn" onclick="Trip.backToList()">&larr; 返回</button>'
                + '<div class="trip-detail-actions">'
                + (trip.is_owner ? '<button class="trip-action-btn' + (window._userStatus === 'guest' ? ' guest-disabled' : '') + '" onclick="Trip.openShareModal()">👥</button>' : '')
                + (trip.is_owner ? '<button class="trip-action-btn' + (window._userStatus === 'guest' ? ' guest-disabled' : '') + '" onclick="Trip.openTripModal(\'' + escapeAttr(trip.id) + '\')">✏️</button>' : '')
                + '<button class="trip-action-btn" onclick="Trip.showExportMenu()" title="下载导出">⬇️</button>'
                + '</div>'
                + '</div>'
                + '<h2 class="trip-detail-title">' + escapeHtml(trip.title) + ownerTag + '</h2>'
                + '<div class="trip-detail-meta">'
                + (trip.destination ? escapeHtml(trip.destination) + ' · ' : '')
                + trip.date_from + ' ~ ' + trip.date_to
                + '</div>'
                + (trip.purpose ? '<div class="trip-detail-purpose">' + escapeHtml(trip.purpose) + '</div>' : '');
        }

        // Body: items grouped by date
        var bodyEl = document.getElementById('trip-detail-body');
        if (bodyEl) {
            var items = trip.items || [];
            if (items.length === 0) {
                bodyEl.innerHTML = '<div class="trip-empty">暂无条目，点击 + 添加</div>';
            } else {
                // Group by date
                var groups = {};
                items.forEach(function(iw) {
                    var item = iw.item || iw;
                    var date = item.date || 'unknown';
                    if (!groups[date]) groups[date] = [];
                    groups[date].push(iw);
                });
                var dates = Object.keys(groups).sort();
                var html = '';
                dates.forEach(function(date) {
                    html += '<div class="trip-day-section">';
                    html += '<div class="trip-day-header">' + formatDateLabel(date) + '</div>';
                    groups[date].forEach(function(iw) {
                        var item = iw.item || iw;
                        var photos = iw.photos || [];
                        html += renderItemRow(item, photos);
                    });
                    html += '</div>';
                });
                bodyEl.innerHTML = html;
            }
        }

        // Summary
        var summaryEl = document.getElementById('trip-detail-summary');
        if (summaryEl) {
            var rs = trip.reimburse_summary || {};
            var total = (trip.total_amount || 0).toFixed(2);
            var cur = trip.currency === 'CNY' ? '¥' : 'CA$';
            summaryEl.innerHTML = '<div class="trip-summary-bar">'
                + '<div class="trip-summary-total">合计 ' + cur + total + '</div>'
                + '<div class="trip-summary-status">'
                + statusPill('approved', rs.approved)
                + statusPill('submitted', rs.submitted)
                + statusPill('pending', rs.pending)
                + statusPill('rejected', rs.rejected)
                + '</div>'
                + '</div>';
        }
    }

    function renderItemRow(item, photos) {
        var t = TYPE_MAP[item.item_type || item.type] || TYPE_MAP.misc;
        var s = STATUS_MAP[item.reimburse_status] || STATUS_MAP.pending;
        var cur = (item.currency === 'CNY') ? '¥' : '$';
        var photoHtml = '';
        if (photos && photos.length > 0) {
            photoHtml = '<div class="trip-item-photos">';
            photos.forEach(function(p) {
                var url = '/api/uploads/' + _currentTrip.user_id + '/' + p.id + '.' + (p.filename.split('.').pop() || 'jpg');
                photoHtml += '<img class="trip-item-thumb" src="' + url + '" onclick="event.stopPropagation(); Trip.viewPhoto(\'' + url + '\')">';
            });
            photoHtml += '</div>';
        }

        return '<div class="trip-item-row" onclick="Trip.openItemModal(\'' + escapeAttr(item.id) + '\')">'
            + '<div class="trip-item-icon">' + t.icon + '</div>'
            + '<div class="trip-item-content">'
            + '<div class="trip-item-desc">' + escapeHtml(item.description || t.label) + '</div>'
            + (item.notes ? '<div class="trip-item-notes">' + escapeHtml(item.notes) + '</div>' : '')
            + photoHtml
            + '</div>'
            + '<div class="trip-item-right">'
            + '<div class="trip-item-amount">' + cur + (item.amount || 0).toFixed(2) + '</div>'
            + '<div class="trip-item-status ' + s.cls + '">' + s.label + '</div>'
            + '</div>'
            + '</div>';
    }

    function statusPill(status, count) {
        if (!count) return '';
        var s = STATUS_MAP[status];
        if (!s) return '';
        return '<span class="trip-pill ' + s.cls + '">' + s.label + ' ' + count + '</span>';
    }

    function backToList() {
        _currentTripId = null;
        _currentTrip = null;
        loadTrips();
    }

    function openTrip(id) {
        showDetail(id);
    }

    // ─── Trip CRUD Modal ───
    function openTripModal(editId) {
        var overlay = document.getElementById('trip-modal-overlay');
        if (!overlay) return;

        var isEdit = !!editId;
        var trip = isEdit ? _currentTrip : null;

        var today = new Date().toISOString().slice(0, 10);
        var tomorrow = new Date(Date.now() + 86400000).toISOString().slice(0, 10);

        overlay.innerHTML = '<div class="trip-modal" onclick="event.stopPropagation()">'
            + '<h3>' + (isEdit ? '编辑行程' : '新建行程') + '</h3>'
            + '<div class="trip-modal-field"><label>标题 *</label>'
            + '<input type="text" id="trip-m-title" value="' + escapeAttr(trip ? trip.title : '') + '" placeholder="例: 北京出差"></div>'
            + '<div class="trip-modal-field"><label>目的地</label>'
            + '<input type="text" id="trip-m-dest" value="' + escapeAttr(trip ? trip.destination : '') + '" placeholder="例: 北京"></div>'
            + '<div class="trip-modal-row">'
            + '<div class="trip-modal-field"><label>开始日期 *</label>'
            + '<input type="date" id="trip-m-from" value="' + (trip ? trip.date_from : today) + '"></div>'
            + '<div class="trip-modal-field"><label>结束日期 *</label>'
            + '<input type="date" id="trip-m-to" value="' + (trip ? trip.date_to : tomorrow) + '"></div>'
            + '</div>'
            + '<div class="trip-modal-field"><label>目的</label>'
            + '<input type="text" id="trip-m-purpose" value="' + escapeAttr(trip ? trip.purpose : '') + '" placeholder="例: 客户拜访"></div>'
            + '<div class="trip-modal-field"><label>备注</label>'
            + '<textarea id="trip-m-notes" rows="2" placeholder="补充信息...">' + escapeHtml(trip ? trip.notes : '') + '</textarea></div>'
            + '<div class="trip-modal-field"><label>币种</label>'
            + '<select id="trip-m-currency">'
            + '<option value="CAD"' + ((trip && trip.currency === 'CAD') || !trip ? ' selected' : '') + '>CAD</option>'
            + '<option value="CNY"' + (trip && trip.currency === 'CNY' ? ' selected' : '') + '>CNY</option>'
            + '</select></div>'
            + '<div class="trip-modal-actions">'
            + (isEdit ? '<button class="btn btn-danger-text" onclick="Trip.deleteTrip()">删除</button>' : '<span></span>')
            + '<div>'
            + '<button class="btn btn-secondary" onclick="Trip.closeTripModal()">取消</button>'
            + '<button class="btn btn-primary" onclick="Trip.submitTrip(\'' + (editId || '') + '\')">保存</button>'
            + '</div></div></div>';

        overlay.style.display = 'flex';
        overlay.onclick = function() { closeTripModal(); };
    }

    function closeTripModal() {
        var overlay = document.getElementById('trip-modal-overlay');
        if (overlay) overlay.style.display = 'none';
    }

    async function submitTrip(editId) {
        var title = (document.getElementById('trip-m-title') || {}).value || '';
        var dest = (document.getElementById('trip-m-dest') || {}).value || '';
        var dateFrom = (document.getElementById('trip-m-from') || {}).value || '';
        var dateTo = (document.getElementById('trip-m-to') || {}).value || '';
        var purpose = (document.getElementById('trip-m-purpose') || {}).value || '';
        var notes = (document.getElementById('trip-m-notes') || {}).value || '';
        var currency = (document.getElementById('trip-m-currency') || {}).value || 'CAD';

        if (!title.trim()) { showToast('请输入标题', 'error'); return; }
        if (!dateFrom || !dateTo) { showToast('请选择日期', 'error'); return; }

        try {
            var data;
            if (editId) {
                data = await API.updateTrip(editId, {
                    title: title, destination: dest, date_from: dateFrom,
                    date_to: dateTo, purpose: purpose, notes: notes, currency: currency
                });
            } else {
                data = await API.createTrip({
                    title: title, destination: dest, date_from: dateFrom,
                    date_to: dateTo, purpose: purpose, notes: notes, currency: currency
                });
            }
            if (data.success) {
                closeTripModal();
                showToast(editId ? '行程已更新' : '行程已创建');
                if (editId) {
                    showDetail(editId);
                } else if (data.trip) {
                    showDetail(data.trip.id);
                } else {
                    loadTrips();
                }
            } else {
                showToast(data.message || '操作失败', 'error');
            }
        } catch (e) {
            showToast('操作失败', 'error');
        }
    }

    async function deleteTrip() {
        if (!_currentTrip || !_currentTrip.is_owner) return;
        if (!confirm('确定删除此行程？所有条目和照片都会被删除。')) return;

        try {
            var data = await API.deleteTrip(_currentTrip.id);
            if (data.success) {
                closeTripModal();
                showToast('行程已删除');
                _currentTripId = null;
                _currentTrip = null;
                loadTrips();
            } else {
                showToast(data.message || '删除失败', 'error');
            }
        } catch (e) {
            showToast('删除失败', 'error');
        }
    }

    // ─── Item CRUD Modal ───
    function openItemModal(editItemId) {
        var overlay = document.getElementById('trip-item-modal-overlay');
        if (!overlay) return;

        _editingItemId = editItemId || null;
        var item = null;
        if (editItemId && _currentTrip) {
            var found = (_currentTrip.items || []).find(function(iw) {
                return (iw.item || iw).id === editItemId;
            });
            if (found) item = found.item || found;
        }

        var isEdit = !!item;
        var trip = _currentTrip;
        var defaultDate = trip ? trip.date_from : new Date().toISOString().slice(0, 10);

        // Type selector
        var types = ['flight', 'train', 'hotel', 'taxi', 'meal', 'meeting', 'telecom', 'misc'];
        var typeGrid = '<div class="trip-type-grid">';
        types.forEach(function(t) {
            var info = TYPE_MAP[t];
            var selected = (item && (item.item_type || item.type) === t) || (!item && t === 'misc') ? ' selected' : '';
            typeGrid += '<div class="trip-type-option' + selected + '" data-type="' + t + '" onclick="Trip.selectType(this)">'
                + '<span class="trip-type-icon">' + info.icon + '</span>'
                + '<span class="trip-type-label">' + info.label + '</span></div>';
        });
        typeGrid += '</div>';

        // Status selector
        var statuses = ['pending', 'submitted', 'approved', 'rejected', 'na'];
        var statusSel = '<div class="trip-status-selector">';
        statuses.forEach(function(st) {
            var info = STATUS_MAP[st];
            var selected = (item && item.reimburse_status === st) || (!item && st === 'pending') ? ' selected' : '';
            statusSel += '<div class="trip-status-option ' + info.cls + selected + '" data-status="' + st + '" onclick="Trip.selectStatus(this)">'
                + info.label + '</div>';
        });
        statusSel += '</div>';

        var canEditAll = !isEdit || (_currentTrip && _currentTrip.is_owner);

        // Existing photos (edit mode)
        var existingPhotosHtml = '';
        if (isEdit && editItemId && _currentTrip) {
            var found2 = (_currentTrip.items || []).find(function(iw) { return (iw.item || iw).id === editItemId; });
            var photos = found2 ? (found2.photos || []) : [];
            if (photos.length > 0) {
                existingPhotosHtml = '<div class="trip-existing-photos">';
                photos.forEach(function(p) {
                    var url = '/api/uploads/' + _currentTrip.user_id + '/' + p.id + '.' + (p.filename.split('.').pop() || 'jpg');
                    existingPhotosHtml += '<div class="trip-photo-thumb-wrap">'
                        + '<img class="trip-photo-thumb" src="' + url + '">'
                        + (_currentTrip.is_owner ? '<button class="trip-photo-del" onclick="event.stopPropagation(); Trip.deletePhoto(\'' + escapeAttr(p.id) + '\')">×</button>' : '')
                        + '</div>';
                });
                existingPhotosHtml += '</div>';
            }
        }

        // AI analysis section (owner can use, both new and edit)
        var aiSection = '';
        if (canEditAll) {
            var aiHint = isEdit
                ? '上传新票据照片后可重新分析，或粘贴补充信息...'
                : '粘贴行程信息（确认邮件、短信、订单截图文字等），二狗会自动提取信息...';
            var aiDividerText = isEdit ? '或 让二狗重新分析' : '或 让二狗帮你填';
            var tripAiGuestHint = '';
            var tripAiGuestAttr = '';
            if (window._userStatus === 'guest') {
                var _r = window._guestAiRemaining;
                var _hintText = (_r !== undefined && _r > 0) ? '(剩余' + _r + '次)' : (_r !== undefined ? '(已用完)' : '');
                var _warnCls = (_r !== undefined && _r <= 3) ? ' guest-ai-warning' : '';
                tripAiGuestHint = ' <span class="guest-ai-hint' + _warnCls + '">' + _hintText + '</span>';
                tripAiGuestAttr = ' data-guest-ai-action="1"';
            }
            aiSection = '<div class="trip-ai-section">'
                + '<div class="trip-ai-divider"><span>' + aiDividerText + '</span></div>'
                + '<textarea id="trip-ai-text" class="trip-ai-textarea" rows="3" placeholder="' + aiHint + '"></textarea>'
                + '<button class="trip-ai-btn" id="trip-ai-btn" onclick="Trip.analyzeText()"' + tripAiGuestAttr + '>二狗分析 ✨' + tripAiGuestHint + '</button>'
                + '</div>';
        }

        overlay.innerHTML = '<div class="trip-modal" onclick="event.stopPropagation()">'
            + '<h3>' + (isEdit ? '编辑条目' : '添加条目') + '</h3>'
            + (canEditAll ? '<div class="trip-modal-field"><label>类型</label>' + typeGrid + '</div>' : '')
            + (canEditAll ? '<div class="trip-modal-field"><label>日期 *</label>'
                + '<input type="date" id="trip-i-date" value="' + (item ? item.date : defaultDate) + '"></div>' : '')
            + (canEditAll ? '<div class="trip-modal-field"><label>描述</label>'
                + '<input type="text" id="trip-i-desc" value="' + escapeAttr(item ? item.description : '') + '" placeholder="例: 北京→上海 CA1234"></div>' : '')
            + (canEditAll ? '<div class="trip-modal-row">'
                + '<div class="trip-modal-field"><label>金额</label>'
                + '<input type="number" id="trip-i-amount" step="0.01" value="' + (item ? item.amount : '') + '" placeholder="0.00"></div>'
                + '<div class="trip-modal-field"><label>币种</label>'
                + '<select id="trip-i-currency">'
                + '<option value="CAD"' + ((item && item.currency === 'CAD') || !item ? ' selected' : '') + '>CAD</option>'
                + '<option value="CNY"' + (item && item.currency === 'CNY' ? ' selected' : '') + '>CNY</option>'
                + '</select></div></div>' : '')
            + '<div class="trip-modal-field"><label>报销状态</label>' + statusSel + '</div>'
            + (canEditAll ? '<div class="trip-modal-field"><label>备注</label>'
                + '<textarea id="trip-i-notes" rows="2" placeholder="补充信息...">' + escapeHtml(item ? item.notes : '') + '</textarea></div>' : '')
            + (canEditAll ? '<div class="trip-modal-field"><label>票据照片</label>'
                + existingPhotosHtml
                + '<label class="trip-photo-upload-btn" for="trip-i-photos">📷 添加票据</label>'
                + '<input type="file" id="trip-i-photos" multiple accept="image/*" onchange="Trip.handlePhotoSelect(this)" style="display:none">'
                + '<div id="trip-i-photo-preview" class="pm-grid"></div></div>' : '')
            + aiSection
            + '<div class="trip-modal-actions">'
            + (isEdit && _currentTrip && _currentTrip.is_owner ? '<button class="btn btn-danger-text" onclick="Trip.deleteItem(\'' + escapeAttr(editItemId) + '\')">删除</button>' : '<span></span>')
            + '<div>'
            + '<button class="btn btn-secondary" onclick="Trip.closeItemModal()">取消</button>'
            + '<button class="btn btn-primary" onclick="Trip.submitItem()">保存</button>'
            + '</div></div></div>';

        overlay.style.display = 'flex';
        overlay.onclick = function() { closeItemModal(); };
        _itemPhotoManager = new PhotoManager({
            container: '#trip-i-photo-preview',
            onChange: function() {}
        });
    }

    function closeItemModal() {
        var overlay = document.getElementById('trip-item-modal-overlay');
        if (overlay) overlay.style.display = 'none';
        _editingItemId = null;
        if (_itemPhotoManager) _itemPhotoManager.clear();
    }

    function selectType(el) {
        var grid = el.parentElement;
        grid.querySelectorAll('.trip-type-option').forEach(function(o) { o.classList.remove('selected'); });
        el.classList.add('selected');
    }

    function selectStatus(el) {
        var container = el.parentElement;
        container.querySelectorAll('.trip-status-option').forEach(function(o) { o.classList.remove('selected'); });
        el.classList.add('selected');
    }

    function handlePhotoSelect(input) {
        if (!_itemPhotoManager) {
            _itemPhotoManager = new PhotoManager({
                container: '#trip-i-photo-preview',
                onChange: function() {}
            });
        }
        var files = input.files;
        if (files && files.length > 0) {
            _itemPhotoManager.addFiles(files);
        }
        input.value = '';
    }

    async function submitItem() {
        if (!_currentTrip) return;

        var selectedType = document.querySelector('.trip-type-option.selected');
        var type = selectedType ? selectedType.dataset.type : 'misc';

        var selectedStatus = document.querySelector('.trip-status-option.selected');
        var reimburseStatus = selectedStatus ? selectedStatus.dataset.status : 'pending';

        var dateEl = document.getElementById('trip-i-date');
        var descEl = document.getElementById('trip-i-desc');
        var amountEl = document.getElementById('trip-i-amount');
        var currencyEl = document.getElementById('trip-i-currency');
        var notesEl = document.getElementById('trip-i-notes');

        try {
            var data;
            if (_editingItemId) {
                // Update
                var updateData = { reimburse_status: reimburseStatus };
                if (_currentTrip.is_owner) {
                    updateData.type = type;
                    if (dateEl) updateData.date = dateEl.value;
                    if (descEl) updateData.description = descEl.value;
                    if (amountEl) updateData.amount = parseFloat(amountEl.value) || 0;
                    if (currencyEl) updateData.currency = currencyEl.value;
                    if (notesEl) updateData.notes = notesEl.value;
                }
                data = await API.updateTripItem(_editingItemId, updateData);
            } else {
                // Create
                if (!dateEl || !dateEl.value) { showToast('请选择日期', 'error'); return; }
                data = await API.createTripItem(_currentTrip.id, {
                    type: type,
                    date: dateEl.value,
                    description: descEl ? descEl.value : '',
                    amount: amountEl ? parseFloat(amountEl.value) || 0 : 0,
                    currency: currencyEl ? currencyEl.value : 'CAD',
                    reimburse_status: reimburseStatus,
                    notes: notesEl ? notesEl.value : ''
                });
            }

            if (data.success) {
                // Upload photos if any
                var itemId = _editingItemId || (data.item && data.item.id);
                var pendingFiles = _itemPhotoManager ? _itemPhotoManager.getFiles() : [];
                if (itemId && pendingFiles.length > 0) {
                    await API.uploadTripItemPhotos(itemId, pendingFiles);
                }
                closeItemModal();
                showToast(_editingItemId ? '条目已更新' : '条目已添加');
                showDetail(_currentTrip.id);
            } else {
                showToast(data.message || '操作失败', 'error');
            }
        } catch (e) {
            showToast('操作失败', 'error');
        }
    }

    async function deleteItem(itemId) {
        if (!confirm('确定删除此条目？')) return;
        try {
            var data = await API.deleteTripItem(itemId);
            if (data.success) {
                closeItemModal();
                showToast('条目已删除');
                showDetail(_currentTrip.id);
            } else {
                showToast(data.message || '删除失败', 'error');
            }
        } catch (e) {
            showToast('删除失败', 'error');
        }
    }

    // ─── AI Analysis (图片 + 文字，支持拆单) ───
    async function analyzeText() {
        var textEl = document.getElementById('trip-ai-text');
        var text = textEl ? textEl.value.trim() : '';

        // 收集待上传的照片，转成 base64（通过 PhotoManager 压缩后发给 AI）
        var images = [];
        var pendingFiles = _itemPhotoManager ? _itemPhotoManager.getFiles() : [];
        if (pendingFiles.length > 0) {
            try {
                images = await _itemPhotoManager.getBase64();
            } catch (e) {
                console.warn('[Trip] photo to base64 failed', e);
            }
            if (images.length < pendingFiles.length) {
                showToast('有图片读取失败，已跳过', 'error');
            }
        }

        if (!text && images.length === 0) {
            showToast('请选择票据照片或粘贴行程信息', 'error');
            return;
        }

        var btn = document.getElementById('trip-ai-btn');
        if (btn) { btn.disabled = true; btn.textContent = '分析中...'; }

        try {
            var payload = {};
            if (images.length > 0) payload.images = images;
            if (text) payload.text = text;

            var resp = await API.analyzeTripItem(payload);

            if (resp.success && resp.items && resp.items.length > 0) {
                if (resp.items.length === 1) {
                    // 单条：直接填入表单
                    _fillFormFromAI(resp.items[0]);
                    showToast('已自动填充 ✨');
                } else {
                    // 多条：检测到拆单，显示多条预览
                    _showMultiItemPreview(resp.items);
                }
            } else {
                showToast(resp.message || '无法解析，请手动填写', 'error');
            }
        } catch (e) {
            console.error('[Trip] analyzeText error:', e);
            showToast('分析失败', 'error');
        } finally {
            if (btn) { btn.disabled = false; btn.textContent = '二狗分析 ✨'; }
        }
    }

    // File → base64 DataURL
    // 多条预览：AI检测到多个差旅事件（拆单场景）
    function _showMultiItemPreview(items) {
        var TYPE_LABELS = {
            flight: '✈️ 机票', train: '🚄 火车', hotel: '🏨 住宿',
            taxi: '🚕 出行', meal: '🍽️ 餐饮', meeting: '📋 会议',
            telecom: '📱 通讯', misc: '📎 其他'
        };
        var CURRENCY_SYM = { CAD: 'CA$', CNY: '¥', USD: '$' };

        var listHtml = items.map(function(item, idx) {
            var sym = CURRENCY_SYM[item.currency] || item.currency || '';
            var amtStr = item.amount != null ? sym + item.amount : '金额未知';
            var label = TYPE_LABELS[item.type] || item.type || '其他';
            return '<div class="trip-multi-item" data-idx="' + idx + '">'
                + '<label class="trip-multi-check">'
                + '<input type="checkbox" checked data-idx="' + idx + '"> '
                + '<span class="trip-multi-label">' + label + '</span>'
                + '<span class="trip-multi-desc">' + escapeHtml(item.description || '') + '</span>'
                + '<span class="trip-multi-amt">' + amtStr + '</span>'
                + '</label>'
                + (item.date ? '<span class="trip-multi-date">' + item.date + '</span>' : '')
                + '</div>';
        }).join('');

        // 覆盖当前弹窗内容，显示多条预览
        var overlay = document.getElementById('trip-item-modal-overlay');
        if (!overlay) return;

        // 先存数据，onclick 中通过 window._tripMultiItems 引用
        window._tripMultiItems = items;

        overlay.innerHTML = '<div class="trip-modal trip-multi-modal" onclick="event.stopPropagation()">'
            + '<h3>🔍 检测到 ' + items.length + ' 条差旅记录</h3>'
            + '<p class="trip-multi-hint">多张图/一张图蕴含多个事件，可选择要创建的条目：</p>'
            + '<div class="trip-multi-list" id="trip-multi-list">' + listHtml + '</div>'
            + '<div class="trip-modal-actions">'
            + '<button class="btn btn-secondary" onclick="Trip.closeItemModal()">取消</button>'
            + '<div>'
            + '<button class="btn btn-secondary" onclick="Trip.fillFirstItem(window._tripMultiItems)">只填第1条</button>'
            + '<button class="btn btn-primary" onclick="Trip.createAllItems(window._tripMultiItems)">全部创建</button>'
            + '</div>'
            + '</div></div>';
    }

    function _fillFormFromAI(parsed) {
        // Type
        if (parsed.type) {
            var typeEl = document.querySelector('.trip-type-option[data-type="' + parsed.type + '"]');
            if (typeEl) selectType(typeEl);
        }
        // Date
        if (parsed.date) {
            var dateEl = document.getElementById('trip-i-date');
            if (dateEl) dateEl.value = parsed.date;
        }
        // Description
        if (parsed.description) {
            var descEl = document.getElementById('trip-i-desc');
            if (descEl) descEl.value = parsed.description;
        }
        // Amount
        if (parsed.amount) {
            var amountEl = document.getElementById('trip-i-amount');
            if (amountEl) amountEl.value = parsed.amount;
        }
        // Currency
        if (parsed.currency) {
            var currencyEl = document.getElementById('trip-i-currency');
            if (currencyEl) currencyEl.value = parsed.currency;
        }
        // Notes
        if (parsed.notes) {
            var notesEl = document.getElementById('trip-i-notes');
            if (notesEl) notesEl.value = parsed.notes;
        }
    }

    // 多条预览：只用第1条，重新打开填充好的 item 弹窗
    function fillFirstItem(items) {
        if (!items || !items.length) return;
        closeItemModal();
        setTimeout(function() {
            openItemModal(null);
            setTimeout(function() { _fillFormFromAI(items[0]); showToast('已填入第1条 ✨'); }, 50);
        }, 50);
    }

    // 多条预览：批量创建所有选中的条目
    async function createAllItems(items) {
        if (!_currentTrip) return;

        // 读取选中状态
        var checkboxes = document.querySelectorAll('#trip-multi-list input[type="checkbox"]');
        var selected = items.filter(function(_, idx) {
            return !checkboxes[idx] || checkboxes[idx].checked;
        });

        if (selected.length === 0) { showToast('请至少选择一条', 'error'); return; }

        var btn = document.querySelector('.trip-multi-modal .btn-primary');
        if (btn) { btn.disabled = true; btn.textContent = '创建中...'; }

        var trip = _currentTrip;
        var defaultDate = trip.date_from || new Date().toISOString().slice(0, 10);
        var ok = 0;

        for (var i = 0; i < selected.length; i++) {
            var item = selected[i];
            try {
                var data = await API.createTripItem(trip.id, {
                    type: item.type || 'misc',
                    date: item.date || defaultDate,
                    description: item.description || '',
                    amount: item.amount || 0,
                    currency: item.currency || trip.currency || 'CAD',
                    reimburse_status: 'pending',
                    notes: item.notes || ''
                });
                if (data.success) ok++;
            } catch (e) {
                console.error('[Trip] createAllItems error', e);
            }
        }

        closeItemModal();
        showToast('已创建 ' + ok + '/' + selected.length + ' 条记录 ✨');
        showDetail(trip.id);
    }

    // ─── Delete Photo ───
    async function deletePhoto(photoId) {
        if (!confirm('删除这张票据？')) return;
        try {
            var data = await API.deleteTripPhoto(photoId);
            if (data.success) {
                showToast('已删除');
                // Remove photo from DOM inline, stay in edit view
                var btns = document.querySelectorAll('.trip-photo-del');
                btns.forEach(function(btn) {
                    if (btn.getAttribute('onclick') && btn.getAttribute('onclick').indexOf(photoId) !== -1) {
                        var wrap = btn.closest('.trip-photo-thumb-wrap');
                        if (wrap) wrap.remove();
                    }
                });
            } else {
                showToast(data.message || '删除失败', 'error');
            }
        } catch (e) {
            showToast('删除失败', 'error');
        }
    }

    // ─── Share Modal (delegated to ShareModal component) ───
    function openShareModal() {
        if (!_currentTrip || !_currentTrip.is_owner) return;
        ShareModal.openCollaborate({
            collaborators: _currentTrip.collaborators || [],
            onAdd: function(friendId, role) {
                API.addTripCollaborator(_currentTrip.id, friendId, role).then(function(data) {
                    if (data.success) {
                        showToast('已添加协作者');
                        ShareModal.close();
                        showDetail(_currentTrip.id);
                    } else {
                        showToast(data.message || '添加失败', 'error');
                    }
                }).catch(function() { showToast('添加失败', 'error'); });
            },
            onRemove: function(userId) {
                API.removeTripCollaborator(_currentTrip.id, userId).then(function(data) {
                    if (data.success) {
                        showToast('已移除');
                        ShareModal.close();
                        showDetail(_currentTrip.id);
                    } else {
                        showToast(data.message || '移除失败', 'error');
                    }
                }).catch(function() { showToast('移除失败', 'error'); });
            }
        });
    }

    function closeShareModal() {
        ShareModal.close();
    }

    // ─── Export ───
    function showExportMenu() {
        if (!_currentTrip) return;
        var trip = _currentTrip;
        var hasPhotos = (trip.items || []).some(function(iw) {
            return (iw.photos || []).length > 0;
        });

        var overlay = document.getElementById('trip-modal-overlay');
        if (!overlay) return;

        overlay.innerHTML = '<div class="trip-modal" onclick="event.stopPropagation()" style="max-width:320px">'
            + '<h3>📥 下载导出</h3>'
            + '<p style="color:var(--text-secondary);font-size:0.9em;margin:0 0 16px">选择要下载的内容：</p>'
            + '<div style="display:flex;flex-direction:column;gap:10px">'
            + '<button class="btn btn-primary" onclick="Trip.exportBundle()" style="justify-content:flex-start;gap:10px;padding:12px 16px;text-align:left">'
            + '📦 <span><strong>打包下载（Excel + 照片）</strong><br><small style="opacity:.7;font-weight:normal">完整报销材料，按事项分文件夹</small></span>'
            + '</button>'
            + '<button class="btn btn-secondary" onclick="Trip.exportXLSX()" style="justify-content:flex-start;gap:10px;padding:12px 16px;text-align:left">'
            + '📊 <span><strong>仅报销清单 (.xlsx)</strong><br><small style="opacity:.7;font-weight:normal">Excel 直接打开，按日期分类</small></span>'
            + '</button>'
            + (hasPhotos
                ? '<button class="btn btn-secondary" onclick="Trip.exportPhotos()" style="justify-content:flex-start;gap:10px;padding:12px 16px;text-align:left">'
                  + '🖼️ <span><strong>仅票据照片 (.zip)</strong><br><small style="opacity:.7;font-weight:normal">按日期分文件夹打包</small></span>'
                  + '</button>'
                : '')
            + '</div>'
            + '<div class="trip-modal-actions" style="margin-top:16px">'
            + '<span></span><button class="btn btn-secondary" onclick="Trip.closeTripModal()">取消</button>'
            + '</div>'
            + '</div>';

        overlay.style.display = 'flex';
        overlay.onclick = function() { closeTripModal(); };
    }

    function exportXLSX() {
        if (!_currentTrip) return;
        closeTripModal();
        var a = document.createElement('a');
        a.href = '/api/trips/' + encodeURIComponent(_currentTrip.id) + '/export/xlsx';
        a.download = '';
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
    }

    function exportBundle() {
        if (!_currentTrip) return;
        closeTripModal();
        showToast('正在打包报销材料，请稍候...');
        var a = document.createElement('a');
        a.href = '/api/trips/' + encodeURIComponent(_currentTrip.id) + '/export/bundle';
        a.download = '';
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
    }

    function exportPhotos() {
        if (!_currentTrip) return;
        closeTripModal();
        showToast('正在打包票据，请稍候...');
        var a = document.createElement('a');
        a.href = '/api/trips/' + encodeURIComponent(_currentTrip.id) + '/export/photos';
        a.download = '';
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
    }

    // ─── Photo viewer ───
    function viewPhoto(url) {
        window.open(url, '_blank');
    }

    // ─── Helpers ───
    function formatDateLabel(dateStr) {
        var d = new Date(dateStr + 'T00:00:00');
        var days = ['日', '一', '二', '三', '四', '五', '六'];
        return dateStr + ' 周' + days[d.getDay()];
    }

    function escapeHtml(s) {
        if (!s) return '';
        return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
    }

    function escapeAttr(s) {
        if (!s) return '';
        return s.replace(/'/g, "\\'").replace(/"/g, '&quot;');
    }

    // ─── FAB handler ───
    function handleFab() {
        if (isGuestRestricted('创建差旅')) return;
        if (_view === 'detail' && _currentTrip) {
            openItemModal();
        } else {
            openTripModal();
        }
    }

    return {
        init: init,
        openTrip: openTrip,
        backToList: backToList,
        openTripModal: openTripModal,
        closeTripModal: closeTripModal,
        submitTrip: submitTrip,
        deleteTrip: deleteTrip,
        openItemModal: openItemModal,
        closeItemModal: closeItemModal,
        selectType: selectType,
        selectStatus: selectStatus,
        handlePhotoSelect: handlePhotoSelect,
        submitItem: submitItem,
        deleteItem: deleteItem,
        openShareModal: openShareModal,
        closeShareModal: closeShareModal,
        showExportMenu: showExportMenu,
        exportBundle: exportBundle,
        exportXLSX: exportXLSX,
        exportPhotos: exportPhotos,
        viewPhoto: viewPhoto,
        analyzeText: analyzeText,
        fillFirstItem: fillFirstItem,
        createAllItems: createAllItems,
        deletePhoto: deletePhoto,
        handleFab: handleFab
    };
})();
