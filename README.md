# PaneExplorer

A lightweight, multi-pane file explorer built with Tauri v2, Rust, and TypeScript. Also available as a [terminal UI](#terminal-ui-panex-tui).

**[Website](https://ivapo.github.io/PanEx/)**

## Features

- Two side-by-side directory panes (split right/down for more)
- Navigate folders with double-click, go up with the back button
- Inline folder expansion — click the toggle arrow to expand folders tree-view style
- File selection — click, Cmd/Ctrl+click to toggle, Shift+click for range select
- Full keyboard navigation with focus cursor
- Copy, cut, paste files between panes
- Multi-file drag and drop between panes (move or Option+drop to copy)
- Drag files onto folders within the same pane
- Create new files and folders via right-click context menu
- Open current directory in terminal (auto-detects iTerm2 on macOS)
- Favorite locations — star icon to bookmark directories, quick-access dropdown when editing the path
- Custom default applications — configure per-extension open commands independently for GUI and TUI via `~/.panex/config.toml`
- Sortable columns — click Name, Extension, Size, or Date Modified headers to sort (persists across sessions)
- Show/hide hidden files toggle
- Native performance — Rust handles all filesystem operations
- 4 themes: Dark, Light, 3.1 (retro), TUI (terminal)

## Keyboard Shortcuts

### Navigation

| Action | Mac | Windows |
|---|---|---|
| Move selection up/down | `↑` / `↓` | same |
| Open / enter folder | `Enter` | same |
| Go up directory | `Backspace` | same |
| Switch pane focus | `Tab` | same |
| Deselect / exit keyboard nav | `Esc` | same |

### Selection

| Action | Mac | Windows |
|---|---|---|
| Extend selection | `Shift+↑` / `Shift+↓` | same |
| Select all | `⌘A` | `Ctrl+A` |

### File Operations

| Action | Mac | Windows |
|---|---|---|
| Copy | `⌘C` | `Ctrl+C` |
| Cut | `⌘X` | `Ctrl+X` |
| Paste | `⌘V` | `Ctrl+V` |
| Delete | `⌘⌫` | `Delete` |
| Rename | `F2` | same |

### Pane Management (Desktop Only)

| Action | Mac | Windows |
|---|---|---|
| Split right | `⌘→` | `Ctrl+→` |
| Split down | `⌘↓` | `Ctrl+↓` |
| Close pane | `⌘W` | `Ctrl+W` |

> On web, use the split/close buttons in the pane header.

### Other

| Action | Mac | Windows |
|---|---|---|
| Toggle hidden files | `⌘.` | `Ctrl+.` |

## Configuration

PanEx uses a global config file at `~/.panex/config.toml` (created automatically). You can configure favorite locations and custom default applications for opening files:

```toml
[favorites]
paths = ["~/dev", "~/Documents"]

[open.gui]
".md" = "Visual Studio Code"
".pdf" = "Preview"

[open.tui]
".md" = "nvim"
".rs" = "nvim"
```

GUI values are app names (passed to `open -a` on macOS). TUI values are terminal commands. Extensions not listed fall back to the OS default.

## Tech Stack

- **Backend:** Rust (Tauri v2)
- **Frontend:** TypeScript + HTML/CSS (vanilla, no framework)
- **Bundler:** Vite
- **Package manager:** Bun

## macOS: "App is damaged" fix

macOS quarantines apps downloaded from the internet that aren't code-signed. To fix this, open Terminal and run:

```bash
xattr -cr /Applications/PaneExplorer.app
```

If you dragged it somewhere else, replace the path accordingly. Then open the app normally.

## Getting Started

```bash
# Install dependencies
bun install

# Run in development
bun run tauri dev

# Build for production
bun run tauri build
```

## Terminal UI (panex-tui)

PanEx also ships as a terminal app powered by [ratatui](https://github.com/ratatui/ratatui) — no GUI needed.

```bash
cargo install panex-tui
panex
```

Multi-pane splits, vim-style keys, file operations, search, sorting, and hidden files toggle — all in the terminal. See the full [panex-tui README](crates/panex-tui/README.md) for keybindings and details.

## Support

If you find PanEx useful, consider [supporting development on Ko-fi](https://ko-fi.com/ivapo).

## License

[MIT](LICENSE)
