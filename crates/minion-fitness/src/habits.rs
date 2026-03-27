//! Habit tracking

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, Result};

/// How frequently a habit should be performed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HabitFrequency {
    Daily,
    Weekly,
    Monthly,
}

/// A trackable habit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Habit {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub frequency: HabitFrequency,
    pub created_at: DateTime<Utc>,
    pub completions: Vec<DateTime<Utc>>,
}

/// Manages a collection of habits
pub struct HabitTracker {
    habits: Vec<Habit>,
}

impl HabitTracker {
    /// Create a new empty habit tracker
    pub fn new() -> Self {
        Self { habits: Vec::new() }
    }

    /// Add a new habit and return the created record
    pub fn add_habit(
        &mut self,
        name: &str,
        description: Option<String>,
        frequency: HabitFrequency,
    ) -> Result<Habit> {
        if name.is_empty() {
            return Err(Error::Habit("Habit name cannot be empty".to_string()));
        }

        let habit = Habit {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            description,
            frequency,
            created_at: Utc::now(),
            completions: Vec::new(),
        };

        self.habits.push(habit.clone());
        Ok(habit)
    }

    /// Mark a habit as completed for the current moment
    pub fn complete_habit(&mut self, id: &str) -> Result<()> {
        let habit = self
            .habits
            .iter_mut()
            .find(|h| h.id == id)
            .ok_or_else(|| Error::Habit(format!("Habit not found: {}", id)))?;

        habit.completions.push(Utc::now());
        Ok(())
    }

    /// Get a habit by its id
    pub fn get(&self, id: &str) -> Option<&Habit> {
        self.habits.iter().find(|h| h.id == id)
    }

    /// List all habits
    pub fn list(&self) -> &[Habit] {
        &self.habits
    }

    /// Calculate the current streak for a habit.
    ///
    /// For daily habits, counts consecutive days with at least one completion
    /// going backward from today. For weekly habits, counts consecutive
    /// ISO weeks. For monthly habits, counts consecutive months.
    pub fn current_streak(&self, id: &str) -> usize {
        let habit = match self.habits.iter().find(|h| h.id == id) {
            Some(h) => h,
            None => return 0,
        };

        if habit.completions.is_empty() {
            return 0;
        }

        match habit.frequency {
            HabitFrequency::Daily => Self::daily_streak(&habit.completions),
            HabitFrequency::Weekly => Self::weekly_streak(&habit.completions),
            HabitFrequency::Monthly => Self::monthly_streak(&habit.completions),
        }
    }

    /// Calculate completion rate for a habit (0.0 to 1.0).
    ///
    /// Measures the ratio of periods with completions to total periods
    /// since the habit was created (up to and including the current period).
    pub fn completion_rate(&self, id: &str) -> f64 {
        let habit = match self.habits.iter().find(|h| h.id == id) {
            Some(h) => h,
            None => return 0.0,
        };

        let today = Utc::now().date_naive();
        let created_date = habit.created_at.date_naive();

        match habit.frequency {
            HabitFrequency::Daily => {
                let total_days = (today - created_date).num_days().max(0) as usize + 1;
                if total_days == 0 {
                    return 0.0;
                }
                let unique_days: std::collections::HashSet<NaiveDate> =
                    habit.completions.iter().map(|dt| dt.date_naive()).collect();
                unique_days.len().min(total_days) as f64 / total_days as f64
            }
            HabitFrequency::Weekly => {
                let created_week =
                    created_date.iso_week().week() as i64 + created_date.year() as i64 * 53;
                let current_week = today.iso_week().week() as i64 + today.year() as i64 * 53;
                let total_weeks = (current_week - created_week).max(0) as usize + 1;
                if total_weeks == 0 {
                    return 0.0;
                }
                let unique_weeks: std::collections::HashSet<(i32, u32)> = habit
                    .completions
                    .iter()
                    .map(|dt| {
                        let d = dt.date_naive();
                        (d.year(), d.iso_week().week())
                    })
                    .collect();
                unique_weeks.len().min(total_weeks) as f64 / total_weeks as f64
            }
            HabitFrequency::Monthly => {
                let created_month = created_date.year() as i64 * 12 + created_date.month0() as i64;
                let current_month = today.year() as i64 * 12 + today.month0() as i64;
                let total_months = (current_month - created_month).max(0) as usize + 1;
                if total_months == 0 {
                    return 0.0;
                }
                let unique_months: std::collections::HashSet<(i32, u32)> = habit
                    .completions
                    .iter()
                    .map(|dt| {
                        let d = dt.date_naive();
                        (d.year(), d.month())
                    })
                    .collect();
                unique_months.len().min(total_months) as f64 / total_months as f64
            }
        }
    }

