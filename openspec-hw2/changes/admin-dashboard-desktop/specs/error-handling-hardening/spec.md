## ADDED Requirements

### Requirement: Admin API permission middleware
All admin API endpoints SHALL use a shared permission-checking middleware/extractor that verifies the caller's role. The middleware SHALL support two levels: `require_admin` (admin or owner) and `require_owner` (owner only).

#### Scenario: Admin-level endpoint accessed by user
- **WHEN** a user with role=user calls GET /api/admin/dashboard
- **THEN** the middleware returns HTTP 403 `{"success": false, "error": "权限不足"}` before the handler executes

#### Scenario: Owner-level endpoint accessed by admin
- **WHEN** a user with role=admin calls POST /api/admin/users/{id}/role
- **THEN** the middleware returns HTTP 403 `{"success": false, "error": "仅系统所有者可执行此操作"}`

#### Scenario: Admin-level endpoint accessed by admin
- **WHEN** a user with role=admin calls GET /api/admin/dashboard
- **THEN** the middleware allows the request through to the handler

### Requirement: Consistent error format for admin endpoints
All admin API endpoints SHALL follow the existing `{"success": false, "error": "..."}` response format for errors. Specific error codes SHALL be used for permission errors to allow frontend to react appropriately.

#### Scenario: Permission denied response
- **WHEN** any admin endpoint returns a permission error
- **THEN** the response body contains `{"success": false, "error": "PERMISSION_DENIED", "message": "..."}`
