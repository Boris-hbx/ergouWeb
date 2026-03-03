## ADDED Requirements

### Requirement: Global error interception
The system SHALL intercept all uncaught frontend errors via `window.onerror` and `window.addEventListener('unhandledrejection')`.

#### Scenario: Uncaught synchronous error
- **WHEN** a JS runtime error is thrown and not caught
- **THEN** `window.onerror` captures the error message, source, line, column, and error object

#### Scenario: Unhandled promise rejection
- **WHEN** a Promise rejects without a catch handler
- **THEN** `unhandledrejection` listener captures the rejection reason

### Requirement: Breadcrumb ring buffer
The system SHALL maintain a ring buffer of the 20 most recent operations in memory. Only two categories SHALL be recorded:
- `api`: `METHOD /path → STATUS (duration_ms)` from `API.request()`
- `nav`: Tab/view switch identifier from route/tab changes

No click text, input content, or console output SHALL be recorded.

#### Scenario: API call breadcrumb
- **WHEN** `API.request()` completes (success or failure)
- **THEN** a breadcrumb entry `{ ts, cat: "api", msg: "POST /api/todos → 200 (145ms)" }` is pushed to the buffer

#### Scenario: Navigation breadcrumb
- **WHEN** user switches tab or view
- **THEN** a breadcrumb entry `{ ts, cat: "nav", msg: "trips" }` is pushed to the buffer

#### Scenario: Buffer overflow
- **WHEN** the buffer has 20 entries and a new one arrives
- **THEN** the oldest entry is evicted

### Requirement: Error report payload
The system SHALL send error reports containing: `error_message`, `stack`, `app_version` (from cache version string), `url`, `user_agent`, `screen_size`, `network_online`, `user_id` (if logged in), `breadcrumbs` (array), `timestamp` (ISO 8601).

#### Scenario: Error report structure
- **WHEN** an error is captured
- **THEN** a JSON payload with all specified fields is assembled for submission

### Requirement: Error report endpoint
The system SHALL provide `POST /api/client-errors` that accepts error reports without authentication (to allow login page errors). IP-level rate limiting: 10 reports per minute.

#### Scenario: Successful error submission
- **WHEN** a valid error report is POSTed
- **THEN** the server stores it in `client_errors` table and returns 200

#### Scenario: Rate limit exceeded
- **WHEN** more than 10 reports arrive from the same IP within 1 minute
- **THEN** the server returns 429

### Requirement: Frontend flood protection
The system SHALL limit error reports per page session to 10 and deduplicate by `error_message`.

#### Scenario: Session report limit
- **WHEN** 10 errors have been reported in the current page session
- **THEN** subsequent errors are captured locally but not sent to the server

#### Scenario: Duplicate deduplication
- **WHEN** the same `error_message` occurs again
- **THEN** it is not reported again

### Requirement: Offline buffering
The system SHALL buffer failed error reports in localStorage (max 5 entries). On next page load, buffered reports SHALL be re-sent.

#### Scenario: Network unavailable during report
- **WHEN** the POST to `/api/client-errors` fails (network error)
- **THEN** the report is stored in localStorage under a dedicated key

#### Scenario: Buffered reports flushed on load
- **WHEN** the page loads and localStorage contains buffered error reports
- **THEN** they are re-sent to the server and removed from localStorage on success

### Requirement: SQLite storage and auto-cleanup
The system SHALL store errors in `client_errors` table with columns: id, error_message, stack, app_version, url, user_agent, screen_size, network_online, user_id, breadcrumbs (JSON), created_at. Records older than 14 days SHALL be auto-cleaned daily.

#### Scenario: Auto-cleanup
- **WHEN** the daily cleanup task runs
- **THEN** records with `created_at` older than 14 days are deleted
