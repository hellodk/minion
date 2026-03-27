//! MINION Fitness & Wellness Module
//!
//! Workout planning, habit tracking, and health metrics.

pub mod habits;
pub mod nutrition;
pub mod tracking;
pub mod workouts;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Workout error: {0}")]
    Workout(String),

    #[error("Habit error: {0}")]
    Habit(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_workout() {
        let err = Error::Workout("test workout error".to_string());
        assert!(err.to_string().contains("Workout error"));
        assert!(err.to_string().contains("test workout error"));
    }

    #[test]
    fn test_error_habit() {
        let err = Error::Habit("test habit error".to_string());
        assert!(err.to_string().contains("Habit error"));
        assert!(err.to_string().contains("test habit error"));
    }

    #[test]
    fn test_result_type() {
        let ok_result: Result<i32> = Ok(42);
        assert_eq!(ok_result.unwrap(), 42);

        let err_result: Result<i32> = Err(Error::Workout("test".to_string()));
        assert!(err_result.is_err());
    }

    #[test]
    fn test_error_debug() {
        let err = Error::Workout("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Workout"));
    }
}
