# todui — Technical Spec

Stage 2 of 3. Source: `.kimi/intents/todui-intent.md`.

## Stack

- Rust 2021 edition, single binary crate `todui`.
- Deps (pin in Cargo.toml):
  - `ratatui = "0.29"` — TUI rendering
  - `crossterm = "0.28"` — terminal backend + events
  - `serde = { version = "1", features = ["derive"] }`
  - `serde_json = "1"`
  - `chrono = { version = "0.4", default-features = false, features = ["clock", "serde"] }`
  - `directories = "5"` — XDG data dir
  - `thiserror = "2"` — error types
- No uuid crate: task ids are `u64` from a `next_id` counter in the store file.
- Dev deps: `tempfile = "3"` for store tests.

## File layout

```
Cargo.toml
src/
  main.rs        — entry: terminal setup/teardown, event loop
  model.rs       — Task, Priority, Recurrence, Store schema (serde)
  store.rs       — load/save JSON, atomic write, corrupt-file recovery, XDG path
  app.rs         — App state, mutation ops, undo stack, recurrence spawn
  ui.rs          — ratatui rendering: sidebar, task list, status bar, input line
  input.rs       — key event → Action mapping, modeless keybinds
```

## Data model (`model.rs`)

```rust
#[derive(Serialize, Deserialize)]
pub struct Store {
    pub version: u32,        // = 1
    pub next_id: u64,
    pub tasks: Vec<Task>,
}

pub struct Task {
    pub id: u64,
    pub title: String,
    pub project: Option<String>,      // None => Inbox
    pub contexts: Vec<String>,        // @tags, stored without '@'
    pub priority: Option<Priority>,   // High | Medium | Low
    pub due: Option<NaiveDate>,
    pub notes: Option<String>,
    pub next_action: bool,
    pub recurrence: Option<Recurrence>,
    pub created: DateTime<Utc>,
    pub completed: Option<DateTime<Utc>>,  // Some => done
}

pub enum Recurrence { Daily, Weekly, Monthly, EveryNDays(u32) }
```

- Recurrence spawns next occurrence anchored to the **due date** (if no due date,
  anchor to completion date). New occurrence: fresh id, `completed: None`,
  `created: now`, due advanced per rule.
- Monthly = same day-of-month next month, clamped to month's last day.

## Persistence (`store.rs`)

- Path: `directories::ProjectDirs::from("dev", "todui", "todui").data_dir()/todos.json`
  → resolves to `~/.local/share/todui/todos.json`. Env override `TODUI_DATA_FILE`
  for tests/manual runs.
- Load: missing file → empty store. Parse failure → rename to `todos.json.bak`
  (overwrite old `.bak`), return empty store + `recovered: bool` flag surfaced in
  status bar.
- Save: serialize pretty JSON → write `todos.json.tmp` → `fsync` → rename over
  `todos.json` (atomic). Called after every mutating op (autosave).

## App state (`app.rs`)

```rust
pub struct App {
    pub store: Store,
    pub focus: Focus,                  // Sidebar | TaskList
    pub sidebar_sel: SidebarItem,      // Inbox | All | Project(String) | Context(String)
    pub list_index: usize,             // into filtered+sorted view
    pub sort: SortMode,                // Due | Priority | Created | Title
    pub search: String,                // active text filter ("" = off)
    pub input: Option<InputPrompt>,    // add/edit prompt state
    pub undo: Vec<InverseOp>,
    pub status: String,                // status bar message
    pub should_quit: bool,
}
```

- `visible_tasks()`: filter by sidebar_sel + completed-flag handling + search
  substring (case-insensitive, matches title/project/contexts/notes), then sort
  per `sort`. Completed tasks sort to bottom regardless of sort mode.
- `InverseOp` variants: `RemoveAdded(u64)`, `RestoreDeleted(Task)`, 
  `RestoreEdited(Task)`, `Uncomplete { id: u64, spawned_id: Option<u64> }`
  (drops recurrence-spawned task on undo). Undo stack capped at 100 entries.
- Mutation ops (`add_task`, `edit_task`, `toggle_complete`, `delete_task`,
  `set_priority`, `set_due`, `set_recurrence`, `toggle_next_action`,
  `move_to_project`, `set_contexts`) each: apply → push inverse → autosave.

## Keybinds (`input.rs`) — modeless, simple arrows

| Key | Action |
|-----|--------|
| `↑`/`↓` | move in focused pane |
| `Tab` / `Shift-Tab` | switch focus sidebar ↔ task list |
| `Enter` | toggle complete (list) / select filter (sidebar) |
| `a` | add task (input prompt: title, then follow-up prompts for project/due/etc. via `e`) |
| `e` | edit selected task field (prompt menu) |
| `d` | delete selected task |
| `n` | toggle next-action flag |
| `p` | cycle priority None→High→Med→Low |
| `t` | set due date (prompt, `YYYY-MM-DD`, empty clears) |
| `r` | cycle recurrence None→Daily→Weekly→Monthly |
| `/` | search prompt (live filter, `Esc` clears) |
| `s` | cycle sort mode |
| `u` | undo |
| `q` | quit (when no prompt active) |
| `Esc` | cancel prompt / clear search |

Input prompts render in the bottom line; `Enter` commits, `Esc` cancels.
While a prompt is active, letter keys go to the prompt, not actions.

## UI (`ui.rs`)

- Layout: horizontal split — sidebar fixed 26 cols left, task list right;
  bottom 1-line status bar (and input prompt line when active).
- Sidebar: `Inbox (n)`, `All (n)`, `── Projects ──` + each project with count,
  `── Contexts ──` + each @context with count. Highlight selected item.
- Task row: `[x]`/`[ ]`, title, `@ctx` tags, `!H/!M/!L` priority badge,
  due date (red if overdue, yellow if today), `↻` if recurring, `▸` if next action.
- Status bar: current filter, sort mode, search string, transient message,
  `recovered from corrupt store` warning when applicable.
- Empty states: "No tasks — press `a` to add".

## Main loop (`main.rs`)

- crossterm raw mode + alternate screen; panic hook restores terminal.
- Loop: draw → poll event (250ms tick) → map key → apply action → autosave on
  mutation → quit on `should_quit`.

## Architectural constraints

- Single crate, no async, no network code, no unsafe.
- All mutations go through `app.rs` ops (undo + autosave invariant).
- `ui.rs` is pure render (no mutation); `input.rs` maps keys to an `Action`
  enum consumed by `main.rs` — keeps chunks decoupled for parallel build.
- Errors: `thiserror` `StoreError` for IO/parse; UI-level ops return `Result`
  only where IO is involved; save failures surface in status bar, never crash.

## Verification surface

- `cargo build` green, `cargo test` green, `cargo clippy -- -D warnings` clean.
- Unit tests: store round-trip, corrupt recovery, recurrence math (incl.
  month clamp), undo for each op, visible_tasks filter/sort.
- Manual smoke: `cargo run` — add/complete/delete/undo/search/sort/quit;
  verify JSON file contents.
