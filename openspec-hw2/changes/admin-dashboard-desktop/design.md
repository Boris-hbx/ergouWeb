## Context

The current admin functionality is a small panel embedded at the bottom of the Settings view (`admin.js`, 212 lines). It provides read-only stats (DAU/WAU, feature counts, AI token totals) and basic user approval. The panel is only visible when the API call to `/api/admin/dashboard` succeeds (role=admin check).

This change upgrades the admin experience to a full-featured console with 7 capabilities: RBAC, user management, chat monitoring, AI dashboard, risk monitoring, system monitoring, and audit logging.

**Current state:**
- Backend: `routes/admin.rs` has 5 endpoints (dashboard, pending-users, approve, reject, security-events, restore)
- Frontend: `admin.js` (212 lines) renders into a `<div>` inside the settings view
- Auth: `users.role` is TEXT with values `admin` or `user`; first user gets `admin`
- DB: `security_events`, `ai_usage`, `conversations`, `chat_messages` tables already exist

**Constraints:**
- Vanilla JS (no framework) — all UI is imperative DOM manipulation
- Single SQLite file with global mutex — queries must be efficient
- API keys stay in environment variables, never in DB

## Goals / Non-Goals

**Goals:**
- Standalone admin console as a top-level view (not buried in settings)
- Three-tier RBAC: owner > admin > user
- Complete user lifecycle management
- AI conversation browsing for quality and security monitoring
- Token consumption visibility by model, user, and time period
- Security event dashboard with conversation linking
- System health at a glance
- Immutable audit trail for all admin operations

**Non-Goals:**
- Mobile-optimized admin layout (desktop-first, functional but not polished on mobile)
- Real-time WebSocket updates (polling/manual refresh is fine)
- Model configuration write UI (API keys stay in env vars)
- Data export/download functionality (future consideration)
- Automated security response (admin reviews and acts manually)

## Decisions

### 1. Admin console as a new view tab (not a separate page)

**Decision:** Add an "admin" view in the existing SPA view-switching system (like tasks, life, health, etc.), gated by role check.

**Alternatives considered:**
- Separate `/admin.html` page: Cleaner isolation but duplicates layout, auth, navigation code. Harder to maintain.
- Modal/overlay: Too small for the amount of content.

**Rationale:** The app already has a view-switching pattern. Adding another view is the lowest-friction approach. The navigation entry is conditionally rendered based on role from the login/session response.

### 2. Role migration: admin → owner

**Decision:** Add a DB migration that converts the existing `admin` role to `owner` for exactly one user (the first admin by created_at). All other admins remain `admin`. New installations set the first registrant as `owner`.

**Rationale:** This is a safe, one-time migration. The existing `boris_dev` force-admin migration in `db.rs` will be updated to set `owner` instead.

### 3. Backend permission extractors

**Decision:** Create two new Axum extractors: `AdminUserId` (requires role=admin or owner) and `OwnerUserId` (requires role=owner). These replace the current `require_admin()` helper function approach.

**Alternatives considered:**
- Middleware layer: More complex, harder to apply selectively to routes.
- Keep helper function: Works but requires manual calls in every handler.

**Rationale:** Extractors are idiomatic Axum. They fail fast before the handler body, produce clean error responses, and are composable. The existing `UserId` / `ActiveUserId` pattern is the precedent.

### 4. Frontend admin module structure

**Decision:** Create a new `admin-panel.js` file (replacing the old `admin.js`) organized as a module with sub-sections:

```
AdminPanel = {
    init(),           // Setup, check role, render skeleton
    showSection(id),  // Switch between sub-sections
    Users: { ... },   // User management
    Chats: { ... },   // Conversation monitor
    AI: { ... },      // AI dashboard
    Risk: { ... },    // Risk monitoring
    System: { ... },  // System status
    Audit: { ... },   // Audit log
}
```

**Rationale:** Single file keeps it simple (no build step). Internal namespacing via sub-objects keeps code organized. Old `admin.js` is small enough to fully replace.

### 5. Admin console layout: sidebar navigation + content area

**Decision:** The admin view uses an internal sidebar (left) with section links and a content area (right). Not tabs—a vertical nav scales better with 6+ sections.

