# TEST-work-zhongshu

> Task: T-174
> Scope: work task table "zhongshu" owner view, area, engage, attention view

## Backend

- `work_tasks` includes top-level `area` and `engage` fields with empty-string defaults.
- Existing databases get `area` and `engage` through idempotent migrations.
- `GET /api/work/tasks` returns `area` and `engage` on each item.
- `POST /api/work/tasks` accepts `area` and `engage`.
- `PATCH /api/work/tasks/:id` updates `area` and `engage`.
- `GET /api/work/columns` seeds built-in `area` and `engage` rows for new users.
- Existing users get missing built-in `area` and `engage` rows without duplicates.
- `engage` is a sys column; its options are fixed by the app.
- `area` is a built-in select column but not sys; users may edit its options.

## Frontend

- Entering the work task table defaults to the "zhongshu" view.
- The view segment contains seven views: zhongshu, table, board, calendar, person, distribution, attention.
- The table view still works and can edit/filter/group `area` and `engage`.
- The create dialog includes `area` and `engage` through the column configuration.
- The zhongshu segment is highlighted on first entry; the table segment is not highlighted until selected.
- Zhongshu action area is split into `decide`, `push`, and `do` sub-sections.
- Zhongshu shows all `decide` and `push` tasks, but only shows `do` tasks when they are P0 or due within 2 days.
- Zhongshu risk means overdue or due within 2 days, and risk items are not duplicated if already visible in action/track sections.
- Zhongshu shows stale `track` items when overdue or due within 3 days while not `doing`; other `track` items are folded.
- Zhongshu counts `inform` items without expanding them by default.
- Clicking a zhongshu card opens the existing T-100 detail drawer.
- Changing `engage` from a zhongshu card moves the card to the correct section after re-render.
- Attention view groups by `area` and shows `decide + push` action count, overdue count, P0 count, stale track count, and total.
- Clicking an attention row or metric drills down to table view with preset filters.

## Compatibility

- No new `/api/work/*` endpoints are introduced.
- No new LLM tool names are introduced.
- Existing work views keep excluding `status === "done"` except documented calendar behavior.
