//! Activity and progress tracking

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, Result};

/// A single body-measurement record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyMetric {
    pub id: String,
    pub date: DateTime<Utc>,
    pub weight_kg: Option<f64>,
    pub body_fat_percent: Option<f64>,
    pub notes: Option<String>,
}

/// Aggregated progress information
#[derive(Debug, Clone)]
pub struct ProgressSummary {
    pub entries: usize,
    pub latest_weight: Option<f64>,
    pub weight_change: Option<f64>,
    pub avg_weight: Option<f64>,
}

/// Tracks body metrics over time
pub struct ProgressTracker {
    metrics: Vec<BodyMetric>,
}

impl ProgressTracker {
    /// Create a new empty progress tracker
    pub fn new() -> Self {
        Self {
            metrics: Vec::new(),
        }
    }

    /// Record a new body metric and return the created record
    pub fn record(
        &mut self,
        weight_kg: Option<f64>,
        body_fat_percent: Option<f64>,
        notes: Option<String>,
    ) -> Result<BodyMetric> {
        if weight_kg.is_none() && body_fat_percent.is_none() && notes.is_none() {
            return Err(Error::Workout(
                "At least one metric must be provided".to_string(),
            ));
        }
        if let Some(w) = weight_kg {
            if w <= 0.0 {
                return Err(Error::Workout("Weight must be positive".to_string()));
            }
        }
        if let Some(bf) = body_fat_percent {
            if !(0.0..=100.0).contains(&bf) {
                return Err(Error::Workout(
                    "Body fat percentage must be between 0 and 100".to_string(),
                ));
            }
        }

        let metric = BodyMetric {
            id: Uuid::new_v4().to_string(),
            date: Utc::now(),
            weight_kg,
            body_fat_percent,
            notes,
        };

        self.metrics.push(metric.clone());
        Ok(metric)
    }

    /// List all recorded metrics
    pub fn list(&self) -> &[BodyMetric] {
        &self.metrics
    }

    /// Get the most recently recorded metric
    pub fn latest(&self) -> Option<&BodyMetric> {
        self.metrics.last()
    }

    /// Compute a summary of all recorded progress
    pub fn summary(&self) -> ProgressSummary {
        if self.metrics.is_empty() {
            return ProgressSummary {
                entries: 0,
                latest_weight: None,
                weight_change: None,
                avg_weight: None,
            };
        }

        let weights: Vec<f64> = self.metrics.iter().filter_map(|m| m.weight_kg).collect();

        let latest_weight = self.metrics.iter().rev().find_map(|m| m.weight_kg);

        let weight_change = if weights.len() >= 2 {
            Some(weights[weights.len() - 1] - weights[0])
        } else {
            None
        };

        let avg_weight = if weights.is_empty() {
            None
        } else {
            Some(weights.iter().sum::<f64>() / weights.len() as f64)
        };

        ProgressSummary {
            entries: self.metrics.len(),
            latest_weight,
            weight_change,
            avg_weight,
        }
    }
}

