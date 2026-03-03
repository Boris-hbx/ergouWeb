## ADDED Requirements

### Requirement: User list with search, sort, and filter
The admin console SHALL display a list of all registered users with columns: display name, username, role, status, last active time, registration date. The list SHALL support text search (by username/display name), filtering by status and role, and sorting by any column.

#### Scenario: Search by username
- **WHEN** admin types "boris" in the search field
- **THEN** the list shows only users whose username or display name contains "boris" (case-insensitive)

#### Scenario: Filter by status
- **WHEN** admin selects status filter "pending"
- **THEN** the list shows only users with status=pending

#### Scenario: Sort by last active time
- **WHEN** admin clicks the "Last Active" column header
- **THEN** the list re-sorts by last_active descending (most recent first)

### Requirement: User detail panel
Selecting a user in the list SHALL open a detail panel showing: registration info, last active time, feature usage counts (todos, expenses, trips, conversations), and AI token consumption summary.

#### Scenario: View user details
- **WHEN** admin clicks on a user row
- **THEN** a side panel opens showing that user's profile, usage stats, and AI consumption

#### Scenario: User with no activity
- **WHEN** admin views details of a user who has never logged in
- **THEN** all usage stats show 0, last active shows "Never"

### Requirement: Approve and reject pending users
Admin SHALL be able to approve (status → active) or reject (status → rejected) pending users directly from the user list or detail panel.

#### Scenario: Approve a pending user
- **WHEN** admin clicks "Approve" on a pending user
- **THEN** the user's status changes to active, a notification is sent to the user, the list refreshes

#### Scenario: Reject a pending user
- **WHEN** admin clicks "Reject" on a pending user and confirms
- **THEN** the user's status changes to rejected, a notification is sent to the user

### Requirement: Suspend and restore users
Admin SHALL be able to suspend active users (status → suspended) and restore suspended users (status → active).

#### Scenario: Suspend an active user
- **WHEN** admin clicks "Suspend" on an active user and confirms
- **THEN** the user's status changes to suspended, their active sessions are invalidated

#### Scenario: Suspend the owner
- **WHEN** admin attempts to suspend a user with role=owner
- **THEN** the system rejects with an error: "无法封禁系统所有者"

#### Scenario: Restore a suspended user
- **WHEN** admin clicks "Restore" on a suspended user
- **THEN** the user's status changes to active

### Requirement: Force logout user
Admin SHALL be able to force-logout a user by invalidating all their active sessions.

#### Scenario: Force logout
- **WHEN** admin clicks "Force Logout" on a user
- **THEN** all session records for that user are deleted from the sessions table
- **THEN** the user must re-login on their next request
