# TEST-praxis-contacts

> Task: T-283
> Scope: Praxis contacts v0.1

## Backend API

- `GET /api/praxis/contacts` requires owner/admin; normal user and guest receive 403.
- `POST /api/praxis/contacts` creates a contact for the current admin user only.
- `PATCH /api/praxis/contacts/:id` updates only the current admin user's contact.
- `DELETE /api/praxis/contacts/:id` soft-deletes the current admin user's contact.
- `name` is trimmed and must be 1-60 characters.
- `layer` accepts only `core`, `important`, or `normal`.
- `lastQuality` accepts only `shallow`, `effective`, `deep`, or `null`.
- All list results filter by `user_id` and `deleted = 0`.

## Frontend UX

- Praxis opens from Work Hub for owner/admin only; the T-282 locked UX remains for other roles.
- The Praxis shell renders a banner, eight board tabs/cards, and the relation board by default.
- The relation board draws the self node at the left midpoint, three right-facing half-ellipses, and contact nodes on arcs.
- Empty state still draws arcs and self node, then prompts for adding a contact.
- Add contact form supports name, layer, last contact date, quality, risk flag, cycle-off flag, and note.
- Clicking a node opens a compact detail/editor with update and delete actions.
- Node states follow the spec: `hi`, `dim`, `solid`, `light`, and risk red dot overlay.
