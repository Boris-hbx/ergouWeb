## Use Cases

### Use Case: Grant admin privileges to a user

**Primary Actor:** Owner
**Scope:** Admin Console
**Level:** User goal

**Stakeholders and Interests:**
- Owner — wants to delegate monitoring duties without giving away full control
- Target user — gains access to admin console

**Preconditions:**
- Owner is logged in and has role=owner
- Target user exists with role=user and status=active

**Success Guarantee (Postconditions):**
- Target user's role is changed to admin
- Target user can now see and access the admin console
- An audit log entry is recorded

**Trigger:** Owner opens user management and selects a user to promote

**Main Success Scenario:**
1. Owner navigates to the admin console's user management section.
2. System displays the user list with current roles, statuses, and last active times.
3. Owner selects a user and chooses "Set as Admin".
4. System shows a confirmation dialog warning this grants full admin console access.
5. Owner confirms.
6. System updates the user's role to admin and records an audit log entry.
7. System shows success confirmation.

**Extensions:**
- 3a. Target user is already admin: System shows current role, no action available.
- 3b. Target user is suspended/pending: System blocks promotion — only active users can be promoted.
- 5a. Owner cancels: No change made.

---

### Use Case: Revoke admin privileges from a user

**Primary Actor:** Owner
**Scope:** Admin Console
**Level:** User goal

**Stakeholders and Interests:**
- Owner — wants to remove someone's admin access
- Target admin — loses admin console visibility

**Preconditions:**
- Owner is logged in with role=owner
- Target user has role=admin

**Success Guarantee (Postconditions):**
- Target user's role is changed back to user
- Admin console is no longer visible to them
- Audit log entry recorded

**Trigger:** Owner decides to revoke an admin's access

**Main Success Scenario:**
1. Owner navigates to user management.
2. Owner selects an admin user and chooses "Revoke Admin".
3. System shows confirmation dialog.
4. Owner confirms.
5. System updates role to user and records audit log.
6. System shows success confirmation.

**Extensions:**
- 2a. Target is the owner themselves: System prevents self-demotion.

---

### Use Case: Manage user accounts

**Primary Actor:** Admin (or Owner)
**Scope:** Admin Console
**Level:** User goal

**Stakeholders and Interests:**
- Admin — needs to review, approve, suspend, or restore user accounts
- Users — affected by admin decisions on their account status

**Preconditions:**
- Actor is logged in with role=admin or role=owner

**Success Guarantee (Postconditions):**
- User account status is updated as intended
- Audit log records the operation

**Trigger:** Admin opens user management to handle a pending, suspicious, or problematic user

**Main Success Scenario:**
1. Admin opens the user management section.
2. System shows the full user list with search, sort, and filter controls (by status, role, activity).
3. Admin selects a user.
4. System shows a detail panel: registration info, last active time, feature usage stats, AI token consumption.
5. Admin performs an action (approve / reject / suspend / restore).
6. System applies the change, records an audit log, and refreshes the list.

**Extensions:**
- 3a. Admin searches by username or display name: System filters the list in real time.
- 5a. Admin tries to change a role (not owner): System blocks — only owner can change roles.
- 5b. Admin tries to suspend an owner: System blocks — owner cannot be suspended.
- 5c. Reject a pending user: System sends a notification to the user and marks status=rejected.

---

### Use Case: Browse AI conversation history

**Primary Actor:** Admin (or Owner)
**Scope:** Admin Console
**Level:** User goal

**Stakeholders and Interests:**
- Admin — wants to monitor AI quality, detect misuse, debug user-reported issues
- Users — their conversations are being reviewed (privacy consideration)

**Preconditions:**
- Actor is logged in with admin or owner role
- Conversations exist in the system

**Success Guarantee (Postconditions):**
- Admin has viewed the desired conversation(s)
- No data is modified (read-only operation)

**Trigger:** Admin wants to review what users are asking the AI assistant

**Main Success Scenario:**
1. Admin opens the conversation monitoring section.
2. System shows a conversation list: title, user name, message count, token usage, last updated time.
3. Admin filters by user and/or date range.
4. Admin selects a conversation.
5. System displays the full conversation thread (user messages + AI responses) in a readable chat layout.
6. Admin reviews the content.

