# panex-tui

A terminal UI file explorer with multi-pane support, built with [ratatui](https://github.com/ratatui/ratatui). Part of the [PanEx](https://github.com/Ivapo/PanEx) project.

## Install

```bash
cargo install panex-tui
```

## Usage

```bash
# Open in current directory
panex

# Open in a specific directory
cd ~/Projects && panex
```

## Features

- Multi-pane layout — split vertically (`|`) or horizontally (`-`), close with `W`
- Keyboard-driven navigation with vim-style keys (`j`/`k`) or arrow keys
- File operations: copy (`y`), cut (`x`), paste (`p`), rename (`r`/`F2`), delete (`d`)
- Type-ahead search and `/` filter mode
- Sortable columns — cycle field with `s`, toggle direction with `S`
- Show/hide hidden files (`.`)
- Editable path bar (`e`) with `~` expansion
- Open files in default app (`o`) or open directory in terminal (`t`)
- Create new files (`n`) and folders (`N`)
- Multi-select with `Shift+j`/`Shift+k`, select all with `Ctrl+a`

## Keyboard Shortcuts

| Key | Action |
|---|---|
| `j` / `k` or `Up` / `Down` | Move focus |
| `Enter` | Open file / enter folder |
| `Backspace` | Go up one directory |
| `~` | Go to home directory |
| `Tab` | Switch pane |
| `\|` | Split pane vertically |
| `-` | Split pane horizontally |
| `W` | Close pane |
| `y` | Copy |
| `x` | Cut |
| `p` or `Ctrl+v` | Paste |
| `r` / `F2` | Rename |
| `d` / `Delete` | Delete (move to trash) |
| `n` | New file |
| `N` | New folder |
| `o` | Open in default app |
| `t` | Open in terminal |
| `/` or `Ctrl+f` | Search |
| `s` | Cycle sort field |
| `S` | Toggle sort direction |
| `.` | Toggle hidden files |
| `e` | Edit path |
| `Ctrl+a` | Select all |
| `Esc` | Deselect / cancel |
| `q` | Quit |

## License

[MIT](../../LICENSE)
