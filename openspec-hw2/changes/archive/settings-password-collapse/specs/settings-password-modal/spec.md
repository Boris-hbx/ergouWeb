## ADDED Requirements

### Requirement: Password section displays as a single action button
The settings page SHALL replace the inline password form (3 inputs + hint + submit button) with a single "修改密码" action button. This button SHALL be styled consistently with other settings section items and occupy minimal vertical space.

#### Scenario: Registered user sees password button
- **WHEN** a registered user opens the settings page
- **THEN** the password section displays a single "修改密码" button instead of the full form

#### Scenario: Guest user does not see password button
- **WHEN** a guest user opens the settings page
- **THEN** the password section (including the button) is hidden entirely

### Requirement: Password modal opens on button tap
The system SHALL display a modal dialog when the user taps the "修改密码" button. The modal SHALL contain the current password field, new password field, confirm password field, a warning hint about password recovery, and a submit button.

#### Scenario: Open password modal
- **WHEN** user taps the "修改密码" button in settings
- **THEN** a modal overlay appears with the three password input fields, the warning hint, and a submit button
- **THEN** all input fields are empty
- **THEN** focus is placed on the current password field

### Requirement: Password modal can be dismissed
The user SHALL be able to close the modal without making changes by tapping the close button or tapping the overlay backdrop. Dismissing the modal SHALL discard any entered data without confirmation.

#### Scenario: Close via close button
- **WHEN** the password modal is open and user taps the close button (X)
- **THEN** the modal closes and any entered data is discarded

#### Scenario: Close via backdrop tap
- **WHEN** the password modal is open and user taps the overlay area outside the modal
- **THEN** the modal closes and any entered data is discarded

#### Scenario: Close via Escape key
- **WHEN** the password modal is open and user presses the Escape key
- **THEN** the modal closes and any entered data is discarded

### Requirement: Password validation in modal
The system SHALL perform the same validation as the current inline form: current password is required, new password minimum 8 characters, confirm password must match new password. Validation errors SHALL be shown as toast messages. The modal SHALL remain open on validation failure with field values preserved.

#### Scenario: Empty current password
- **WHEN** user submits the modal with current password field empty
- **THEN** system shows error toast "请输入当前密码"
- **THEN** modal stays open with all field values preserved

#### Scenario: New password too short
- **WHEN** user submits with new password shorter than 8 characters
- **THEN** system shows error toast "新密码至少需要 8 个字符"
- **THEN** modal stays open

#### Scenario: Passwords do not match
- **WHEN** user submits with new password and confirm password not matching
- **THEN** system shows error toast "两次输入的新密码不一致"
- **THEN** modal stays open

### Requirement: Successful password change closes modal
Upon successful password change, the system SHALL show a success toast, clear all fields, and automatically close the modal.

#### Scenario: Password changed successfully
- **WHEN** user submits valid passwords and server confirms the change
- **THEN** system shows success toast "密码修改成功"
- **THEN** all fields are cleared and the modal closes automatically

### Requirement: Server-side and network errors keep modal open
If the server rejects the request or a network error occurs, the system SHALL show an error toast and keep the modal open so the user can retry.

#### Scenario: Server rejects current password
- **WHEN** server responds with an error (e.g., wrong current password)
- **THEN** system shows error toast with server message
- **THEN** modal stays open with field values preserved

#### Scenario: Network failure
- **WHEN** the API call fails due to network error
- **THEN** system shows error toast "密码修改失败"
- **THEN** modal stays open
