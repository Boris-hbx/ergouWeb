# TEST: Work Compound WIP Gate
> Task: T-282
> Date: 2026-06-29

## Cases

1. Hub shows Compound card
   - Open Work Hub.
   - Expect a visible card titled `复利` after `任务表`.
   - Expect locked/default description to be `开发中`.

2. Non-privileged users are blocked
   - Use a session where `/api/auth/me` returns role `user`, `guest`, missing role, or fails before user data is available.
   - Click `复利`.
   - Expect no placeholder page navigation.
   - Expect toast text `请联系管理员 Boris`.

3. Owner and admin can enter placeholder
   - Use a session where `/api/auth/me` returns role `owner` or `admin`.
   - Click `复利`.
   - Expect Work Hub to hide and the `复利` placeholder view to show.
   - Click back.
   - Expect return to Work Hub.

4. WIP gate is reusable
   - Add another entry to `WIP_FEATURES` with its card and view ids.
   - Expect `Work.refreshFeatureGates()` and `Work.openWipFeature(key)` to handle lock/unlock without feature-specific role logic.

5. Backend security boundary remains explicit
   - Confirm no new Compound API endpoint was added.
   - Confirm front-end code documents that future Compound APIs must use `AdminUserId`.
