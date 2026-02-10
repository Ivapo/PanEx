# File Operations

All filesystem and OS operations go through Rust via Tauri commands. The frontend never accesses the filesystem directly.

## Commands

| Command | Rust function | Description |
|---------|--------------|-------------|
| `read_dir` | `fs_ops::read_directory` | Lists directory contents (name, path, is_dir, size, modified). Backend returns directories first, then alphabetical — but the frontend re-sorts based on the user's chosen column (see Sortable Columns in `docs/ui.md`). |
| `get_home_dir` | `dirs::home_dir` | Returns the user's home directory path. |
| `get_parent_dir` | `Path::parent` | Returns the parent directory of a given path. |
| `open_entry` | `fs_ops::open_entry` | Opens a file/folder with the OS default app. Uses `open` on macOS, `xdg-open` on Linux, `cmd /C start` on Windows. |
| `rename_entry` | `fs_ops::rename_entry` | Renames a file or folder. Validates the target doesn't already exist. |
| `delete_entry` | `fs_ops::delete_entry` | Moves a file or folder to the OS trash (via `trash` crate). Does not permanently delete. |
| `copy_entry` | `fs_ops::copy_entry` | Copies a file or directory (recursively) to a destination directory. Returns the destination path. |
| `move_entry` | `fs_ops::move_entry` | Moves a file or directory to a destination directory. Tries `fs::rename` first (fast, same volume), falls back to copy + delete for cross-volume moves. Returns the destination path. |
| `calculate_dir_size` | `fs_ops::calculate_directory_size` | Recursively calculates the total disk usage of a directory. Uses actual disk blocks (`stat.blocks * 512`) on Unix for accurate sizes (handles sparse files correctly). Silently skips entries on permission errors. |

## Copy / Cut / Paste (In-App Clipboard)

File copy/cut/paste uses an in-app clipboard (`fileClipboard` in `main.ts`), not the system clipboard. This allows copying files between panes without relying on OS clipboard APIs.

- **Copy** (`⌘C` / `Ctrl+C` or context menu): Stores selected entries with mode `copy`.
- **Cut** (`⌘X` / `Ctrl+X` or context menu): Stores selected entries with mode `cut`. Clipboard is cleared after pasting.
- **Paste** (`⌘V` / `Ctrl+V` or context menu): Pastes into the active pane's current directory.

**Cross-directory paste**: Uses the same `handleDrop` logic as drag-and-drop (copy or move based on clipboard mode).

**Same-directory paste**: Detected when the source entries' parent matches the destination directory. Shows a conflict dialog with "Add Copy" / "Replace" / "Cancel". The "Add Copy" option stages through the parent directory to create a real duplicate (since `copyEntry` to the same directory is a no-op), then moves the staged copy back with a unique name.

## Directory Sizes

Directories display their total disk usage in the Size column, computed asynchronously after each directory load. Sizes show "--" as a placeholder until the computation finishes, then update in-place without a full re-render.

- **Async & throttled**: Computations are queued and limited to 2 concurrent calls to avoid blocking the Rust thread pool. Sizes trickle in gradually rather than stalling the UI.
- **Cached**: Results are stored in an in-memory `dirSizeCache` (keyed by path) for the session. Navigating back to a previously visited directory shows sizes immediately.
- **Disk usage, not logical size**: On Unix, uses `stat.blocks * 512` (actual disk blocks) instead of `metadata.len()`. This gives accurate sizes for sparse files (e.g., Docker.raw reports real usage, not the pre-allocated virtual size). Individual file sizes also use disk usage for consistency.
- **Sort integration**: When sorting by Size, directories use their cached computed size (or 0 if not yet computed).
- **Browser mode**: Uses the File System Access API to recursively walk directories and sum `file.size`.

## Design Decisions

- **Rust-only filesystem access**: We initially used `@tauri-apps/plugin-shell` for opening files from the frontend, but it silently failed. Switching to a Rust command using `std::process::Command` was more reliable and consistent with the "all operations through Rust" pattern. The shell plugin was fully removed.
- **Trash instead of permanent delete**: Uses the `trash` crate (v5) so deleted items can be recovered from macOS Trash (or equivalent on other platforms).
- **Rename validation**: `rename_entry` checks that the source exists and the destination name doesn't conflict before performing the rename.
- **Copy/move separation**: The Rust backend handles the raw copy/move operations. Name conflict resolution (replace, keep both, cancel) is handled entirely by the frontend before invoking the backend commands.
- **Cross-volume move fallback**: `move_entry` first attempts `fs::rename` which is instant on the same volume. If that fails (cross-volume), it falls back to a full copy followed by deleting the source.

## Error Handling

All Rust functions return `Result<T, String>`. Errors are caught in the frontend and displayed via `alert()` (sufficient for MVP). Future improvement: custom toast/notification system.
