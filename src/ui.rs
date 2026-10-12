use crate::app::{App, Focus, SidebarItem, SortMode};
use crate::model::{Priority, Task};
use chrono::Local;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let has_input = app.input.is_some();
    let mut v_constraints = vec![Constraint::Min(1)];
    if has_input {
        v_constraints.push(Constraint::Length(1));
    }
    v_constraints.push(Constraint::Length(2));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(v_constraints)
        .split(area);

    let main_area = chunks[0];
    let status_area = *chunks.last().expect("status bar");
    let input_area = if has_input {
        Some(chunks[1])
    } else {
        None
    };

    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(26), Constraint::Min(1)])
        .split(main_area);

    render_sidebar(frame, app, h_chunks[0]);
    render_task_list(frame, app, h_chunks[1]);

    if app.celebration_active() {
        render_celebration_overlay(frame, main_area);
    }

    if app.show_help {
        render_help_overlay(frame, main_area);
    }

    if let (Some(prompt), Some(input_rect)) = (&app.input, input_area) {
        let (label, buf) = prompt.label_and_buffer(&app.input_buffer);
        let line = Line::from(vec![
            Span::raw(label),
            Span::raw(" "),
            Span::raw(buf),
            Span::styled("▌", Style::default().add_modifier(Modifier::SLOW_BLINK)),
        ]);
        frame.render_widget(
            Paragraph::new(line).style(Style::default().bg(Color::Black)),
            input_rect,
        );
    }

    render_status_bar(frame, app, status_area);
}

fn render_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let items = app.sidebar_items();
    let sidebar_focus = app.focus == Focus::Sidebar;
    let mut selectable = 0usize;
    let mut showed_projects = false;
    let mut showed_contexts = false;

    let mut list_items: Vec<ListItem> = Vec::new();

    for item in &items {
        match item {
            SidebarItem::Inbox => {
                list_items.push(sidebar_row(
                    format!("Inbox ({})", app.task_count(item)),
                    sidebar_focus && app.sidebar_index == selectable,
                ));
                selectable += 1;
            }
            SidebarItem::All => {
                list_items.push(sidebar_row(
                    format!("All ({})", app.task_count(item)),
                    sidebar_focus && app.sidebar_index == selectable,
                ));
                selectable += 1;
            }
            SidebarItem::Project(name) => {
                if !showed_projects {
                    list_items.push(separator_row("── Projects ──"));
                    showed_projects = true;
                }
                list_items.push(sidebar_row(
                    format!("{} ({})", name, app.task_count(item)),
                    sidebar_focus && app.sidebar_index == selectable,
                ));
                selectable += 1;
            }
            SidebarItem::Context(name) => {
                if !showed_contexts {
                    list_items.push(separator_row("── Contexts ──"));
                    showed_contexts = true;
                }
                list_items.push(sidebar_row(
                    format!("@{} ({})", name, app.task_count(item)),
                    sidebar_focus && app.sidebar_index == selectable,
                ));
                selectable += 1;
            }
        }
    }

    let block = Block::default().borders(Borders::ALL).title("todui");
    let list = List::new(list_items).block(block);
    frame.render_widget(list, area);
}

fn sidebar_row(label: String, selected: bool) -> ListItem<'static> {
    let style = if selected {
        Style::default()
            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default()
    };
    ListItem::new(Line::from(Span::styled(label, style)))
}

fn separator_row(label: &str) -> ListItem<'static> {
    ListItem::new(Line::from(Span::styled(
        label.to_string(),
        Style::default().add_modifier(Modifier::DIM),
    )))
}

fn render_task_list(frame: &mut Frame, app: &App, area: Rect) {
    let today = Local::now().date_naive();
    let tasks = app.visible_tasks();
    let list_focus = app.focus == Focus::TaskList;
    let title = list_block_title(&app.sidebar_sel, &app.search);

    let border_style = if app.celebration_active() {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style);

    if tasks.is_empty() {
        frame.render_widget(
            Paragraph::new("No tasks — press 'a' to add")
                .block(block)
                .alignment(Alignment::Center),
            area,
        );
        return;
    }

    let items: Vec<ListItem> = tasks
        .iter()
        .map(|task| ListItem::new(task_line(task, today)))
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );
    let mut state = ListState::default();
    if list_focus {
        state.select(Some(app.list_index.min(tasks.len().saturating_sub(1))));
    }
    frame.render_stateful_widget(list, area, &mut state);
}

