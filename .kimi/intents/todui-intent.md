# todui — Intent

Date: 2026-10-12
Stage: 1 of 3 (SDLC planning)

## Mission

Local-only TUI todo app called **todui**. New repo at `~/Projects/todui`.
Build is also a **speed test of cursor-agent composer workers** orchestrated
via herdr panes (Mode A one-shots) — plan must chunk work for parallel dispatch.

## Decisions (from intent Q&A, user-driven)

| Area | Decision |
|------|----------|
| Language | Rust |
| TUI framework | ratatui (+ crossterm backend) |
| Storage | JSON file at `~/.local/share/todui/todos.json` (respect `XDG_DATA_HOME`) |
| Feature scope | Full GTD |
| GTD elements | Inbox + projects + contexts (@tags) + next-action flag |
| Recurring | Simple repeat: daily / weekly / monthly / every-N-days, anchored to due date |
| Keybinds | Simple arrows + enter + letter shortcuts (modeless, NOT vim) |
| Layout | Sidebar (projects/contexts) + task list pane |
| Extras | Search + sort + undo |

## Functional requirements

- Task CRUD: add, edit, complete/toggle, delete.
- Task fields: title, project, contexts (@tags), priority, due date, notes,
  next-action flag, recurrence rule, created/completed timestamps.
- Inbox: tasks with no project land in Inbox; process into projects.
- Sidebar: lists Inbox + all projects + contexts; selection filters task list.
- Search: live text filter over current list.
- Sort: by due date / priority / created / title (user-switchable).
- Undo: undo stack for mutating ops (add/edit/complete/delete), e.g. `u`.
- Recurrence: completing a recurring task spawns next occurrence per rule.

## Non-functional

- Local-only: no network, no accounts, no sync.
- Fast startup (<50ms target), single static binary.
- JSON autosave on mutation; atomic write (tmp + rename) to avoid corruption.
- Graceful handling of missing/corrupt store (backup + fresh start with warning).

## Explicit non-goals

- No sync/cloud/server.
- No plugins, no themes config (ratatui default palette fine).
- No vim-style modal keybinds.
- No someday/maybe or review workflow (beyond inbox processing).

## Orchestration constraints (from mission brief)

- Implementation by cursor-agent workers via `cursor-delegate` skill +
  `~/.local/bin/cursor-task` wrapper, dispatched into herdr panes.
- Default worker model: `composer-2.5-fast`. Escalate to `grok *-fast` only if
  composer too weak. Heavy tier (`grok-4.7-xhigh`, `glm-5.2-max`) needs explicit
  user approval per task.
- `kimi-*` models via cursor: FORBIDDEN.
- Wrapper exit 3 (credit exhaustion) → fall back to Kimi subagent tiers, inform user.
- Independent chunks run in parallel herdr panes (Mode A one-shots).
- File-overlapping chunks → `cursor-task --worktree`, review diffs before merge.
- Orchestrator (Kimi K3) verifies every worker output (build + tests) before relaying.
- Per-chunk report: model, duration, files changed, test result.
- Plan stage must tag each work item with its cursor model tier.
- User approves plan before Phase 1 implementation.
