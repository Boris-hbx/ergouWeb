## ADDED Requirements

### Requirement: Token consumption overview by model and period
The AI dashboard SHALL display total token consumption (input + output) broken down by LLM provider (Claude, Kimi, Doubao) and time period (today, 7 days, 30 days).

#### Scenario: View token overview
- **WHEN** admin opens the AI dashboard section
- **THEN** the system displays a summary grid: rows = providers (Claude, Kimi, Doubao, Total), columns = time periods (Today, 7 Days, 30 Days), cells = token counts (input/output/total)

#### Scenario: No usage for a provider
- **WHEN** no AI calls have been made via Kimi
- **THEN** the Kimi row shows 0 for all periods

### Requirement: Provider status display
The AI dashboard SHALL display the configuration status of each LLM provider: configured (API key present) or not configured.

#### Scenario: Provider with API key set
- **WHEN** the ANTHROPIC_API_KEY environment variable is set and non-empty
- **THEN** the Claude provider shows a green "Configured" badge

#### Scenario: Provider without API key
- **WHEN** the KIMI_API_KEY environment variable is not set
- **THEN** the Kimi provider shows a gray "Not Configured" badge

### Requirement: Per-user AI consumption ranking
The AI dashboard SHALL display a ranked table of users by total token consumption, showing: user display name, message count, input tokens, output tokens, total tokens, primary model used.

#### Scenario: View user ranking
- **WHEN** admin views the AI dashboard
- **THEN** users are listed in descending order of total tokens consumed (input + output)

#### Scenario: Click user row for conversation detail
- **WHEN** admin clicks on a user in the ranking table
- **THEN** the conversation monitor opens filtered to that user's conversations

### Requirement: Model configuration is read-only
The AI dashboard SHALL display current model configuration (default model, fallback order) but SHALL NOT allow modification through the UI. API keys SHALL NOT be displayed or stored in the database.

#### Scenario: View model config
- **WHEN** admin views the AI dashboard
- **THEN** the current default model and fallback order are displayed (e.g., "Claude → Doubao → Kimi")

#### Scenario: No edit controls for model config
- **WHEN** admin views the model configuration section
- **THEN** no edit buttons or input fields are shown for API keys or model configuration
