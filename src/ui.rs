use crate::app::{App, Focus, SidebarItem, SortMode};
use crate::model::{Priority, Task};
use chrono::{Local, NaiveDate};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

const ACCENT: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;
const PRIO_HIGH: Color = Color::Red;
const PRIO_MED: Color = Color::Yellow;
const PRIO_LOW: Color = Color::Blue;
const DUE_OVERDUE: Color = Color::Red;
const DUE_TODAY: Color = Color::Yellow;
const DUE_SOON: Color = Color::Green;
const ERR: Color = Color::Red;

const SIDEBAR_COLS: u16 = 26;

pub fn main_content_rect(full: Rect, has_input: bool) -> Rect {
    let mut v_constraints = vec![Constraint::Min(1)];
    if has_input {
        v_constraints.push(Constraint::Length(1));
    }
    v_constraints.push(Constraint::Length(2));
    Layout::default()
        .direction(Direction::Vertical)
        .constraints(v_constraints)
        .split(full)[0]
}

pub fn help_lines() -> Vec<Line<'static>> {
    let dim = Style::default().fg(DIM);
    let head = Style::default().add_modifier(Modifier::BOLD);
    let key = Style::default().fg(ACCENT);

    let mut lines: Vec<Line> = Vec::new();
    let section = |lines: &mut Vec<Line>, title: &str| {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(title.to_string(), head)));
    };
    let entry = |lines: &mut Vec<Line>, k: &str, desc: &str| {
        lines.push(Line::from(vec![
            Span::styled(format!("  {k:<16}"), key),
            Span::raw(desc.to_string()),
        ]));
    };

    lines.push(Line::from(Span::styled("todui — local GTD todo", head)));
    lines.push(Line::from(Span::styled(
        "Tasks live in Inbox until given a project. Contexts are",
        dim,
    )));
    lines.push(Line::from(Span::styled(
        "@tags for filtering. Completing a recurring task spawns",
        dim,
    )));
    lines.push(Line::from(Span::styled(
        "the next occurrence. All changes autosave to",
        dim,
    )));
    lines.push(Line::from(Span::styled(
        "~/.local/share/todui/todos.json and are undoable.",
        dim,
    )));

    section(&mut lines, "Navigation");
    entry(&mut lines, "↑ / ↓", "move within pane");
    entry(&mut lines, "Tab / Shift+Tab", "switch sidebar ↔ task list");
    entry(&mut lines, "Enter", "complete task / apply sidebar filter");
    section(&mut lines, "Editing");
    entry(&mut lines, "a", "add task to current view");
    entry(&mut lines, "e", "edit title");
    entry(&mut lines, "N", "edit notes");
    entry(&mut lines, "o", "set project (empty = back to Inbox)");
    entry(&mut lines, "c", "set contexts (space-separated @tags)");
    entry(&mut lines, "t", "set due date YYYY-MM-DD (empty clears)");
    entry(&mut lines, "d", "delete task");
    section(&mut lines, "Task state");
    entry(&mut lines, "n", "toggle next-action flag");
    entry(&mut lines, "p", "cycle priority: none → high → med → low");
    entry(&mut lines, "r", "cycle recurrence: daily → weekly → monthly");
    section(&mut lines, "Find");
    entry(&mut lines, "/", "search (filters live as you type)");
    entry(&mut lines, "s", "cycle sort: due → priority → created → title");
    entry(&mut lines, "u", "undo last change");
    section(&mut lines, "General");
    entry(&mut lines, "?", "toggle this help");
    entry(&mut lines, "q", "quit");
    entry(&mut lines, "Esc", "cancel prompt / clear search / dismiss message");
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Prompts: Enter commits · Esc cancels".to_string(),
        dim,
    )));
    lines.push(Line::from(Span::styled(
        "Close help: ? · Esc · q".to_string(),
        dim,
    )));

    lines
}

pub fn help_content_len() -> u16 {
    help_lines().len() as u16
}

fn help_popup_height(area: Rect) -> u16 {
    help_content_len()
        .saturating_add(2)
        .min(area.height.saturating_sub(2))
}

pub fn help_max_scroll(area: Rect) -> u16 {
    let inner = help_popup_height(area).saturating_sub(2);
    help_content_len().saturating_sub(inner)
}

