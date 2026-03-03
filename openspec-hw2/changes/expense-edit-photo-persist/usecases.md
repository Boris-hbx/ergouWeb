## Use Cases

### Use Case: Delete Existing Photos While Editing Expense

**Primary Actor:** Registered User
**Scope:** Expense Module
**Level:** User goal

**Stakeholders and Interests:**
- User — wants to remove incorrect or outdated receipt photos without leaving the edit view

**Preconditions:**
- User is logged in
- An expense entry exists with one or more attached photos
- User is in the edit view of that entry

**Success Guarantee (Postconditions):**
- Selected photos are permanently deleted from server
- User remains in the edit view and can continue editing or delete more photos
- Photo display is refreshed to reflect deletions

**Trigger:** User taps the delete button on an existing photo in the expense edit view

**Main Success Scenario:**
1. User opens an expense entry for editing.
2. System displays the edit form with existing photos, each showing a delete button.
3. User taps the delete button on a photo.
4. System asks for confirmation.
5. User confirms.
6. System deletes the photo via API and removes it from the display.
7. User remains in the edit view. User can delete more photos, add new photos, or modify other fields.
8. User taps save or cancel when done.

**Extensions:**
- 4a. User cancels confirmation: No deletion occurs. Edit view unchanged.
- 6a. API call fails (network error or server error): System shows error toast. Photo remains displayed. Edit view stays open.
- 7a. User deletes all photos: Photo area shows empty state. User can still add new photos or save.

---

### Use Case: Delete Existing Photos While Editing Trip Item

**Primary Actor:** Trip Owner
**Scope:** Trip Module
**Level:** User goal

**Stakeholders and Interests:**
- Trip owner — wants to remove multiple incorrect ticket/receipt photos in one editing session

**Preconditions:**
- User is the trip owner
- A trip item exists with one or more attached photos
- User is in the edit view of that item

**Success Guarantee (Postconditions):**
- Selected photos are permanently deleted from server
- User remains in the edit view (not kicked back to detail view)
- Photo display is refreshed inline

**Trigger:** User taps the delete button on an existing photo in the trip item edit view

**Main Success Scenario:**
1. User opens a trip item for editing.
2. System displays the edit form with existing photos, each showing a delete button.
3. User taps the delete button on a photo.
4. System asks for confirmation.
5. User confirms.
6. System deletes the photo via API and removes it from the display.
7. User remains in the edit view. User can continue deleting more photos or editing other fields.

**Extensions:**
- 4a. User cancels confirmation: No deletion. Edit view unchanged.
- 6a. API call fails: System shows error toast. Photo remains. Edit view stays open.
