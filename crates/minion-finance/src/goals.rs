//! Financial goal planning

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{Error, Result};

/// A financial goal with a target amount and optional deadline.
#[derive(Debug, Clone)]
pub struct FinancialGoal {
    pub id: String,
    pub name: String,
    pub target_amount: f64,
    pub current_amount: f64,
    pub deadline: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Manages a collection of financial goals in memory.
pub struct GoalManager {
    goals: Vec<FinancialGoal>,
}

impl GoalManager {
    /// Creates a new empty goal manager.
    pub fn new() -> Self {
        Self { goals: Vec::new() }
    }

    /// Adds a new financial goal. The target amount must be positive.
    pub fn add(
        &mut self,
        name: &str,
        target_amount: f64,
        deadline: Option<DateTime<Utc>>,
    ) -> Result<FinancialGoal> {
        if name.is_empty() {
            return Err(Error::Account("Goal name cannot be empty".to_string()));
        }
        if target_amount <= 0.0 {
            return Err(Error::Account("Target amount must be positive".to_string()));
        }

        let goal = FinancialGoal {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            target_amount,
            current_amount: 0.0,
            deadline,
            created_at: Utc::now(),
        };

        self.goals.push(goal.clone());
        Ok(goal)
    }

    /// Returns a reference to the goal with the given ID.
    pub fn get(&self, id: &str) -> Option<&FinancialGoal> {
        self.goals.iter().find(|g| g.id == id)
    }

    /// Returns a slice of all goals.
    pub fn list(&self) -> &[FinancialGoal] {
        &self.goals
    }

    /// Updates the current progress amount for a goal.
    /// The amount must be non-negative.
    pub fn update_progress(&mut self, id: &str, current_amount: f64) -> Result<()> {
        if current_amount < 0.0 {
            return Err(Error::Account(
                "Progress amount cannot be negative".to_string(),
            ));
        }

        let goal = self
            .goals
            .iter_mut()
            .find(|g| g.id == id)
            .ok_or_else(|| Error::Account(format!("Goal not found: {id}")))?;

        goal.current_amount = current_amount;
        Ok(())
    }

    /// Returns the progress percentage for a goal (0.0 to 100.0+).
    /// Returns `None` if the goal is not found.
    pub fn progress_percent(&self, id: &str) -> Option<f64> {
        self.goals.iter().find(|g| g.id == id).map(|g| {
            if g.target_amount.abs() < f64::EPSILON {
                0.0
            } else {
                (g.current_amount / g.target_amount) * 100.0
            }
        })
    }

    /// Returns whether the goal has been achieved (current >= target).
    /// Returns `None` if the goal is not found.
    pub fn is_achieved(&self, id: &str) -> Option<bool> {
        self.goals
            .iter()
            .find(|g| g.id == id)
            .map(|g| g.current_amount >= g.target_amount)
    }
}

impl Default for GoalManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_goal() {
        let mut mgr = GoalManager::new();
        let goal = mgr.add("Emergency Fund", 10000.0, None).unwrap();

        assert_eq!(goal.name, "Emergency Fund");
        assert!((goal.target_amount - 10000.0).abs() < f64::EPSILON);
        assert!((goal.current_amount - 0.0).abs() < f64::EPSILON);
        assert!(goal.deadline.is_none());
    }

    #[test]
    fn test_add_goal_with_deadline() {
        let mut mgr = GoalManager::new();
        let deadline = Utc::now() + chrono::Duration::days(365);
        let goal = mgr.add("Vacation", 5000.0, Some(deadline)).unwrap();

        assert!(goal.deadline.is_some());
    }

    #[test]
    fn test_add_goal_validation() {
        let mut mgr = GoalManager::new();

        assert!(mgr.add("", 1000.0, None).is_err());
        assert!(mgr.add("Bad", 0.0, None).is_err());
        assert!(mgr.add("Bad", -100.0, None).is_err());
    }

    #[test]
    fn test_get_goal() {
        let mut mgr = GoalManager::new();
        let goal = mgr.add("House", 200000.0, None).unwrap();

        assert!(mgr.get(&goal.id).is_some());
        assert_eq!(mgr.get(&goal.id).unwrap().name, "House");
        assert!(mgr.get("nonexistent").is_none());
    }

    #[test]
    fn test_list_goals() {
        let mut mgr = GoalManager::new();
        assert!(mgr.list().is_empty());

        mgr.add("Goal A", 1000.0, None).unwrap();
        mgr.add("Goal B", 2000.0, None).unwrap();
        assert_eq!(mgr.list().len(), 2);
    }

    #[test]
    fn test_update_progress() {
        let mut mgr = GoalManager::new();
        let goal = mgr.add("Fund", 10000.0, None).unwrap();
        let id = goal.id.clone();

        mgr.update_progress(&id, 5000.0).unwrap();
        assert!((mgr.get(&id).unwrap().current_amount - 5000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_update_progress_validation() {
        let mut mgr = GoalManager::new();
        let goal = mgr.add("Fund", 10000.0, None).unwrap();

        assert!(mgr.update_progress(&goal.id, -1.0).is_err());
        assert!(mgr.update_progress("nonexistent", 100.0).is_err());
    }

    #[test]
    fn test_progress_percent() {
        let mut mgr = GoalManager::new();
        let goal = mgr.add("Fund", 10000.0, None).unwrap();
        let id = goal.id.clone();

        assert!((mgr.progress_percent(&id).unwrap() - 0.0).abs() < f64::EPSILON);

        mgr.update_progress(&id, 2500.0).unwrap();
        assert!((mgr.progress_percent(&id).unwrap() - 25.0).abs() < 0.01);

        mgr.update_progress(&id, 10000.0).unwrap();
        assert!((mgr.progress_percent(&id).unwrap() - 100.0).abs() < 0.01);

        assert!(mgr.progress_percent("nonexistent").is_none());
    }

    #[test]
    fn test_is_achieved() {
        let mut mgr = GoalManager::new();
        let goal = mgr.add("Fund", 1000.0, None).unwrap();
        let id = goal.id.clone();

        assert_eq!(mgr.is_achieved(&id), Some(false));

        mgr.update_progress(&id, 999.99).unwrap();
        assert_eq!(mgr.is_achieved(&id), Some(false));

        mgr.update_progress(&id, 1000.0).unwrap();
        assert_eq!(mgr.is_achieved(&id), Some(true));

        mgr.update_progress(&id, 1500.0).unwrap();
        assert_eq!(mgr.is_achieved(&id), Some(true));

        assert!(mgr.is_achieved("nonexistent").is_none());
    }

    #[test]
    fn test_default_impl() {
        let mgr = GoalManager::default();
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_over_100_percent_progress() {
        let mut mgr = GoalManager::new();
        let goal = mgr.add("Small Goal", 100.0, None).unwrap();
        let id = goal.id.clone();

        mgr.update_progress(&id, 150.0).unwrap();
        assert!((mgr.progress_percent(&id).unwrap() - 150.0).abs() < 0.01);
        assert_eq!(mgr.is_achieved(&id), Some(true));
    }
}
