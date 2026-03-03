## ADDED Requirements

### Requirement: Self-hosted Eruda.js
The system SHALL host `eruda.min.js` at `frontend/assets/vendor/eruda.min.js`. The file SHALL NOT be included in SW cache lists.

#### Scenario: Eruda file available
- **WHEN** a request is made to `/assets/vendor/eruda.min.js`
- **THEN** the file is served from the static file directory

### Requirement: URL parameter trigger
The system SHALL load and initialize Eruda when the URL contains `?debug=1`. The user MUST be logged in. The debug state SHALL be persisted in `localStorage('eruda_enabled')`. `?debug=0` SHALL disable and remove Eruda.

#### Scenario: Enable via URL
- **WHEN** a logged-in user navigates to any page with `?debug=1`
- **THEN** Eruda is dynamically loaded and initialized, and `eruda_enabled=1` is stored in localStorage

#### Scenario: Disable via URL
- **WHEN** a user navigates with `?debug=0`
- **THEN** Eruda is destroyed, `eruda_enabled` is removed from localStorage

#### Scenario: Persist across refresh
- **WHEN** Eruda was enabled and the page is refreshed (without ?debug param)
- **THEN** Eruda is re-initialized from the localStorage state

### Requirement: Hidden gesture trigger
The system SHALL enable Eruda when the user taps the version number 5 times consecutively. A second 5-tap sequence SHALL disable it.

#### Scenario: Five-tap activation
- **WHEN** the user taps the version/about element 5 times within 3 seconds
- **THEN** Eruda is loaded and initialized

### Requirement: Graceful failure
Loading Eruda SHALL NOT block or affect normal application functionality.

#### Scenario: Eruda load failure
- **WHEN** `eruda.min.js` fails to load (offline, network error)
- **THEN** no error is shown to the user and the app continues normally
