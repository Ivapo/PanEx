# UI

## Themes

Four switchable themes, toggled via a button in the global toolbar at the top of the window. The selected theme persists across restarts via `localStorage` (key: `paneexplorer_theme`). The active theme is applied as a `data-theme` attribute on the `<html>` element; CSS custom properties are overridden per theme.

Clicking the theme button cycles: **Dark → Light → 3.1 → TUI → Dark**.

### Dark (default — Catppuccin Mocha)

| Variable | Value |
|----------|-------|
| `--bg-primary` | `#1e1e2e` |
| `--bg-secondary` | `#313244` |
| `--bg-hover` | `#45475a` |
| `--text-primary` | `#cdd6f4` |
| `--text-secondary` | `#a6adc8` |
| `--border-color` | `#585b70` |
| `--accent` | `#89b4fa` |
| `--danger` | `#e64553` |

Font: system sans-serif. Border radius: 4px / 8px.

### Light (Catppuccin Latte)

| Variable | Value |
|----------|-------|
| `--bg-primary` | `#eff1f5` |
| `--bg-secondary` | `#dce0e8` |
| `--bg-hover` | `#ccd0da` |
| `--text-primary` | `#4c4f69` |
| `--text-secondary` | `#6c6f85` |
| `--border-color` | `#bcc0cc` |
| `--accent` | `#1e66f5` |
| `--danger` | `#d20f39` |

Font: system sans-serif. Border radius: 4px / 8px.

### TUI (Cobalt)

| Variable | Value |
|----------|-------|
| `--bg-primary` | `#00254b` |
| `--bg-secondary` | `#003572` |
| `--bg-hover` | `#004999` |
| `--text-primary` | `#ffffff` |
| `--text-secondary` | `#aaccee` |
| `--border-color` | `#1a6baa` |
| `--accent` | `#ffc600` |
| `--danger` | `#ff5555` |

Font: `"SF Mono", "Menlo", "Consolas", monospace`. Border radius: 0 (flat/sharp corners).

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

### Pane Navigation
Each pane header has a back button (arrow) and a home button (house icon). The back button navigates to the parent directory. The home button navigates to the user's home directory.

### Keyboard Shortcuts

Full keyboard navigation is supported. Shortcuts are platform-aware — modifiers adjust for Mac vs Windows, and for desktop (Tauri) vs web (browser) to avoid conflicts with browser defaults.

**Focus cursor**: A distinct outline ring shows which row the keyboard cursor is on, separate from the selection highlight. The cursor only appears in the active pane and only during keyboard navigation — mouse movement hides it. `Esc` clears selection and exits keyboard nav mode.

**Hover suppression**: During keyboard navigation, a `.keyboard-nav` CSS class is added to panes to suppress hover highlights, preventing two rows from being visually emphasized at once. Mouse movement removes this class.

| Category | Action | Mac | Windows |
|----------|--------|-----|---------|
| Navigation | Move up/down | `↑` / `↓` | same |
| | Open / enter folder | `Enter` | same |
| | Go up directory | `Backspace` | same |
| | Switch pane | `Tab` | same |
| | Deselect / exit nav | `Esc` | same |
| Selection | Extend selection | `Shift+↑/↓` | same |
| | Select all | `⌘A` | `Ctrl+A` |
| File Ops | Copy | `⌘C` | `Ctrl+C` |
| | Cut | `⌘X` | `Ctrl+X` |
| | Paste | `⌘V` | `Ctrl+V` |
| | Delete | `⌘⌫` | `Delete` |
| | Rename | `F2` | same |
| Pane Mgmt | Split right | `⌘→` | `Ctrl+→` |
| | Split down | `⌘↓` | `Ctrl+↓` |
| | Close pane | `⌘W` | `Ctrl+W` |
| Other | Toggle hidden files | `⌘.` | `Ctrl+.` |

**Web-specific**: Split shortcuts use an extra `Alt` modifier (`⌘⌥→` / `Ctrl+Alt+→`) to avoid browser conflicts. Close pane has no web shortcut (`⌘W` / `Ctrl+W` would close the browser tab).

**Guard conditions**: Shortcuts are disabled when a dialog overlay is open or an input/textarea is focused (e.g., during inline rename).

**Data model**: `focusIndex: number` on `PaneState` tracks the keyboard cursor position within the flat display list. Set to `0` when loading a directory, `-1` when no focus. `activePaneId` tracks which pane receives keyboard input. A module-level `fileClipboard: { entries: FileEntry[], mode: 'copy' | 'cut' } | null` stores the in-app clipboard.

### Hidden Files Toggle

`⌘.` / `Ctrl+.` toggles visibility of dotfiles (files/folders starting with `.`). The state is tracked by a module-level `showHidden` boolean. When toggled, all panes reload from disk and re-filter. Expanded folder children also respect the filter.

### Context Menu
Right-click (or Ctrl+click on Mac) a file/folder row to show a floating context menu with Open, Copy, Cut, Paste, Rename, and Delete options. Each item shows its keyboard shortcut hint on the right side. Auto-dismisses on click outside, Escape, or another right-click. Repositions if it would overflow the window edge.