pub fn rel_due(due: NaiveDate, today: NaiveDate, done: bool) -> (String, Style) {
    if done {
        return (
            due.format("%Y-%m-%d").to_string(),
            Style::default().fg(DIM),
        );
    }
    let days = (due - today).num_days();
    if days < 0 {
        let overdue = -days;
        let label = if overdue == 1 {
            "overdue 1d".to_string()
        } else if overdue < 14 {
            format!("overdue {overdue}d")
        } else {
            format!("overdue {}w", overdue / 7)
        };
        return (label, Style::default().fg(DUE_OVERDUE));
    }
    if days == 0 {
        return (
            "today".to_string(),
            Style::default().fg(DUE_TODAY),
        );
    }
    if days == 1 {
        return (
            "tomorrow".to_string(),
            Style::default().fg(DUE_SOON),
        );
    }
    if (2..=13).contains(&days) {
        let style = if days <= 3 {
            Style::default().fg(DUE_SOON)
        } else {
            Style::default()
        };
        return (format!("in {days}d"), style);
    }
    (
        format!("in {}w", days / 7),
        Style::default(),
    )
}

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let has_input = app.input.is_some();
    let main_area = main_content_rect(area, has_input);

    let mut v_constraints = vec![Constraint::Min(1)];
    if has_input {
        v_constraints.push(Constraint::Length(1));
    }
    v_constraints.push(Constraint::Length(2));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(v_constraints)
        .split(area);

    let status_area = *chunks.last().expect("status bar");
    let input_area = if has_input {
        Some(chunks[1])
    } else {
        None
    };

    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(SIDEBAR_COLS), Constraint::Min(1)])
        .split(main_area);

    render_sidebar(frame, app, h_chunks[0]);
    render_task_list(frame, app, h_chunks[1]);

    if app.celebration_active() {
        render_celebration_overlay(frame, main_area, app);
    }

    if app.show_help {
        render_help_overlay(frame, main_area, app);
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

fn pane_block(title: &str, focused: bool) -> Block<'static> {
    let border = if focused {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(DIM)
    };
    let title_style = if focused {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title(Span::styled(title.to_string(), title_style))
}

fn render_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let items = app.sidebar_items();
    let sidebar_focus = app.focus == Focus::Sidebar;
    let inner_w = area.width.saturating_sub(2) as usize;
    let mut selectable = 0usize;
    let mut showed_projects = false;
    let mut showed_contexts = false;
    let mut list_items: Vec<ListItem> = Vec::new();

    for item in &items {
        match item {
            SidebarItem::Inbox => {
                list_items.push(sidebar_row(
                    "Inbox",
                    app.task_count(item),
                    inner_w,
                    sidebar_focus && app.sidebar_index == selectable,
                ));
                selectable += 1;
            }
            SidebarItem::All => {
                list_items.push(sidebar_row(
                    "All",
                    app.task_count(item),
                    inner_w,
                    sidebar_focus && app.sidebar_index == selectable,
                ));
                selectable += 1;
            }
            SidebarItem::Project(name) => {
                if !showed_projects {
                    list_items.push(section_header("Projects"));
                    showed_projects = true;
                }
                list_items.push(sidebar_row(
                    name.as_str(),
                    app.task_count(item),
                    inner_w,
                    sidebar_focus && app.sidebar_index == selectable,
                ));
                selectable += 1;
            }
            SidebarItem::Context(name) => {
                if !showed_contexts {
                    list_items.push(section_header("Contexts"));
                    showed_contexts = true;
                }
                list_items.push(sidebar_row(
                    &format!("@{name}"),
                    app.task_count(item),
                    inner_w,
                    sidebar_focus && app.sidebar_index == selectable,
                ));
                selectable += 1;
            }
        }
    }

    let block = pane_block("todui", sidebar_focus);
    let list = List::new(list_items).block(block);
    frame.render_widget(list, area);
}

fn sidebar_row(label: &str, count: usize, inner_width: usize, selected: bool) -> ListItem<'static> {
    let count_str = format!("({count})");
    let label_len = label.chars().count();
    let count_len = count_str.chars().count();
    let pad = inner_width.saturating_sub(label_len + count_len);
    let text = format!("{label}{:pad$}{count_str}", "", pad = pad);
    let style = if selected {
        Style::default()
            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default()
    };
    ListItem::new(Line::from(Span::styled(text, style)))
}

fn section_header(title: &str) -> ListItem<'static> {
    ListItem::new(Line::from(Span::styled(
        title.to_string(),
        Style::default()
            .fg(DIM)
            .add_modifier(Modifier::UNDERLINED),
    )))
}