**Extensions:**
- 2a. Large number of conversations: System paginates results (e.g. 20 per page).
- 3a. No conversations match filters: System shows an empty state with a message.
- 5a. Conversation contains a flagged security event: System highlights the flagged message with a risk badge.

---

### Use Case: Review AI token consumption and model usage

**Primary Actor:** Admin (or Owner)
**Scope:** Admin Console
**Level:** User goal

**Stakeholders and Interests:**
- Owner — needs to control AI costs, understand spending patterns
- Admin — monitors usage for anomalies

**Preconditions:**
- Actor has admin or owner role
- AI usage records exist in ai_usage table

**Success Guarantee (Postconditions):**
- Admin understands current token consumption by model, by user, and by time period

**Trigger:** Admin wants to check AI costs or investigate unusual usage

**Main Success Scenario:**
1. Admin opens the AI dashboard section.
2. System displays an overview: total tokens (input + output) for today / 7 days / 30 days, broken down by model (Claude, Kimi, Doubao).
3. System shows provider status: which API keys are configured (without revealing the keys).
4. System shows a per-user consumption ranking table: user, messages, input tokens, output tokens, primary model used.
5. Admin reviews the data to identify cost drivers or unusual patterns.

**Extensions:**
- 2a. No usage data for a period: System shows zero values.
- 4a. Admin clicks a user row: System filters conversation monitor to that user's conversations.

---

### Use Case: Investigate a security risk event

**Primary Actor:** Admin (or Owner)
**Scope:** Admin Console
**Level:** User goal

**Stakeholders and Interests:**
- Admin — needs to assess severity and decide on action
- Owner — ultimately responsible for system security
- Flagged user — may be innocent or malicious

**Preconditions:**
- Security events exist in security_events table
- Actor has admin or owner role

**Success Guarantee (Postconditions):**
- Admin has reviewed the event, understands the context, and optionally taken action (suspend user, dismiss event)

**Trigger:** Admin sees risk alerts on the dashboard or receives a notification about a high-severity event

**Main Success Scenario:**
1. Admin opens the risk monitoring section.
2. System displays security events sorted by severity (high → medium → low), with event type, user, description, and timestamp.
3. Admin selects a high-severity event.
4. System shows event details and a link to the related conversation.
5. Admin clicks through to view the full conversation context.
6. Admin decides to suspend the user or dismiss the event as a false positive.
7. System records the admin's decision in the audit log.

**Extensions:**
- 2a. Filter by severity level or event type: System narrows the list.
- 2b. Filter by user: System shows only events for that user, revealing repeat-offender patterns.
- 6a. Admin suspends the user: System changes user status to suspended, records audit log.
- 6b. Admin dismisses: System marks the event as reviewed (admin_notified=1).

---

### Use Case: View system health status

**Primary Actor:** Admin (or Owner)
**Scope:** Admin Console
**Level:** User goal

**Stakeholders and Interests:**
- Admin/Owner — wants to know if the system is healthy without SSH-ing into the server

**Preconditions:**
- Actor has admin or owner role

**Success Guarantee (Postconditions):**
- Admin sees current system health metrics

**Trigger:** Admin opens the admin console or wants to check system status

**Main Success Scenario:**
1. Admin opens the system status section.
2. System displays: app version, server uptime, database file size, total row counts for key tables.
3. System displays: upload storage usage (total files, total size).
4. System displays: recent error count (client_errors table, last 24h).
5. Admin identifies any concerns (large DB, many errors) and takes action outside the app if needed.

**Extensions:**
- 2a. Database file is unusually large: System highlights the value in a warning color.

---

### Use Case: Review admin audit trail

**Primary Actor:** Owner
**Scope:** Admin Console
**Level:** User goal

**Stakeholders and Interests:**
- Owner — wants to verify what admin actions have been taken, especially by other admins

**Preconditions:**
- Owner is logged in
- Audit log entries exist

**Success Guarantee (Postconditions):**
- Owner has reviewed the audit log and understands recent admin operations

**Trigger:** Owner wants to check what admins have been doing

**Main Success Scenario:**
1. Owner opens the audit log section.
2. System shows a chronological list: timestamp, admin user, action type, target user/resource, details.
3. Owner filters by admin user or action type.
4. Owner reviews the entries.

**Extensions:**
- 2a. Large number of entries: System paginates (50 per page).
- 3a. No entries match filter: System shows empty state.

**Open Questions:**
- Should admins also see the audit log (read-only), or owner only?