    /// Count consecutive days (ending today) that have at least one completion
    fn daily_streak(completions: &[DateTime<Utc>]) -> usize {
        let today = Utc::now().date_naive();
        let unique_days: std::collections::BTreeSet<NaiveDate> =
            completions.iter().map(|dt| dt.date_naive()).collect();

        let mut streak = 0usize;
        let mut check = today;

        loop {
            if unique_days.contains(&check) {
                streak += 1;
                match check.pred_opt() {
                    Some(prev) => check = prev,
                    None => break,
                }
            } else {
                break;
            }
        }

        streak
    }

    /// Count consecutive ISO weeks (ending this week) with at least one completion
    fn weekly_streak(completions: &[DateTime<Utc>]) -> usize {
        let today = Utc::now().date_naive();
        let current_year = today.year();
        let current_week = today.iso_week().week();

        let unique_weeks: std::collections::BTreeSet<(i32, u32)> = completions
            .iter()
            .map(|dt| {
                let d = dt.date_naive();
                (d.year(), d.iso_week().week())
            })
            .collect();

        let mut streak = 0usize;
        let mut year = current_year;
        let mut week = current_week;

        loop {
            if unique_weeks.contains(&(year, week)) {
                streak += 1;
                if week == 1 {
                    year -= 1;
                    // Last ISO week of previous year
                    week = NaiveDate::from_ymd_opt(year, 12, 28)
                        .map(|d| d.iso_week().week())
                        .unwrap_or(52);
                } else {
                    week -= 1;
                }
            } else {
                break;
            }
        }

        streak
    }

    /// Count consecutive months (ending this month) with at least one completion
    fn monthly_streak(completions: &[DateTime<Utc>]) -> usize {
        let today = Utc::now().date_naive();
        let current_year = today.year();
        let current_month = today.month();

        let unique_months: std::collections::BTreeSet<(i32, u32)> = completions
            .iter()
            .map(|dt| {
                let d = dt.date_naive();
                (d.year(), d.month())
            })
            .collect();

        let mut streak = 0usize;
        let mut year = current_year;
        let mut month = current_month;

        loop {
            if unique_months.contains(&(year, month)) {
                streak += 1;
                if month == 1 {
                    year -= 1;
                    month = 12;
                } else {
                    month -= 1;
                }
            } else {
                break;
            }
        }

        streak
    }
}