### Inline Rename
Triggered from context menu. Replaces the filename span with a text input. For files, only the name (not extension) is selected. Enter commits the rename, Escape or blur cancels.

### Delete Confirmation Dialog
Modal overlay with "Move to Trash" and "Cancel" buttons. Cancel is focused by default (safety). Dismisses on Escape or clicking outside the dialog. Uses a red accent (`var(--danger)`) for the danger button. Supports multi-delete — when multiple items are selected, the prompt shows the count (e.g., "Move 5 items to Trash?"). All dialogs are draggable by their title bar.

### File Selection
Click a file or folder row to select it. Only one pane can have an active selection at a time — clicking in another pane clears the previous one's selection. Selection resets when navigating into a folder, going up, or going home. Clicking anywhere on a pane (header, empty space, rows) makes it the active pane.

| Action | Result |
|--------|--------|
| Click row | Select (clears previous selection) |
| Cmd/Ctrl + click | Toggle item in/out of selection |
| Shift + click | Range select from last clicked item |
| `↑` / `↓` | Move focus cursor + select |
| `Shift+↑` / `Shift+↓` | Extend selection from cursor |
| `⌘A` / `Ctrl+A` | Select all |
| `Esc` | Deselect all |
| Click in different pane | Clears other pane's selection |

**Multi-drag**: Dragging a selected item drags all selected items together. The drag ghost shows the item count (e.g. "3 items"). Dragging an unselected item drags only that one. Conflict resolution dialogs appear per-file when dropping multiple items.

**Data model**: Selection is tracked per-pane via `selectedPaths: Set<string>` and `lastClickedPath: string | null` on `PaneState`. Shift+click computes the range from the display list (the flattened tree including expanded children). Selection changes update CSS classes in-place without re-rendering, preserving scroll position.

### Inline Folder Expansion (Tree View)
Click the toggle arrow (▶/▼) on a folder row to expand it inline, showing its children indented below. This works recursively — expanding nested folders stacks the indentation. Click the arrow again to collapse.

Clicking anywhere else on the folder row selects it (does not expand). Double-click navigates into the folder as the pane root.

Collapsing a parent folder also collapses all expanded children underneath it. Navigating into a folder (double-click), going up, or going home resets the expansion state.

**Data model**: Expanded folders are tracked per-pane via `expandedPaths: Set<string>` and `childrenCache: Map<string, FileEntry[]>` on `PaneState`. Children are loaded on demand via `fs.readDir()` when a folder is first expanded and cached until collapsed or the pane navigates away.

**CSS**: Indentation uses a `--depth` CSS variable on each row, applied as `padding-left: calc(base + depth * 20px)`. The 3.1 and TUI themes override with 16px steps to match their tighter row spacing.

### File Opening
Double-click or context menu Open. Directories navigate into them; files open in the OS default application.

### Drag and Drop Between Panes
Drag files or folders from one pane and drop them onto any other pane's file list. Supports single and multi-file drag — if multiple items are selected, dragging any selected item drags the entire selection. Dragging an unselected item drags only that one. Panes are identified by string IDs (not indices) in the drag data.

- **Move (default)**: Drag and drop moves the file(s) — they disappear from the source pane and appear in the target.
- **Copy (Option/Alt held)**: Hold the Option key (Alt on Windows/Linux) while dropping to copy instead. The files stay in the source and appear in the target.
- **Same-pane drop prevention**: Dropping onto the same pane is a no-op.
- **Visual feedback**: The dragged row gets reduced opacity (0.4). The target pane's file list gets an accent-colored outline highlight. A compact drag ghost shows the icon + filename for single items, or "N items" for multi-selection.
- **Name conflict dialog**: When dropping multiple files, conflict resolution is handled per-file. When a file with the same name already exists in the target, a modal appears with three choices:
  - **Replace** — Deletes the existing file in the target, then performs the move/copy.
  - **Add Copy** — Creates a duplicate with a ` (2)` suffix (incrementing if needed). For same-directory paste, stages through the parent directory to create a real copy.
  - **Cancel** — Aborts the operation.

**Tauri note**: Native `dragDropEnabled` is set to `false` in `tauri.conf.json` so that Tauri doesn't intercept HTML5 drag-and-drop events within the webview. The `text/plain` MIME type is used for `dataTransfer` instead of a custom type for WebKit compatibility.

## UI Fixes Applied

### Double-click text selection (fixed)
**Problem**: Double-clicking a file row to open it also triggered the browser's default text selection behavior, highlighting the filename text.
**Fix**: Added `e.preventDefault()` and `window.getSelection()?.removeAllRanges()` to the `dblclick` handler in `pane.ts`. This is a pure UI fix — the file open logic was unaffected.

### Context menu dismissing on Ctrl+click release (fixed)
**Problem**: On macOS, Ctrl+click triggers `contextmenu` on mousedown. When the user releases the mouse button, a `click` event fires and immediately dismisses the menu.
**Fix**: Changed the dismiss listener from `click` to `mousedown` with a `setTimeout(100ms)` delay, so the full click cycle from the Ctrl+click completes before the dismiss listener is active. Menu items still use `click` handlers internally.
