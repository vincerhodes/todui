use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, InputPrompt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    MoveUp,
    MoveDown,
    NextFocus,
    PrevFocus,
    Activate,
    Quit,
    DeleteSelected,
    ToggleNext,
    CyclePriority,
    CycleRecurrence,
    CycleSort,
    Undo,
    ClearSearch,
    ClearStatus,
    ToggleHelp,
    CloseHelp,
    ScrollHelp(i16),
    StartPrompt(InputPrompt),
    CommitPrompt(InputPrompt),
    CancelPrompt,
    Backspace,
    InsertChar(char),
}

pub fn map_key(key: KeyEvent, app: &App) -> Option<Action> {
    if app.show_help {
        return map_help_key(key);
    }
    if let Some(prompt) = app.input.clone() {
        return map_prompt_key(key, prompt);
    }
    map_normal_key(key, app)
}

fn map_help_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') => Some(Action::CloseHelp),
        KeyCode::Up => Some(Action::ScrollHelp(-1)),
        KeyCode::Down => Some(Action::ScrollHelp(1)),
        KeyCode::PageUp => Some(Action::ScrollHelp(-10)),
        KeyCode::PageDown => Some(Action::ScrollHelp(10)),
        _ => None,
    }
}

fn map_prompt_key(key: KeyEvent, prompt: InputPrompt) -> Option<Action> {
    match key.code {
        KeyCode::Enter => Some(Action::CommitPrompt(prompt)),
        KeyCode::Esc => Some(Action::CancelPrompt),
        KeyCode::Backspace => Some(Action::Backspace),
        KeyCode::Char(c) if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
            Some(Action::InsertChar(c))
        }
        _ => None,
    }
}

fn map_normal_key(key: KeyEvent, app: &App) -> Option<Action> {
    match key.code {
        KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Down => Some(Action::MoveDown),
        KeyCode::Tab => Some(Action::NextFocus),
        KeyCode::BackTab => Some(Action::PrevFocus),
        KeyCode::Enter => Some(Action::Activate),
        KeyCode::Char('a') => Some(Action::StartPrompt(InputPrompt::AddTitle)),
        KeyCode::Char('e') => selected_id(app).map(|id| {
            Action::StartPrompt(InputPrompt::EditTitle(id))
        }),
        KeyCode::Char('o') => selected_id(app).map(|id| {
            Action::StartPrompt(InputPrompt::EditProject(id))
        }),
        KeyCode::Char('c') => selected_id(app).map(|id| {
            Action::StartPrompt(InputPrompt::EditContexts(id))
        }),
        KeyCode::Char('N') => selected_id(app).map(|id| {
            Action::StartPrompt(InputPrompt::EditNotes(id))
        }),
        KeyCode::Char('t') => selected_id(app).map(|id| {
            Action::StartPrompt(InputPrompt::EditDue(id))
        }),
        KeyCode::Char('d') => Some(Action::DeleteSelected),
        KeyCode::Char('n') => Some(Action::ToggleNext),
        KeyCode::Char('p') => Some(Action::CyclePriority),
        KeyCode::Char('r') => Some(Action::CycleRecurrence),
        KeyCode::Char('/') => Some(Action::StartPrompt(InputPrompt::Search)),
        KeyCode::Char('s') => Some(Action::CycleSort),
        KeyCode::Char('u') => Some(Action::Undo),
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('?') => Some(Action::ToggleHelp),
        KeyCode::Esc if !app.search.is_empty() => Some(Action::ClearSearch),
        KeyCode::Esc => Some(Action::ClearStatus),
        _ => None,
    }
}

fn selected_id(app: &App) -> Option<u64> {
    app.selected_task().map(|t| t.id)
}
