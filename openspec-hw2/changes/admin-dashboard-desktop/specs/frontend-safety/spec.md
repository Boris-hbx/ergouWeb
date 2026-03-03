## ADDED Requirements

### Requirement: Admin console route guard
The frontend SHALL check the current user's role before rendering the admin console. If role is not `admin` or `owner`, the admin navigation entry SHALL be hidden and direct navigation to the admin view SHALL be blocked.

#### Scenario: User with role=user navigates to admin view
- **WHEN** a user with role=user attempts to access the admin view via URL manipulation or JS console
- **THEN** the system redirects to the default view (tasks) and the admin panel is not rendered

#### Scenario: Role downgrade during session
- **WHEN** a user's role is changed from admin to user while they are viewing the admin console
- **THEN** on the next API call, the system returns 403, and the frontend hides the admin console

### Requirement: Role-aware navigation rendering
The main navigation SHALL conditionally render the admin entry based on the user's role returned by the login/session API.

#### Scenario: Login response includes role
- **WHEN** user logs in successfully
- **THEN** the API response includes the user's role field, and the frontend uses it to determine admin entry visibility