fn list_block_title(sel: &SidebarItem, search: &str) -> String {
    let base = match sel {
        SidebarItem::Inbox => "Inbox".to_string(),
        SidebarItem::All => "All".to_string(),
        SidebarItem::Project(p) => p.clone(),
        SidebarItem::Context(c) => format!("@{c}"),
    };
    if search.is_empty() {
        base
    } else {
        format!("{base}/{search}")
    }
}

fn task_line(task: &Task, today: chrono::NaiveDate) -> Line<'static> {
    let done = task.completed.is_some();
    let row_style = if done {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM)
    } else {
        Style::default()
    };

    let check = if done { "[x]" } else { "[ ]" };
    let mut spans = vec![
        Span::styled(format!("{check} "), row_style),
        Span::styled(task.title.clone(), row_style),
    ];

    for ctx in &task.contexts {
        spans.push(Span::styled(format!(" @{ctx}"), row_style));
    }

    if let Some(p) = &task.priority {
        let badge = match p {
            Priority::High => "!H",
            Priority::Medium => "!M",
            Priority::Low => "!L",
        };
        spans.push(Span::styled(format!(" {badge}"), row_style));
    }

    if let Some(due) = task.due {
        let due_style = if done {
            row_style
        } else if due < today {
            Style::default().fg(Color::Red)
        } else if due == today {
            Style::default().fg(Color::Yellow)
        } else {
            row_style
        };
        spans.push(Span::styled(format!(" {}", due.format("%Y-%m-%d")), due_style));
    }

    if task.recurrence.is_some() {
        spans.push(Span::styled(" ↻", row_style));
    }
    if task.next_action {
        spans.push(Span::styled(" ▸", row_style));
    }

    Line::from(spans)
}

const STATUS_HINTS: &str =
    "a add  e title  N notes  o proj  c ctx  t due  d del  n next  p pri  r recur  / search  s sort  u undo  ? help  q quit";

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Min(10)])
        .split(rows[0]);

    if let Some(message) = app.status_message() {
        frame.render_widget(
            Paragraph::new(message).style(Style::default().add_modifier(Modifier::DIM)),
            top[0],
        );
    }

    let mut right_spans = vec![Span::styled(
        sort_label(app.sort),
        Style::default().add_modifier(Modifier::DIM),
    )];
    if app.store_file.recovered {
        right_spans.push(Span::raw(" "));
        right_spans.push(Span::styled(
            "RECOVERED",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
    }

    frame.render_widget(
        Paragraph::new(Line::from(right_spans)).alignment(Alignment::Right),
        top[1],
    );

    frame.render_widget(
        Paragraph::new(STATUS_HINTS).style(Style::default().add_modifier(Modifier::DIM)),
        rows[1],
    );
}

fn render_help_overlay(frame: &mut Frame, area: Rect) {
    let popup_w = 52u16.min(area.width);
    let popup_h = 16u16.min(area.height);
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup = Rect {
        x,
        y,
        width: popup_w,
        height: popup_h,
    };
    let dim = Style::default().add_modifier(Modifier::DIM);
    let head = Style::default().add_modifier(Modifier::BOLD);
    let lines = vec![
        Line::from(Span::styled("Help", head)),
        Line::from(""),
        Line::from(Span::styled("Navigation", head)),
        Line::from("  ↑/↓ move   Tab/Shift+Tab focus   Enter select/complete"),
        Line::from(""),
        Line::from(Span::styled("Editing", head)),
        Line::from("  a add   e title   N notes   o project   c contexts   t due   d delete"),
        Line::from(""),
        Line::from(Span::styled("Task ops", head)),
        Line::from("  n next-action   p priority   r recurrence   Enter toggle complete"),
        Line::from(""),
        Line::from(Span::styled("Find", head)),
        Line::from("  / search   s sort   u undo"),
        Line::from(""),
        Line::from(Span::styled("General", head)),
        Line::from("  ? help   q quit   Esc clear search/status"),
        Line::from(""),
        Line::from(Span::styled("Prompts: Enter commits · Esc cancels", dim)),
        Line::from(Span::styled("Close help: ? · Esc · q", dim)),
    ];
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Shortcuts")
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Black));
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}

fn render_celebration_overlay(frame: &mut Frame, area: Rect) {
    let popup_w = 18u16.min(area.width);
    let popup_h = 3u16.min(area.height);
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup = Rect {
        x,
        y,
        width: popup_w,
        height: popup_h,
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(Color::Black));
    frame.render_widget(
        Paragraph::new("🎉 Done!")
            .block(block)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        popup,
    );
}

fn sort_label(sort: SortMode) -> &'static str {
    match sort {
        SortMode::Due => "Due",
        SortMode::Priority => "Priority",
        SortMode::Created => "Created",
        SortMode::Title => "Title",
    }
}
