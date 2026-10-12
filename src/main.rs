mod app;
mod input;
mod model;
mod store;
mod ui;

use std::io::{self, stdout};
use std::panic;
use std::time::Duration;

use app::{App, Focus, InputPrompt};
use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use input::Action;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use store::StoreFile;

fn main() {
    if let Err(e) = run() {
        restore_terminal();
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let store_file = StoreFile::load()?;
    let mut app = App::new(store_file);
    if app.store_file.recovered {
        app.status = "Store was corrupt; backed up to .bak — starting fresh".to_string();
    }

    install_panic_hook();
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let result = loop {
        terminal.draw(|frame| ui::render(frame, &app))?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if let Some(action) = input::map_key(key, &app) {
                    apply_action(&mut app, action);
                }
            }
        }

        if app.should_quit {
            break Ok(());
        }
    };

    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn apply_action(app: &mut App, action: Action) {
    match action {
        Action::MoveUp => app.move_up(),
        Action::MoveDown => app.move_down(),
        Action::NextFocus => app.next_focus(),
        Action::PrevFocus => app.prev_focus(),
        Action::Activate => match app.focus {
            Focus::Sidebar => app.select_sidebar(),
            Focus::TaskList => app.toggle_complete(),
        },
        Action::Quit => app.should_quit = true,
        Action::DeleteSelected => app.delete_selected(),
        Action::ToggleNext => app.toggle_next_action(),
        Action::CyclePriority => app.cycle_priority(),
        Action::CycleRecurrence => app.cycle_recurrence(),
        Action::CycleSort => app.cycle_sort(),
        Action::Undo => app.undo(),
        Action::ClearSearch => app.search.clear(),
        Action::StartPrompt(prompt) => {
            app.input_buffer = prompt_initial_buffer(app, &prompt);
            app.input = Some(prompt);
        }
        Action::CommitPrompt(prompt) => {
            let buffer = std::mem::take(&mut app.input_buffer);
            match prompt {
                InputPrompt::AddTitle => app.add_task(buffer),
                InputPrompt::EditTitle(_) => app.set_title_selected(buffer),
                InputPrompt::EditDue(_) => app.set_due_selected(&buffer),
                InputPrompt::EditProject(_) => app.set_project_selected(&buffer),
                InputPrompt::EditContexts(_) => app.set_contexts_selected(&buffer),
                InputPrompt::Search => app.search = buffer,
            }
            app.input = None;
        }
        Action::CancelPrompt => {
            app.input = None;
            app.input_buffer.clear();
        }
        Action::Backspace => {
            app.input_buffer.pop();
            if matches!(app.input, Some(InputPrompt::Search)) {
                app.search = app.input_buffer.clone();
                app.list_index = 0;
            }
        }
        Action::InsertChar(c) => {
            app.input_buffer.push(c);
            if matches!(app.input, Some(InputPrompt::Search)) {
                app.search = app.input_buffer.clone();
                app.list_index = 0;
            }
        }
    }
}

fn prompt_initial_buffer(app: &App, prompt: &InputPrompt) -> String {
    match prompt {
        InputPrompt::AddTitle => String::new(),
        InputPrompt::Search => app.search.clone(),
        InputPrompt::EditTitle(_) | InputPrompt::EditDue(_) | InputPrompt::EditProject(_)
        | InputPrompt::EditContexts(_) => {
            let Some(task) = app.selected_task() else {
                return String::new();
            };
            match prompt {
                InputPrompt::EditTitle(_) => task.title.clone(),
                InputPrompt::EditDue(_) => task
                    .due
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_default(),
                InputPrompt::EditProject(_) => task.project.clone().unwrap_or_default(),
                InputPrompt::EditContexts(_) => task
                    .contexts
                    .iter()
                    .map(|c| format!("@{c}"))
                    .collect::<Vec<_>>()
                    .join(" "),
                _ => String::new(),
            }
        }
    }
}

fn install_panic_hook() {
    let original = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        restore_terminal();
        original(info);
    }));
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen);
}
