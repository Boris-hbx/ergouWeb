// ========== 阿宝对话组件 ==========
var Abao = (function() {
    var panel = null;
    var overlay = null;
    var messagesContainer = null;
    var inputEl = null;
    var sendBtn = null;
    var thinkingEl = null;
    var scrollBottomBtn = null;

    var conversationId = null;
    var isOpen = false;
    var isSending = false;
    var autoScroll = true;
    var thinkingTimer = null;

    // ─── 手势物理常量 ───
    var GESTURE = {
        DEAD_ZONE: 10,              // 点击 vs 滑动区分 (px)
        SCROLL_TOP_TOLERANCE: 5,    // 消息区"已到顶"判定 (px)
        VELOCITY_THRESHOLD: 0.5,    // 快速轻扫阈值 (px/ms)
        DISTANCE_RATIO: 0.33,       // 拖过 1/3 面板高度则执行
        SNAP_BACK_DURATION: 350,    // 弹回动画时长 (ms)
        SNAP_BACK_EASING: 'cubic-bezier(0.2, 0.9, 0.3, 1.1)', // 弹簧 overshoot
        CLOSE_DURATION: 300,        // 关闭动画时长 (ms)
        OPEN_DURATION: 350,         // 打开动画时长 (ms)
        HISTORY_SIZE: 5             // 速度计算用最近 N 个触摸点
    };

    // 手势状态
    var gesture = {
        animating: false,           // 动画进行中，阻止新手势
        // 🐾 上滑拉出
        paw: {
            active: false,
            startY: 0,
            startX: 0,
            pastDeadZone: false,
            history: []             // [{y, t}, ...]
        },
        // 面板下滑关闭
        panel: {
            active: false,
            startY: 0,
            startX: 0,
            commitment: 'UNDECIDED', // UNDECIDED | PANEL_DRAGGING | SCROLLING
            history: [],
            touchTarget: null
        }
    };

    // ─── Init ───
    function init() {
        panel = document.getElementById('abao-panel');
        overlay = document.getElementById('abao-overlay');
        messagesContainer = document.getElementById('abao-messages');
        inputEl = document.getElementById('abao-input');
        sendBtn = document.getElementById('abao-send');
        scrollBottomBtn = document.getElementById('abao-scroll-bottom');

        if (!panel) return;

        // Send button
        if (sendBtn) {
            sendBtn.addEventListener('click', sendMessage);
            if (window._userStatus === 'guest') {
                sendBtn.setAttribute('data-guest-ai-action', '1');
            }
        }

        // Enter to send, Shift+Enter for newline
        if (inputEl) {
            inputEl.addEventListener('keydown', function(e) {
                if (e.key === 'Enter' && !e.shiftKey) {
                    e.preventDefault();
                    sendMessage();
                }
            });

            // Auto-resize textarea
            inputEl.addEventListener('input', function() {
                this.style.height = 'auto';
                this.style.height = Math.min(this.scrollHeight, 120) + 'px';
            });
        }

        // Overlay click to close
        if (overlay) {
            overlay.addEventListener('click', close);
        }

        // Scroll detection for auto-scroll
        if (messagesContainer) {
            messagesContainer.addEventListener('scroll', function() {
                var el = messagesContainer;
                var atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 50;
                autoScroll = atBottom;
                if (scrollBottomBtn) {
                    scrollBottomBtn.style.display = atBottom ? 'none' : 'block';
                }
            });
        }

        // Scroll-to-bottom button
        if (scrollBottomBtn) {
            scrollBottomBtn.addEventListener('click', function() {
                scrollToBottom();
                autoScroll = true;
                scrollBottomBtn.style.display = 'none';
            });
        }

        // Shortcut buttons (including 新对话)
        var shortcuts = document.querySelectorAll('.abao-shortcut-btn');
        shortcuts.forEach(function(btn) {
            btn.addEventListener('click', function() {
                // 新对话按钮: data-text="" and id="abao-new-chat"
                if (btn.id === 'abao-new-chat') {
                    conversationId = null;
                    clearMessages();
                    addSystemMessage('新对话已开始');
                    return;
                }
                var text = btn.getAttribute('data-text');
                if (text && inputEl) {
                    inputEl.value = text;
                    sendMessage();
                }
            });
        });

        // Long-press on avatar → open About dialog
        var avatar = document.getElementById('header-avatar');
        if (avatar) {
            var longPressTimer = null;
            var longPressed = false;
            avatar.addEventListener('mousedown', function() {
                longPressed = false;
                longPressTimer = setTimeout(function() {
                    longPressed = true;
                    if (typeof openAbout === 'function') openAbout();
                }, 600);
            });
            avatar.addEventListener('mouseup', function() { clearTimeout(longPressTimer); });
            avatar.addEventListener('mouseleave', function() { clearTimeout(longPressTimer); });
            avatar.addEventListener('touchstart', function(e) {
                longPressed = false;
                longPressTimer = setTimeout(function() {
                    longPressed = true;
                    if (typeof openAbout === 'function') openAbout();
                }, 600);
            }, { passive: true });
            avatar.addEventListener('touchend', function() { clearTimeout(longPressTimer); });
            // Override click: show avatar menu (not Abao), but skip if long-pressed
            avatar.onclick = function(e) {
                if (longPressed) { longPressed = false; return; }
                toggleAvatarMenu(e);
            };
        }

        // ─── Mobile gesture system ───
        if (_isMobile) {
            initPawGesture();
            initPanelGesture();
        }

        // Virtual keyboard adaptation
        if (window.visualViewport) {
            window.visualViewport.addEventListener('resize', function() {
                if (isOpen && panel) {
                    var keyboardHeight = window.innerHeight - window.visualViewport.height;
                    if (keyboardHeight > 100) {
                        panel.style.height = window.visualViewport.height * 0.8 + 'px';
                    } else {
                        panel.style.height = '';
                    }
                }
            });
        }
    }

    // ─── 速度计算 ───
    function computeVelocity(history) {
        if (history.length < 2) return 0;
        var first = history[0];
        var last = history[history.length - 1];
        var dt = last.t - first.t;
        if (dt <= 0) return 0;
        return (last.y - first.y) / dt; // px/ms, 正=下滑, 负=上滑
    }

    function addToHistory(history, y) {
        var now = Date.now();
        history.push({ y: y, t: now });
        while (history.length > GESTURE.HISTORY_SIZE) {
            history.shift();
        }
    }

    // ─── 判断是否应启动手势（排除交互元素） ───
    function shouldStartGesture(target) {
        if (!target) return true;
        var tag = target.tagName;
        if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'BUTTON' || tag === 'A') return false;
        // 也检查最近的可交互祖先
        if (target.closest('button, a, input, textarea, [contenteditable]')) return false;
        return true;
    }

    // ─── 🐾 上滑拉出面板 ───
    function initPawGesture() {
        var pawBtn = document.getElementById('mobile-nav-abao');
        if (!pawBtn) return;

        pawBtn.addEventListener('touchstart', function(e) {
            if (gesture.animating) return;
            var touch = e.touches[0];
            gesture.paw.active = true;
            gesture.paw.startY = touch.clientY;
            gesture.paw.startX = touch.clientX;
            gesture.paw.pastDeadZone = false;
            gesture.paw.history = [{ y: touch.clientY, t: Date.now() }];
        }, { passive: true });

        // Move/End on document since finger leaves button area
        document.addEventListener('touchmove', function(e) {
            if (!gesture.paw.active) return;
            var touch = e.touches[0];
            var deltaY = gesture.paw.startY - touch.clientY; // positive = upward
            var deltaX = Math.abs(touch.clientX - gesture.paw.startX);

            addToHistory(gesture.paw.history, touch.clientY);

            // Dead zone check
            if (!gesture.paw.pastDeadZone) {
                if (Math.abs(deltaY) < GESTURE.DEAD_ZONE && deltaX < GESTURE.DEAD_ZONE) return;
                // Horizontal swipe → not our gesture
                if (deltaX > Math.abs(deltaY)) {
                    gesture.paw.active = false;
                    return;
                }
                // Downward swipe from paw → not our gesture (might be closing if open)
                if (deltaY < 0) {
                    gesture.paw.active = false;
                    return;
                }
                gesture.paw.pastDeadZone = true;
                // Start dragging: show panel in gesture mode
                if (!isOpen) {
                    panel.classList.add('gesture-dragging');
                    panel.classList.remove('open', 'closing');
                }
            }

            if (gesture.paw.pastDeadZone && !isOpen) {
                var panelHeight = panel.offsetHeight;
                // fingerTravel = how far up the user has swiped
                var fingerTravel = Math.max(0, gesture.paw.startY - touch.clientY);
                var translateY = Math.max(0, panelHeight - fingerTravel);
                panel.style.transform = 'translateY(' + translateY + 'px)';
                panel.style.opacity = Math.min(1, fingerTravel / (panelHeight * 0.5));
            }
        }, { passive: true });

        document.addEventListener('touchend', function(e) {
            if (!gesture.paw.active) return;
            gesture.paw.active = false;

            // Not past dead zone → treat as click (toggle)
            if (!gesture.paw.pastDeadZone) {
                toggle();
                return;
            }

            // Past dead zone → decide open or snap back
            if (!isOpen) {
                var panelHeight = panel.offsetHeight;
                var velocity = -computeVelocity(gesture.paw.history); // negate: upward is positive
                var lastTouch = gesture.paw.history[gesture.paw.history.length - 1];
                var fingerTravel = gesture.paw.startY - (lastTouch ? lastTouch.y : gesture.paw.startY);
                var openRatio = fingerTravel / panelHeight;

                if (velocity > GESTURE.VELOCITY_THRESHOLD || openRatio > GESTURE.DISTANCE_RATIO) {
                    // Complete open
                    completeOpen();
                } else {
                    // Snap back (hide)
                    snapBackHide();
                }
            }
        }, { passive: true });

        document.addEventListener('touchcancel', function() {
            if (gesture.paw.active) {
                gesture.paw.active = false;
                if (gesture.paw.pastDeadZone && !isOpen) {
                    snapBackHide();
                }
            }
        }, { passive: true });
    }

    function completeOpen() {
        gesture.animating = true;
        // CRITICAL: add .open BEFORE removing gesture-dragging
        // so visibility never flashes to hidden (both provide visibility: visible)
        panel.classList.add('open');
        isOpen = true;
        if (overlay) overlay.classList.add('open');
        document.getElementById('header-avatar')?.classList.add('abao-active');
        // Keyboard shortcut: add listener (guard against duplicates)
        document.removeEventListener('keydown', _abaoKeydownHandler);
        document.addEventListener('keydown', _abaoKeydownHandler);
        panel.classList.remove('gesture-dragging');

        // .open's transition handles the rest: animate from current inline
        // position to class values (translateY(0), opacity 1)
        // Force reflow so browser registers current inline values as start point
        panel.offsetHeight;
        // Clear inline styles → .open class values become target → transition plays
        panel.style.transform = '';
        panel.style.opacity = '';

        setTimeout(function() {
            gesture.animating = false;
            // Load conversation history if none loaded
            if (messagesContainer && messagesContainer.children.length === 0) {
                addSystemMessage('有什么事？说吧。');
            }
        }, GESTURE.OPEN_DURATION);
    }

    function snapBackHide() {
        gesture.animating = true;
        // Keep visibility via inline style while animating away
        panel.style.visibility = 'visible';
        panel.classList.remove('gesture-dragging');
        panel.style.transition = 'transform ' + GESTURE.SNAP_BACK_DURATION + 'ms ' + GESTURE.SNAP_BACK_EASING + ', opacity ' + GESTURE.SNAP_BACK_DURATION + 'ms ease';
        panel.style.transform = 'translateY(100%)';
        panel.style.opacity = '0';
        setTimeout(function() {
            panel.style.transition = '';
            panel.style.transform = '';
            panel.style.opacity = '';
            panel.style.visibility = '';
            gesture.animating = false;
        }, GESTURE.SNAP_BACK_DURATION);
    }

    // ─── 面板下滑关闭手势 ───
    function initPanelGesture() {
        panel.addEventListener('touchstart', function(e) {
            if (gesture.animating || !isOpen) return;
            if (!shouldStartGesture(e.target)) {
                gesture.panel.active = false;
                return;
            }
            var touch = e.touches[0];
            gesture.panel.active = true;
            gesture.panel.startY = touch.clientY;
            gesture.panel.startX = touch.clientX;
            gesture.panel.commitment = 'UNDECIDED';
            gesture.panel.history = [{ y: touch.clientY, t: Date.now() }];
            gesture.panel.touchTarget = e.target;
        }, { passive: true });

        panel.addEventListener('touchmove', function(e) {
            if (!gesture.panel.active || !isOpen) return;
            var touch = e.touches[0];
            var deltaY = touch.clientY - gesture.panel.startY; // positive = downward
            var deltaX = Math.abs(touch.clientX - gesture.panel.startX);

            addToHistory(gesture.panel.history, touch.clientY);

            // Commitment phase
            if (gesture.panel.commitment === 'UNDECIDED') {
                if (Math.abs(deltaY) < GESTURE.DEAD_ZONE && deltaX < GESTURE.DEAD_ZONE) return;

                // Horizontal → not our gesture
                if (deltaX > Math.abs(deltaY)) {
                    gesture.panel.active = false;
                    return;
                }

                // Upward swipe → not closing
                if (deltaY < 0) {
                    gesture.panel.commitment = 'SCROLLING';
                    return;
                }

                // Downward swipe: decide based on touch area
                var target = gesture.panel.touchTarget;
                var inMessages = target && (target === messagesContainer || messagesContainer.contains(target));
                var inHeader = target && (target.closest('.abao-header') || target.closest('.abao-drag-bar'));
                var inShortcuts = target && target.closest('.abao-shortcuts');

                if (inHeader) {
                    gesture.panel.commitment = 'PANEL_DRAGGING';
                } else if (inMessages) {
                    if (messagesContainer.scrollTop <= GESTURE.SCROLL_TOP_TOLERANCE) {
                        gesture.panel.commitment = 'PANEL_DRAGGING';
                    } else {
                        gesture.panel.commitment = 'SCROLLING';
                    }
                } else if (inShortcuts) {
                    gesture.panel.commitment = 'PANEL_DRAGGING';
                } else {
                    // Other areas (input area etc.) — just panel drag
                    gesture.panel.commitment = 'PANEL_DRAGGING';
                }

                if (gesture.panel.commitment === 'PANEL_DRAGGING') {
                    panel.style.transition = 'none';
                }
            }

            if (gesture.panel.commitment === 'PANEL_DRAGGING') {
                // Prevent native scroll
                e.preventDefault();
                var clampedDelta = Math.max(0, deltaY);
                panel.style.transform = 'translateY(' + clampedDelta + 'px)';
            }
        }, { passive: false }); // passive: false to allow preventDefault

        panel.addEventListener('touchend', function(e) {
            if (!gesture.panel.active) return;
            gesture.panel.active = false;

            if (gesture.panel.commitment !== 'PANEL_DRAGGING') return;

            var panelHeight = panel.offsetHeight;
            var velocity = computeVelocity(gesture.panel.history); // positive = downward
            var lastTouch = gesture.panel.history[gesture.panel.history.length - 1];
            var distance = lastTouch ? (lastTouch.y - gesture.panel.startY) : 0;
            var distanceRatio = distance / panelHeight;

            if (velocity > GESTURE.VELOCITY_THRESHOLD) {
                // Fast downward swipe → close
                gestureClose();
            } else if (velocity < -GESTURE.VELOCITY_THRESHOLD) {
                // Fast upward swipe → snap back (with overshoot)
                gestureSnapBack();
            } else if (distanceRatio > GESTURE.DISTANCE_RATIO) {
                // Slow drag but far enough → close
                gestureClose();
            } else {
                // Not enough → snap back
                gestureSnapBack();
            }
        }, { passive: true });

        panel.addEventListener('touchcancel', function() {
            if (gesture.panel.active && gesture.panel.commitment === 'PANEL_DRAGGING') {
                gesture.panel.active = false;
                gestureSnapBack();
            }
        }, { passive: true });
    }

    function gestureClose() {
        gesture.animating = true;
        pawNod();
        // Remove keyboard shortcut listener
        document.removeEventListener('keydown', _abaoKeydownHandler);
        panel.style.transition = 'transform ' + GESTURE.CLOSE_DURATION + 'ms cubic-bezier(0.4, 0, 1, 1), opacity ' + GESTURE.CLOSE_DURATION + 'ms ease';
        panel.style.transform = 'translateY(100%)';
        panel.style.opacity = '0';
        if (overlay) overlay.classList.remove('open');
        document.getElementById('header-avatar')?.classList.remove('abao-active');
        setTimeout(function() {
            isOpen = false;
            panel.classList.remove('open');
            panel.style.transition = '';
            panel.style.transform = '';
            panel.style.opacity = '';
            panel.style.height = '';
            gesture.animating = false;
        }, GESTURE.CLOSE_DURATION);
    }

    function gestureSnapBack() {
        gesture.animating = true;
        panel.style.transition = 'transform ' + GESTURE.SNAP_BACK_DURATION + 'ms ' + GESTURE.SNAP_BACK_EASING;
        panel.style.transform = 'translateY(0)';
        setTimeout(function() {
            panel.style.transition = '';
            panel.style.transform = '';
            gesture.animating = false;
        }, GESTURE.SNAP_BACK_DURATION);
    }

    // ─── Keyboard shortcut: B to toggle (named for proper add/remove) ───
    function _abaoKeydownHandler(e) {
        if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA' || e.target.isContentEditable) return;
        if (e.key === 'b' || e.key === 'B') {
            toggle();
        }
    }

    // ─── Open/Close ───
    function open() {
        if (!panel || gesture.animating) return;
        isOpen = true;
        panel.classList.remove('closing', 'gesture-dragging');
        panel.style.transform = '';
        panel.style.opacity = '';
        panel.classList.add('open');
        if (overlay) overlay.classList.add('open');
        document.getElementById('header-avatar')?.classList.add('abao-active');

        // Keyboard shortcut: add listener (guard against duplicates)
        document.removeEventListener('keydown', _abaoKeydownHandler);
        document.addEventListener('keydown', _abaoKeydownHandler);

        // Mobile: don't auto-focus to avoid keyboard pushing panel up
        if (!_isMobile && inputEl) inputEl.focus();

        // Load conversation history if none loaded
        if (messagesContainer && messagesContainer.children.length === 0) {
            addSystemMessage('有什么事？说吧。');
        }
    }

    function pawNod() {
        var icon = document.getElementById('abao-tab-icon');
        if (!icon) return;
        icon.classList.remove('patrol-nod');
        void icon.offsetWidth; // reflow to restart animation
        icon.classList.add('patrol-nod');
        icon.addEventListener('animationend', function() {
            icon.classList.remove('patrol-nod');
        }, { once: true });
    }

    function close() {
        if (!panel || gesture.animating) return;
        isOpen = false;
        pawNod();
        // Remove keyboard shortcut listener
        document.removeEventListener('keydown', _abaoKeydownHandler);
        // Notify patrol system: leaving abao (logo wiggle)
        document.dispatchEvent(new CustomEvent('patrol:leaveAbao'));
        // Mobile: closing animation (CSS handles .closing class)
        panel.classList.add('closing');
        panel.classList.remove('open', 'gesture-dragging');
        if (overlay) overlay.classList.remove('open');
        document.getElementById('header-avatar')?.classList.remove('abao-active');
        // Reset panel height and clean up closing class after animation
        setTimeout(function() {
            panel.classList.remove('closing');
            panel.style.height = '';
            panel.style.transform = '';
            panel.style.opacity = '';
        }, 450);
    }

    function toggle() {
        if (gesture.animating) return;
        if (isOpen) close();
        else open();
    }

    // ─── Messages ───
    var _isMobile = window.matchMedia('(max-width: 768px)').matches;

    function createUserAvatarContent() {
        var avatarValue = localStorage.getItem('userAvatar') || '';
        // Custom uploaded image — use DOM API to safely set src attribute
        if (avatarValue && avatarValue.startsWith('data:image/')) {
            var img = document.createElement('img');
            img.src = avatarValue;
            return img.outerHTML;
        }
        // Preset image (hardcoded paths, safe)
        var presets = (typeof AVATAR_PRESETS !== 'undefined') ? AVATAR_PRESETS : { 'preset:cat': 'assets/images/preset-cat.png', 'preset:panda': 'assets/images/preset-panda.png' };
        if (presets[avatarValue]) {
            return '<img src="' + presets[avatarValue] + '">';
        }
        // Initial letter
        return window._userInitial || 'B';
    }

    function wrapWithAvatar(role, msgEl) {
        if (!_isMobile) return msgEl;
        var row = document.createElement('div');
        row.className = 'abao-msg-row ' + role;
        var avatar = document.createElement('div');
        avatar.className = 'abao-msg-avatar';
        if (role === 'assistant') {
            avatar.textContent = '🐾';
        } else {
            avatar.innerHTML = createUserAvatarContent();
        }
        row.appendChild(avatar);
        row.appendChild(msgEl);
        return row;
    }

    function addMessage(role, text) {
        if (!messagesContainer) return;
        var msg = document.createElement('div');
        msg.className = 'abao-msg ' + role;
        msg.textContent = text;
        messagesContainer.appendChild(wrapWithAvatar(role, msg));
        if (autoScroll) scrollToBottom();
    }

    function addSystemMessage(text) {
        if (!messagesContainer) return;
        var msg = document.createElement('div');
        msg.className = 'abao-msg assistant';
        msg.style.opacity = '0.7';
        msg.style.fontSize = '13px';
        msg.textContent = text;
        messagesContainer.appendChild(wrapWithAvatar('assistant', msg));
        if (autoScroll) scrollToBottom();
    }

    function addToolInfo(toolCalls) {
        if (!messagesContainer || !toolCalls || toolCalls.length === 0) return;
        for (var i = 0; i < toolCalls.length; i++) {
            var tc = toolCalls[i];
            if (!tc.result || !tc.result.success) continue;
            // Show task card for create_todo
            if (tc.tool === 'create_todo' && tc.result.text) {
                var card = document.createElement('div');
                card.className = 'abao-task-card';
                card.innerHTML = '<div class="abao-task-card-title"></div>' +
                    '<div class="abao-task-card-meta"></div>';
                card.querySelector('.abao-task-card-title').textContent = tc.result.text;
                card.querySelector('.abao-task-card-meta').textContent =
                    (tc.result.tab || 'today') + ' \u00b7 ' + quadrantLabel(tc.result.quadrant || '');
                messagesContainer.appendChild(card);
            }
            // Show reminder card for create_reminder
            if (tc.tool === 'create_reminder' && tc.result.text) {
                var rcard = document.createElement('div');
                rcard.className = 'abao-task-card abao-reminder-card';
                rcard.innerHTML = '<div class="abao-task-card-title"></div>' +
                    '<div class="abao-task-card-meta"></div>' +
                    '<div class="abao-reminder-actions"><button class="abao-reminder-cancel">取消提醒</button></div>';
                rcard.querySelector('.abao-task-card-title').textContent = '\ud83d\udd14 ' + tc.result.text;
                rcard.querySelector('.abao-task-card-meta').textContent = tc.result.display_time || tc.result.remind_at || '';
                rcard.querySelector('.abao-reminder-cancel').setAttribute('data-id', tc.result.id || '');
                rcard.querySelector('.abao-reminder-cancel').addEventListener('click', function() {
                    var rid = this.getAttribute('data-id');
                    API.cancelReminder(rid).then(function(res) {
                        if (res && res.success) {
                            rcard.style.opacity = '0.5';
                            rcard.querySelector('.abao-reminder-cancel').textContent = '已取消';
                            rcard.querySelector('.abao-reminder-cancel').disabled = true;
                        }
                    });
                });
                messagesContainer.appendChild(rcard);
                // Prompt push permission if not yet subscribed
                maybePromptPush();
            }
        }
    }

    function maybePromptPush() {
        if (typeof Notifications === 'undefined' || !Notifications.isPushSupported) return;
        if (!Notifications.isPushSupported()) return;
        if (Notifications.isPushGranted()) return; // already subscribed
        if (sessionStorage.getItem('pushPromptDismissed')) return;

        // Show a push permission prompt card
        var card = document.createElement('div');
        card.className = 'abao-task-card abao-push-prompt';
        card.innerHTML = '<div class="abao-task-card-title">📲 开启推送通知</div>' +
            '<div class="abao-task-card-meta">开启后，提醒到点时即使 App 没打开也能收到通知</div>' +
            '<div class="abao-reminder-actions">' +
            '<button class="abao-push-enable-btn">开启推送</button>' +
            '<button class="abao-push-dismiss-btn">暂不开启</button>' +
            '</div>';
        card.querySelector('.abao-push-enable-btn').addEventListener('click', function() {
            Notifications.requestAndSubscribe().then(function(ok) {
                if (ok) {
                    card.innerHTML = '<div class="abao-task-card-title">✅ 推送通知已开启</div>';
                    setTimeout(function() { card.style.opacity = '0.5'; }, 2000);
                }
            });
        });
        card.querySelector('.abao-push-dismiss-btn').addEventListener('click', function() {
            card.style.display = 'none';
            sessionStorage.setItem('pushPromptDismissed', '1');
        });
        messagesContainer.appendChild(card);
        if (autoScroll) scrollToBottom();
    }

    function clearMessages() {
        if (messagesContainer) messagesContainer.innerHTML = '';
    }

    function showThinking() {
        if (!messagesContainer) return;
        thinkingEl = document.createElement('div');
        thinkingEl.className = 'abao-thinking';
        thinkingEl.innerHTML = '<div class="abao-thinking-dot"></div>' +
            '<div class="abao-thinking-dot"></div>' +
            '<div class="abao-thinking-dot"></div>' +
            '<span class="abao-thinking-text"></span>';
        messagesContainer.appendChild(wrapWithAvatar('assistant', thinkingEl));
        if (autoScroll) scrollToBottom();

        // Progressive thinking text
        var textEl = thinkingEl.querySelector('.abao-thinking-text');
        thinkingTimer = setTimeout(function() {
            if (textEl) {
                textEl.style.display = 'inline';
                textEl.textContent = '二狗正在想...';
            }
        }, 3000);

        setTimeout(function() {
            if (textEl && textEl.style.display === 'inline') {
                textEl.textContent = '这个问题有点复杂，再等等...';
            }
        }, 8000);
    }

    function hideThinking() {
        if (thinkingEl) {
            // On mobile, thinkingEl is wrapped in .abao-msg-row; remove the wrapper
            var toRemove = thinkingEl.parentNode && thinkingEl.parentNode.classList &&
                thinkingEl.parentNode.classList.contains('abao-msg-row')
                ? thinkingEl.parentNode : thinkingEl;
            if (toRemove && toRemove.parentNode) {
                toRemove.parentNode.removeChild(toRemove);
            }
        }
        thinkingEl = null;
        if (thinkingTimer) {
            clearTimeout(thinkingTimer);
            thinkingTimer = null;
        }
    }

    function scrollToBottom() {
        if (messagesContainer) {
            messagesContainer.scrollTop = messagesContainer.scrollHeight;
        }
    }

    // ─── Page context for AI awareness ───
    function getPageContext() {
        // Detect which page is visible
        var page = '';
        var detailId = '';

        if (typeof currentPage === 'string') {
            page = currentPage;
        } else {
            // Fallback: detect by visible view
            var views = ['todo', 'routine', 'review', 'english', 'life', 'settings'];
            for (var i = 0; i < views.length; i++) {
                var el = document.getElementById(views[i] + '-view');
                if (el && el.style.display !== 'none' && el.offsetParent !== null) {
                    page = views[i];
                    break;
                }
            }
        }

        // Detect open detail/modal
        if (page === 'life' || page === 'expense') {
            var expDetail = document.getElementById('expense-detail-overlay');
            if (expDetail && expDetail.style.display !== 'none') {
                // Get current detail ID from Expense module
                if (typeof Expense !== 'undefined' && Expense.getCurrentDetailId) {
                    detailId = Expense.getCurrentDetailId() || '';
                }
            }
            page = 'expense';
        } else if (page === 'english') {
            var detailView = document.querySelector('.english-detail-view');
            if (detailView && detailView.style.display !== 'none') {
                if (typeof English !== 'undefined' && English.getCurrentId) {
                    detailId = English.getCurrentId() || '';
                }
            }
        } else if (page === 'todo') {
            var taskCard = document.getElementById('task-card-overlay');
            if (taskCard && taskCard.style.display !== 'none') {
                var idEl = document.getElementById('task-card-id');
                if (idEl) detailId = idEl.textContent || '';
            }
        }

        if (!page) return undefined;
        var ctx = { page: page };
        if (detailId) ctx.detail_id = detailId;
        return ctx;
    }

    // ─── Send message ───
    async function sendMessage() {
        if (!inputEl || isSending) return;
        // Guest AI exhausted check
        if (window._userStatus === 'guest' && window._guestAiRemaining <= 0) {
            showToast('AI 体验次数已用完 — 注册账户解锁无限使用', 'warning');
            return;
        }
        var text = inputEl.value.trim();
        if (!text) return;

        // Clear input
        inputEl.value = '';
        inputEl.style.height = 'auto';

        // Add user message
        addMessage('user', text);

        // Disable input
        isSending = true;
        if (sendBtn) sendBtn.disabled = true;
        showThinking();

        // Notify patrol system: AI thinking
        document.dispatchEvent(new CustomEvent('patrol:chatStatus', { detail: { status: 'thinking' } }));

        try {
            var resp = await fetch('/api/chat', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                credentials: 'same-origin',
                body: JSON.stringify({
                    message: text,
                    conversation_id: conversationId || undefined,
                    page_context: getPageContext()
                })
            });

            if (resp.status === 401) {
                window.location.href = '/login.html';
                return;
            }

            var data = await resp.json();

            // 对话不存在（服务器重启丢失了 session），自动重置后重发，用户无感知
            if (resp.status === 404 || data.message === '对话不存在') {
                conversationId = null;
                // 直接用新对话重发，不再添加用户气泡
                var retryResp = await fetch('/api/chat', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    credentials: 'same-origin',
                    body: JSON.stringify({ message: text, page_context: getPageContext() })
                });
                data = await retryResp.json();
            }

            hideThinking();

            if (data.conversation_id) {
                conversationId = data.conversation_id;
            }

            // Sync guest AI remaining
            if (data.ai_remaining !== undefined) {
                window._guestAiRemaining = data.ai_remaining;
                var guestEl = document.getElementById('guest-ai-count');
                if (guestEl) guestEl.textContent = data.ai_remaining;
                updateAbaoGuestHint();
                if (typeof updateAllGuestAiHints === 'function') updateAllGuestAiHints();
            }

            if (data.success && data.reply) {
                addMessage('assistant', data.reply);
                // Show tool call results (task cards etc.)
                if (data.tool_calls) {
                    addToolInfo(data.tool_calls);
                    // Refresh task list if tools modified data
                    refreshTasksIfNeeded(data.tool_calls);
                }
            } else if (data.message) {
                addMessage('error', sanitizeErrorMessage(data.message));
            } else {
                addMessage('error', '二狗想了太久，请重试一下');
            }
        } catch (err) {
            hideThinking();
            addMessage('error', '网络错误，请检查连接');
            console.error('[Abao] send error:', err);
        } finally {
            isSending = false;
            if (sendBtn) sendBtn.disabled = false;
            if (!_isMobile && inputEl) inputEl.focus();
            // Notify patrol system: AI done
            document.dispatchEvent(new CustomEvent('patrol:chatStatus', { detail: { status: 'idle' } }));
        }
    }

    // ─── Refresh UI if tool calls modified data ───
    function refreshTasksIfNeeded(toolCalls) {
        if (!toolCalls) return;
        var refreshed = {};
        for (var i = 0; i < toolCalls.length; i++) {
            var tool = toolCalls[i].tool || '';
            // Todos
            if (!refreshed.todo && (tool.indexOf('todo') >= 0 || tool === 'batch_update_todos' || tool === 'create_reminder')) {
                refreshed.todo = true;
                if (typeof loadItems === 'function') loadItems();
            }
            // Routines
            if (!refreshed.routine && tool.indexOf('routine') >= 0) {
                refreshed.routine = true;
                if (typeof window.loadRoutines === 'function') window.loadRoutines();
            }
            // Reviews
            if (!refreshed.review && tool.indexOf('review') >= 0) {
                refreshed.review = true;
                if (typeof window.loadReviews === 'function') window.loadReviews();
            }
            // English/Learning
            if (!refreshed.english && tool.indexOf('english_scenario') >= 0) {
                refreshed.english = true;
                if (typeof English !== 'undefined' && English.init) English.init();
            }
            // Expenses
            if (!refreshed.expense && tool.indexOf('expense') >= 0) {
                refreshed.expense = true;
                if (typeof Expense !== 'undefined' && Expense.init) Expense.init();
            }
            // Trips
            if (!refreshed.trip && tool.indexOf('trip') >= 0) {
                refreshed.trip = true;
                if (typeof Trip !== 'undefined' && Trip.init) Trip.init();
            }
        }
    }

    // ─── Load conversation history ───
    async function loadConversation(convId) {
        try {
            var resp = await fetch('/api/conversations/' + encodeURIComponent(convId) + '/messages', {
                credentials: 'same-origin'
            });
            var data = await resp.json();
            if (data.success && data.items) {
                clearMessages();
                conversationId = convId;
                for (var i = 0; i < data.items.length; i++) {
                    var msg = data.items[i];
                    if (msg.role === 'user' || msg.role === 'assistant') {
                        addMessage(msg.role, msg.content_text || '');
                    }
                }
            }
        } catch (err) {
            console.error('[Abao] load conversation error:', err);
        }
    }

    // ─── Helpers ───
    function escapeHtml(text) {
        var div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    function sanitizeErrorMessage(msg) {
        if (!msg || typeof msg !== 'string') return '出了点问题，请重试';
        // Catch raw technical errors that shouldn't be shown to users
        if (/API request failed|error sending request|https?:\/\/\S+/i.test(msg)) {
            console.warn('[Abao] raw error suppressed:', msg);
            return 'AI 服务暂时不可用，请稍后重试';
        }
        if (/No text in .* response|parse error|Failed to parse/i.test(msg)) {
            return 'AI 服务响应异常，请稍后重试';
        }
        return msg;
    }

    function quadrantLabel(q) {
        var labels = {
            'important-urgent': '优先处理',
            'important-not-urgent': '翻牌子',
            'not-important-urgent': '短平快',
            'not-important-not-urgent': '待分类'
        };
        return labels[q] || q;
    }

    // ─── Guest AI hint ───
    function updateAbaoGuestHint() {
        if (window._userStatus !== 'guest') return;
        var hint = document.getElementById('abao-guest-hint');
        if (!hint) {
            // Create hint element next to input
            var inputArea = document.querySelector('.abao-input-area');
            if (inputArea) {
                hint = document.createElement('div');
                hint.id = 'abao-guest-hint';
                hint.style.cssText = 'font-size:11px;color:var(--text-muted,#8b949e);padding:2px 12px 4px;text-align:right;';
                inputArea.parentNode.insertBefore(hint, inputArea);
            }
        }
        if (hint) {
            var r = window._guestAiRemaining || 0;
            if (r > 0) {
                hint.textContent = 'AI 剩余 ' + r + ' 次';
                hint.style.color = '';
            } else {
                hint.innerHTML = 'AI 次数已用完 — <a href="/login.html" style="color:var(--primary-color,#667eea)">注册解锁</a>';
                if (inputEl) { inputEl.disabled = true; inputEl.placeholder = '注册账户解锁无限 AI 对话'; }
                if (sendBtn) sendBtn.disabled = true;
            }
        }
    }

    // ─── Public API ───
    return {
        init: init,
        open: open,
        close: close,
        toggle: toggle,
        loadConversation: loadConversation,
        isOpen: function() { return isOpen; },
        updateGuestHint: updateAbaoGuestHint
    };
})();

// Initialize on DOM ready
document.addEventListener('DOMContentLoaded', function() {
    Abao.init();
    // Show guest AI hint after a brief delay (wait for checkAuth)
    setTimeout(function() { Abao.updateGuestHint(); }, 500);
});
