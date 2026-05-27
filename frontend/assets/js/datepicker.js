// ========== 智能日期选择器 ==========
//
// 两种使用模式:
//   1) modal 模式(默认,SPEC-025 todo modal):toggleDatePicker() 无参,
//      读写隐藏 input `#modal-due-date`,锚定 `#date-display` 按钮。
//   2) callback 模式(T-104,任意调用方,如工作任务表 date 单元格):
//      toggleDatePicker({ anchor, initial, onSelect })
//        anchor:   触发元素(用于定位 popover 弹出位置)
//        initial:  初始日期字符串 'YYYY-MM-DD' 或空
//        onSelect: function(ymd) — 选完调用('' = 清空)
//      此模式不读写任何隐藏 input,弹层用完关闭后自动归位到原 DOM 父节点。
//
// 两种模式共用一个 popover DOM(`#date-popover`),callback 模式开时
// 临时把它 reparent 到 document.body,避免被 modal 的 display:none 隐藏。

(function() {
    var isOpen = false;
    var currentDate = null; // Date object or null
    var calendarYear, calendarMonth;
    // T-104: callback 模式状态 — null 表示在 modal 模式
    var _cbMode = null;  // { anchor, onSelect, originalParent, originalNextSibling }

    // --- Natural language date parser (CN + EN) ---
    function parseNaturalDate(input) {
        if (!input) return null;
        input = input.trim().toLowerCase();
        var today = new Date();
        today.setHours(0, 0, 0, 0);

        // Exact: today/tod/今天
        if (input === 'today' || input === 'tod' || input === '今天') return today;

        // tomorrow/tom/明天
        if (input === 'tomorrow' || input === 'tom' || input === '明天') return addDays(today, 1);

        // 后天
        if (input === '后天') return addDays(today, 2);

        // Relative: 3d, 1w, 2w
        var relMatch = input.match(/^(\d+)\s*(d|w|天|周)$/);
        if (relMatch) {
            var n = parseInt(relMatch[1]);
            var unit = relMatch[2];
            if (unit === 'd' || unit === '天') return addDays(today, n);
            if (unit === 'w' || unit === '周') return addDays(today, n * 7);
        }

        // X天后 / X周后
        var cnRelMatch = input.match(/^(\d+)\s*(天后|周后|天|周)$/);
        if (cnRelMatch) {
            var n2 = parseInt(cnRelMatch[1]);
            var u2 = cnRelMatch[2];
            if (u2.startsWith('天')) return addDays(today, n2);
            if (u2.startsWith('周')) return addDays(today, n2 * 7);
        }

        // eow (end of week = Friday)
        if (input === 'eow' || input === '周末') {
            var fri = getNextWeekday(today, 5); // Friday
            return fri;
        }

        // eom (end of month) / 月底
        if (input === 'eom' || input === '月底') {
            return new Date(today.getFullYear(), today.getMonth() + 1, 0);
        }

        // 下个月
        if (input === '下个月' || input === '下月') {
            return new Date(today.getFullYear(), today.getMonth() + 1, 1);
        }

        // Weekday names: mon-sun, 周一-周日, 下周X
        var weekdayMap = {
            'mon': 1, 'tue': 2, 'wed': 3, 'thu': 4, 'fri': 5, 'sat': 6, 'sun': 0,
            'monday': 1, 'tuesday': 2, 'wednesday': 3, 'thursday': 4, 'friday': 5, 'saturday': 6, 'sunday': 0,
            '周一': 1, '周二': 2, '周三': 3, '周四': 4, '周五': 5, '周六': 6, '周日': 0
        };

        // 下周X
        var nextWeekMatch = input.match(/^(next\s+)?(mon|tue|wed|thu|fri|sat|sun|monday|tuesday|wednesday|thursday|friday|saturday|sunday)$/);
        if (nextWeekMatch) {
            var wd = weekdayMap[nextWeekMatch[2]];
            if (wd !== undefined) return getNextWeekday(today, wd);
        }

        var cnNextWeek = input.match(/^下(周[一二三四五六日])$/);
        if (cnNextWeek) {
            var cnWd = weekdayMap[cnNextWeek[1]];
            if (cnWd !== undefined) {
                var d = getNextWeekday(today, cnWd);
                // Ensure it's actually next week (at least 7 days out)
                if (d - today < 7 * 86400000) d = addDays(d, 7);
                return d;
            }
        }

        // Just weekday name (nearest upcoming)
        if (weekdayMap[input] !== undefined) {
            return getNextWeekday(today, weekdayMap[input]);
        }

        // Absolute: jan 15, 1/15, 2/28
        var absMatch = input.match(/^(\d{1,2})[\/\-](\d{1,2})$/);
        if (absMatch) {
            var m = parseInt(absMatch[1]) - 1;
            var dd = parseInt(absMatch[2]);
            var year = today.getFullYear();
            var candidate = new Date(year, m, dd);
            if (candidate < today) candidate.setFullYear(year + 1);
            return candidate;
        }

        var monthNames = {
            'jan': 0, 'feb': 1, 'mar': 2, 'apr': 3, 'may': 4, 'jun': 5,
            'jul': 6, 'aug': 7, 'sep': 8, 'oct': 9, 'nov': 10, 'dec': 11
        };
        var monthMatch = input.match(/^(jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)\s+(\d{1,2})$/);
        if (monthMatch) {
            var mm = monthNames[monthMatch[1]];
            var ddd = parseInt(monthMatch[2]);
            var yr = today.getFullYear();
            var c = new Date(yr, mm, ddd);
            if (c < today) c.setFullYear(yr + 1);
            return c;
        }

        return null;
    }

    function addDays(date, n) {
        var d = new Date(date);
        d.setDate(d.getDate() + n);
        return d;
    }

    function getNextWeekday(from, targetDay) {
        var d = new Date(from);
        var diff = targetDay - d.getDay();
        if (diff <= 0) diff += 7;
        d.setDate(d.getDate() + diff);
        return d;
    }

    // --- Relative date formatting ---
    function formatRelativeDate(dateStr) {
        if (!dateStr) return '';
        var target = new Date(dateStr);
        target.setHours(0, 0, 0, 0);
        var today = new Date();
        today.setHours(0, 0, 0, 0);
        var diff = Math.round((target - today) / 86400000);
        var dayNames = ['日', '一', '二', '三', '四', '五', '六'];
        var dayName = '周' + dayNames[target.getDay()];

        if (diff === 0) return '今天';
        if (diff === 1) return '明天';
        if (diff === 2) return '后天';
        if (diff < 0) return '已过期 ' + Math.abs(diff) + ' 天';
        if (diff <= 7) return diff + '天后 (' + dayName + ')';
        if (diff <= 14) {
            var m = String(target.getMonth() + 1);
            var d = String(target.getDate());
            return dayName + ' (' + m + '/' + d + ')';
        }
        var m2 = String(target.getMonth() + 1);
        var d2 = String(target.getDate());
        return m2 + '月' + d2 + '日 (' + diff + '天后)';
    }

    function getDueDateClass(dateStr) {
        if (!dateStr) return '';
        var target = new Date(dateStr);
        target.setHours(0, 0, 0, 0);
        var today = new Date();
        today.setHours(0, 0, 0, 0);
        var diff = Math.round((target - today) / 86400000);
        if (diff < 0) return 'due-overdue';
        if (diff === 0) return 'due-today';
        if (diff <= 2) return 'due-soon';
        return 'due-normal';
    }

    // --- Chip rendering ---
    function renderDateChips() {
        var container = document.getElementById('date-chips');
        if (!container) return;

        var today = new Date();
        today.setHours(0, 0, 0, 0);
        var dayNames = ['日', '一', '二', '三', '四', '五', '六'];

        var chips = [];
        chips.push({ label: '今天', date: today });
        chips.push({ label: '明天', date: addDays(today, 1) });

        // Next Monday
        var nextMon = getNextWeekday(today, 1);
        chips.push({ label: '周' + dayNames[nextMon.getDay()], date: nextMon });

        // Weekend (Saturday)
        var sat = getNextWeekday(today, 6);
        if (sat.getTime() !== nextMon.getTime()) {
            chips.push({ label: '周末', date: sat });
        }

        // End of month
        var eom = new Date(today.getFullYear(), today.getMonth() + 1, 0);
        if (eom > today) {
            chips.push({ label: '月底', date: eom });
        }

        var html = '';
        chips.forEach(function(chip) {
            var dateStr = toDateString(chip.date);
            var isActive = currentDate && toDateString(currentDate) === dateStr;
            html += '<button type="button" class="date-chip' + (isActive ? ' active' : '') +
                    '" onclick="selectDate(\'' + dateStr + '\')">' + chip.label + '</button>';
        });
        container.innerHTML = html;
    }

    // --- Mini calendar ---
    function renderMiniCalendar() {
        var container = document.getElementById('date-calendar');
        if (!container) return;

        var today = new Date();
        today.setHours(0, 0, 0, 0);
        var year = calendarYear;
        var month = calendarMonth;

        var firstDay = new Date(year, month, 1).getDay();
        var daysInMonth = new Date(year, month + 1, 0).getDate();
        var monthNames = ['1月', '2月', '3月', '4月', '5月', '6月', '7月', '8月', '9月', '10月', '11月', '12月'];

        var html = '<div class="cal-header">';
        html += '<button type="button" class="cal-nav" onclick="calendarPrev()">&lt;</button>';
        html += '<span class="cal-title">' + year + '年' + monthNames[month] + '</span>';
        html += '<button type="button" class="cal-nav" onclick="calendarNext()">&gt;</button>';
        html += '</div>';
        html += '<div class="cal-grid">';
        html += '<span class="cal-day-name">一</span><span class="cal-day-name">二</span><span class="cal-day-name">三</span>';
        html += '<span class="cal-day-name">四</span><span class="cal-day-name">五</span><span class="cal-day-name">六</span>';
        html += '<span class="cal-day-name">日</span>';

        // Adjust firstDay to Monday-start (0=Mon..6=Sun)
        var startOffset = (firstDay + 6) % 7;
        for (var i = 0; i < startOffset; i++) {
            html += '<span class="cal-day empty"></span>';
        }

        for (var d = 1; d <= daysInMonth; d++) {
            var cellDate = new Date(year, month, d);
            var dateStr = toDateString(cellDate);
            var cls = 'cal-day';
            if (cellDate.getTime() === today.getTime()) cls += ' today';
            if (currentDate && toDateString(currentDate) === dateStr) cls += ' selected';
            if (cellDate < today) cls += ' past';
            html += '<button type="button" class="' + cls + '" onclick="selectDate(\'' + dateStr + '\')">' + d + '</button>';
        }
        html += '</div>';
        container.innerHTML = html;
    }

    // --- Helpers ---
    function toDateString(date) {
        var y = date.getFullYear();
        var m = String(date.getMonth() + 1).padStart(2, '0');
        var d = String(date.getDate()).padStart(2, '0');
        return y + '-' + m + '-' + d;
    }

    function updateDisplay() {
        var textEl = document.getElementById('date-text');
        var clearEl = document.getElementById('date-clear');
        var hiddenInput = document.getElementById('modal-due-date');

        if (currentDate) {
            textEl.textContent = formatRelativeDate(toDateString(currentDate));
            textEl.className = 'date-text ' + getDueDateClass(toDateString(currentDate));
            clearEl.style.display = 'inline';
            hiddenInput.value = toDateString(currentDate);
        } else {
            textEl.textContent = '设置日期';
            textEl.className = 'date-text';
            clearEl.style.display = 'none';
            hiddenInput.value = '';
        }
    }

    // --- Public API (global functions) ---
    window.toggleDatePicker = function(opts) {
        // T-104: callback 模式分支(opts 含 anchor)
        if (opts && opts.anchor) {
            return _openCallbackMode(opts);
        }

        // ─── modal 模式(原 SPEC-025 todo 弹窗) ───
        if (typeof modalMode !== 'undefined' && modalMode === 'view') {
            if (typeof switchToEditMode === 'function') switchToEditMode();
        }
        var popover = document.getElementById('date-popover');
        if (isOpen) {
            popover.style.display = 'none';
            isOpen = false;
        } else {
            var now = currentDate || new Date();
            calendarYear = now.getFullYear();
            calendarMonth = now.getMonth();
            renderDateChips();
            renderMiniCalendar();
            popover.style.display = 'block';
            isOpen = true;
            document.getElementById('date-nl-input').value = '';
            document.getElementById('date-nl-preview').textContent = '';
            // Position fixed popover relative to the trigger button
            var btn = document.getElementById('date-display');
            if (btn) {
                var rect = btn.getBoundingClientRect();
                popover.style.left = rect.left + 'px';
                popover.style.top = (rect.bottom + 4) + 'px';
                // Keep within viewport
                requestAnimationFrame(function() {
                    var pr = popover.getBoundingClientRect();
                    if (pr.right > window.innerWidth - 8) {
                        popover.style.left = Math.max(8, window.innerWidth - pr.width - 8) + 'px';
                    }
                    if (pr.bottom > window.innerHeight - 8) {
                        popover.style.top = Math.max(8, rect.top - pr.height - 4) + 'px';
                    }
                });
            }
        }
    };

    // T-104:打开 callback 模式 — 工作任务表 date 单元格 / 任意非 modal 调用方
    function _openCallbackMode(opts) {
        var popover = document.getElementById('date-popover');
        if (!popover) {
            console.error('[Datepicker] #date-popover not in DOM');
            return;
        }
        // 已经在 callback 模式开着 → 当成 toggle 关闭
        if (_cbMode) {
            _closeCallbackMode();
            return;
        }
        // 把 popover 临时 reparent 到 body,免得被 modal `display:none` 隐藏
        _cbMode = {
            anchor: opts.anchor,
            onSelect: opts.onSelect || function() {},
            originalParent: popover.parentNode,
            originalNextSibling: popover.nextSibling,
        };
        document.body.appendChild(popover);

        // 解析初始日期
        currentDate = null;
        if (opts.initial) {
            var s = ('' + opts.initial).trim();
            // 支持 'YYYY-MM-DD' / 'MM-DD'(补当前年)
            if (/^\d{4}-\d{2}-\d{2}$/.test(s)) {
                var d = new Date(s + 'T00:00:00');
                if (!isNaN(d.getTime())) currentDate = d;
            } else if (/^\d{2}-\d{2}$/.test(s)) {
                var yr = new Date().getFullYear();
                var d2 = new Date(yr + '-' + s + 'T00:00:00');
                if (!isNaN(d2.getTime())) currentDate = d2;
            }
        }
        var now = currentDate || new Date();
        calendarYear = now.getFullYear();
        calendarMonth = now.getMonth();
        renderDateChips();
        renderMiniCalendar();

        // 清自然语言输入
        var nl = document.getElementById('date-nl-input');
        var nlp = document.getElementById('date-nl-preview');
        if (nl) nl.value = '';
        if (nlp) nlp.textContent = '';

        popover.style.display = 'block';
        // z-index 必须高于工作模块抽屉(1050)和 wt-modal(1080)
        popover.style.zIndex = '1100';
        isOpen = true;

        // 锚点定位
        var rect = opts.anchor.getBoundingClientRect();
        popover.style.position = 'fixed';
        popover.style.left = rect.left + 'px';
        popover.style.top = (rect.bottom + 4) + 'px';
        requestAnimationFrame(function() {
            var pr = popover.getBoundingClientRect();
            if (pr.right > window.innerWidth - 8) {
                popover.style.left = Math.max(8, window.innerWidth - pr.width - 8) + 'px';
            }
            if (pr.bottom > window.innerHeight - 8) {
                popover.style.top = Math.max(8, rect.top - pr.height - 4) + 'px';
            }
        });
    }

    function _closeCallbackMode() {
        var popover = document.getElementById('date-popover');
        if (!popover) { _cbMode = null; isOpen = false; return; }
        popover.style.display = 'none';
        popover.style.position = '';
        popover.style.left = '';
        popover.style.top = '';
        popover.style.zIndex = '';
        isOpen = false;
        // 归位到原 DOM 父节点(不破坏 todo modal 布局)
        if (_cbMode && _cbMode.originalParent) {
            if (_cbMode.originalNextSibling && _cbMode.originalNextSibling.parentNode === _cbMode.originalParent) {
                _cbMode.originalParent.insertBefore(popover, _cbMode.originalNextSibling);
            } else {
                _cbMode.originalParent.appendChild(popover);
            }
        }
        _cbMode = null;
    }

    window.selectDate = function(dateStr) {
        // T-104: callback 模式 — 不写 #modal-due-date,直接调 onSelect
        if (_cbMode) {
            var cb = _cbMode.onSelect;
            _closeCallbackMode();
            try { cb(dateStr); } catch (e) { console.error('[Datepicker] onSelect failed:', e); }
            return;
        }
        currentDate = new Date(dateStr + 'T00:00:00');
        updateDisplay();
        renderDateChips();
        renderMiniCalendar();
        var popover = document.getElementById('date-popover');
        popover.style.display = 'none';
        isOpen = false;
    };

    window.clearDueDate = function(e) {
        if (e) e.stopPropagation();
        // T-104: callback 模式 — 把 '' 喂给 onSelect 表示清空
        if (_cbMode) {
            var cb = _cbMode.onSelect;
            _closeCallbackMode();
            try { cb(''); } catch (e2) { console.error('[Datepicker] onSelect failed:', e2); }
            return;
        }
        currentDate = null;
        updateDisplay();
        var popover = document.getElementById('date-popover');
        popover.style.display = 'none';
        isOpen = false;
    };

    window.calendarPrev = function() {
        calendarMonth--;
        if (calendarMonth < 0) { calendarMonth = 11; calendarYear--; }
        renderMiniCalendar();
    };

    window.calendarNext = function() {
        calendarMonth++;
        if (calendarMonth > 11) { calendarMonth = 0; calendarYear++; }
        renderMiniCalendar();
    };

    // Expose formatRelativeDate and getDueDateClass for tasks.js
    window.formatRelativeDate = formatRelativeDate;
    window.getDueDateClass = getDueDateClass;

    // Sync with modal open: read hidden input value
    window.syncDatePicker = function() {
        var val = document.getElementById('modal-due-date').value;
        if (val) {
            currentDate = new Date(val + 'T00:00:00');
        } else {
            currentDate = null;
        }
        updateDisplay();
        isOpen = false;
        var popover = document.getElementById('date-popover');
        if (popover) popover.style.display = 'none';
    };

    // Natural language input handler
    document.addEventListener('DOMContentLoaded', function() {
        var nlInput = document.getElementById('date-nl-input');
        var nlPreview = document.getElementById('date-nl-preview');
        if (!nlInput) return;

        nlInput.addEventListener('input', function() {
            var parsed = parseNaturalDate(nlInput.value);
            if (parsed) {
                nlPreview.textContent = '→ ' + formatRelativeDate(toDateString(parsed)) + ' (' + toDateString(parsed) + ')';
                nlPreview.classList.add('valid');
            } else if (nlInput.value.trim()) {
                nlPreview.textContent = '无法识别';
                nlPreview.classList.remove('valid');
            } else {
                nlPreview.textContent = '';
                nlPreview.classList.remove('valid');
            }
        });

        nlInput.addEventListener('keydown', function(e) {
            if (e.key === 'Enter') {
                e.preventDefault();
                var parsed = parseNaturalDate(nlInput.value);
                if (parsed) {
                    window.selectDate(toDateString(parsed));
                }
            }
            if (e.key === 'Escape') {
                e.preventDefault();
                // T-104: callback 模式直接关掉,不要走 modal toggle 逻辑
                if (_cbMode) {
                    _closeCallbackMode();
                } else {
                    window.toggleDatePicker();
                }
            }
            if (e.key === 'Backspace' && !nlInput.value) {
                window.clearDueDate();
            }
        });
    });

    // Close popover when clicking outside
    document.addEventListener('mousedown', function(e) {
        if (!isOpen) return;
        // T-104: callback 模式 — popover 已 reparent 到 body,用 popover + anchor 自己判
        if (_cbMode) {
            var popover = document.getElementById('date-popover');
            var anchor = _cbMode.anchor;
            if (popover && !popover.contains(e.target) && (!anchor || !anchor.contains(e.target))) {
                _closeCallbackMode();
            }
            return;
        }
        var picker = document.getElementById('smart-date-picker');
        if (picker && !picker.contains(e.target)) {
            document.getElementById('date-popover').style.display = 'none';
            isOpen = false;
        }
    });
})();
