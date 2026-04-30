# PDF Layout Editor: V2 Implementation Plan

## 1. UX/UI for Multiple Boxes per Field
Currently, a `FormField` in `formtype.json` maps 1:1 to a visual bounding box. When a logical field (like a long address or email) needs to span across multiple non-contiguous boxes on the PDF, we encounter an edge case.

### Architecture Choice: Array vs. Duplicate Entries
**Recommendation: Ordered Duplicate Entries in `FormType.fields`**
Instead of changing the core `FormField` struct to hold a `Vec<Bounds>`, we maintain the current structure but allow multiple `FormField` entries with the *exact same `key`*. 
- **Why?** It keeps the JSON schema simple and backward-compatible. The order of these elements in the `fields` array inherently defines the text-flow order (Box 1 -> Box 2 -> Box 3).

### Sidebar UI Updates
To manage multiple boxes for the same field intuitively:
1. **Grouping**: In the sidebar (`fields_sidebar_list`), group `FormField` items by their `key`.
2. **Visual Hierarchy**: 
   - Display the `key` (e.g., `registeredAddress`) as a parent header.
   - Underneath, show sub-items for each box: `Box 1`, `Box 2`, etc.
3. **Action Buttons**:
   - **Plus Icon (+)** next to the parent key: Duplicates the last box of this field, placing it slightly below the original so the user can drag it to its new position.
   - **Trash Icon (🗑)** next to each sub-box: Deletes that specific box. *Constraint: The trash icon is disabled or hidden if it's the last remaining box for that field.*
4. **Ordering**: Add simple Up/Down arrow buttons next to the sub-boxes to adjust their logical order in the array, as the array order dictates text flow.

## 2. Text Rendering & Wrapping Strategy (PDF Generation)
When a user types into a single input field on the dashboard, the PDF generator (`bir-print` / Typst) needs to render that string across the multiple physical boxes.

**Implementation Strategy:**
- When parsing the `formtype.json`, group the physical fields by `key`.
- If a key has $>1$ box, calculate the total available character capacity based on the `width` and `font_size` of each box.
- **Word/Character Wrapping**: The PDF generator must token-split the string. It fills `Box 1` until the character limit is reached, then spills the remaining string into `Box 2`, and so on.
- **Note**: This file serves as documentation to ensure the PDF generation team implements this exact spanning logic using Typst's layout engine.

## 3. Keyboard Shortcuts & Global Actions
We will bind global shortcuts within `main.rs` and `actions.rs` to streamline the developer experience.

**New Actions to Register:**
- `ZoomIn` (`Cmd + =` / `Ctrl + =`)
- `ZoomOut` (`Cmd + -` / `Ctrl + -`)
- `ResetZoom` (`Cmd + 0` / `Ctrl + 0`)
- `ToggleEditMode` (`Cmd + E` / `Ctrl + E`)
- `SaveLayout` (`Cmd + S` / `Ctrl + S`)

**Implementation:**
1. Define these in `crates/bir-desktop/src/global_actions.rs`.
2. Bind them in `crates/bir-desktop/src/platform/mod.rs` inside `bind_global_keys()`.
3. Handle them in `crates/bir-desktop/src/app.rs` by routing the event to the active `PdfLayoutEditorView`.

## 4. View Mode vs. Edit Mode Visibility
- **View Mode**: All boxes are rendered. The boxes use a translucent background (`cx.theme().secondary.opacity(0.2)`). If the user clicks (focuses) on a box, its color changes to `warning` (yellowish translucent) to signify focus.
- **Edit Mode**: When `Cmd + E` is pressed or a box enters edit mode, *only* the boxes belonging to the currently selected field key are visible. All other fields are hidden to reduce clutter.
