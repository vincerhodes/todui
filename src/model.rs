use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Store {
    pub version: u32,
    pub next_id: u64,
    pub tasks: Vec<Task>,
}

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
pub enum Priority {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Recurrence {
    Daily,
    Weekly,
    Monthly,
    EveryNDays(u32),
}

impl Store {
    pub fn new() -> Self {
        Self {
            version: 1,
            next_id: 1,
            tasks: Vec::new(),
        }
    }
}

impl Recurrence {
    pub fn next_due(&self, anchor: NaiveDate) -> NaiveDate {
        match self {
            Recurrence::Daily => anchor + chrono::Duration::days(1),
            Recurrence::Weekly => anchor + chrono::Duration::days(7),
            Recurrence::EveryNDays(n) => anchor + chrono::Duration::days(i64::from(*n)),
            Recurrence::Monthly => add_one_month_clamped(anchor),
        }
    }
}

fn add_one_month_clamped(date: NaiveDate) -> NaiveDate {
    let (year, month) = if date.month() == 12 {
        (date.year() + 1, 1u32)
    } else {
        (date.year(), date.month() + 1)
    };
    let day = date.day();
    let last_day = last_day_of_month(year, month);
    let clamped = day.min(last_day);
    NaiveDate::from_ymd_opt(year, month, clamped).expect("valid month/day")
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
    if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
            .and_then(|d| d.pred_opt())
            .expect("valid date")
            .day()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
            .and_then(|d| d.pred_opt())
            .expect("valid date")
            .day()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn store_serde_round_trip() {
        let created = Utc.with_ymd_and_hms(2026, 1, 10, 12, 0, 0).unwrap();
        let completed = Utc.with_ymd_and_hms(2026, 2, 1, 8, 30, 0).unwrap();
        let store = Store {
            version: 1,
            next_id: 3,
            tasks: vec![
                Task {
                    id: 1,
                    title: "minimal".into(),
                    project: None,
                    contexts: vec![],
                    priority: None,
                    due: None,
                    notes: None,
                    next_action: false,
                    recurrence: None,
                    created,
                    completed: None,
                },
                Task {
                    id: 2,
                    title: "full".into(),
                    project: Some("inbox".into()),
                    contexts: vec!["home".into(), "errands".into()],
                    priority: Some(Priority::High),
                    due: Some(NaiveDate::from_ymd_opt(2026, 3, 15).unwrap()),
                    notes: Some("note".into()),
                    next_action: true,
                    recurrence: Some(Recurrence::EveryNDays(3)),
                    created,
                    completed: Some(completed),
                },
            ],
        };

        let json = serde_json::to_string(&store).unwrap();
        let back: Store = serde_json::from_str(&json).unwrap();
        assert_eq!(back.version, store.version);
        assert_eq!(back.next_id, store.next_id);
        assert_eq!(back.tasks.len(), 2);
        assert_eq!(back.tasks[1].priority, Some(Priority::High));
        assert_eq!(back.tasks[1].recurrence, Some(Recurrence::EveryNDays(3)));
    }

    #[test]
    fn next_due_daily() {
        let anchor = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        assert_eq!(
            Recurrence::Daily.next_due(anchor),
            NaiveDate::from_ymd_opt(2026, 1, 2).unwrap()
        );
    }

    #[test]
    fn next_due_weekly() {
        let anchor = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        assert_eq!(
            Recurrence::Weekly.next_due(anchor),
            NaiveDate::from_ymd_opt(2026, 1, 8).unwrap()
        );
    }

    #[test]
    fn next_due_every_n_days() {
        let anchor = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        assert_eq!(
            Recurrence::EveryNDays(3).next_due(anchor),
            NaiveDate::from_ymd_opt(2026, 1, 4).unwrap()
        );
    }

    #[test]
    fn next_due_monthly_normal() {
        let anchor = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        assert_eq!(
            Recurrence::Monthly.next_due(anchor),
            NaiveDate::from_ymd_opt(2026, 2, 15).unwrap()
        );
    }

    #[test]
    fn next_due_monthly_clamp() {
        let anchor = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
        assert_eq!(
            Recurrence::Monthly.next_due(anchor),
            NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
        );
    }

    #[test]
    fn next_due_monthly_year_rollover() {
        let anchor = NaiveDate::from_ymd_opt(2025, 12, 15).unwrap();
        assert_eq!(
            Recurrence::Monthly.next_due(anchor),
            NaiveDate::from_ymd_opt(2026, 1, 15).unwrap()
        );
    }
}
