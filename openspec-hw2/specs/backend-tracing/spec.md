## ADDED Requirements

### Requirement: Tracing initialization
The system SHALL initialize `tracing-subscriber` in `main.rs` with compact format output. Log level SHALL default to `info` in production, configurable via `RUST_LOG` env var.

#### Scenario: Server startup logging
- **WHEN** the server starts
- **THEN** `tracing-subscriber` is initialized and all subsequent `tracing` macro calls produce structured log output

### Requirement: TraceLayer middleware
The system SHALL add `tower-http::TraceLayer` middleware to the Axum router, automatically logging every HTTP request with method, path, status code, and duration.

#### Scenario: Request logged automatically
- **WHEN** any HTTP request is processed
- **THEN** a log line like `INFO [http] 200 PUT /api/trips/42 89ms` is emitted

### Requirement: Log level conventions
The system SHALL follow these log level conventions for new code:
- `error!` — faults requiring investigation (DB errors, external API failures)
- `warn!` — recoverable anomalies (rate limiting, token expiry)
- `info!` — key business events (user registration, AI call completion)
- `debug!` — development details (SQL params, request bodies)

#### Scenario: DB error logged at error level
- **WHEN** a database operation fails
- **THEN** the error is logged with `error!` macro including relevant context

### Requirement: Gradual eprintln migration
New code SHALL use `tracing` macros. Existing `eprintln!()` calls SHALL NOT be migrated in bulk; they SHALL be replaced when the file is modified for other reasons.

#### Scenario: New route uses tracing
- **WHEN** a developer adds a new route handler
- **THEN** error and info logging uses `tracing` macros, not `eprintln!()`
