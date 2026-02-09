# File Operations

All filesystem and OS operations go through Rust via Tauri commands. The frontend never accesses the filesystem directly.

## Commands

| Command | Rust function | Description |
|---------|--------------|-------------|
| `read_dir` | `fs_ops::read_directory` | Lists directory contents (name, path, is_dir, size, modified). Results sorted: directories first, then alphabetical. |
| `get_home_dir` | `dirs::home_dir` | Returns the user's home directory path. |
| `get_parent_dir` | `Path::parent` | Returns the parent directory of a given path. |
| `open_entry` | `fs_ops::open_entry` | Opens a file/folder with the OS default app. Uses `open` on macOS, `xdg-open` on Linux, `cmd /C start` on Windows. |
| `rename_entry` | `fs_ops::rename_entry` | Renames a file or folder. Validates the target doesn't already exist. |
| `delete_entry` | `fs_ops::delete_entry` | Moves a file or folder to the OS trash (via `trash` crate). Does not permanently delete. |

## Design Decisions

- **Rust-only filesystem access**: We initially used `@tauri-apps/plugin-shell` for opening files from the frontend, but it silently failed. Switching to a Rust command using `std::process::Command` was more reliable and consistent with the "all operations through Rust" pattern. The shell plugin was fully removed.
- **Trash instead of permanent delete**: Uses the `trash` crate (v5) so deleted items can be recovered from macOS Trash (or equivalent on other platforms).
- **Rename validation**: `rename_entry` checks that the source exists and the destination name doesn't conflict before performing the rename.

## Error Handling

All Rust functions return `Result<T, String>`. Errors are caught in the frontend and displayed via `alert()` (sufficient for MVP). Future improvement: custom toast/notification system.
