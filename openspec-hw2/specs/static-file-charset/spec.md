### Requirement: Text responses include charset=utf-8
The server SHALL append `; charset=utf-8` to the Content-Type header of all HTTP responses whose Content-Type starts with `text/` and does not already contain a `charset` parameter.

#### Scenario: CSS file served with charset
- **WHEN** a browser requests a CSS file (e.g., `/assets/css/style.css`)
- **THEN** the response Content-Type SHALL be `text/css; charset=utf-8`

#### Scenario: JS file served with charset
- **WHEN** a browser requests a JS file via ServeDir (e.g., `/assets/js/app.js`)
- **THEN** the response Content-Type SHALL be `text/javascript; charset=utf-8`

#### Scenario: HTML file charset preserved
- **WHEN** a browser requests an HTML page (e.g., `/` or `/index.html`)
- **THEN** the response Content-Type SHALL remain `text/html; charset=utf-8` (already set by Html wrapper)

#### Scenario: Non-text content-type unaffected
- **WHEN** a browser requests a binary file (e.g., PNG, ICO)
- **THEN** the response Content-Type SHALL NOT be modified (e.g., `image/png` stays as-is)

### Requirement: Service Worker script includes charset
The server SHALL serve `/sw.js` with Content-Type `application/javascript; charset=utf-8`.

#### Scenario: sw.js served with charset
- **WHEN** a browser requests `/sw.js`
- **THEN** the response Content-Type SHALL be `application/javascript; charset=utf-8`
