## Use Cases

### Use Case: Change Account Password

**Primary Actor:** Registered User
**Scope:** Settings Module
**Level:** User goal

**Stakeholders and Interests:**
- User — wants to change password securely without disrupting the settings page experience
- System — must validate credentials and persist the new password

**Preconditions:**
- User is logged in with a registered account (not guest)
- User is on the settings page

**Success Guarantee (Postconditions):**
- Password is updated in the system
- User receives confirmation
- Modal is closed and settings page returns to normal state

**Trigger:** User decides to change their password

**Main Success Scenario:**
1. User taps the "修改密码" button in the settings page.
2. System displays a modal dialog with fields for current password, new password, and confirm new password.
3. User fills in all three fields.
4. User taps the submit button in the modal.
5. System validates: current password is provided, new password is at least 8 characters, confirmation matches.
6. System sends the change request to the server.
7. Server verifies the current password and updates to the new password.
8. System shows a success message, clears the fields, and closes the modal.

**Extensions:**
- 1a. User is a guest: The "修改密码" button is not shown. Use case does not apply.
- 5a. Validation fails (empty current password, short new password, mismatch): System shows an error toast. Modal stays open with fields intact so user can correct.
- 7a. Server rejects (wrong current password): System shows error toast. Modal stays open.
- 7b. Network error: System shows error toast. Modal stays open.
- *a. User taps outside the modal or taps a close button at any step: Modal closes, all entered data is discarded. No changes made.

---

### Use Case: Dismiss Password Modal Without Changes

**Primary Actor:** Registered User
**Scope:** Settings Module
**Level:** Subfunction

**Stakeholders and Interests:**
- User — opened the modal by mistake or changed their mind

**Preconditions:**
- Password modal is open

**Success Guarantee (Postconditions):**
- Modal is closed
- No password change is made
- Any entered data is discarded

**Trigger:** User wants to cancel the password change

**Main Success Scenario:**
1. User taps the close button or taps outside the modal overlay.
2. System closes the modal and discards any entered data.

**Extensions:**
- 1a. User has partially filled the form: Data is discarded without confirmation (low-risk action, no data loss).
