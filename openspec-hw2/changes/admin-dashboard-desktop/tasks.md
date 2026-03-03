## 1. Database Migration & RBAC Foundation

- [x] 1.1 Add `admin_audit_log` table to `db.rs` schema (id, admin_user_id, action_type, target_user_id, target_resource, details, created_at)
- [x] 1.2 Add migration: convert first `admin` user to `owner` role; update `boris_dev` force-admin logic to set `owner`
- [x] 1.3 Create `AdminUserId` extractor in `auth.rs` — requires role=admin or owner, returns 403 otherwise
- [x] 1.4 Create `OwnerUserId` extractor in `auth.rs` — requires role=owner, returns 403 otherwise
- [x] 1.5 Add `insert_audit_log()` helper function in `routes/admin.rs`
- [x] 1.6 Update login/session API response to include `role` field

## 2. Backend: User Management APIs

- [x] 2.1 `GET /api/admin/users` — full user list with search, filter (status/role), sort, includes per-user usage stats (feature counts + AI token summary)
- [x] 2.2 `PUT /api/admin/users/{id}/role` — owner-only role change (user↔admin), with audit log
- [x] 2.3 `POST /api/admin/users/{id}/force-logout` — delete all sessions for target user, with audit log
- [x] 2.4 Update existing `approve_user` and `reject_user` to insert audit log entries
- [x] 2.5 Update existing `restore_user` to insert audit log entry
- [x] 2.6 Prevent suspension/role-change of owner (guard in handlers)

## 3. Backend: Conversation Monitor APIs

- [x] 3.1 `GET /api/admin/conversations` — paginated list (limit/offset), with filters (user_id, date_from, date_to), returns title, user_name, message_count, token_sum, updated_at
- [x] 3.2 `GET /api/admin/conversations/{id}/messages` — all messages for a conversation, including role, content_text, token_count, created_at

## 4. Backend: AI Dashboard APIs

- [x] 4.1 `GET /api/admin/ai-usage` — token consumption by model and period (today/7d/30d), per-user ranking
- [x] 4.2 `GET /api/admin/ai-usage/providers` — provider config status (check env var presence, don't expose keys)

## 5. Backend: Risk Monitor & System Status APIs

- [x] 5.1 Enhance existing `GET /api/admin/security-events` — add filters (severity, event_type, user_id), pagination, include conversation_id for linking
- [x] 5.2 `GET /api/admin/system-status` — app version, uptime, DB file size, table row counts, upload storage size, recent error counts
- [x] 5.3 `GET /api/admin/audit-log` — paginated list with filters (admin_user, action_type)

## 6. Backend: Route Registration

- [x] 6.1 Register all new admin routes in `main.rs` under the existing `/api/admin` nest
- [x] 6.2 Migrate existing admin endpoints to use `AdminUserId` extractor (replace manual `require_admin` calls)

## 7. Frontend: Admin Console Skeleton

- [x] 7.1 Add admin view container in `index.html` (sidebar nav + content area)
- [x] 7.2 Create `admin-panel.css` with admin layout styles (sidebar, cards, tables, stat boxes)
- [x] 7.3 Create `admin-panel.js` skeleton with `AdminPanel.init()`, section switching, role-gated visibility
- [x] 7.4 Add admin nav entry (conditionally rendered based on role from login response)
- [x] 7.5 Wire up view switching: clicking admin nav → show admin view, hide other views

## 8. Frontend: Overview Dashboard

- [x] 8.1 Build overview section: summary cards (total users, DAU, pending users, AI tokens today, security events today)
- [x] 8.2 Quick-action links from overview cards to respective sections

## 9. Frontend: User Management Section

- [x] 9.1 User list table with search input, role/status filter dropdowns, sortable columns
- [x] 9.2 User detail side panel: profile info, usage stats, AI consumption
- [x] 9.3 Action buttons: Approve, Reject, Suspend, Restore, Force Logout (contextual by status)
- [x] 9.4 Role change UI (owner only): "Set as Admin" / "Revoke Admin" with confirmation dialog
- [x] 9.5 Hide role-change controls for non-owner admins

## 10. Frontend: Conversation Monitor Section

- [x] 10.1 Conversation list with user filter dropdown, date range pickers, pagination
- [x] 10.2 Conversation detail view: chat-like layout showing user/assistant messages with timestamps
- [x] 10.3 Security event badges on flagged messages (link from risk monitor)

## 11. Frontend: AI Dashboard Section

- [x] 11.1 Token consumption grid: providers × periods, with totals
- [x] 11.2 Provider status badges (Configured / Not Configured)
- [x] 11.3 Per-user consumption ranking table (click to jump to conversation monitor filtered by user)
- [x] 11.4 Model config display (read-only: default model, fallback order)

## 12. Frontend: Risk Monitor Section

- [x] 12.1 Security event list with severity badges (color-coded), filters (severity, type, user)
- [x] 12.2 Event detail: description + "View Conversation" link (opens chat monitor at that conversation)
- [x] 12.3 Risk user summary: users with 3+ events, event count, last event time
- [x] 12.4 Action buttons: Mark Reviewed, Suspend User (with confirmation)

## 13. Frontend: System Status & Audit Log Sections

- [x] 13.1 System status card: version, uptime, DB size, table counts, storage usage, error counts
- [x] 13.2 Audit log table: timestamp, admin, action, target, details, with filters and pagination

## 14. Cleanup & Integration

- [x] 14.1 Remove old `admin.js` — all functionality replaced by `admin-panel.js`
- [x] 14.2 Remove old admin section from settings view in `index.html`
- [x] 14.3 Update cache version numbers in `index.html` for new CSS/JS files
- [x] 14.4 Run `cargo test` — all existing tests pass
- [x] 14.5 Run `cargo clippy` — no warnings
- [ ] 14.6 Manual smoke test: login as owner, verify all 7 admin sections render and function
