//! Nutrition logging and tracking

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, Result};

/// Type of meal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MealType {
    Breakfast,
    Lunch,
    Dinner,
    Snack,
}

/// A single food entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoodEntry {
    pub id: String,
    pub name: String,
    pub calories: f64,
    pub protein_g: f64,
    pub carbs_g: f64,
    pub fat_g: f64,
    pub date: DateTime<Utc>,
    pub meal: MealType,
}

/// Aggregated nutrition data for a single day
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyNutrition {
    pub total_calories: f64,
    pub total_protein: f64,
    pub total_carbs: f64,
    pub total_fat: f64,
    pub entries: usize,
}

/// Tracks food entries
pub struct NutritionLog {
    entries: Vec<FoodEntry>,
}

impl NutritionLog {
    /// Create a new empty nutrition log
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Log a food entry and return the created record
    pub fn log_food(
        &mut self,
        name: &str,
        calories: f64,
        protein: f64,
        carbs: f64,
        fat: f64,
        meal: MealType,
    ) -> Result<FoodEntry> {
        if name.is_empty() {
            return Err(Error::Workout("Food name cannot be empty".to_string()));
        }
        if calories < 0.0 {
            return Err(Error::Workout("Calories cannot be negative".to_string()));
        }

        let entry = FoodEntry {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            calories,
            protein_g: protein,
            carbs_g: carbs,
            fat_g: fat,
            date: Utc::now(),
            meal,
        };

        self.entries.push(entry.clone());
        Ok(entry)
    }

    /// Get a summary of nutrition for a specific date
    pub fn daily_summary(&self, date: NaiveDate) -> DailyNutrition {
        let day_entries: Vec<&FoodEntry> = self
            .entries
            .iter()
            .filter(|e| e.date.date_naive() == date)
            .collect();

        DailyNutrition {
            total_calories: day_entries.iter().map(|e| e.calories).sum(),
            total_protein: day_entries.iter().map(|e| e.protein_g).sum(),
            total_carbs: day_entries.iter().map(|e| e.carbs_g).sum(),
            total_fat: day_entries.iter().map(|e| e.fat_g).sum(),
            entries: day_entries.len(),
        }
    }

    /// List all food entries
    pub fn list(&self) -> &[FoodEntry] {
        &self.entries
    }

    /// List food entries filtered by meal type
    pub fn list_by_meal(&self, meal: MealType) -> Vec<&FoodEntry> {
        self.entries.iter().filter(|e| e.meal == meal).collect()
    }
}

