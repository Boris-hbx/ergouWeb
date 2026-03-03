## Use Cases

### Use Case: View page with Chinese text rendered correctly

**Primary Actor:** End user (browser)
**Scope:** Next web application
**Level:** User goal

**Stakeholders and Interests:**
- End user — sees all Chinese UI text rendered correctly, no garbled characters
- Developer — static files served with proper encoding headers

**Preconditions:**
- Application is deployed and accessible
- Static assets (CSS/JS) contain Chinese characters (toast messages, UI labels)

**Success Guarantee (Postconditions):**
- All `text/*` responses include `charset=utf-8` in Content-Type header
- Chinese text in JS-rendered UI elements displays correctly
- No mojibake/乱码 on any page

**Trigger:** User opens the application in a browser

**Main Success Scenario:**
1. User navigates to the application URL.
2. Server returns HTML with `Content-Type: text/html; charset=utf-8`.
3. Browser requests CSS files. Server returns them with `Content-Type: text/css; charset=utf-8`.
4. Browser requests JS files. Server returns them with `Content-Type: text/javascript; charset=utf-8`.
5. Browser parses all files as UTF-8. Chinese text renders correctly throughout the UI.

**Extensions:**
- 3a. Browser has cached old response without charset: Service Worker update (new cache version) triggers re-fetch; new response includes charset.
- 4a. `sw.js` is requested: Server returns with `Content-Type: application/javascript; charset=utf-8`.

**Open Questions:**
- None — fix is straightforward and already implemented.
