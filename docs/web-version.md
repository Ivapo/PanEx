# Web Version

PaneExplorer runs in the browser (Chrome/Edge) using the File System Access API. Same codebase, no server required — just static files.

## Architecture

A filesystem abstraction layer (`src/fs.ts`) defines an `FsBackend` interface with two implementations:

- **TauriFs** — wraps existing `invoke()` calls to the Rust backend. Used when `window.__TAURI_INTERNALS__` exists.
- **BrowserFs** — uses the File System Access API (`showDirectoryPicker`, `FileSystemDirectoryHandle`, etc.). Used in the browser.

Detection is automatic at module load time. All filesystem calls in `main.ts` and `pane.ts` go through the shared `fs` singleton.

## Commands Abstracted

| Command | Tauri | Browser |
|---------|-------|---------|
| `readDir(path)` | `invoke("read_dir")` | `dirHandle.entries()` + `getFile()` for metadata |
| `getHomeDir()` | `invoke("get_home_dir")` | `showDirectoryPicker()` — user picks a root folder |
| `getParentDir(path)` | `invoke("get_parent_dir")` | String manipulation: strip last path segment |
| `openEntry(path)` | `invoke("open_entry")` | `getFile()` → `createObjectURL()` → `window.open()` |
| `renameEntry(path, newName)` | `invoke("rename_entry")` | `handle.move(newName)` or copy+delete fallback |
| `deleteEntry(path)` | `invoke("delete_entry")` | `parentHandle.removeEntry(name, {recursive})` |
| `copyEntry(source, destDir)` | `invoke("copy_entry")` | Read source file → write to dest handle (recursive for dirs) |
| `moveEntry(source, destDir)` | `invoke("move_entry")` | Copy + delete source |

## Browser-Specific Behavior

- **Folder picker on launch**: Since the browser can't access the home directory, `getHomeDir()` prompts the user with `showDirectoryPicker()`. The selected folder becomes the root.
- **Navigation is sandboxed**: Users can only browse within the directory tree they picked. Going "up" past the root stays at the root.
- **Permanent delete**: No trash in the browser. The delete dialog warns "This item will be permanently deleted" instead of "Move to Trash".
- **File opening**: Opens files in a new tab via `createObjectURL` (images render, text/PDF display, other files download).
- **Handle caching**: `BrowserFs` maintains a `Map<string, FileSystemDirectoryHandle>` cache for resolved paths, avoiding redundant handle traversal.
- **Rename**: Uses the `move()` method on `FileSystemFileHandle` if available (Chrome 86+), otherwise falls back to copy+delete.
- **Keyboard shortcuts**: Pane management shortcuts (split right/down, close pane) are disabled on the web to avoid conflicts with browser shortcuts. Use the split/close buttons in the pane header instead. All other shortcuts (navigation, selection, file operations, hidden files toggle) work the same.

## Web Banner

In browser mode, a banner appears at the top: "You're using the web version — Download the native app for the full experience". Not shown in Tauri.

## Building

```bash
# Web build — outputs to dist-web/
bun run build:web

# Dev server (works for both Tauri and browser testing)
bun run dev
```

The web build (`vite build --mode web`) externalizes `@tauri-apps/api/core` since it's not needed. Output goes to `dist-web/` (gitignored). Deploy these static files to any host.

## Browser Support

Requires the File System Access API — **Chrome 86+** and **Edge 86+** only. Firefox and Safari do not support `showDirectoryPicker`.

## Not a PWA

No service worker, no install prompt, no offline support. This is intentionally a hosted web app for trying out PaneExplorer before downloading the native version.

## Files

| File | Role |
|------|------|
| `src/fs.ts` | `FsBackend` interface, `TauriFs`, `BrowserFs`, auto-detection |
| `src/file-system-access.d.ts` | TypeScript declarations for File System Access API |
| `vite.config.ts` | Dual build config (`--mode web` for browser output) |