impl Default for ProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_progress_tracker() {
        let tracker = ProgressTracker::new();
        assert!(tracker.list().is_empty());
        assert!(tracker.latest().is_none());
    }

    #[test]
    fn test_default_progress_tracker() {
        let tracker = ProgressTracker::default();
        assert!(tracker.list().is_empty());
    }

    #[test]
    fn test_record_weight() {
        let mut tracker = ProgressTracker::new();
        let metric = tracker.record(Some(80.0), None, None).unwrap();

        assert_eq!(metric.weight_kg, Some(80.0));
        assert!(metric.body_fat_percent.is_none());
        assert!(metric.notes.is_none());
        assert!(!metric.id.is_empty());
        assert_eq!(tracker.list().len(), 1);
    }

    #[test]
    fn test_record_body_fat() {
        let mut tracker = ProgressTracker::new();
        let metric = tracker.record(None, Some(15.0), None).unwrap();
        assert_eq!(metric.body_fat_percent, Some(15.0));
    }

    #[test]
    fn test_record_notes_only() {
        let mut tracker = ProgressTracker::new();
        let metric = tracker
            .record(None, None, Some("Feeling good".to_string()))
            .unwrap();
        assert_eq!(metric.notes, Some("Feeling good".to_string()));
    }

    #[test]
    fn test_record_all_fields() {
        let mut tracker = ProgressTracker::new();
        let metric = tracker
            .record(Some(75.5), Some(18.0), Some("Morning weigh-in".to_string()))
            .unwrap();

        assert_eq!(metric.weight_kg, Some(75.5));
        assert_eq!(metric.body_fat_percent, Some(18.0));
        assert_eq!(metric.notes, Some("Morning weigh-in".to_string()));
    }

    #[test]
    fn test_record_no_metrics() {
        let mut tracker = ProgressTracker::new();
        let result = tracker.record(None, None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_record_zero_weight() {
        let mut tracker = ProgressTracker::new();
        let result = tracker.record(Some(0.0), None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_record_negative_weight() {
        let mut tracker = ProgressTracker::new();
        let result = tracker.record(Some(-5.0), None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_record_body_fat_out_of_range() {
        let mut tracker = ProgressTracker::new();

        let result = tracker.record(None, Some(-1.0), None);
        assert!(result.is_err());

        let result = tracker.record(None, Some(101.0), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_record_body_fat_boundary_values() {
        let mut tracker = ProgressTracker::new();

        let metric = tracker.record(None, Some(0.0), None).unwrap();
        assert_eq!(metric.body_fat_percent, Some(0.0));

        let metric = tracker.record(None, Some(100.0), None).unwrap();
        assert_eq!(metric.body_fat_percent, Some(100.0));
    }

    #[test]
    fn test_list_metrics() {
        let mut tracker = ProgressTracker::new();
        tracker.record(Some(80.0), None, None).unwrap();
        tracker.record(Some(79.5), None, None).unwrap();
        tracker.record(Some(79.0), None, None).unwrap();

        let metrics = tracker.list();
        assert_eq!(metrics.len(), 3);
        assert_eq!(metrics[0].weight_kg, Some(80.0));
        assert_eq!(metrics[2].weight_kg, Some(79.0));
    }

    #[test]
    fn test_latest_metric() {
        let mut tracker = ProgressTracker::new();
        tracker.record(Some(80.0), None, None).unwrap();
        tracker.record(Some(79.5), None, None).unwrap();
        tracker.record(Some(79.0), None, None).unwrap();

        let latest = tracker.latest().unwrap();
        assert_eq!(latest.weight_kg, Some(79.0));
    }

    #[test]
    fn test_latest_metric_empty() {
        let tracker = ProgressTracker::new();
        assert!(tracker.latest().is_none());
    }

    #[test]
    fn test_summary_empty() {
        let tracker = ProgressTracker::new();
        let summary = tracker.summary();

        assert_eq!(summary.entries, 0);
        assert!(summary.latest_weight.is_none());
        assert!(summary.weight_change.is_none());
        assert!(summary.avg_weight.is_none());
    }

    #[test]
    fn test_summary_single_entry() {
        let mut tracker = ProgressTracker::new();
        tracker.record(Some(80.0), None, None).unwrap();

        let summary = tracker.summary();
        assert_eq!(summary.entries, 1);
        assert_eq!(summary.latest_weight, Some(80.0));
        assert!(summary.weight_change.is_none()); // need at least 2 for change
        assert!((summary.avg_weight.unwrap() - 80.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_summary_multiple_entries() {
        let mut tracker = ProgressTracker::new();
        tracker.record(Some(80.0), None, None).unwrap();
        tracker.record(Some(79.0), None, None).unwrap();
        tracker.record(Some(78.0), None, None).unwrap();

        let summary = tracker.summary();
        assert_eq!(summary.entries, 3);
        assert_eq!(summary.latest_weight, Some(78.0));
        // weight_change = last - first = 78.0 - 80.0 = -2.0
        assert!((summary.weight_change.unwrap() - (-2.0)).abs() < f64::EPSILON);
        // avg = (80 + 79 + 78) / 3 = 79.0
        assert!((summary.avg_weight.unwrap() - 79.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_summary_weight_gain() {
        let mut tracker = ProgressTracker::new();
        tracker.record(Some(70.0), None, None).unwrap();
        tracker.record(Some(72.0), None, None).unwrap();
        tracker.record(Some(75.0), None, None).unwrap();

        let summary = tracker.summary();
        assert!((summary.weight_change.unwrap() - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_summary_mixed_entries() {
        let mut tracker = ProgressTracker::new();
        // Some entries have weight, some don't
        tracker.record(Some(80.0), Some(20.0), None).unwrap();
        tracker
            .record(None, None, Some("Rest day".to_string()))
            .unwrap();
        tracker.record(Some(79.0), Some(19.5), None).unwrap();

        let summary = tracker.summary();
        assert_eq!(summary.entries, 3);
        // Latest weight comes from the last metric that has one
        assert_eq!(summary.latest_weight, Some(79.0));
        // Weight change is between first weight (80) and last weight (79)
        assert!((summary.weight_change.unwrap() - (-1.0)).abs() < f64::EPSILON);
        // Average weight is over entries that have weight: (80 + 79) / 2
        assert!((summary.avg_weight.unwrap() - 79.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_summary_no_weights() {
        let mut tracker = ProgressTracker::new();
        tracker.record(None, Some(18.0), None).unwrap();
        tracker
            .record(None, None, Some("Just a note".to_string()))
            .unwrap();

        let summary = tracker.summary();
        assert_eq!(summary.entries, 2);
        assert!(summary.latest_weight.is_none());
        assert!(summary.weight_change.is_none());
        assert!(summary.avg_weight.is_none());
    }

    #[test]
    fn test_body_metric_serialization() {
        let metric = BodyMetric {
            id: "test-id".to_string(),
            date: Utc::now(),
            weight_kg: Some(75.0),
            body_fat_percent: Some(15.0),
            notes: Some("Morning".to_string()),
        };

        let json = serde_json::to_string(&metric).unwrap();
        let deserialized: BodyMetric = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "test-id");
        assert_eq!(deserialized.weight_kg, Some(75.0));
        assert_eq!(deserialized.body_fat_percent, Some(15.0));
    }

    #[test]
    fn test_unique_metric_ids() {
        let mut tracker = ProgressTracker::new();
        let m1 = tracker.record(Some(80.0), None, None).unwrap();
        let m2 = tracker.record(Some(79.0), None, None).unwrap();
        assert_ne!(m1.id, m2.id);
    }
}