fn render_task_list(frame: &mut Frame, app: &App, area: Rect) {
    let today = Local::now().date_naive();
    let tasks = app.visible_tasks();
    let list_focus = app.focus == Focus::TaskList;
    let title = list_block_title(&app.sidebar_sel, &app.search, app.sort);
    let row_width = area.width.saturating_sub(2) as usize;

    let block = pane_block(&title, list_focus);

    if tasks.is_empty() {
        let empty = vec![
            Line::from("No tasks here."),
            Line::from("press a to add · ? for help"),
        ];
        frame.render_widget(
            Paragraph::new(empty)
                .block(block)
                .alignment(Alignment::Center),
            area,
        );
        return;
    }

    let items: Vec<ListItem> = tasks
        .iter()
        .map(|task| ListItem::new(task_line(task, today, row_width)))
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

fn list_block_title(sel: &SidebarItem, search: &str, sort: SortMode) -> String {
    let base = match sel {
        SidebarItem::Inbox => "Inbox".to_string(),
        SidebarItem::All => "All".to_string(),
        SidebarItem::Project(p) => p.clone(),
        SidebarItem::Context(c) => format!("@{c}"),
    };
    let filter = if search.is_empty() {
        base
    } else {
        format!("{base}/{search}")
    };
    format!("{filter} — sort:{}", sort_slug(sort))
}

fn sort_slug(sort: SortMode) -> &'static str {
    match sort {
        SortMode::Due => "due",
        SortMode::Priority => "priority",
        SortMode::Created => "created",
        SortMode::Title => "title",
    }
}

fn span_display_width(span: &Span) -> usize {
    span.content.chars().count()
}

fn task_line(task: &Task, today: NaiveDate, width: usize) -> Line<'static> {
    let done = task.completed.is_some();
    let muted = Style::default().fg(DIM);

    let prio_gutter = match task.priority {
        Some(Priority::High) => Span::styled("● ", Style::default().fg(PRIO_HIGH)),
        Some(Priority::Medium) => Span::styled("● ", Style::default().fg(PRIO_MED)),
        Some(Priority::Low) => Span::styled("● ", Style::default().fg(PRIO_LOW)),
        None => Span::raw("  "),
    };

    let check = if done {
        Span::styled("[✓] ", Style::default().fg(Color::Green))
    } else {
        Span::styled("[ ] ", muted)
    };

    let title_style = if done {
        Style::default()
            .fg(DIM)
            .add_modifier(Modifier::CROSSED_OUT)
    } else {
        Style::default()
    };
    let mut left: Vec<Span> = vec![prio_gutter, check, Span::styled(task.title.clone(), title_style)];

    for ctx in &task.contexts {
        let ctx_style = if done {
            Style::default().fg(DIM)
        } else {
            Style::default().fg(ACCENT)
        };
        left.push(Span::styled(format!(" @{ctx}"), ctx_style));
    }

    let mut right: Vec<Span> = Vec::new();
    if task.recurrence.is_some() {
        let style = if done {
            Style::default().fg(DIM)
        } else {
            Style::default().fg(Color::Magenta)
        };
        right.push(Span::styled("↻", style));
    }
    if task.next_action {
        let style = if done {
            Style::default().fg(DIM)
        } else {
            Style::default()
                .fg(DUE_SOON)
                .add_modifier(Modifier::BOLD)
        };
        if !right.is_empty() {
            right.push(Span::raw(" "));
        }
        right.push(Span::styled("▸", style));
    }
    if let Some(due) = task.due {
        let (label, due_style) = rel_due(due, today, done);
        if !right.is_empty() {
            right.push(Span::raw(" "));
        }
        right.push(Span::styled(label, due_style));
    }

    let left_w: usize = left.iter().map(span_display_width).sum();
    let right_w: usize = right.iter().map(span_display_width).sum();
    let pad = width.saturating_sub(left_w + right_w);
    left.push(Span::raw(" ".repeat(pad)));
    left.extend(right);
    Line::from(left)
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
        let style = if message.starts_with("Invalid") || message.starts_with("Error") {
            Style::default().fg(ERR)
        } else {
            Style::default().fg(DIM)
        };
        frame.render_widget(Paragraph::new(message).style(style), top[0]);
    }

    let mut right_spans = vec![Span::styled(
        sort_label(app.sort),
        Style::default().fg(DIM),
    )];
    if app.store_file.recovered {
        right_spans.push(Span::raw(" "));
        right_spans.push(Span::styled(
            "RECOVERED",
            Style::default().fg(ERR).add_modifier(Modifier::BOLD),
        ));
    }

    frame.render_widget(
        Paragraph::new(Line::from(right_spans)).alignment(Alignment::Right),
        top[1],
    );

    frame.render_widget(
        Paragraph::new(STATUS_HINTS).style(Style::default().fg(DIM)),
        rows[1],
    );
}

