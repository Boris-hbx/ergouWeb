## ADDED Requirements

### Requirement: Existing expense photos show delete button in edit mode
When editing an expense entry, each existing photo SHALL display a delete button (×). The delete button SHALL only appear for the entry owner.

#### Scenario: Edit mode shows delete buttons on existing photos
- **WHEN** user opens an expense entry for editing that has 3 existing photos
- **THEN** all 3 photos are displayed with a × delete button on each

### Requirement: Deleting an expense photo stays in edit mode
When user deletes an existing photo, the system SHALL remove the photo via API, update the display inline, and keep the edit view open. The edit view SHALL NOT close or navigate away.

#### Scenario: Delete one photo and continue editing
- **WHEN** user taps × on an existing photo and confirms
- **THEN** system calls `DELETE /api/expenses/photos/{photo_id}`
- **THEN** the photo is removed from the display
- **THEN** the edit view remains open with all other fields and photos intact

#### Scenario: Delete multiple photos consecutively
- **WHEN** user deletes a photo, then immediately deletes another photo
- **THEN** both deletions succeed without leaving the edit view
- **THEN** remaining photos and form fields are unaffected

#### Scenario: Delete last remaining photo
- **WHEN** user deletes the only remaining photo
- **THEN** the photo area shows empty (no photos) but the edit view stays open
- **THEN** user can still add new photos or save other changes

### Requirement: Photo deletion requires confirmation
The system SHALL display a confirmation dialog before deleting a photo. If user cancels, no deletion occurs.

#### Scenario: User confirms deletion
- **WHEN** user taps × and system shows confirmation dialog
- **THEN** user confirms and photo is deleted

#### Scenario: User cancels deletion
- **WHEN** user taps × and system shows confirmation dialog
- **THEN** user cancels and photo remains unchanged

### Requirement: Photo deletion error keeps edit view open
If the API call fails, the system SHALL show an error toast and keep the edit view open with the photo still displayed.

#### Scenario: Network error during deletion
- **WHEN** user confirms photo deletion but API call fails
- **THEN** system shows error toast "删除失败"
- **THEN** the photo remains displayed and edit view stays open

### Requirement: Trip photo deletion stays in edit mode
When deleting a trip item photo, the system SHALL remove the photo from the display inline and keep the edit view open, instead of closing the edit modal and navigating to the detail view.

#### Scenario: Delete trip photo stays in edit view
- **WHEN** trip owner taps × on a photo in trip item edit mode and confirms
- **THEN** system deletes the photo via API
- **THEN** the photo is removed from the display
- **THEN** the trip item edit view remains open (does NOT close to detail view)

#### Scenario: Delete multiple trip photos consecutively
- **WHEN** trip owner deletes two photos one after another in edit mode
- **THEN** both deletions succeed without leaving the edit view

### Requirement: Frontend API method for expense photo deletion
The frontend SHALL have an `API.deleteExpensePhoto(photoId)` method that calls `DELETE /api/expenses/photos/{photo_id}`.

#### Scenario: API method exists and works
- **WHEN** `API.deleteExpensePhoto('photo-123')` is called
- **THEN** it sends `DELETE /api/expenses/photos/photo-123` to the server
- **THEN** returns the server response `{ success: true/false }`
