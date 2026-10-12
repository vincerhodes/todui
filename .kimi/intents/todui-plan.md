# todui — Execution Plan

Stage 3 of 3. Read `.kimi/intents/todui-intent.md` + `.kimi/intents/todui-spec.md`
first; this plan is self-contained for execution. Repo: `~/Projects/todui`
(git init done, branch `main`).

## Orchestration contract (locked)

- Workers: `cursor-task` wrapper (`~/.local/bin/cursor-task`) in herdr panes, Mode A one-shots:
  ```bash
  PANE=$(herdr pane split --direction right | jq -r .result.pane_id)
  herdr pane run "$PANE" cursor-task --model composer-2.5-fast --cwd /home/jimmy/Projects/todui "<prompt>"
  herdr pane wait-output "$PANE"
  ```
- All chunks: **composer-2.5-fast** (default; speed test). Escalate grok-fast only
  on composer failure; heavy tier needs explicit Jimmy approval. kimi-* via cursor = forbidden.
- Exit 3 → fall back to Kimi K2.8 subagent, report to Jimmy.
- Chunks A/B/C touch disjoint files → same cwd, no worktrees. Chunk D touches
  `input.rs`+`main.rs` only — also disjoint.
- Orchestrator verifies every chunk: `cargo build && cargo test` before next phase.
- Per-chunk report: model, duration, files changed, test result, log path
  (`~/.kimi-code/cursor-runs/*.log`).

## Contract files (paste VERBATIM into worker prompts)

Workers see nothing of this repo's context. Each prompt embeds the relevant
contract below verbatim plus its own file mission.

### Cargo.toml (Chunk A writes this; B/C must NOT touch it)

```toml
[package]
name = "todui"
version = "0.1.0"
edition = "2021"

[dependencies]
ratatui = "0.29"
crossterm = "0.28"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", default-features = false, features = ["clock", "serde"] }
directories = "5"
thiserror = "2"

[dev-dependencies]
tempfile = "3"
```

### model.rs contract (A implements; B/C code against it)

```rust
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Store { pub version: u32, pub next_id: u64, pub tasks: Vec<Task> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: u64,
    pub title: String,
    pub project: Option<String>,
    pub contexts: Vec<String>,
    pub priority: Option<Priority>,
    pub due: Option<NaiveDate>,
    pub notes: Option<String>,
    pub next_action: bool,
    pub recurrence: Option<Recurrence>,
    pub created: DateTime<Utc>,
    pub completed: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority { High, Medium, Low }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Recurrence { Daily, Weekly, Monthly, EveryNDays(u32) }

impl Store { pub fn new() -> Self }  // version 1, next_id 1, empty tasks
impl Recurrence { pub fn next_due(&self, anchor: NaiveDate) -> NaiveDate }
// Daily +1d, Weekly +7d, EveryNDays(n) +n d, Monthly same day next month clamped
```

### app.rs contract (B implements; C codes against it)

```rust
use crate::model::{Priority, Recurrence, Store, Task};
use crate::store::StoreFile;

pub enum Focus { Sidebar, TaskList }
pub enum SidebarItem { Inbox, All, Project(String), Context(String) }
pub enum SortMode { Due, Priority, Created, Title }
pub enum InputPrompt { /* AddTitle, EditTitle(u64), EditDue(u64), Search — B decides exact shape; C only reads app.input.is_some() and renders prompt text */ }

pub struct App {
    pub store_file: StoreFile,
    pub focus: Focus,
    pub sidebar_sel: SidebarItem,
    pub sidebar_index: usize,
    pub list_index: usize,
    pub sort: SortMode,
    pub search: String,
    pub input: Option<InputPrompt>,
    pub status: String,
    pub should_quit: bool,
}

impl App {
    pub fn new(store_file: StoreFile) -> Self;
    pub fn sidebar_items(&self) -> Vec<SidebarItem>;     // Inbox, All, projects sorted, contexts sorted, each used by UI with counts
    pub fn task_count(&self, item: &SidebarItem) -> usize;
    pub fn visible_tasks(&self) -> Vec<&Task>;           // filtered by sidebar_sel+search, sorted, completed last
    pub fn selected_task(&self) -> Option<&Task>;
    // mutations (each applies, pushes undo inverse, autosaves, sets status on error):
    pub fn add_task(&mut self, title: String);
    pub fn delete_selected(&mut self);
    pub fn toggle_complete(&mut self);                   // spawns recurrence occurrence
    pub fn cycle_priority(&mut self);
    pub fn cycle_recurrence(&mut self);
    pub fn toggle_next_action(&mut self);
    pub fn set_due_selected(&mut self, text: &str);      // parse YYYY-MM-DD, empty clears
    pub fn undo(&mut self);
    // nav:
    pub fn move_up(&mut self); pub fn move_down(&mut self);
    pub fn next_focus(&mut self); pub fn prev_focus(&mut self);
    pub fn cycle_sort(&mut self);
    pub fn select_sidebar(&mut self);                    // Enter on sidebar
}
```

Note: exact `InputPrompt` internals are B's choice, but C needs
`app.input.as_ref().map(|p| p.label_and_buffer()) -> Option<(&str, &str)>`
for the prompt line — pin that method in B's prompt.

