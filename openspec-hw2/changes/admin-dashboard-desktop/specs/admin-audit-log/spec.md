## ADDED Requirements

### Requirement: Audit log table schema
The system SHALL maintain an `admin_audit_log` table with columns: id (TEXT PK), admin_user_id (TEXT FK), action_type (TEXT), target_user_id (TEXT, nullable), target_resource (TEXT, nullable), details (TEXT), created_at (TEXT).

#### Scenario: Table exists on startup
- **WHEN** the server starts
- **THEN** the admin_audit_log table exists with the specified schema

### Requirement: Automatic audit logging for admin operations
The system SHALL automatically insert an audit log entry for every admin operation: role changes, user approval/rejection, user suspension/restoration, force logout, marking security events as reviewed.

#### Scenario: Role change logged
- **WHEN** owner promotes a user to admin
- **THEN** an audit log entry is created with action_type="role_change", target_user_id=the promoted user, details="user→admin"

#### Scenario: User suspension logged
- **WHEN** admin suspends a user
- **THEN** an audit log entry is created with action_type="suspend_user", target_user_id=the suspended user

#### Scenario: Security event review logged
- **WHEN** admin marks a security event as reviewed
- **THEN** an audit log entry is created with action_type="review_security_event", target_resource=event_id

### Requirement: Audit log viewing interface
The admin console SHALL display the audit log as a chronological list with columns: timestamp, admin user, action type, target, details. The list SHALL support filtering by admin user and action type, with pagination (50 per page).

#### Scenario: View audit log
- **WHEN** admin opens the audit log section
- **THEN** the system displays the 50 most recent audit entries, newest first

#### Scenario: Filter by action type
- **WHEN** admin selects filter "role_change"
- **THEN** only role change entries are displayed

### Requirement: Audit log is append-only
Audit log entries SHALL NOT be deletable or editable through any API endpoint. The only operation allowed is reading/listing.

#### Scenario: No delete endpoint
- **WHEN** any request is made to delete an audit log entry
- **THEN** no such endpoint exists; the request returns 404

### Requirement: Audit log visibility
Both admin and owner roles SHALL be able to view the audit log. The audit log SHALL be read-only for all roles.

#### Scenario: Admin views audit log
- **WHEN** a user with role=admin opens the audit log
- **THEN** the full audit log is visible (read-only)
