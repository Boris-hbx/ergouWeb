## ADDED Requirements

### Requirement: Server information display
The system monitor SHALL display: application version, server start time, current uptime.

#### Scenario: View server info
- **WHEN** admin opens the system monitor section
- **THEN** the system shows app version (from build config or Cargo.toml), server start time, and uptime in human-readable format (e.g., "3 days 5 hours")

### Requirement: Database statistics
The system monitor SHALL display: SQLite database file size, row counts for key tables (users, todos, conversations, chat_messages, expenses, trips).

#### Scenario: View database stats
- **WHEN** admin opens the system monitor
- **THEN** the system shows DB file size (e.g., "24.5 MB") and row counts for each major table

### Requirement: Storage usage display
The system monitor SHALL display: total upload directory size, number of uploaded files.

#### Scenario: View storage info
- **WHEN** admin opens the system monitor
- **THEN** the system shows total upload size (e.g., "156 MB") and file count

### Requirement: Recent error summary
The system monitor SHALL display the count of client errors in the last 24 hours and the last 7 days.

#### Scenario: View error summary
- **WHEN** admin opens the system monitor
- **THEN** the system shows "Client Errors: 3 (24h) / 12 (7d)"

#### Scenario: High error count warning
- **WHEN** the 24-hour error count exceeds 20
- **THEN** the error count is displayed in a warning color
