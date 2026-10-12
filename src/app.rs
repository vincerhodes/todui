use chrono::{NaiveDate, Utc};
use std::cmp::Ordering;
use std::time::Instant;

use crate::model::{Priority, Recurrence, Task};
use crate::store::StoreFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    TaskList,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarItem {
    Inbox,
    All,
    Project(String),
    Context(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Due,
    Priority,
    Created,
    Title,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputPrompt {
    AddTitle,
    EditTitle(u64),
    EditDue(u64),
    EditProject(u64),
    EditContexts(u64),
    EditNotes(u64),
    Search,
}

impl InputPrompt {
    pub fn label_and_buffer<'a>(&self, buffer: &'a str) -> (&'static str, &'a str) {
        match self {
            InputPrompt::AddTitle => ("Title: ", buffer),
            InputPrompt::EditTitle(_) => ("Edit title: ", buffer),
            InputPrompt::EditDue(_) => ("Due (YYYY-MM-DD): ", buffer),
            InputPrompt::EditProject(_) => ("Project (empty = inbox): ", buffer),
            InputPrompt::EditContexts(_) => ("Contexts (space-separated): ", buffer),
            InputPrompt::EditNotes(_) => ("Notes: ", buffer),
            InputPrompt::Search => ("Search: ", buffer),
        }
    }
}

enum InverseOp {
    RemoveAdded(u64),
    RestoreDeleted(Task),
    RestoreEdited(Task),
    Uncomplete {
        id: u64,
        spawned_id: Option<u64>,
    },
}

const UNDO_CAP: usize = 100;
pub const STATUS_MESSAGE_MS: u128 = 5000;

pub fn status_message_live(elapsed_ms: u128) -> bool {
    elapsed_ms <= STATUS_MESSAGE_MS
}

pub struct App {
    pub store_file: StoreFile,
    pub focus: Focus,
    pub sidebar_sel: SidebarItem,
    pub sidebar_index: usize,
    pub list_index: usize,
    pub sort: SortMode,
    pub search: String,
    pub input: Option<InputPrompt>,
    pub input_buffer: String,
    status: Option<(Instant, String)>,
    pub should_quit: bool,
    pub celebration: Option<Instant>,
    pub show_help: bool,
    pub help_scroll: u16,
    undo: Vec<InverseOp>,
}

impl App {
    pub fn new(store_file: StoreFile) -> Self {
        Self {
            store_file,
            focus: Focus::Sidebar,
            sidebar_sel: SidebarItem::Inbox,
            sidebar_index: 0,
            list_index: 0,
            sort: SortMode::Due,
            search: String::new(),
            input: None,
            input_buffer: String::new(),
            status: None,
            should_quit: false,
            celebration: None,
            show_help: false,
            help_scroll: 0,
            undo: Vec::new(),
        }
    }

    pub fn set_status(&mut self, message: String) {
        self.status = Some((Instant::now(), message));
    }

    pub fn clear_status(&mut self) {
        self.status = None;
    }

    pub fn status_active(&self) -> bool {
        self.status
            .as_ref()
            .is_some_and(|(since, _)| status_message_live(since.elapsed().as_millis()))
    }

    pub fn status_message(&self) -> Option<&str> {
        if self.status_active() {
            self.status.as_ref().map(|(_, msg)| msg.as_str())
        } else {
            None
        }
    }

    pub fn expire_stale_status(&mut self) {
        if self.status.is_some() && !self.status_active() {
            self.status = None;
        }
    }

    pub fn sidebar_items(&self) -> Vec<SidebarItem> {
        let mut projects: Vec<String> = self
            .store_file
            .store
            .tasks
            .iter()
            .filter_map(|t| t.project.clone())
            .collect();
        projects.sort_by_key(|a| a.to_lowercase());
        projects.dedup();

        let mut contexts: Vec<String> = self
            .store_file
            .store
            .tasks
            .iter()
            .flat_map(|t| t.contexts.iter().cloned())
            .collect();
        contexts.sort_by_key(|a| a.to_lowercase());
        contexts.dedup();

        let mut items = vec![SidebarItem::Inbox, SidebarItem::All];
        items.extend(projects.into_iter().map(SidebarItem::Project));
        items.extend(contexts.into_iter().map(SidebarItem::Context));
        items
    }

    pub fn task_count(&self, item: &SidebarItem) -> usize {
        self.store_file
            .store
            .tasks
            .iter()
            .filter(|t| t.completed.is_none() && Self::matches_sidebar(t, item))
            .count()
    }

    pub fn visible_tasks(&self) -> Vec<&Task> {
        let mut tasks: Vec<&Task> = self
            .store_file
            .store
            .tasks
            .iter()
            .filter(|t| Self::matches_sidebar(t, &self.sidebar_sel))
            .filter(|t| Self::matches_search(t, &self.search))
            .collect();

        tasks.sort_by(|a, b| Self::compare_tasks(a, b, self.sort));
        tasks
    }

    pub fn selected_task(&self) -> Option<&Task> {
        self.visible_tasks().get(self.list_index).copied()
    }

    pub fn add_task(&mut self, title: String) {
        let id = self.store_file.store.next_id;
        self.store_file.store.next_id += 1;
        let task = Task {
            id,
            title,
            project: None,
            contexts: Vec::new(),
            priority: None,
            due: None,
            notes: None,
            next_action: false,
            recurrence: None,
            created: Utc::now(),
            completed: None,
        };
        self.store_file.store.tasks.push(task);
        self.push_undo(InverseOp::RemoveAdded(id));
        self.autosave();
        self.clamp_list_index();
    }

    pub fn delete_selected(&mut self) {
        let Some(id) = self.selected_task().map(|t| t.id) else {
            return;
        };
        let Some(pos) = self.store_file.store.tasks.iter().position(|t| t.id == id) else {
            return;
        };
        let removed = self.store_file.store.tasks.remove(pos);
        self.push_undo(InverseOp::RestoreDeleted(removed));
        self.autosave();
        self.clamp_list_index();
    }

    pub fn toggle_complete(&mut self) {
        let Some(id) = self.selected_task().map(|t| t.id) else {
            return;
        };
        let Some(pos) = self.store_file.store.tasks.iter().position(|t| t.id == id) else {
            return;
        };
        let task = &self.store_file.store.tasks[pos];
        if task.completed.is_some() {
            let before = task.clone();
            self.store_file.store.tasks[pos].completed = None;
            self.push_undo(InverseOp::RestoreEdited(before));
        } else {
            let before = task.clone();
            self.store_file.store.tasks[pos].completed = Some(Utc::now());

            let mut spawned_id = None;
            if let Some(recurrence) = before.recurrence {
                let anchor = before
                    .due
                    .unwrap_or_else(|| Utc::now().date_naive());
                let new_due = recurrence.next_due(anchor);
                let new_id = self.store_file.store.next_id;
                self.store_file.store.next_id += 1;
                let spawned = Task {
                    id: new_id,
                    title: before.title.clone(),
                    project: before.project.clone(),
                    contexts: before.contexts.clone(),
                    priority: before.priority,
                    due: Some(new_due),
                    notes: before.notes.clone(),
                    next_action: before.next_action,
                    recurrence: before.recurrence,
                    created: Utc::now(),
                    completed: None,
                };
                self.store_file.store.tasks.push(spawned);
                spawned_id = Some(new_id);
            }
            self.celebration = Some(Instant::now());
            self.push_undo(InverseOp::Uncomplete {
                id: before.id,
                spawned_id,
            });
        }
        self.autosave();
        self.clamp_list_index();
    }

    pub fn cycle_priority(&mut self) {
        let Some(id) = self.selected_task().map(|t| t.id) else {
            return;
        };
        let Some(pos) = self.store_file.store.tasks.iter().position(|t| t.id == id) else {
            return;
        };
        let before = self.store_file.store.tasks[pos].clone();
        let next = match before.priority {
            None => Some(Priority::High),
            Some(Priority::High) => Some(Priority::Medium),
            Some(Priority::Medium) => Some(Priority::Low),
            Some(Priority::Low) => None,
        };
        self.store_file.store.tasks[pos].priority = next;
        self.push_undo(InverseOp::RestoreEdited(before));
        self.autosave();
        self.clamp_list_index();
    }

    pub fn cycle_recurrence(&mut self) {
        let Some(id) = self.selected_task().map(|t| t.id) else {
            return;
        };
        let Some(pos) = self.store_file.store.tasks.iter().position(|t| t.id == id) else {
            return;
        };
        let before = self.store_file.store.tasks[pos].clone();
        let next = match before.recurrence {
            None => Some(Recurrence::Daily),
            Some(Recurrence::Daily) => Some(Recurrence::Weekly),
            Some(Recurrence::Weekly) => Some(Recurrence::Monthly),
            Some(Recurrence::Monthly) | Some(Recurrence::EveryNDays(_)) => None,
        };
        self.store_file.store.tasks[pos].recurrence = next;
        self.push_undo(InverseOp::RestoreEdited(before));
        self.autosave();
        self.clamp_list_index();
    }

    pub fn toggle_next_action(&mut self) {
        let Some(id) = self.selected_task().map(|t| t.id) else {
            return;
        };
        let Some(pos) = self.store_file.store.tasks.iter().position(|t| t.id == id) else {
            return;
        };
        let before = self.store_file.store.tasks[pos].clone();
        self.store_file.store.tasks[pos].next_action = !before.next_action;
        self.push_undo(InverseOp::RestoreEdited(before));
        self.autosave();
        self.clamp_list_index();
    }

    pub fn set_due_selected(&mut self, text: &str) {
        let Some(id) = self.selected_task().map(|t| t.id) else {
            return;
        };
        let Some(pos) = self.store_file.store.tasks.iter().position(|t| t.id == id) else {
            return;
        };
        let new_due = if text.is_empty() {
            None
        } else {
            match NaiveDate::parse_from_str(text, "%Y-%m-%d") {
                Ok(d) => Some(d),
                Err(_) => {
                    self.set_status(format!("Invalid date '{text}', use YYYY-MM-DD"));
                    return;
                }
            }
        };
        let before = self.store_file.store.tasks[pos].clone();
        self.store_file.store.tasks[pos].due = new_due;
        self.push_undo(InverseOp::RestoreEdited(before));
        self.autosave();
        self.clamp_list_index();
    }

    pub fn set_title_selected(&mut self, title: String) {
        let Some(id) = self.selected_task().map(|t| t.id) else {
            return;
        };
        let Some(task) = self.store_file.store.tasks.iter_mut().find(|t| t.id == id) else {
            return;
        };
        let before = task.clone();
        task.title = title;
        self.push_undo(InverseOp::RestoreEdited(before));
        self.autosave();
    }

    pub fn set_project_selected(&mut self, text: &str) {
        let Some(id) = self.selected_task().map(|t| t.id) else {
            return;
        };
        let Some(task) = self.store_file.store.tasks.iter_mut().find(|t| t.id == id) else {
            return;
        };
        let before = task.clone();
        let trimmed = text.trim();
        task.project = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
        self.push_undo(InverseOp::RestoreEdited(before));
        self.autosave();
        self.clamp_list_index();
    }

    pub fn set_contexts_selected(&mut self, text: &str) {
        let Some(id) = self.selected_task().map(|t| t.id) else {
            return;
        };
        let Some(task) = self.store_file.store.tasks.iter_mut().find(|t| t.id == id) else {
            return;
        };
        let before = task.clone();
        task.contexts = text
            .split_whitespace()
            .map(|c| c.trim_start_matches('@').to_string())
            .filter(|c| !c.is_empty())
            .collect();
        self.push_undo(InverseOp::RestoreEdited(before));
        self.autosave();
    }

    pub fn set_notes_selected(&mut self, text: String) {
        let Some(id) = self.selected_task().map(|t| t.id) else {
            return;
        };
        let Some(task) = self.store_file.store.tasks.iter_mut().find(|t| t.id == id) else {
            return;
        };
        let before = task.clone();
        task.notes = if text.is_empty() {
            None
        } else {
            Some(text)
        };
        self.push_undo(InverseOp::RestoreEdited(before));
        self.autosave();
    }

    pub fn celebration_active(&self) -> bool {
        const CELEBRATION_MS: u128 = 600;
        self.celebration
            .is_some_and(|start| start.elapsed().as_millis() <= CELEBRATION_MS)
    }

    pub fn undo(&mut self) {
        let Some(op) = self.undo.pop() else {
            self.set_status("Nothing to undo".to_string());
            return;
        };
        match op {
            InverseOp::RemoveAdded(id) => {
                self.store_file.store.tasks.retain(|t| t.id != id);
            }
            InverseOp::RestoreDeleted(task) => {
                self.store_file.store.tasks.push(task);
            }
            InverseOp::RestoreEdited(task) => {
                if let Some(pos) = self
                    .store_file
                    .store
                    .tasks
                    .iter()
                    .position(|t| t.id == task.id)
                {
                    self.store_file.store.tasks[pos] = task;
                }
            }
            InverseOp::Uncomplete { id, spawned_id } => {
                if let Some(pos) = self.store_file.store.tasks.iter().position(|t| t.id == id) {
                    self.store_file.store.tasks[pos].completed = None;
                }
                if let Some(sid) = spawned_id {
                    self.store_file.store.tasks.retain(|t| t.id != sid);
                }
            }
        }
        self.autosave();
        self.clamp_list_index();
    }

    pub fn move_up(&mut self) {
        match self.focus {
            Focus::Sidebar => {
                self.sidebar_index = self.sidebar_index.saturating_sub(1);
            }
            Focus::TaskList => {
                self.list_index = self.list_index.saturating_sub(1);
            }
        }
    }

    pub fn move_down(&mut self) {
        match self.focus {
            Focus::Sidebar => {
                let max = self.sidebar_items().len().saturating_sub(1);
                if self.sidebar_index < max {
                    self.sidebar_index += 1;
                }
            }
            Focus::TaskList => {
                let max = self.visible_tasks().len().saturating_sub(1);
                if self.list_index < max {
                    self.list_index += 1;
                }
            }
        }
    }

    pub fn next_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Sidebar => Focus::TaskList,
            Focus::TaskList => Focus::Sidebar,
        };
    }

    pub fn prev_focus(&mut self) {
        self.next_focus();
    }

    pub fn cycle_sort(&mut self) {
        self.sort = match self.sort {
            SortMode::Due => SortMode::Priority,
            SortMode::Priority => SortMode::Created,
            SortMode::Created => SortMode::Title,
            SortMode::Title => SortMode::Due,
        };
        self.clamp_list_index();
    }

    pub fn select_sidebar(&mut self) {
        let items = self.sidebar_items();
        if let Some(item) = items.get(self.sidebar_index) {
            self.sidebar_sel = item.clone();
            self.list_index = 0;
        }
    }

    fn matches_sidebar(task: &Task, item: &SidebarItem) -> bool {
        match item {
            SidebarItem::Inbox => task.project.is_none(),
            SidebarItem::All => true,
            SidebarItem::Project(p) => task.project.as_deref() == Some(p.as_str()),
            SidebarItem::Context(c) => task.contexts.iter().any(|x| x == c),
        }
    }

    fn matches_search(task: &Task, search: &str) -> bool {
        if search.is_empty() {
            return true;
        }
        let needle = search.to_lowercase();
        if task.title.to_lowercase().contains(&needle) {
            return true;
        }
        if task
            .project
            .as_ref()
            .is_some_and(|p| p.to_lowercase().contains(&needle))
        {
            return true;
        }
        if task
            .contexts
            .iter()
            .any(|c| c.to_lowercase().contains(&needle))
        {
            return true;
        }
        if task
            .notes
            .as_ref()
            .is_some_and(|n| n.to_lowercase().contains(&needle))
        {
            return true;
        }
        false
    }

    fn compare_tasks(a: &Task, b: &Task, sort: SortMode) -> Ordering {
        let a_done = a.completed.is_some();
        let b_done = b.completed.is_some();
        match (a_done, b_done) {
            (false, true) => Ordering::Less,
            (true, false) => Ordering::Greater,
            _ => Self::compare_incomplete_or_completed_group(a, b, sort),
        }
    }

    fn compare_incomplete_or_completed_group(a: &Task, b: &Task, sort: SortMode) -> Ordering {
        match sort {
            SortMode::Due => match (a.due, b.due) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(da), Some(db)) => da.cmp(&db),
            },
            SortMode::Priority => {
                fn rank(p: Option<Priority>) -> u8 {
                    match p {
                        Some(Priority::High) => 0,
                        Some(Priority::Medium) => 1,
                        Some(Priority::Low) => 2,
                        None => 3,
                    }
                }
                rank(a.priority).cmp(&rank(b.priority))
            }
            SortMode::Created => a.created.cmp(&b.created),
            SortMode::Title => a
                .title
                .to_lowercase()
                .cmp(&b.title.to_lowercase())
                .then_with(|| a.id.cmp(&b.id)),
        }
    }

    fn push_undo(&mut self, op: InverseOp) {
        self.undo.push(op);
        if self.undo.len() > UNDO_CAP {
            let drop = self.undo.len() - UNDO_CAP;
            self.undo.drain(0..drop);
        }
    }

    fn autosave(&mut self) {
        if let Err(e) = self.store_file.save() {
            self.set_status(e.to_string());
        }
    }

    fn clamp_list_index(&mut self) {
        let len = self.visible_tasks().len();
        if len == 0 {
            self.list_index = 0;
        } else if self.list_index >= len {
            self.list_index = len - 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    fn app_with_tasks(tasks: Vec<Task>) -> App {
        let _guard = crate::store::test_env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("todos.json");
        std::env::set_var("TODUI_DATA_FILE", &path);
        let mut store_file = StoreFile::load().unwrap();
        store_file.store.tasks = tasks;
        store_file.store.next_id = store_file
            .store
            .tasks
            .iter()
            .map(|t| t.id)
            .max()
            .unwrap_or(0)
            + 1;
        store_file.save().unwrap();
        let store_file = StoreFile::load().unwrap();
        std::mem::forget(dir);
        App::new(store_file)
    }

    fn sample_task(id: u64, title: &str) -> Task {
        Task {
            id,
            title: title.to_string(),
            project: None,
            contexts: Vec::new(),
            priority: None,
            due: None,
            notes: None,
            next_action: false,
            recurrence: None,
            created: Utc.with_ymd_and_hms(2024, 1, id as u32, 0, 0, 0).unwrap(),
            completed: None,
        }
    }

    #[test]
    fn add_delete_undo_restore_round_trip() {
        let mut app = app_with_tasks(vec![]);
        app.add_task("Alpha".to_string());
        assert_eq!(app.store_file.store.tasks.len(), 1);
        app.list_index = 0;
        app.delete_selected();
        assert!(app.store_file.store.tasks.is_empty());
        app.undo();
        assert_eq!(app.store_file.store.tasks.len(), 1);
        assert_eq!(app.store_file.store.tasks[0].title, "Alpha");
    }

    #[test]
    fn set_title_project_contexts_and_undo() {
        let mut app = app_with_tasks(vec![sample_task(1, "Old")]);
        app.list_index = 0;
        app.set_title_selected("New".to_string());
        app.set_contexts_selected("@home @desk");
        app.set_project_selected("work");
        let task = &app.store_file.store.tasks[0];
        assert_eq!(task.title, "New");
        assert_eq!(task.project.as_deref(), Some("work"));
        assert_eq!(task.contexts, vec!["home".to_string(), "desk".to_string()]);
        app.undo();
        app.undo();
        app.undo();
        let task = &app.store_file.store.tasks[0];
        assert_eq!(task.title, "Old");
        assert_eq!(task.project, None);
        assert!(task.contexts.is_empty());
        app.set_project_selected("   ");
        assert_eq!(app.store_file.store.tasks[0].project, None);
    }

    #[test]
    fn set_notes_and_undo() {
        let mut app = app_with_tasks(vec![sample_task(1, "T")]);
        app.list_index = 0;
        app.set_notes_selected("Line one".to_string());
        assert_eq!(
            app.store_file.store.tasks[0].notes.as_deref(),
            Some("Line one")
        );
        app.set_notes_selected(String::new());
        assert!(app.store_file.store.tasks[0].notes.is_none());
        app.undo();
        assert_eq!(
            app.store_file.store.tasks[0].notes.as_deref(),
            Some("Line one")
        );
        app.undo();
        assert!(app.store_file.store.tasks[0].notes.is_none());
    }

    #[test]
    fn celebration_on_complete_not_on_uncomplete() {
        let mut app = app_with_tasks(vec![sample_task(1, "T")]);
        app.list_index = 0;
        assert!(app.celebration.is_none());
        app.toggle_complete();
        assert!(app.celebration.is_some());
        assert!(app.store_file.store.tasks[0].completed.is_some());
        let celebrated_at = app.celebration;
        app.toggle_complete();
        assert!(app.store_file.store.tasks[0].completed.is_none());
        assert_eq!(app.celebration, celebrated_at);
    }

    #[test]
    fn toggle_complete_recurring_spawns_and_undo_removes_spawn() {
        let due = NaiveDate::from_ymd_opt(2024, 6, 1).unwrap();
        let mut task = sample_task(1, "Repeat");
        task.recurrence = Some(Recurrence::Daily);
        task.due = Some(due);
        let mut app = app_with_tasks(vec![task]);
        app.list_index = 0;
        app.toggle_complete();
        assert_eq!(app.store_file.store.tasks.len(), 2);
        let completed = app
            .store_file
            .store
            .tasks
            .iter()
            .find(|t| t.id == 1)
            .unwrap();
        assert!(completed.completed.is_some());
        let spawned = app
            .store_file
            .store
            .tasks
            .iter()
            .find(|t| t.id != 1)
            .unwrap();
        assert_eq!(spawned.due, Some(due.succ_opt().unwrap()));
        assert!(spawned.completed.is_none());
        app.undo();
        assert_eq!(app.store_file.store.tasks.len(), 1);
        assert!(app.store_file.store.tasks[0].completed.is_none());
    }

    #[test]
    fn cycle_priority_full_cycle() {
        let mut app = app_with_tasks(vec![sample_task(1, "P")]);
        app.list_index = 0;
        assert_eq!(app.selected_task().unwrap().priority, None);
        app.cycle_priority();
        assert_eq!(app.selected_task().unwrap().priority, Some(Priority::High));
        app.cycle_priority();
        assert_eq!(
            app.selected_task().unwrap().priority,
            Some(Priority::Medium)
        );
        app.cycle_priority();
        assert_eq!(app.selected_task().unwrap().priority, Some(Priority::Low));
        app.cycle_priority();
        assert_eq!(app.selected_task().unwrap().priority, None);
    }

    #[test]
    fn set_due_selected_valid_invalid_empty() {
        let mut app = app_with_tasks(vec![sample_task(1, "D")]);
        app.list_index = 0;
        app.set_due_selected("2025-03-15");
        assert_eq!(
            app.selected_task().unwrap().due,
            Some(NaiveDate::from_ymd_opt(2025, 3, 15).unwrap())
        );
        app.set_due_selected("");
        assert!(app.selected_task().unwrap().due.is_none());
        app.set_due_selected("not-a-date");
        assert!(
            app.status_message()
                .is_some_and(|s| s.contains("Invalid date"))
        );
        assert!(app.selected_task().unwrap().due.is_none());
    }

    #[test]
    fn visible_tasks_inbox_filter() {
        let mut inbox = sample_task(1, "In");
        inbox.project = None;
        let mut proj = sample_task(2, "Out");
        proj.project = Some("Work".to_string());
        let app = app_with_tasks(vec![inbox, proj]);
        let titles: Vec<_> = app.visible_tasks().into_iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, vec!["In"]);
    }

    #[test]
    fn visible_tasks_project_filter() {
        let mut app = app_with_tasks(vec![]);
        let mut a = sample_task(1, "A");
        a.project = Some("Work".to_string());
        app.store_file.store.tasks.push(a);
        app.sidebar_sel = SidebarItem::Project("Work".to_string());
        assert_eq!(app.visible_tasks().len(), 1);
        assert_eq!(app.visible_tasks()[0].title, "A");
    }

    #[test]
    fn visible_tasks_context_filter() {
        let mut app = app_with_tasks(vec![]);
        let mut tagged = sample_task(1, "Tagged");
        tagged.contexts = vec!["home".to_string()];
        app.store_file.store.tasks.push(tagged);
        app.store_file.store.tasks.push(sample_task(2, "Other"));
        app.sidebar_sel = SidebarItem::Context("home".to_string());
        assert_eq!(app.visible_tasks().len(), 1);
        assert_eq!(app.visible_tasks()[0].title, "Tagged");
    }

    #[test]
    fn visible_tasks_search_substring() {
        let mut app = app_with_tasks(vec![]);
        let mut t = sample_task(1, "FindMe");
        t.notes = Some("secret".to_string());
        app.store_file.store.tasks.push(t);
        app.store_file.store.tasks.push(sample_task(2, "Nope"));
        app.search = "secret".to_string();
        assert_eq!(app.visible_tasks().len(), 1);
        app.search = "findme".to_string();
        assert_eq!(app.visible_tasks().len(), 1);
    }

    #[test]
    fn visible_tasks_sort_due() {
        let mut app = app_with_tasks(vec![]);
        let mut late = sample_task(1, "Late");
        late.due = Some(NaiveDate::from_ymd_opt(2024, 12, 1).unwrap());
        let mut early = sample_task(2, "Early");
        early.due = Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
        let mut none = sample_task(3, "None");
        app.store_file.store.tasks = vec![late, early, none];
        app.sort = SortMode::Due;
        let order: Vec<_> = app
            .visible_tasks()
            .into_iter()
            .map(|t| t.title.as_str())
            .collect();
        assert_eq!(order, vec!["Early", "Late", "None"]);
    }

    #[test]
    fn visible_tasks_sort_priority() {
        let mut app = app_with_tasks(vec![]);
        let mut low = sample_task(1, "Low");
        low.priority = Some(Priority::Low);
        let mut high = sample_task(2, "High");
        high.priority = Some(Priority::High);
        let none = sample_task(3, "None");
        app.store_file.store.tasks = vec![low, high, none];
        app.sort = SortMode::Priority;
        let order: Vec<_> = app
            .visible_tasks()
            .into_iter()
            .map(|t| t.title.as_str())
            .collect();
        assert_eq!(order, vec!["High", "Low", "None"]);
    }

    #[test]
    fn visible_tasks_sort_created() {
        let mut app = app_with_tasks(vec![]);
        let old = sample_task(1, "Old");
        let young = sample_task(2, "Young");
        app.store_file.store.tasks = vec![young, old];
        app.sort = SortMode::Created;
        let order: Vec<_> = app
            .visible_tasks()
            .into_iter()
            .map(|t| t.title.as_str())
            .collect();
        assert_eq!(order, vec!["Old", "Young"]);
    }

    #[test]
    fn visible_tasks_sort_title() {
        let mut app = app_with_tasks(vec![]);
        let b = sample_task(1, "beta");
        let a = sample_task(2, "Alpha");
        app.store_file.store.tasks = vec![b, a];
        app.sort = SortMode::Title;
        let order: Vec<_> = app
            .visible_tasks()
            .into_iter()
            .map(|t| t.title.as_str())
            .collect();
        assert_eq!(order, vec!["Alpha", "beta"]);
    }

    #[test]
    fn visible_tasks_completed_always_bottom() {
        let mut app = app_with_tasks(vec![]);
        let mut done = sample_task(1, "Done");
        done.completed = Some(Utc::now());
        done.due = Some(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap());
        let mut open = sample_task(2, "Open");
        open.due = Some(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());
        app.store_file.store.tasks = vec![done, open];
        app.sort = SortMode::Due;
        let order: Vec<_> = app
            .visible_tasks()
            .into_iter()
            .map(|t| t.title.as_str())
            .collect();
        assert_eq!(order, vec!["Open", "Done"]);
    }

    #[test]
    fn status_message_live_boundary() {
        assert!(status_message_live(0));
        assert!(status_message_live(STATUS_MESSAGE_MS));
        assert!(!status_message_live(STATUS_MESSAGE_MS + 1));
    }

    #[test]
    fn set_status_and_clear() {
        let mut app = app_with_tasks(vec![]);
        assert!(app.status_message().is_none());
        app.set_status("hello".to_string());
        assert_eq!(app.status_message(), Some("hello"));
        app.clear_status();
        assert!(app.status_message().is_none());
    }
}
