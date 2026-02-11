---
title: I built a multi-pane file explorer with Tauri + Rust + TypeScript
tags: showdev, opensource, rust, typescript
---

Every file explorer gives you one window. Want to compare two folders? Open another window, resize, drag them side by side. Every time.

I got tired of it, so I built **PanEx** — a lightweight file explorer where you can split panes right or down, browse multiple directories at once, and drag files between them.

## Demo

<!-- embed your video here -->

## What it does

- **Multi-pane browsing** — split right or down, as many panes as you want
- **Drag & drop** between panes (hold Alt to copy instead of move)
- **Full keyboard navigation** — arrows, tab between panes, enter to open, Cmd+F to search
- **4 themes** — Dark, Light, Retro 3.1 (Windows-style), and TUI
- **Sortable columns** — click to sort by name, extension, size, or date
- **Inline folder expansion** — expand folders in-place without navigating away
- **Breadcrumb path bar** — click any segment to jump to a parent directory
- **Per-pane search** — real-time filtering with Cmd+F

## Tech stack

- **Rust** backend via Tauri v2 — all filesystem operations happen in Rust
- **Vanilla TypeScript** frontend — no React, no Vue, no framework. Just TS + HTML + CSS
- **~1500 lines of CSS** for 4 complete themes
- Builds to native apps on **macOS, Windows, and Linux**

I deliberately avoided frameworks to keep it fast and lean. The entire frontend is a handful of TS files with manual DOM updates — directory reads feel instant.

## Try it

- **Web demo:** [panex web demo](https://ivapo.github.io/PanEx/demo/)
- **Download:** [macOS / Windows / Linux](https://ivapo.github.io/PanEx/)
- **Source:** [github.com/Ivapo/PanEx](https://github.com/Ivapo/PanEx)

The web demo runs entirely in-browser with a simulated filesystem so you can try it without installing anything.

## What's next

Still early — feedback and contributions welcome. If you find it useful, the repo is open source and stars are appreciated.