```
┌─────────────────────────────────────────────┐
│  Admin Console                    [user ▾]  │
├──────────┬──────────────────────────────────┤
│ Overview │  [Dashboard cards / stats]       │
│ Users    │                                  │
│ Chats    │  Content area renders            │
│ AI       │  based on selected section       │
│ Risk     │                                  │
│ System   │                                  │
│ Audit    │                                  │
└──────────┴──────────────────────────────────┘
```

### 6. New API endpoints

| Endpoint | Method | Auth | Purpose |
|----------|--------|------|---------|
| `/api/admin/users` | GET | admin | Full user list with stats |
| `/api/admin/users/{id}/role` | PUT | owner | Change user role |
| `/api/admin/users/{id}/force-logout` | POST | admin | Invalidate sessions |
| `/api/admin/conversations` | GET | admin | List all conversations (paginated) |
| `/api/admin/conversations/{id}/messages` | GET | admin | Get messages for a conversation |
| `/api/admin/ai-usage` | GET | admin | Token stats by model/period |
| `/api/admin/ai-usage/providers` | GET | admin | Provider config status |
| `/api/admin/system-status` | GET | admin | Server info, DB stats, storage |
| `/api/admin/audit-log` | GET | admin | Audit log entries (paginated) |

Existing endpoints retained: `dashboard`, `pending-users`, `approve`, `reject`, `security-events`, `restore`.

### 7. Audit log implementation

**Decision:** Create `admin_audit_log` table. Insert audit entries directly in route handlers (not via middleware) since only specific operations need logging.

**Rationale:** A helper function `insert_audit_log(db, admin_id, action, target, details)` called at each operation site is simple and explicit. Middleware-based audit would be overkill for ~10 specific operations.

### 8. Conversation pagination strategy

**Decision:** Use offset-based pagination (LIMIT/OFFSET) for conversation listing. Page size = 20.

**Alternatives considered:**
- Cursor-based: Better for large datasets, but conversations table is small (thousands, not millions).
- Load-all: Would block the DB mutex too long.

**Rationale:** OFFSET is simple to implement in both backend and frontend. The conversation table has an index on `(user_id, updated_at DESC)` which supports efficient sorted queries.

## Risks / Trade-offs

**[Risk] DB mutex contention from admin queries** → Admin queries (user stats, conversation lists) may hold the global lock for longer than typical user operations. Mitigation: Keep queries efficient with proper indexes. Admin usage is low-frequency, so contention is unlikely in practice.

**[Risk] Role migration breaks existing admin login** → If migration runs wrong, the existing admin could lose access. Mitigation: Migration is conservative—promotes exactly one admin to owner, doesn't demote anyone. Rollback: manually `UPDATE users SET role='admin' WHERE role='owner'`.

**[Risk] Conversation data volume** → A user with 500 conversations × 50 messages each = 25K rows. Loading all messages at once would be slow. Mitigation: Conversations are listed without messages; messages load only when expanded. Pagination limits list queries.

**[Risk] Privacy concern with conversation monitoring** → Admin reading all user conversations could be sensitive. Mitigation: This is an owner-operated personal productivity tool with known users, not a public SaaS. Audit log records admin access patterns for accountability.

**[Risk] Large admin.js file** → Putting all 7 sections in one file could exceed 1000 lines. Trade-off accepted: Vanilla JS without modules means one file is simplest. Internal organization with sub-objects keeps it manageable.

## Migration Plan

1. **DB migration** (automatic on server start):
   - Add `admin_audit_log` table
   - Convert first `admin` user to `owner` role
   - Update `boris_dev` force-admin logic to set `owner`

2. **Backend deployment**: New routes are additive (no breaking changes to existing endpoints). Old admin endpoints continue to work.

3. **Frontend deployment**: Replace `admin.js` with `admin-panel.js`. Add `admin-panel.css`. Update `index.html` with new admin view container and script/style references.

4. **Rollback**: If issues arise, revert frontend files. Backend migration is safe — `owner` role is a superset of `admin`, so reverting frontend still works with the old admin check (`role = 'admin' OR role = 'owner'`).
