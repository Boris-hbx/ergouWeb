## ADDED Requirements

### Requirement: Conversation list for all users
The admin console SHALL display a paginated list of all conversations across all users. Each row SHALL show: conversation title, user display name, message count, total token consumption, last updated time.

#### Scenario: Load conversation list
- **WHEN** admin opens the conversation monitor section
- **THEN** the system displays the 20 most recent conversations across all users, ordered by last updated time descending

#### Scenario: Pagination
- **WHEN** admin scrolls to the bottom of the list or clicks "Load more"
- **THEN** the system loads the next 20 conversations

### Requirement: Filter conversations by user and date
The conversation list SHALL support filtering by specific user (dropdown) and date range (from/to date pickers).

#### Scenario: Filter by user
- **WHEN** admin selects a user from the filter dropdown
- **THEN** only that user's conversations are displayed

#### Scenario: Filter by date range
- **WHEN** admin sets date range to "2026-02-01" through "2026-02-28"
- **THEN** only conversations with updated_at within that range are displayed

#### Scenario: Combined filters
- **WHEN** admin selects a user AND a date range
- **THEN** both filters are applied (AND logic)

### Requirement: View full conversation content
Admin SHALL be able to expand a conversation to view the complete message thread in a chat-like layout, showing user messages and AI responses with timestamps.

#### Scenario: Expand a conversation
- **WHEN** admin clicks on a conversation row
- **THEN** the system loads and displays all messages in that conversation in chronological order, with role indicators (user/assistant) and timestamps

#### Scenario: Conversation with security events
- **WHEN** admin views a conversation that has associated security events
- **THEN** the flagged messages are highlighted with a risk badge/indicator

### Requirement: Conversation monitor is read-only
The conversation monitor SHALL be strictly read-only. Admin SHALL NOT be able to delete, edit, or reply to any conversation or message.

#### Scenario: No edit controls shown
- **WHEN** admin views a conversation
- **THEN** no delete, edit, or reply buttons are displayed
