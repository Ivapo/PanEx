# PaneExplorer

A lightweight, multi-pane file explorer built with Tauri v2, Rust, and TypeScript.

## Features

- Two side-by-side directory panes (split right/down for more)
- Navigate folders with double-click, go up with the back button
- Inline folder expansion — click the toggle arrow to expand folders tree-view style
- File selection — click, Cmd/Ctrl+click to toggle, Shift+click for range select
- Full keyboard navigation with focus cursor
- Copy, cut, paste files between panes
- Multi-file drag and drop between panes (move or Option+drop to copy)
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

## Tech Stack

- **Backend:** Rust (Tauri v2)
- **Frontend:** TypeScript + HTML/CSS (vanilla, no framework)
- **Bundler:** Vite
- **Package manager:** Bun

## Getting Started

```bash
# Install dependencies
bun install

# Run in development
bun run tauri dev

# Build for production
bun run tauri build
```

## License

[MIT](LICENSE)