impl Default for HabitTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_new_habit_tracker() {
        let tracker = HabitTracker::new();
        assert!(tracker.list().is_empty());
    }

    #[test]
    fn test_default_habit_tracker() {
        let tracker = HabitTracker::default();
        assert!(tracker.list().is_empty());
    }

    #[test]
    fn test_add_habit() {
        let mut tracker = HabitTracker::new();
        let habit = tracker
            .add_habit(
                "Exercise",
                Some("Daily workout".to_string()),
                HabitFrequency::Daily,
            )
            .unwrap();

        assert_eq!(habit.name, "Exercise");
        assert_eq!(habit.description, Some("Daily workout".to_string()));
        assert_eq!(habit.frequency, HabitFrequency::Daily);
        assert!(habit.completions.is_empty());
        assert!(!habit.id.is_empty());
        assert_eq!(tracker.list().len(), 1);
    }

    #[test]
    fn test_add_habit_empty_name() {
        let mut tracker = HabitTracker::new();
        let result = tracker.add_habit("", None, HabitFrequency::Daily);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_habit_no_description() {
        let mut tracker = HabitTracker::new();
        let habit = tracker
            .add_habit("Meditate", None, HabitFrequency::Daily)
            .unwrap();
        assert!(habit.description.is_none());
    }

    #[test]
    fn test_complete_habit() {
        let mut tracker = HabitTracker::new();
        let habit = tracker
            .add_habit("Read", None, HabitFrequency::Daily)
            .unwrap();
        let id = habit.id.clone();

        assert!(tracker.complete_habit(&id).is_ok());

        let habit = tracker.get(&id).unwrap();
        assert_eq!(habit.completions.len(), 1);
    }

    #[test]
    fn test_complete_habit_multiple_times() {
        let mut tracker = HabitTracker::new();
        let habit = tracker
            .add_habit("Stretch", None, HabitFrequency::Daily)
            .unwrap();
        let id = habit.id.clone();

        tracker.complete_habit(&id).unwrap();
        tracker.complete_habit(&id).unwrap();
        tracker.complete_habit(&id).unwrap();

        let habit = tracker.get(&id).unwrap();
        assert_eq!(habit.completions.len(), 3);
    }

    #[test]
    fn test_complete_habit_not_found() {
        let mut tracker = HabitTracker::new();
        let result = tracker.complete_habit("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_habit() {
        let mut tracker = HabitTracker::new();
        let habit = tracker
            .add_habit("Water", None, HabitFrequency::Daily)
            .unwrap();
        let id = habit.id.clone();

        let found = tracker.get(&id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Water");
    }

    #[test]
    fn test_get_habit_not_found() {
        let tracker = HabitTracker::new();
        assert!(tracker.get("nonexistent").is_none());
    }

    #[test]
    fn test_list_habits() {
        let mut tracker = HabitTracker::new();
        tracker
            .add_habit("H1", None, HabitFrequency::Daily)
            .unwrap();
        tracker
            .add_habit("H2", None, HabitFrequency::Weekly)
            .unwrap();
        tracker
            .add_habit("H3", None, HabitFrequency::Monthly)
            .unwrap();

        let habits = tracker.list();
        assert_eq!(habits.len(), 3);
        assert_eq!(habits[0].name, "H1");
        assert_eq!(habits[1].name, "H2");
        assert_eq!(habits[2].name, "H3");
    }

    #[test]
    fn test_current_streak_no_completions() {
        let mut tracker = HabitTracker::new();
        let habit = tracker
            .add_habit("Test", None, HabitFrequency::Daily)
            .unwrap();
        assert_eq!(tracker.current_streak(&habit.id), 0);
    }

    #[test]
    fn test_current_streak_nonexistent_habit() {
        let tracker = HabitTracker::new();
        assert_eq!(tracker.current_streak("nonexistent"), 0);
    }

    #[test]
    fn test_current_streak_daily_today() {
        let mut tracker = HabitTracker::new();
        let habit = tracker
            .add_habit("Test", None, HabitFrequency::Daily)
            .unwrap();
        let id = habit.id.clone();

        tracker.complete_habit(&id).unwrap();
        assert_eq!(tracker.current_streak(&id), 1);
    }

    #[test]
    fn test_current_streak_daily_consecutive() {
        let mut tracker = HabitTracker::new();
        let habit = tracker
            .add_habit("Test", None, HabitFrequency::Daily)
            .unwrap();
        let id = habit.id.clone();

        // Manually inject completions for the last 3 days including today
        let habit_mut = tracker.habits.iter_mut().find(|h| h.id == id).unwrap();
        let now = Utc::now();
        habit_mut.completions.push(now - Duration::days(2));
        habit_mut.completions.push(now - Duration::days(1));
        habit_mut.completions.push(now);

        assert_eq!(tracker.current_streak(&id), 3);
    }

    #[test]
    fn test_current_streak_daily_broken() {
        let mut tracker = HabitTracker::new();
        let habit = tracker
            .add_habit("Test", None, HabitFrequency::Daily)
            .unwrap();
        let id = habit.id.clone();

        // Complete today and 3 days ago (gap at yesterday)
        let habit_mut = tracker.habits.iter_mut().find(|h| h.id == id).unwrap();
        let now = Utc::now();
        habit_mut.completions.push(now - Duration::days(3));
        habit_mut.completions.push(now);

        // Streak should be 1 (only today)
        assert_eq!(tracker.current_streak(&id), 1);
    }

    #[test]
    fn test_current_streak_monthly() {
        let mut tracker = HabitTracker::new();
        let habit = tracker
            .add_habit("Monthly Review", None, HabitFrequency::Monthly)
            .unwrap();
        let id = habit.id.clone();

        // Complete this month
        tracker.complete_habit(&id).unwrap();
        assert_eq!(tracker.current_streak(&id), 1);
    }

    #[test]
    fn test_completion_rate_no_completions() {
        let mut tracker = HabitTracker::new();
        let habit = tracker
            .add_habit("Test", None, HabitFrequency::Daily)
            .unwrap();

        assert_eq!(tracker.completion_rate(&habit.id), 0.0);
    }

    #[test]
    fn test_completion_rate_nonexistent_habit() {
        let tracker = HabitTracker::new();
        assert_eq!(tracker.completion_rate("nonexistent"), 0.0);
    }

    #[test]
    fn test_completion_rate_daily_one_day() {
        let mut tracker = HabitTracker::new();
        let habit = tracker
            .add_habit("Test", None, HabitFrequency::Daily)
            .unwrap();
        let id = habit.id.clone();

        // Complete today — habit was also created today, so rate = 1/1 = 1.0
        tracker.complete_habit(&id).unwrap();
        assert!((tracker.completion_rate(&id) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_completion_rate_bounded() {
        let mut tracker = HabitTracker::new();
        let habit = tracker
            .add_habit("Test", None, HabitFrequency::Daily)
            .unwrap();
        let id = habit.id.clone();

        // Multiple completions on the same day should not push rate above 1.0
        tracker.complete_habit(&id).unwrap();
        tracker.complete_habit(&id).unwrap();
        tracker.complete_habit(&id).unwrap();

        let rate = tracker.completion_rate(&id);
        assert!(rate <= 1.0);
        assert!(rate > 0.0);
    }

    #[test]
    fn test_habit_frequency_serialization() {
        let frequencies = vec![
            HabitFrequency::Daily,
            HabitFrequency::Weekly,
            HabitFrequency::Monthly,
        ];
        for f in &frequencies {
            let json = serde_json::to_string(f).unwrap();
            let deserialized: HabitFrequency = serde_json::from_str(&json).unwrap();
            assert_eq!(*f, deserialized);
        }
    }

    #[test]
    fn test_habit_serialization() {
        let habit = Habit {
            id: "test-id".to_string(),
            name: "Meditate".to_string(),
            description: Some("10 minutes daily".to_string()),
            frequency: HabitFrequency::Daily,
            created_at: Utc::now(),
            completions: vec![Utc::now()],
        };

        let json = serde_json::to_string(&habit).unwrap();
        let deserialized: Habit = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "test-id");
        assert_eq!(deserialized.name, "Meditate");
        assert_eq!(deserialized.frequency, HabitFrequency::Daily);
        assert_eq!(deserialized.completions.len(), 1);
    }

    #[test]
    fn test_unique_habit_ids() {
        let mut tracker = HabitTracker::new();
        let h1 = tracker
            .add_habit("H1", None, HabitFrequency::Daily)
            .unwrap();
        let h2 = tracker
            .add_habit("H2", None, HabitFrequency::Daily)
            .unwrap();
        assert_ne!(h1.id, h2.id);
    }
}