fn render_help_overlay(frame: &mut Frame, area: Rect, app: &App) {
    let lines = help_lines();
    let content_len = lines.len() as u16;
    let popup_w = 72u16.min(area.width.saturating_sub(4)).max(30);
    let popup_h = help_popup_height(area);
    let inner_h = popup_h.saturating_sub(2);
    let scrollable = content_len > inner_h;
    let title = if scrollable {
        " Help (↑↓ scroll) "
    } else {
        " Help "
    };

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
        .border_type(BorderType::Rounded)
        .title(title)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(Color::Black));
    frame.render_widget(ratatui::widgets::Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .scroll((app.help_scroll, 0)),
        popup,
    );
}

fn render_celebration_overlay(frame: &mut Frame, area: Rect, app: &App) {
    let Some(start) = app.celebration else {
        return;
    };
    let frame_n = start.elapsed().as_millis() as u64 / 80;
    let chars = ['*', '+', '·', '✦'];
    let colors = [ACCENT, Color::Yellow, Color::Green];
    let w = area.width as usize;
    let h = area.height as usize;
    if w == 0 || h == 0 {
        return;
    }
    let buf = frame.buffer_mut();
    for i in 0..40usize {
        let x = (i * 37 + frame_n as usize * 13) % w;
        let y = (i * 53 + frame_n as usize * 7) % h;
        let ch = chars[i % chars.len()];
        let color = colors[i % colors.len()];
        let px = area.x + x as u16;
        let py = area.y + y as u16;
        buf.set_string(px, py, ch.to_string(), Style::default().fg(color));
    }
}

fn sort_label(sort: SortMode) -> &'static str {
    match sort {
        SortMode::Due => "Due",
        SortMode::Priority => "Priority",
        SortMode::Created => "Created",
        SortMode::Title => "Title",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn label_and_fg(due: NaiveDate, today: NaiveDate, done: bool) -> (String, Color) {
        let (s, style) = rel_due(due, today, done);
        (s, style.fg.unwrap_or(Color::Reset))
    }

    #[test]
    fn rel_due_today() {
        let today = d(2026, 10, 12);
        let (s, fg) = label_and_fg(today, today, false);
        assert_eq!(s, "today");
        assert_eq!(fg, DUE_TODAY);
    }

    #[test]
    fn rel_due_tomorrow() {
        let today = d(2026, 10, 12);
        let (s, fg) = label_and_fg(d(2026, 10, 13), today, false);
        assert_eq!(s, "tomorrow");
        assert_eq!(fg, DUE_SOON);
    }

    #[test]
    fn rel_due_in_3d() {
        let today = d(2026, 10, 12);
        let (s, fg) = label_and_fg(d(2026, 10, 15), today, false);
        assert_eq!(s, "in 3d");
        assert_eq!(fg, DUE_SOON);
    }

    #[test]
    fn rel_due_in_10d() {
        let today = d(2026, 10, 12);
        let (s, fg) = label_and_fg(d(2026, 10, 22), today, false);
        assert_eq!(s, "in 10d");
        assert_eq!(fg, Color::Reset);
    }

    #[test]
    fn rel_due_in_2w() {
        let today = d(2026, 10, 12);
        let (s, fg) = label_and_fg(d(2026, 10, 26), today, false);
        assert_eq!(s, "in 2w");
        assert_eq!(fg, Color::Reset);
    }

    #[test]
    fn rel_due_overdue_1d() {
        let today = d(2026, 10, 12);
        let (s, fg) = label_and_fg(d(2026, 10, 11), today, false);
        assert_eq!(s, "overdue 1d");
        assert_eq!(fg, DUE_OVERDUE);
    }

    #[test]
    fn rel_due_overdue_10d() {
        let today = d(2026, 10, 12);
        let (s, fg) = label_and_fg(d(2026, 10, 2), today, false);
        assert_eq!(s, "overdue 10d");
        assert_eq!(fg, DUE_OVERDUE);
    }

    #[test]
    fn rel_due_done_absolute() {
        let today = d(2026, 10, 12);
        let due = d(2026, 9, 1);
        let (s, fg) = label_and_fg(due, today, true);
        assert_eq!(s, "2026-09-01");
        assert_eq!(fg, DIM);
    }
}
