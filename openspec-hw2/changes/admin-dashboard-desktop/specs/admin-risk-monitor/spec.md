## ADDED Requirements

### Requirement: Risk event dashboard
The admin console SHALL display security events from the security_events table, sorted by severity (high → medium → low) then by created_at descending. High-severity events SHALL be visually prominent (red highlight).

#### Scenario: View risk events
- **WHEN** admin opens the risk monitoring section
- **THEN** the system displays security events with columns: severity badge, event type, user, description, timestamp

#### Scenario: High-severity event display
- **WHEN** a security event has severity=high
- **THEN** it appears at the top with a red severity badge

### Requirement: Filter risk events
The event list SHALL support filtering by severity level, event type, and user.

#### Scenario: Filter by severity
- **WHEN** admin selects severity filter "high"
- **THEN** only high-severity events are shown

#### Scenario: Filter by user
- **WHEN** admin selects a specific user
- **THEN** only events from that user are shown, revealing repeat-offender patterns

### Requirement: Link risk events to conversations
Each security event that has an associated conversation_id SHALL provide a clickable link to view the full conversation context in the chat monitor.

#### Scenario: Navigate from event to conversation
- **WHEN** admin clicks the "View Conversation" link on a security event
- **THEN** the chat monitor opens showing the conversation where the event occurred, with the flagged message highlighted

#### Scenario: Event without conversation link
- **WHEN** a security event has no conversation_id
- **THEN** no conversation link is shown (only the event description)

### Requirement: Risk user summary
The risk monitor SHALL show a summary of users with multiple security events, including event count and last event timestamp.

#### Scenario: Repeat offender visibility
- **WHEN** a user has triggered 3+ security events
- **THEN** the risk user summary section lists that user with their total event count and last event time

### Requirement: Admin action on risk events
Admin SHALL be able to mark events as reviewed and optionally suspend the associated user directly from the risk monitor.

#### Scenario: Mark event as reviewed
- **WHEN** admin clicks "Mark Reviewed" on a security event
- **THEN** the event's admin_notified field is set to 1

#### Scenario: Suspend user from risk event
- **WHEN** admin clicks "Suspend User" on a security event and confirms
- **THEN** the associated user's status changes to suspended, an audit log entry is recorded
