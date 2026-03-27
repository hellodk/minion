//! Workout planning and tracking

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, Result};

/// Type of exercise
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExerciseType {
    Strength,
    Cardio,
    Flexibility,
    Balance,
    Sport,
}

/// Targeted muscle group
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MuscleGroup {
    Chest,
    Back,
    Shoulders,
    Arms,
    Core,
    Legs,
    FullBody,
}

/// A single exercise within a workout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exercise {
    pub name: String,
    pub exercise_type: ExerciseType,
    pub muscle_group: MuscleGroup,
    pub sets: Option<u32>,
    pub reps: Option<u32>,
    pub weight_kg: Option<f64>,
    pub duration_minutes: Option<f64>,
    pub calories_burned: Option<f64>,
}

/// A complete workout session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workout {
    pub id: String,
    pub name: String,
    pub exercises: Vec<Exercise>,
    pub date: DateTime<Utc>,
    pub duration_minutes: f64,
    pub notes: Option<String>,
}

/// Tracks a collection of workouts
pub struct WorkoutLog {
    workouts: Vec<Workout>,
}

impl WorkoutLog {
    /// Create a new empty workout log
    pub fn new() -> Self {
        Self {
            workouts: Vec::new(),
        }
    }

    /// Log a new workout and return the created record
    pub fn log_workout(
        &mut self,
        name: &str,
        exercises: Vec<Exercise>,
        duration_minutes: f64,
        notes: Option<String>,
    ) -> Result<Workout> {
        if name.is_empty() {
            return Err(Error::Workout("Workout name cannot be empty".to_string()));
        }
        if duration_minutes < 0.0 {
            return Err(Error::Workout("Duration cannot be negative".to_string()));
        }

        let workout = Workout {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            exercises,
            date: Utc::now(),
            duration_minutes,
            notes,
        };

        self.workouts.push(workout.clone());
        Ok(workout)
    }

    /// Get a workout by its id
    pub fn get(&self, id: &str) -> Option<&Workout> {
        self.workouts.iter().find(|w| w.id == id)
    }

    /// List all workouts
    pub fn list(&self) -> &[Workout] {
        &self.workouts
    }

    /// List workouts within a date range (inclusive)
    pub fn list_by_date_range(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<&Workout> {
        self.workouts
            .iter()
            .filter(|w| w.date >= from && w.date <= to)
            .collect()
    }

    /// Total number of logged workouts
    pub fn total_workouts(&self) -> usize {
        self.workouts.len()
    }

    /// Total duration across all workouts in minutes
    pub fn total_duration(&self) -> f64 {
        self.workouts.iter().map(|w| w.duration_minutes).sum()
    }

    /// Total estimated calories burned across all workouts
    pub fn total_calories(&self) -> f64 {
        self.workouts
            .iter()
            .flat_map(|w| &w.exercises)
            .filter_map(|e| e.calories_burned)
            .sum()
    }
}

impl Default for WorkoutLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn sample_exercise(name: &str, calories: Option<f64>) -> Exercise {
        Exercise {
            name: name.to_string(),
            exercise_type: ExerciseType::Strength,
            muscle_group: MuscleGroup::Chest,
            sets: Some(3),
            reps: Some(10),
            weight_kg: Some(60.0),
            duration_minutes: None,
            calories_burned: calories,
        }
    }

    fn cardio_exercise(name: &str, duration: f64, calories: f64) -> Exercise {
        Exercise {
            name: name.to_string(),
            exercise_type: ExerciseType::Cardio,
            muscle_group: MuscleGroup::FullBody,
            sets: None,
            reps: None,
            weight_kg: None,
            duration_minutes: Some(duration),
            calories_burned: Some(calories),
        }
    }

    #[test]
    fn test_new_workout_log() {
        let log = WorkoutLog::new();
        assert_eq!(log.total_workouts(), 0);
        assert!(log.list().is_empty());
    }

