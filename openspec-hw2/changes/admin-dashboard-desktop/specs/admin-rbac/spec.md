## ADDED Requirements

### Requirement: Three-tier role hierarchy
The system SHALL support three user roles: `owner`, `admin`, `user`. The `owner` role is unique — only one user SHALL have this role at any time. Role hierarchy for privilege checks: owner > admin > user.

#### Scenario: Database migration sets first admin as owner
- **WHEN** the system starts and the users table contains a user with role=admin
- **THEN** the migration changes that user's role to `owner`, all other admin users remain `admin`

#### Scenario: New installation
- **WHEN** the first user registers on a fresh system
- **THEN** that user is assigned role=owner

### Requirement: Owner can grant admin role
The owner SHALL be able to promote any active user (role=user) to role=admin via the admin console.

#### Scenario: Promote a regular user to admin
- **WHEN** owner selects an active user with role=user and confirms promotion
- **THEN** the user's role is updated to `admin` in the database

#### Scenario: Promote a non-active user
- **WHEN** owner attempts to promote a user with status=pending or status=suspended
- **THEN** the system rejects the operation with an error message

### Requirement: Owner can revoke admin role
The owner SHALL be able to demote any admin (role=admin) back to role=user.

#### Scenario: Demote an admin
- **WHEN** owner selects an admin user and confirms demotion
- **THEN** the user's role is updated to `user`

#### Scenario: Owner cannot demote themselves
- **WHEN** owner attempts to change their own role
- **THEN** the system rejects the operation

### Requirement: Admin cannot modify roles
Users with role=admin SHALL NOT be able to grant or revoke any roles. Role management endpoints SHALL reject requests from non-owner users with HTTP 403.

#### Scenario: Admin tries to promote a user
- **WHEN** a user with role=admin calls the role-change API
- **THEN** the system returns HTTP 403 `{"success": false, "error": "仅系统所有者可变更角色"}`

### Requirement: Admin console visibility based on role
The admin console navigation entry SHALL be visible only to users with role=owner or role=admin. Users with role=user SHALL NOT see the admin entry in navigation.

#### Scenario: Regular user loads the app
- **WHEN** a user with role=user loads the application
- **THEN** no admin console entry appears in the navigation

#### Scenario: Admin user loads the app
- **WHEN** a user with role=admin loads the application
- **THEN** the admin console entry appears in the navigation

### Requirement: Backend admin permission guard
All admin API endpoints SHALL verify the caller has role=admin or role=owner. Endpoints that modify roles SHALL additionally verify role=owner.

#### Scenario: Unauthenticated request to admin API
- **WHEN** an unauthenticated request is made to any `/api/admin/*` endpoint
- **THEN** the system returns HTTP 401

#### Scenario: Regular user requests admin API
- **WHEN** a user with role=user requests any `/api/admin/*` endpoint
- **THEN** the system returns HTTP 403

#### Scenario: Admin requests role-change endpoint
- **WHEN** a user with role=admin requests `/api/admin/users/{id}/role`
- **THEN** the system returns HTTP 403
