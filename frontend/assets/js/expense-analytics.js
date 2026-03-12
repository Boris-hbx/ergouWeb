// ========== Expense Analytics Module ==========
// Uses GET /api/expenses/stats?period={day|week|month}&date={YYYY-MM-DD}
var ExpenseAnalytics = (function() {
    var _period = 'month';
    var _currentDate = new Date();
    var _data = null;
    var _loading = false;
    var _swipeStartX = 0;

    var COLORS = {
        '食品杂货': '#4ade80', '餐饮': '#fb923c', '交通': '#60a5fa',
        '购物': '#f472b6', '住房': '#a78bfa', '娱乐': '#facc15',
        '医疗': '#f87171', '教育': '#2dd4bf', '其他': '#94a3b8'
    };

    var WEEKDAYS = ['日', '一', '二', '三', '四', '五', '六'];

    function init() {
        _currentDate = new Date();
        _period = 'month';
        updatePeriodButtons();
        loadData();
        bindSwipeGestures();
        bindKeyboard();
    }

    function dispose() {
        _data = null;
        unbindKeyboard();
    }

    function switchPeriod(p) {
        _period = p;
        // Don't reset date - keep current viewing date context
        updatePeriodButtons();
        loadData();
    }

    function navigateDate(dir) {
        // Don't navigate forward past current period
        if (dir > 0 && isCurrentPeriod()) return;

        if (_period === 'day') {
            _currentDate.setDate(_currentDate.getDate() + dir);
        } else if (_period === 'week') {
            _currentDate.setDate(_currentDate.getDate() + (dir * 7));
        } else {
            _currentDate.setMonth(_currentDate.getMonth() + dir);
        }
        loadData();
    }

    function isCurrentPeriod() {
        var now = new Date();
        if (_period === 'day') {
            return sameDay(_currentDate, now);
        } else if (_period === 'week') {
            return sameWeek(_currentDate, now);
        } else {
            return _currentDate.getFullYear() === now.getFullYear() &&
                   _currentDate.getMonth() === now.getMonth();
        }
    }

    function sameDay(a, b) {
        return a.getFullYear() === b.getFullYear() &&
               a.getMonth() === b.getMonth() &&
               a.getDate() === b.getDate();
    }

    function sameWeek(a, b) {
        var startA = getWeekStart(a);
        var startB = getWeekStart(b);
        return sameDay(startA, startB);
    }

    function getWeekStart(d) {
        var day = d.getDay();
        var diff = d.getDate() - day + (day === 0 ? -6 : 1);
        var s = new Date(d);
        s.setDate(diff);
        return s;
    }

    function updatePeriodButtons() {
        document.querySelectorAll('.expense-analytics-period-btn').forEach(function(btn) {
            btn.classList.toggle('active', btn.dataset.period === _period);
        });
    }

    function updateDateLabel() {
        var label = document.getElementById('expense-analytics-date-label');
        if (!label) return;
        var d = _currentDate;

        if (_period === 'day') {
            var dayName = '星期' + WEEKDAYS[d.getDay()];
            label.textContent = d.getFullYear() + '年' + (d.getMonth() + 1) + '月' + d.getDate() + '日 ' + dayName;
        } else if (_period === 'week') {
            var start = getWeekStart(d);
            var end = new Date(start);
            end.setDate(start.getDate() + 6);
            label.textContent = (start.getMonth() + 1) + '月' + start.getDate() + '日 - ' +
                               (end.getMonth() + 1) + '月' + end.getDate() + '日';
        } else {
            label.textContent = d.getFullYear() + '年' + (d.getMonth() + 1) + '月';
        }

        // Update next button disabled state
        var nextBtn = document.getElementById('expense-analytics-next-btn');
        if (nextBtn) {
            nextBtn.disabled = isCurrentPeriod();
            nextBtn.style.opacity = isCurrentPeriod() ? '0.3' : '';
        }
    }

    function formatDate(d) {
        var y = d.getFullYear();
        var m = String(d.getMonth() + 1).padStart(2, '0');
        var day = String(d.getDate()).padStart(2, '0');
        return y + '-' + m + '-' + day;
    }

    function showState(state) {
        var skeleton = document.getElementById('expense-analytics-skeleton');
        var error = document.getElementById('expense-analytics-error');
        var content = document.getElementById('expense-analytics-content');
        var empty = document.getElementById('expense-analytics-empty');
        if (skeleton) skeleton.style.display = state === 'loading' ? '' : 'none';
        if (error) error.style.display = state === 'error' ? '' : 'none';
        if (content) content.style.display = state === 'content' ? '' : 'none';
        if (empty) empty.style.display = state === 'empty' ? '' : 'none';
    }

    async function loadData() {
        if (_loading) return;
        _loading = true;
        updateDateLabel();
        showState('loading');

        var dateStr = formatDate(_currentDate);
        try {
            var resp = await API.getExpenseStats(_period, dateStr);
            _loading = false;
            if (resp.success && resp.stats) {
                _data = resp.stats;
                if (_data.entry_count === 0) {
                    updateEmptyText();
                    showState('empty');
                } else {
                    showState('content');
                    render();
                }
            } else {
                showState('error');
            }
        } catch(e) {
            _loading = false;
            console.error('[ExpenseAnalytics] loadData error:', e);
            showState('error');
        }
    }

    function retry() {
        loadData();
    }

    function updateEmptyText() {
        var el = document.getElementById('expense-analytics-empty-text');
        if (!el) return;
        var texts = { day: '今天还没有记账哦', week: '本周还没有记账哦', month: '本月还没有记账哦' };
        el.textContent = texts[_period] || '暂无消费记录';
    }

    function render() {
        renderSummary();
        renderPieChart(document.getElementById('expense-pie-canvas'));
        renderLegend(document.getElementById('expense-analytics-legend'));
        renderBarChart(document.getElementById('expense-bar-canvas'));
        renderTagList();
    }

    // ===== Summary Card =====
    function renderSummary() {
        var amountEl = document.getElementById('expense-analytics-amount');
        var countEl = document.getElementById('expense-analytics-count');
        var compEl = document.getElementById('expense-analytics-comparison');
        if (!_data) return;

        if (amountEl) {
            amountEl.textContent = '¥' + formatAmount(_data.total_amount);
        }
        if (countEl) {
            countEl.textContent = _data.entry_count + '笔';
        }
        if (compEl) {
            renderComparison(compEl);
        }
    }

    function renderComparison(el) {
        var comp = _data.comparison;
        if (!comp) { el.textContent = ''; return; }

        var prevTotal = comp.prev_total || 0;
        var curTotal = _data.total_amount || 0;
        var pct = comp.change_percent;

        // Both zero: hide
        if (prevTotal === 0 && curTotal === 0) {
            el.textContent = '';
            el.className = 'ea-summary-comparison';
            return;
        }

        var periodLabels = { day: '较昨日', week: '较上周', month: '较上月' };
        var prefix = periodLabels[_period] || '较上期';

        if (pct === 0) {
            el.textContent = prefix + ' 持平';
            el.className = 'ea-summary-comparison ea-comp-flat';
        } else if (pct > 0) {
            el.innerHTML = prefix + ' +' + Math.abs(pct).toFixed(1) + '% <span class="ea-arrow-up">&#8593;</span>';
            el.className = 'ea-summary-comparison ea-comp-up';
        } else {
            el.innerHTML = prefix + ' -' + Math.abs(pct).toFixed(1) + '% <span class="ea-arrow-down">&#8595;</span>';
            el.className = 'ea-summary-comparison ea-comp-down';
        }
    }

    function formatAmount(n) {
        return n.toLocaleString('zh-CN', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
    }

    // ===== Canvas DPR helper =====
    function setupCanvas(canvas, w, h) {
        var dpr = window.devicePixelRatio || 1;
        canvas.width = w * dpr;
        canvas.height = h * dpr;
        canvas.style.width = w + 'px';
        canvas.style.height = h + 'px';
        var ctx = canvas.getContext('2d');
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        return ctx;
    }

    // ===== Pie Chart (donut) =====
    function renderPieChart(canvas) {
        if (!canvas || !_data) return;
        var cats = prepareCategoryData();
        var pieSection = document.getElementById('expense-analytics-pie-section');
        if (!cats || cats.length === 0) {
            if (pieSection) pieSection.style.display = 'none';
            return;
        }
        if (pieSection) pieSection.style.display = '';

        var wrapper = canvas.parentElement;
        var w = wrapper.clientWidth || 300;
        var h = 220;
        var ctx = setupCanvas(canvas, w, h);

        var cx = w / 2;
        var cy = h / 2;
        var outerR = Math.min(cx, cy) - 10;
        var innerR = outerR * 0.4;
        var total = _data.total_amount;
        var startAngle = -Math.PI / 2;

        ctx.clearRect(0, 0, w, h);

        cats.forEach(function(cat) {
            var sliceAngle = (cat.amount / total) * Math.PI * 2;
            var endAngle = startAngle + sliceAngle;
            var color = COLORS[cat.category] || COLORS['其他'];

            ctx.beginPath();
            ctx.arc(cx, cy, outerR, startAngle, endAngle);
            ctx.arc(cx, cy, innerR, endAngle, startAngle, true);
            ctx.closePath();
            ctx.fillStyle = color;
            ctx.fill();

            if (cat.percentage >= 8) {
                var midAngle = startAngle + sliceAngle / 2;
                var labelR = (outerR + innerR) / 2;
                var lx = cx + Math.cos(midAngle) * labelR;
                var ly = cy + Math.sin(midAngle) * labelR;
                ctx.fillStyle = '#fff';
                ctx.font = 'bold 11px -apple-system, sans-serif';
                ctx.textAlign = 'center';
                ctx.textBaseline = 'middle';
                ctx.fillText(cat.percentage + '%', lx, ly);
            }

            startAngle = endAngle;
        });
    }

    function prepareCategoryData() {
        var cats = _data.category_totals;
        if (!cats || cats.length === 0) return [];

        // Merge beyond top 5 into "其他"
        if (cats.length > 5) {
            var top5 = cats.slice(0, 5);
            var others = cats.slice(5);
            var otherAmount = others.reduce(function(s, c) { return s + c.amount; }, 0);
            var otherCount = others.reduce(function(s, c) { return s + c.count; }, 0);
            var otherPct = _data.total_amount > 0 ? Math.round(otherAmount / _data.total_amount * 1000) / 10 : 0;
            top5.push({ category: '其他', amount: otherAmount, count: otherCount, percentage: otherPct });
            return top5;
        }
        return cats;
    }

    // ===== Legend =====
    function renderLegend(el) {
        if (!el || !_data) return;
        var cats = prepareCategoryData();
        if (!cats || cats.length === 0) { el.innerHTML = ''; return; }

        var html = '';
        cats.forEach(function(cat) {
            var color = COLORS[cat.category] || COLORS['其他'];
            html += '<div class="expense-analytics-legend-item">';
            html += '<span class="expense-analytics-legend-dot" style="background:' + color + '"></span>';
            html += '<span>' + cat.category + '</span>';
            html += '<span style="color:var(--text-secondary);margin-left:2px;">¥' + cat.amount.toFixed(0) + '</span>';
            html += '</div>';
        });
        el.innerHTML = html;
    }

    // ===== Bar Chart =====
    function renderBarChart(canvas) {
        if (!canvas || !_data) return;

        // Hide bar chart for day period
        var barSection = document.getElementById('expense-analytics-bar-section');
        if (_period === 'day') {
            if (barSection) barSection.style.display = 'none';
            return;
        }
        if (barSection) barSection.style.display = '';

        var daily = _data.daily;
        if (!daily || daily.length === 0) return;

        var wrapper = canvas.parentElement;
        var w = wrapper.clientWidth || 300;
        var h = 200;
        var ctx = setupCanvas(canvas, w, h);
        ctx.clearRect(0, 0, w, h);

        var padLeft = 45;
        var padRight = 10;
        var padTop = 20;
        var padBottom = 30;
        var chartW = w - padLeft - padRight;
        var chartH = h - padTop - padBottom;

        var maxVal = Math.max.apply(null, daily.map(function(d) { return d.amount; }));
        if (maxVal === 0) maxVal = 100;

        var gridLines = 4;
        var step = niceStep(maxVal, gridLines);
        var yMax = step * gridLines;
        if (yMax < maxVal) yMax = step * (gridLines + 1);

        var textColor = getComputedStyle(document.documentElement).getPropertyValue('--text-secondary').trim() || '#999';

        // Grid lines
        ctx.strokeStyle = 'rgba(128,128,128,0.15)';
        ctx.lineWidth = 1;
        ctx.fillStyle = textColor;
        ctx.font = '10px -apple-system, sans-serif';
        ctx.textAlign = 'right';
        ctx.textBaseline = 'middle';

        for (var g = 0; g <= gridLines; g++) {
            var yVal = step * g;
            var yPos = padTop + chartH - (yVal / yMax) * chartH;
            ctx.beginPath();
            ctx.moveTo(padLeft, yPos);
            ctx.lineTo(w - padRight, yPos);
            ctx.stroke();
            ctx.fillText(yVal >= 1000 ? (yVal / 1000).toFixed(1) + 'k' : yVal.toFixed(0), padLeft - 5, yPos);
        }

        // Bars
        var barCount = daily.length;
        var gap = Math.max(2, chartW * 0.02);
        var barW = (chartW - gap * (barCount + 1)) / barCount;
        if (barW < 3) { barW = 3; gap = 1; }
        if (barW > 30) barW = 30;
        var totalBarsW = barCount * barW + (barCount + 1) * gap;
        var offsetX = padLeft + (chartW - totalBarsW) / 2 + gap;

        var primaryColor = getComputedStyle(document.documentElement).getPropertyValue('--primary-color').trim() || '#667eea';

        for (var i = 0; i < barCount; i++) {
            var d = daily[i];
            var barH = (d.amount / yMax) * chartH;
            var x = offsetX + i * (barW + gap);
            var y = padTop + chartH - barH;

            if (d.amount > 0) {
                ctx.fillStyle = primaryColor;
                ctx.beginPath();
                var r = Math.min(3, barW / 2);
                roundedRect(ctx, x, y, barW, barH, r);
                ctx.fill();

                if (barH > 20) {
                    ctx.fillStyle = textColor;
                    ctx.font = '9px -apple-system, sans-serif';
                    ctx.textAlign = 'center';
                    ctx.textBaseline = 'bottom';
                    var label = d.amount >= 1000 ? (d.amount / 1000).toFixed(1) + 'k' : d.amount.toFixed(0);
                    ctx.fillText(label, x + barW / 2, y - 2);
                }
            }

            // X-axis label
            ctx.fillStyle = textColor;
            ctx.font = '9px -apple-system, sans-serif';
            ctx.textAlign = 'center';
            ctx.textBaseline = 'top';
            var xLabel = getBarLabel(d.date, i, barCount);
            ctx.fillText(xLabel, x + barW / 2, padTop + chartH + 4);
        }
    }

    function getBarLabel(dateStr, idx, total) {
        var parts = dateStr.split('-');
        var day = parseInt(parts[2]);
        if (total <= 7) {
            var dt = new Date(dateStr + 'T12:00:00');
            return WEEKDAYS[dt.getDay()];
        }
        if (total > 20) {
            if (day === 1 || day % 5 === 0 || day === total) return day + '';
            return '';
        }
        return day + '';
    }

    // ===== Tag List =====
    function renderTagList() {
        var el = document.getElementById('expense-analytics-tag-list');
        var section = document.getElementById('expense-analytics-tags-section');
        if (!el || !_data) return;

        var tags = _data.tag_totals;
        if (!tags || tags.length === 0) {
            if (section) section.style.display = 'none';
            return;
        }
        if (section) section.style.display = '';

        var html = '';
        tags.forEach(function(t) {
            html += '<div class="ea-tag-row">';
            html += '<span class="ea-tag-name">' + t.tag + '</span>';
            html += '<span class="ea-tag-amount">¥' + formatAmount(t.amount) + '</span>';
            html += '<span class="ea-tag-count">' + t.count + '笔</span>';
            html += '</div>';
        });
        el.innerHTML = html;
    }

    // ===== Swipe Gestures =====
    function bindSwipeGestures() {
        var view = document.getElementById('expense-analytics-view');
        if (!view) return;
        view.addEventListener('touchstart', onTouchStart, { passive: true });
        view.addEventListener('touchend', onTouchEnd, { passive: true });
    }

    function onTouchStart(e) {
        if (e.touches.length === 1) {
            _swipeStartX = e.touches[0].clientX;
        }
    }

    function onTouchEnd(e) {
        if (e.changedTouches.length === 1) {
            var dx = e.changedTouches[0].clientX - _swipeStartX;
            if (Math.abs(dx) > 50) {
                // Swipe left = previous period, swipe right = next period
                navigateDate(dx > 0 ? -1 : 1);
            }
        }
    }

    // ===== Keyboard Navigation =====
    var _keyHandler = null;

    function bindKeyboard() {
        _keyHandler = function(e) {
            // Only handle when analytics view is visible
            var view = document.getElementById('expense-analytics-view');
            if (!view || view.style.display === 'none') return;
            if (e.key === 'ArrowLeft') { navigateDate(-1); e.preventDefault(); }
            if (e.key === 'ArrowRight') { navigateDate(1); e.preventDefault(); }
        };
        document.addEventListener('keydown', _keyHandler);
    }

    function unbindKeyboard() {
        if (_keyHandler) {
            document.removeEventListener('keydown', _keyHandler);
            _keyHandler = null;
        }
    }

    // ===== Helpers =====
    function niceStep(max, lines) {
        var rough = max / lines;
        var mag = Math.pow(10, Math.floor(Math.log10(rough)));
        var residual = rough / mag;
        var nice;
        if (residual <= 1.5) nice = 1;
        else if (residual <= 3) nice = 2;
        else if (residual <= 7) nice = 5;
        else nice = 10;
        return nice * mag;
    }

    function roundedRect(ctx, x, y, w, h, r) {
        if (h < r * 2) r = h / 2;
        ctx.moveTo(x + r, y);
        ctx.lineTo(x + w - r, y);
        ctx.arcTo(x + w, y, x + w, y + r, r);
        ctx.lineTo(x + w, y + h);
        ctx.lineTo(x, y + h);
        ctx.lineTo(x, y + r);
        ctx.arcTo(x, y, x + r, y, r);
    }

    return {
        init: init,
        dispose: dispose,
        switchPeriod: switchPeriod,
        navigateDate: navigateDate,
        retry: retry
    };
})();
