## ADDED Requirements

### Requirement: SW error capture
Service Worker catch blocks SHALL log errors with `console.error('[SW]', error)` instead of silently swallowing them. The SW SHALL also listen for `error` and `unhandledrejection` events.

#### Scenario: SW fetch handler error
- **WHEN** the SW fetch handler encounters an error
- **THEN** the catch block logs `console.error('[SW]', error)` before falling back

#### Scenario: SW global error
- **WHEN** an uncaught error occurs in the SW scope
- **THEN** the `error` event listener logs it with `console.error('[SW] uncaught:', event.error)`