### store.rs contract (A implements; B codes against it)

```rust
pub struct StoreFile { pub store: Store, pub recovered: bool, /* path private */ }
impl StoreFile {
    pub fn load() -> io::Result<Self>;   // TODUI_DATA_FILE env override, else XDG data dir todui/todos.json
    pub fn save(&mut self) -> Result<(), StoreError>;  // atomic tmp+rename+fsync
}
#[derive(thiserror::Error, Debug)]
pub enum StoreError { Io(#[from] std::io::Error), Json(#[from] serde_json::Error) }
```

## Chunks (dispatch order)

### Chunk A — data layer — composer-2.5-fast
- Files: `Cargo.toml`, `src/model.rs`, `src/store.rs` (unit tests inline `#[cfg(test)]`).
- Done: `cargo test` green: serde round-trip, `next_due` incl. monthly clamp
  (Jan 31 → Feb 28), missing-file load, corrupt-file → `.bak` + `recovered=true`,
  atomic save round-trip via `TODUI_DATA_FILE` + tempfile.
- Gate: build+test green before B/C results accepted (B/C run in parallel with A
  but are verified after; contract drift fixed by orchestrator at integration).

### Chunk B — app state — composer-2.5-fast (parallel with A, C)
- Files: `src/app.rs` only. Embed model.rs + store.rs contracts verbatim in prompt;
  tell worker "these files exist, do not create them; write only src/app.rs; crate
  may not compile standalone — that is expected, do not fix other files".
- Done criteria: implements full App contract above + undo stack (cap 100) +
  recurrence spawn on toggle_complete + inline tests for ops/undo/filter/sort.
  Method `InputPrompt::label_and_buffer(&self) -> (&str, &str)` included.

### Chunk C — UI render — composer-2.5-fast (parallel with A, B)
- Files: `src/ui.rs` only. Same isolation instruction. Contract: app.rs struct +
  model.rs verbatim in prompt.
- Signature: `pub fn render(frame: &mut Frame, app: &App)`.
- Layout: sidebar 26 cols | task list; bottom status line; prompt line when
  `app.input` is Some. Visuals per spec §UI (badges, overdue red, today yellow,
  `↻` recurring, `▸` next action, completed dimmed/strikethrough-style grey).

### Chunk D — input + main — composer-2.5-fast (sequential, after A+B+C green)
- Files: `src/input.rs`, `src/main.rs`.
- `input.rs`: `pub enum Action { ... }` + `pub fn map_key(key: KeyEvent, app: &App) -> Option<Action>`
  per spec keybind table (modeless; prompts capture keys first).
- `main.rs`: crossterm raw mode + alt screen + panic-hook restore, 250ms poll
  loop, dispatch Action → App methods, quit on `q`/`should_quit`.
- Done: `cargo build && cargo test && cargo clippy -- -D warnings` all clean.

### Chunk E — integration fix pass — orchestrator (Kimi) or composer on failure
- Fix contract drift between A/B/C, wire warnings, run full gate + manual smoke:
  `TODUI_DATA_FILE=/tmp/todui-smoke.json cargo run` → add/complete recurring/
  undo/search/sort/delete/quit; inspect JSON.

## Risks / edge cases (interrogated from spec)

1. **Contract drift** between parallel chunks (App/Store shape) → mitigated by
   verbatim contracts; orchestrator fixes at integration. Highest risk.
2. **ratatui 0.29 API specifics** (Frame::render_widget, Layout::constraints) —
   composer may emit 0.2x-era API → clippy gate catches; escalate to grok-fast
   if composer fails twice on C.
3. **Monthly recurrence clamp** (Jan 31 → Feb 28) — explicit test in A.
4. **Undo of recurring complete** must delete spawned occurrence — `Uncomplete
   { id, spawned_id }` inverse; test in B.
5. **Search vs prompt key routing** — prompts capture letters first; `q` quits
   only when no prompt/search active (D must honor).
6. **Completed-task visibility**: kept in list, sorted bottom, dimmed — spec
   decision, do not "hide completed" (not requested).
7. **Concurrent same-cwd runs (A/B/C)**: disjoint files only; workers forbidden
   from running `cargo` commands that write `Cargo.lock`?? No — A runs cargo
   (writes Cargo.lock); B/C told NOT to run cargo (no compile expected). State
   this in prompts.
8. **Save failure** mid-session → status bar message, never panic.

## Verification (final gate, orchestrator-run)

```bash
cd ~/Projects/todui
cargo build && cargo test && cargo clippy -- -D warnings
TODUI_DATA_FILE=/tmp/todui-smoke.json timeout 5 cargo run || true
```

Acceptance:
- build/test/clippy clean on `main`.
- Smoke: add 3 tasks, complete one recurring (next occurrence spawns with
  advanced due), undo restores, `/` search filters, `s` cycles sort, `q` quits,
  `/tmp/todui-smoke.json` valid JSON matching Store schema.
- Commit per chunk (Conventional Commits), branch `main` (local-only repo, no
  remote — no PR needed).

## Reporting

Per chunk: model, duration, files changed, build/test result, log path.
Final: composer throughput summary (chunks succeeded first-try vs retries,
wall time, fallbacks).
