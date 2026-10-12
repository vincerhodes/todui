# todui

**todui** is a local-only terminal UI for managing todos. Data stays on your machine in a single JSON file—no accounts, sync, or network.

The layout follows [Getting Things Done (GTD)](https://gettingthingsdone.com/) ideas:

- **Inbox** — tasks with no project (capture first, organize later).
- **Projects** — sidebar entries for each project name; assign with `o` (empty project returns a task to the inbox).
- **Contexts** — filter by `@`-style tags (`c` to edit; `@` in input is optional).
- **Next actions** — mark the concrete step with `n` (shown as `▸` in the list).
- **Recurrence** — daily, weekly, or monthly repeats (`r` to cycle); completing a recurring task spawns the next occurrence from its due date (or today if unset).

Tasks support optional due dates, priority (`!H` / `!M` / `!L`), search across title/project/contexts/notes, sort modes, and undo (up to 100 steps). Changes autosave after each edit.

Built with [Ratatui](https://ratatui.rs/) and [crossterm](https://github.com/crossterm-rs/crossterm).

## Requirements

- [Rust](https://www.rust-lang.org/tools/install) (2021 edition; see `Cargo.toml`)

## Build and run

From the repository root:

```bash
cargo run
```

Release build:

```bash
cargo build --release
# binary: target/release/todui
```

Run tests:

```bash
cargo test
```

## Data storage

| Item | Location |
|------|----------|
| Default data file | `~/.local/share/todui/todos.json` |
| Override | Set `TODUI_DATA_FILE` to any path |

On first save, parent directories are created if needed. Writes use a `.tmp` file and atomic rename.

If the JSON file is corrupt on load, it is moved to `todos.json.bak` (same directory), an empty store is started, and the status bar shows **RECOVERED** until you dismiss the message by other actions (initial load also sets a status message).

## Keys

Focus is either the **sidebar** (views) or the **task list**. Use `Tab` / `Shift+Tab` to switch. `↑` / `↓` move within the focused pane.

### Normal mode

| Key | Action |
|-----|--------|
| `↑` / `↓` | Move selection (sidebar or task list) |
| `Tab` | Focus task list (from sidebar) or sidebar (from list) |
| `Shift+Tab` | Same as `Tab` (toggle focus) |
| `Enter` | **Sidebar:** apply highlighted view (Inbox, All, project, or context). **Task list:** toggle complete / incomplete |
| `a` | Add task (prompt for title) |
| `e` | Edit title of selected task |
| `t` | Edit due date (`YYYY-MM-DD`; empty clears due) |
| `o` | Edit project (empty = inbox) |
| `c` | Edit contexts (space-separated, e.g. `@home @desk`) |
| `d` | Delete selected task |
| `n` | Toggle next-action flag |
| `p` | Cycle priority: none → high → medium → low → none |
| `r` | Cycle recurrence: none → daily → weekly → monthly → none |
| `/` | Search (filters current view; live update while typing) |
| `s` | Cycle sort: due → priority → created → title |
| `u` | Undo last change |
| `q` | Quit |
| `Esc` | Clear active search filter (when search text is set and no prompt is open) |

Keys `e`, `t`, `o`, and `c` apply only when a task is selected in the list.

Completed tasks stay in the current view but sort to the bottom. Overdue dues show in red; due today in yellow. Recurring tasks show `↻`.

### Prompt mode

Open when adding/editing or searching. Status hints are replaced by the input line.

| Key | Action |
|-----|--------|
| `Enter` | Commit |
| `Esc` | Cancel |
| `Backspace` | Delete character before cursor |
| Printable characters | Insert (Ctrl/Alt combinations are ignored) |

During search prompt, the list filters as you type; `Backspace` updates the filter immediately.

## Development

- Run tests: `cargo test` and lint: `cargo clippy -- -D warnings`.