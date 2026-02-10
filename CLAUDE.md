# PaneExplorer

A lightweight, multi-pane file explorer built with Tauri v2 + Rust + TypeScript.

## What This Is

A desktop file explorer that lets users view multiple directories side by side. Free version supports 2 panes. Premium ($4.99 one-time via LemonSqueezy) unlocks 3+ panes.

## Tech Stack

- **Backend:** Rust (Tauri v2)
- **Frontend:** TypeScript + HTML/CSS (no framework)
- **Package managers:** Cargo (Rust), Bun (frontend)
- **Target platforms:** macOS first, then Windows and Linux
- **License key validation:** local check, no server required

## Architecture

- Rust handles all filesystem operations (read dir, open files, rename, delete, move/copy)
- Frontend handles layout, rendering panes, drag-and-drop UI, theming
- Communication via Tauri's `invoke` command system (TS calls Rust functions, with typed responses)

## Project Structure

```
pane-explorer/
├── src-tauri/          # Rust backend
│   ├── src/
│   │   ├── main.rs     # Tauri entry point
│   │   ├── commands.rs  # Tauri commands exposed to frontend
│   │   └── fs_ops.rs    # Filesystem operations
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                 # Frontend
│   ├── index.html
│   ├── style.css
│   ├── main.ts          # State management, keyboard shortcuts, sort logic
│   ├── pane.ts          # Pane component logic, column headers, rendering
│   ├── types.ts         # Shared types (FileEntry, PaneState, SortField, etc.)
│   ├── fs.ts            # FsBackend interface, Tauri + Browser implementations
│   ├── layout.ts        # Layout tree operations (split, remove, collect)
│   ├── theme.ts         # Theme cycling and persistence
│   ├── context-menu.ts  # Right-click context menu
│   └── licensing.ts     # Premium license check
├── package.json
├── CLAUDE.md
└── README.md
```

## MVP Features (build in this order)

1. Single pane showing a directory listing (name, size, modified date, icon)
2. Click folder to navigate into it, back button to go up
3. Split layout — two panes side by side, resizable divider
4. Basic file operations: open (default app), rename, delete (move to trash)
5. Drag and drop files between panes (copy/move)
6. Premium gate: "Add Pane" button checks for license key, prompts purchase if free tier

## Post-MVP Features

- Tabs within panes
- Bookmarked/pinned directories
- Search within a directory
- Dark/light theme toggle
- Keyboard shortcuts (arrows to navigate, enter to open, delete, cmd+c/v)
- File preview (images, text, markdown)
- ~~Sort by name/size/date~~ *(done — clickable column headers: Name, Extension, Size, Date Modified; persisted in localStorage)*
- Show/hide hidden files toggle
- File size display for directories

## Premium Boundary

- **Free:** up to 2 panes
- **Premium ($4.99):** unlimited panes, unlocked via license key stored locally

## Conventions

- Use `snake_case` for Rust, `camelCase` for TypeScript
- All filesystem operations go through Rust — never access fs from TS directly
- Tauri commands should be granular (one command per operation)
- Type all Tauri `invoke` responses — define interfaces in `types.ts` that mirror Rust structs
- Prefer vanilla TS — no React/Vue/Svelte unless complexity demands it
- CSS: use CSS custom properties for theming (colors, spacing)
- Error handling: Rust returns Result types, TS catches and shows user-friendly messages
- Keep the app fast — directory reads should feel instant
- Use `strict: true` in tsconfig.json

## Useful Commands

```bash
# Install dependencies
bun install

# Dev mode with hot reload
bun run tauri dev

# Build for production
bun run tauri build
```