impl Default for NutritionLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_nutrition_log() {
        let log = NutritionLog::new();
        assert!(log.list().is_empty());
    }

    #[test]
    fn test_default_nutrition_log() {
        let log = NutritionLog::default();
        assert!(log.list().is_empty());
    }

    #[test]
    fn test_log_food() {
        let mut log = NutritionLog::new();
        let entry = log
            .log_food("Chicken Breast", 165.0, 31.0, 0.0, 3.6, MealType::Lunch)
            .unwrap();

        assert_eq!(entry.name, "Chicken Breast");
        assert_eq!(entry.calories, 165.0);
        assert_eq!(entry.protein_g, 31.0);
        assert_eq!(entry.carbs_g, 0.0);
        assert_eq!(entry.fat_g, 3.6);
        assert_eq!(entry.meal, MealType::Lunch);
        assert!(!entry.id.is_empty());
        assert_eq!(log.list().len(), 1);
    }

    #[test]
    fn test_log_food_empty_name() {
        let mut log = NutritionLog::new();
        let result = log.log_food("", 100.0, 10.0, 20.0, 5.0, MealType::Snack);
        assert!(result.is_err());
    }

    #[test]
    fn test_log_food_negative_calories() {
        let mut log = NutritionLog::new();
        let result = log.log_food("Bad Entry", -50.0, 10.0, 20.0, 5.0, MealType::Dinner);
        assert!(result.is_err());
    }

    #[test]
    fn test_log_food_zero_calories() {
        let mut log = NutritionLog::new();
        let result = log.log_food("Water", 0.0, 0.0, 0.0, 0.0, MealType::Snack);
        assert!(result.is_ok());
    }

    #[test]
    fn test_daily_summary() {
        let mut log = NutritionLog::new();

        log.log_food("Oatmeal", 300.0, 10.0, 50.0, 5.0, MealType::Breakfast)
            .unwrap();
        log.log_food("Chicken", 250.0, 30.0, 5.0, 10.0, MealType::Lunch)
            .unwrap();
        log.log_food("Salad", 150.0, 5.0, 20.0, 7.0, MealType::Dinner)
            .unwrap();

        let today = Utc::now().date_naive();
        let summary = log.daily_summary(today);

        assert_eq!(summary.entries, 3);
        assert!((summary.total_calories - 700.0).abs() < f64::EPSILON);
        assert!((summary.total_protein - 45.0).abs() < f64::EPSILON);
        assert!((summary.total_carbs - 75.0).abs() < f64::EPSILON);
        assert!((summary.total_fat - 22.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_daily_summary_empty_day() {
        let log = NutritionLog::new();
        let today = Utc::now().date_naive();
        let summary = log.daily_summary(today);

        assert_eq!(summary.entries, 0);
        assert_eq!(summary.total_calories, 0.0);
        assert_eq!(summary.total_protein, 0.0);
        assert_eq!(summary.total_carbs, 0.0);
        assert_eq!(summary.total_fat, 0.0);
    }

    #[test]
    fn test_daily_summary_different_day() {
        let mut log = NutritionLog::new();

        // Log food today
        log.log_food("Toast", 200.0, 5.0, 30.0, 3.0, MealType::Breakfast)
            .unwrap();

        // Query a date far in the past — should be empty
        let past = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        let summary = log.daily_summary(past);
        assert_eq!(summary.entries, 0);
        assert_eq!(summary.total_calories, 0.0);
    }

    #[test]
    fn test_list_by_meal() {
        let mut log = NutritionLog::new();

        log.log_food("Eggs", 200.0, 14.0, 1.0, 15.0, MealType::Breakfast)
            .unwrap();
        log.log_food("Toast", 150.0, 4.0, 25.0, 3.0, MealType::Breakfast)
            .unwrap();
        log.log_food("Sandwich", 400.0, 20.0, 40.0, 15.0, MealType::Lunch)
            .unwrap();
        log.log_food("Apple", 95.0, 0.5, 25.0, 0.3, MealType::Snack)
            .unwrap();

        let breakfast = log.list_by_meal(MealType::Breakfast);
        assert_eq!(breakfast.len(), 2);
        assert_eq!(breakfast[0].name, "Eggs");
        assert_eq!(breakfast[1].name, "Toast");

        let lunch = log.list_by_meal(MealType::Lunch);
        assert_eq!(lunch.len(), 1);
        assert_eq!(lunch[0].name, "Sandwich");

        let snack = log.list_by_meal(MealType::Snack);
        assert_eq!(snack.len(), 1);

        let dinner = log.list_by_meal(MealType::Dinner);
        assert!(dinner.is_empty());
    }

    #[test]
    fn test_meal_type_serialization() {
        let meals = vec![
            MealType::Breakfast,
            MealType::Lunch,
            MealType::Dinner,
            MealType::Snack,
        ];
        for m in &meals {
            let json = serde_json::to_string(m).unwrap();
            let deserialized: MealType = serde_json::from_str(&json).unwrap();
            assert_eq!(*m, deserialized);
        }
    }

    #[test]
    fn test_food_entry_serialization() {
        let entry = FoodEntry {
            id: "test-id".to_string(),
            name: "Rice".to_string(),
            calories: 200.0,
            protein_g: 4.0,
            carbs_g: 45.0,
            fat_g: 0.5,
            date: Utc::now(),
            meal: MealType::Dinner,
        };

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: FoodEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "test-id");
        assert_eq!(deserialized.name, "Rice");
        assert_eq!(deserialized.calories, 200.0);
        assert_eq!(deserialized.meal, MealType::Dinner);
    }

    #[test]
    fn test_daily_nutrition_serialization() {
        let summary = DailyNutrition {
            total_calories: 2000.0,
            total_protein: 150.0,
            total_carbs: 250.0,
            total_fat: 60.0,
            entries: 5,
        };

        let json = serde_json::to_string(&summary).unwrap();
        let deserialized: DailyNutrition = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.total_calories, 2000.0);
        assert_eq!(deserialized.entries, 5);
    }

    #[test]
    fn test_unique_food_entry_ids() {
        let mut log = NutritionLog::new();
        let e1 = log
            .log_food("Food1", 100.0, 10.0, 10.0, 5.0, MealType::Snack)
            .unwrap();
        let e2 = log
            .log_food("Food2", 200.0, 20.0, 20.0, 10.0, MealType::Snack)
            .unwrap();
        assert_ne!(e1.id, e2.id);
    }

    #[test]
    fn test_multiple_entries_same_meal() {
        let mut log = NutritionLog::new();

        log.log_food("Coffee", 5.0, 0.3, 0.0, 0.0, MealType::Breakfast)
            .unwrap();
        log.log_food("Eggs", 200.0, 14.0, 1.0, 15.0, MealType::Breakfast)
            .unwrap();
        log.log_food("Bacon", 120.0, 9.0, 0.0, 9.0, MealType::Breakfast)
            .unwrap();

        let breakfast = log.list_by_meal(MealType::Breakfast);
        assert_eq!(breakfast.len(), 3);

        let today = Utc::now().date_naive();
        let summary = log.daily_summary(today);
        assert_eq!(summary.entries, 3);
        assert!((summary.total_calories - 325.0).abs() < f64::EPSILON);
    }
}
