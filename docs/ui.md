# UI

## Theme

Uses Catppuccin Mocha color palette via CSS custom properties defined in `:root` (style.css).

| Variable | Value | Usage |
|----------|-------|-------|
| `--bg-primary` | `#1e1e2e` | App background, pane background |
| `--bg-secondary` | `#313244` | Headers, dialogs, context menu, inputs |
| `--bg-hover` | `#45475a` | Row hover, button hover |
| `--text-primary` | `#cdd6f4` | Main text |
| `--text-secondary` | `#a6adc8` | Path display, file sizes |
| `--border-color` | `#585b70` | Dividers, borders |
| `--accent` | `#89b4fa` | Directory names, focus outlines, divider hover |

## Features

### Split Pane Layout
The layout is a recursive binary tree of splits and panes, supporting arbitrary nesting in both directions. The app starts with two panes side by side (vertical split).

**Splitting**: Each pane header has two split icons on the right:
- **Split Right** (rectangle with vertical line) — creates a new pane to the right of the current one (vertical split).
- **Split Down** (rectangle with horizontal line) — creates a new pane below the current one (horizontal split).

The new pane opens to the same directory as the pane it was split from. Splits can be nested arbitrarily (e.g., split right, then split the new pane down).

**Closing**: When there are more than 2 panes, each pane header shows a close button (x). Closing a pane collapses its parent split, promoting the sibling to take its place in the tree.

**Resizing**: Each split has a draggable divider between its two children. Vertical splits use a `col-resize` cursor (drag left/right), horizontal splits use `row-resize` (drag up/down). The ratio is clamped between 10% and 90%. Minimum pane dimensions: 100px width, 100px height.

**Directory sync**: File operations (move, copy, rename, delete) refresh all panes showing the affected directories, not just the pane where the action originated.

**Premium gate**: Free version allows up to 3 panes. Attempting to split beyond that prompts for a license key.

**Data model**: `LayoutNode = LayoutLeaf { paneId } | LayoutSplit { direction, first, second, ratio }`. Pane state is stored in a `Map<string, PaneState>` keyed by pane ID. Tree operations (`splitPane`, `removePane`, `countLeaves`) are pure functions in `layout.ts`.

### Context Menu
Right-click (or Ctrl+click on Mac) a file/folder row to show a floating context menu with Open, Rename, and Delete options. Auto-dismisses on click outside, Escape, or another right-click. Repositions if it would overflow the window edge.

### Inline Rename
Triggered from context menu. Replaces the filename span with a text input. For files, only the name (not extension) is selected. Enter commits the rename, Escape or blur cancels.

### Delete Confirmation Dialog
Modal overlay with "Move to Trash" and "Cancel" buttons. Cancel is focused by default (safety). Dismisses on Escape or clicking outside the dialog. Uses a red accent (`#e64553`) for the danger button.

### File Opening
Double-click or context menu Open. Directories navigate into them; files open in the OS default application.

### Drag and Drop Between Panes
Drag files or folders from one pane and drop them onto any other pane's file list. Panes are identified by string IDs (not indices) in the drag data.

- **Move (default)**: Drag and drop moves the file — it disappears from the source pane and appears in the target.
- **Copy (Option/Alt held)**: Hold the Option key (Alt on Windows/Linux) while dropping to copy instead. The file stays in the source and appears in the target.
- **Same-pane drop prevention**: Dropping onto the same pane is a no-op.
- **Visual feedback**: The dragged row gets reduced opacity (0.4). The target pane's file list gets an accent-colored outline highlight. A compact drag ghost shows the icon + filename.
- **Name conflict dialog**: When a file with the same name already exists in the target, a modal appears with three choices:
  - **Replace** — Deletes the existing file in the target, then performs the move/copy.
  - **Keep Both** — Performs the move/copy, then auto-renames the new file with a ` (2)` suffix (incrementing if needed).
  - **Cancel** — Aborts the operation.

**Tauri note**: Native `dragDropEnabled` is set to `false` in `tauri.conf.json` so that Tauri doesn't intercept HTML5 drag-and-drop events within the webview. The `text/plain` MIME type is used for `dataTransfer` instead of a custom type for WebKit compatibility.

## UI Fixes Applied

### Double-click text selection (fixed)
**Problem**: Double-clicking a file row to open it also triggered the browser's default text selection behavior, highlighting the filename text.
**Fix**: Added `e.preventDefault()` and `window.getSelection()?.removeAllRanges()` to the `dblclick` handler in `pane.ts`. This is a pure UI fix — the file open logic was unaffected.

### Context menu dismissing on Ctrl+click release (fixed)
**Problem**: On macOS, Ctrl+click triggers `contextmenu` on mousedown. When the user releases the mouse button, a `click` event fires and immediately dismisses the menu.
**Fix**: Changed the dismiss listener from `click` to `mousedown` with a `setTimeout(100ms)` delay, so the full click cycle from the Ctrl+click completes before the dismiss listener is active. Menu items still use `click` handlers internally.
