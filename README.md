# PaneExplorer

A lightweight, multi-pane file explorer built with Tauri v2, Rust, and TypeScript.

## Features

- Two side-by-side directory panes
- Navigate folders with double-click, go up with the back button
- Inline folder expansion — single-click a folder to expand it tree-view style
- Native performance — Rust handles all filesystem operations
- Dark theme out of the box

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
