# panex-tui

A terminal UI file explorer with multi-pane support, built with [ratatui](https://github.com/ratatui/ratatui). Part of the [PanEx](https://github.com/Ivapo/PanEx) project.

**[Website](https://ivapo.github.io/PanEx/)**

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

panex --help      # usage
panex --version   # version
```

## Features

- Multi-pane layout — split vertically (`|`) or horizontally (`_`), close with `W`
- Resizable panes — `+` grows the active pane by 25%, `-` shrinks it
- Keyboard-driven navigation with vim-style keys (`j`/`k`) or arrow keys
- File operations: copy (`y`), cut (`x`), paste (`p`), rename (`r`/`F2`), delete (`d`)
- Delete confirmation dialog with arrow key selection
- Search with `/` or `Ctrl+f`
- Sortable columns — cycle field with `s`, toggle direction with `S`
- Show/hide hidden files (`.`)
- Editable path bar (`e`) with `~` expansion, Tab completion, and segment-wise backspace
- Favorite locations — press `f` to bookmark, `e` to see favorites list
- Custom default applications per extension via `~/.panex/config.toml`
- Open files in default app (`o`) or open directory in terminal (`t`)
- Create new files (`n`) and folders (`N`)
- Multi-select with `Shift+j`/`Shift+k`, select all with `Ctrl+a`
- Mouse support — scroll the pane under the cursor, click to focus a pane / select a row, double-click to open
- Optional Oko tab cards (`O`) — what every other tab in the iTerm2 window is doing
- Built-in help overlay (`?`) with the full keybinding table
- Auto-clearing status messages

## Keyboard Shortcuts

| Key | Action |
|---|---|
| `j` / `k` or `Up` / `Down` | Move focus, wrapping past either end of the list |
| `PgUp` / `PgDn` | Move focus one page up / down (stops at the ends) |
| `g` / `G` | Jump to top / bottom |
| `Enter` | Open file / enter folder |
| `Backspace` | Go up one directory |
| `~` | Go to home directory |
| `Tab` | Switch pane |
| `\|` | Split pane vertically |
| `_` | Split pane horizontally |
| `+` or `=` | Grow active pane by 25% (one step only) |
| `-` | Shrink active pane by 25% (one step only) |
| `W` | Close pane |
| `O` | Open/close the Oko tab cards (only when [oko](https://github.com/Ivapo/oko) is installed) |
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
| `e` | Edit path / show favorites (Tab to autocomplete, Backspace removes path segment) |
| `f` | Toggle current directory as favorite |
| `Ctrl+a` | Select all |
| `Esc` | Deselect / cancel |
| `F5` | Refresh |
| `?` | Toggle help overlay |
| `q` | Quit |

Mouse: the scroll wheel scrolls the pane under the cursor; left click focuses a pane and selects the row under the cursor; double-click does what `Enter` does — enters a folder, opens a file. (To select text in the terminal while PanEx is running, use `Shift`+drag / `Option`+drag as usual for mouse-capture apps.)

## Configuration

PanEx stores its config at `~/.panex/config.toml`. Favorites are managed via keyboard (`f` to toggle, `e` to browse). You can also set custom applications for opening files by extension:

```toml
[open.tui]
".md" = "nvim"
".pdf" = "less"
".rs" = "nvim"
```

Extensions not listed fall back to the OS default (`open` on macOS, `xdg-open` on Linux).

## Tab cards (optional)

If [oko](https://github.com/Ivapo/oko) is installed, `O` opens a pane showing what every
other tab in the same iTerm2 window is doing — working directory, foreground process, and
for a Claude Code tab whether it is working, waiting on you, ready for a prompt or stale,
with how long it has said so. Press `O` again to close it. At most one such pane at a time;
ordinary file panes work alongside it as usual.

A Claude tab's card names it — `✻ claude ◐ working`, Claude Code's own mark in Claude's
orange, then the status indicator and the status — and a plain tab's card names its foreground
job instead. In a card too narrow to hold both the name and the age, the name is what goes.

Inside it, `j`/`k` move the selection (wrapping, as in a file pane), `Enter` jumps iTerm2's
focus to that tab, and `r` renames it — clearing the name puts the tab back to the one oko derives from its directory.
Everything else on the keyboard is ignored there, since the pane shows no files. The mouse
works as it does on a file row: a click selects the card under the cursor, a double-click
jumps to that tab. The selected card is marked only while the card pane is the active one,
so `Tab` away and there is one cursor on screen, not two.

Cards stack in one column until the pane is wide enough for two roomy ones side by side
(about 120 columns), on the grounds that a wide single column reads better than two cramped
ones.

More tabs than the pane can show scroll, as a long directory does: the wheel moves the view a
card at a time, moving the selection off either end brings the view with it, and a thumb on the
right border says where in the list you are.

PanEx reads this from `oko --follow` as a child process rather than linking oko as a
library, so the feature degrades to nothing when oko is absent: **no oko, no shortcut** —
`O` is not bound and the help overlay does not list it. It needs oko 0.1.0 or later (the
version that answers `--version`); an older build on `PATH` is treated as no oko at all.

```sh
cargo install --git https://github.com/Ivapo/oko
```

## License

[MIT](../../LICENSE)