    #[test]
    fn test_default_workout_log() {
        let log = WorkoutLog::default();
        assert_eq!(log.total_workouts(), 0);
    }

    #[test]
    fn test_log_workout() {
        let mut log = WorkoutLog::new();
        let exercises = vec![sample_exercise("Bench Press", Some(150.0))];

        let workout = log
            .log_workout(
                "Push Day",
                exercises,
                45.0,
                Some("Great session".to_string()),
            )
            .unwrap();

        assert_eq!(workout.name, "Push Day");
        assert_eq!(workout.duration_minutes, 45.0);
        assert_eq!(workout.notes, Some("Great session".to_string()));
        assert_eq!(workout.exercises.len(), 1);
        assert!(!workout.id.is_empty());
        assert_eq!(log.total_workouts(), 1);
    }

    #[test]
    fn test_log_workout_empty_name() {
        let mut log = WorkoutLog::new();
        let result = log.log_workout("", vec![], 30.0, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_log_workout_negative_duration() {
        let mut log = WorkoutLog::new();
        let result = log.log_workout("Test", vec![], -10.0, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_log_workout_zero_duration() {
        let mut log = WorkoutLog::new();
        let result = log.log_workout("Rest Day Check-in", vec![], 0.0, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_log_workout_no_exercises() {
        let mut log = WorkoutLog::new();
        let workout = log.log_workout("Stretching", vec![], 15.0, None).unwrap();
        assert!(workout.exercises.is_empty());
    }

    #[test]
    fn test_get_workout() {
        let mut log = WorkoutLog::new();
        let exercises = vec![sample_exercise("Squat", Some(200.0))];
        let workout = log.log_workout("Leg Day", exercises, 60.0, None).unwrap();
        let id = workout.id.clone();

        let found = log.get(&id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Leg Day");
    }

    #[test]
    fn test_get_workout_not_found() {
        let log = WorkoutLog::new();
        assert!(log.get("nonexistent-id").is_none());
    }

    #[test]
    fn test_list_workouts() {
        let mut log = WorkoutLog::new();
        log.log_workout("Day 1", vec![], 30.0, None).unwrap();
        log.log_workout("Day 2", vec![], 45.0, None).unwrap();
        log.log_workout("Day 3", vec![], 60.0, None).unwrap();

        let workouts = log.list();
        assert_eq!(workouts.len(), 3);
        assert_eq!(workouts[0].name, "Day 1");
        assert_eq!(workouts[2].name, "Day 3");
    }

    #[test]
    fn test_list_by_date_range() {
        let mut log = WorkoutLog::new();

        // Log some workouts — they all get Utc::now() as date
        log.log_workout("Today 1", vec![], 30.0, None).unwrap();
        log.log_workout("Today 2", vec![], 45.0, None).unwrap();

        let from = Utc::now() - Duration::hours(1);
        let to = Utc::now() + Duration::hours(1);
        let in_range = log.list_by_date_range(from, to);
        assert_eq!(in_range.len(), 2);

        // Far-past range should return nothing
        let past_from = Utc::now() - Duration::days(30);
        let past_to = Utc::now() - Duration::days(29);
        let out_of_range = log.list_by_date_range(past_from, past_to);
        assert!(out_of_range.is_empty());
    }

    #[test]
    fn test_total_workouts() {
        let mut log = WorkoutLog::new();
        assert_eq!(log.total_workouts(), 0);

        log.log_workout("W1", vec![], 10.0, None).unwrap();
        assert_eq!(log.total_workouts(), 1);

        log.log_workout("W2", vec![], 20.0, None).unwrap();
        assert_eq!(log.total_workouts(), 2);
    }

    #[test]
    fn test_total_duration() {
        let mut log = WorkoutLog::new();
        assert_eq!(log.total_duration(), 0.0);

        log.log_workout("W1", vec![], 30.0, None).unwrap();
        log.log_workout("W2", vec![], 45.0, None).unwrap();
        log.log_workout("W3", vec![], 60.0, None).unwrap();

        assert!((log.total_duration() - 135.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_total_calories() {
        let mut log = WorkoutLog::new();
        assert_eq!(log.total_calories(), 0.0);

        let exercises1 = vec![
            sample_exercise("Bench Press", Some(150.0)),
            sample_exercise("Incline Press", Some(120.0)),
        ];
        let exercises2 = vec![cardio_exercise("Running", 30.0, 300.0)];

        log.log_workout("Push", exercises1, 45.0, None).unwrap();
        log.log_workout("Cardio", exercises2, 30.0, None).unwrap();

        assert!((log.total_calories() - 570.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_total_calories_with_none_entries() {
        let mut log = WorkoutLog::new();

        let exercises = vec![
            sample_exercise("Bench Press", Some(150.0)),
            sample_exercise("Fly", None), // no calories
        ];

        log.log_workout("Push", exercises, 45.0, None).unwrap();
        assert!((log.total_calories() - 150.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_exercise_types() {
        let types = vec![
            ExerciseType::Strength,
            ExerciseType::Cardio,
            ExerciseType::Flexibility,
            ExerciseType::Balance,
            ExerciseType::Sport,
        ];
        for t in &types {
            let json = serde_json::to_string(t).unwrap();
            let deserialized: ExerciseType = serde_json::from_str(&json).unwrap();
            assert_eq!(*t, deserialized);
        }
    }

    #[test]
    fn test_muscle_groups() {
        let groups = vec![
            MuscleGroup::Chest,
            MuscleGroup::Back,
            MuscleGroup::Shoulders,
            MuscleGroup::Arms,
            MuscleGroup::Core,
            MuscleGroup::Legs,
            MuscleGroup::FullBody,
        ];
        for g in &groups {
            let json = serde_json::to_string(g).unwrap();
            let deserialized: MuscleGroup = serde_json::from_str(&json).unwrap();
            assert_eq!(*g, deserialized);
        }
    }

    #[test]
    fn test_workout_serialization() {
        let workout = Workout {
            id: "test-id".to_string(),
            name: "Test Workout".to_string(),
            exercises: vec![sample_exercise("Bench", Some(100.0))],
            date: Utc::now(),
            duration_minutes: 45.0,
            notes: Some("Good workout".to_string()),
        };

        let json = serde_json::to_string(&workout).unwrap();
        let deserialized: Workout = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "test-id");
        assert_eq!(deserialized.name, "Test Workout");
        assert_eq!(deserialized.exercises.len(), 1);
        assert_eq!(deserialized.duration_minutes, 45.0);
    }

    #[test]
    fn test_unique_workout_ids() {
        let mut log = WorkoutLog::new();
        let w1 = log.log_workout("W1", vec![], 10.0, None).unwrap();
        let w2 = log.log_workout("W2", vec![], 20.0, None).unwrap();
        assert_ne!(w1.id, w2.id);
    }

    #[test]
    fn test_multiple_exercises_per_workout() {
        let mut log = WorkoutLog::new();
        let exercises = vec![
            sample_exercise("Bench Press", Some(150.0)),
            cardio_exercise("Treadmill", 10.0, 100.0),
            Exercise {
                name: "Yoga Stretch".to_string(),
                exercise_type: ExerciseType::Flexibility,
                muscle_group: MuscleGroup::FullBody,
                sets: None,
                reps: None,
                weight_kg: None,
                duration_minutes: Some(15.0),
                calories_burned: Some(50.0),
            },
        ];

        let workout = log
            .log_workout("Mixed Session", exercises, 90.0, None)
            .unwrap();
        assert_eq!(workout.exercises.len(), 3);
        assert_eq!(workout.exercises[0].exercise_type, ExerciseType::Strength);
        assert_eq!(workout.exercises[1].exercise_type, ExerciseType::Cardio);
        assert_eq!(
            workout.exercises[2].exercise_type,
            ExerciseType::Flexibility
        );
    }
}
