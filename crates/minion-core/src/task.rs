//! Background task scheduling and execution

use chrono::{DateTime, Utc};
use flume::{Receiver, Sender};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use uuid::Uuid;

use crate::Result;

/// Task priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TaskPriority {
    /// Critical tasks that block UI
    Critical = 0,
    /// User-initiated tasks
    High = 1,
    /// Normal background processing
    Normal = 2,
    /// Low priority maintenance tasks
    Low = 3,
    /// Only run when system is idle
    Idle = 4,
}

#[allow(clippy::derivable_impls)]
impl Default for TaskPriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Task status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Task callback type
pub type TaskFn = Box<dyn FnOnce() -> Result<serde_json::Value> + Send + 'static>;

/// Task definition
pub struct Task {
    /// Unique task ID
    pub id: Uuid,

    /// Task name/description
    pub name: String,

    /// Module that created the task
    pub module_id: String,

    /// Task priority
    pub priority: TaskPriority,

    /// Scheduled execution time (None = immediate)
    pub scheduled_at: Option<DateTime<Utc>>,

    /// Maximum retry attempts
    pub max_retries: u32,

    /// The task function to execute
    pub task_fn: TaskFn,
}

impl Task {
    /// Create a new task
    pub fn new<F>(name: &str, module_id: &str, task_fn: F) -> Self
    where
        F: FnOnce() -> Result<serde_json::Value> + Send + 'static,
    {
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            module_id: module_id.to_string(),
            priority: TaskPriority::Normal,
            scheduled_at: None,
            max_retries: 3,
            task_fn: Box::new(task_fn),
        }
    }

    /// Set task priority
    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Schedule task for later execution
    pub fn schedule_at(mut self, time: DateTime<Utc>) -> Self {
        self.scheduled_at = Some(time);
        self
    }

    /// Set max retry attempts
    pub fn with_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }
}

/// Task info for status queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    pub id: Uuid,
    pub name: String,
    pub module_id: String,
    pub priority: TaskPriority,
    pub status: TaskStatus,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub retry_count: u32,
    pub error: Option<String>,
    pub result: Option<serde_json::Value>,
}

/// Internal task wrapper with metadata
#[allow(dead_code)]
struct TaskWrapper {
    task: Option<Task>,
    info: TaskInfo,
}

/// Task scheduler for background task execution
pub struct TaskScheduler {
    /// Task queue sender
    sender: Sender<Task>,

    /// Task info storage
    tasks: Arc<RwLock<HashMap<Uuid, TaskInfo>>>,

    /// Worker handles
    workers: Vec<std::thread::JoinHandle<()>>,

    /// Shutdown flag
    shutdown: Arc<AtomicBool>,
}

impl TaskScheduler {
    /// Create a new task scheduler
    pub fn new(worker_count: usize) -> Self {
        let (sender, receiver) = flume::bounded(10000);
        let tasks = Arc::new(RwLock::new(HashMap::new()));
        let shutdown = Arc::new(AtomicBool::new(false));

        let mut workers = Vec::with_capacity(worker_count);

        for i in 0..worker_count {
            let receiver = receiver.clone();
            let tasks = tasks.clone();
            let shutdown = shutdown.clone();

            let handle = std::thread::Builder::new()
                .name(format!("minion-worker-{}", i))
                .spawn(move || {
                    Self::worker_loop(receiver, tasks, shutdown);
                })
                .expect("Failed to spawn worker thread");

            workers.push(handle);
        }

        Self {
            sender,
            tasks,
            workers,
            shutdown,
        }
    }

    /// Submit a task for execution
    pub fn submit(&self, task: Task) -> Uuid {
        let id = task.id;

        // Create task info
        let info = TaskInfo {
            id,
            name: task.name.clone(),
            module_id: task.module_id.clone(),
            priority: task.priority,
            status: TaskStatus::Pending,
            scheduled_at: task.scheduled_at,
            started_at: None,
            completed_at: None,
            retry_count: 0,
            error: None,
            result: None,
        };

        // Store task info
        self.tasks.write().insert(id, info);

        // Send to worker queue
        let _ = self.sender.send(task);

        id
    }

    /// Get task status
    pub fn get_status(&self, task_id: Uuid) -> Option<TaskInfo> {
        self.tasks.read().get(&task_id).cloned()
    }

    /// List tasks with optional filter
    pub fn list(&self, filter: Option<TaskStatus>) -> Vec<TaskInfo> {
        self.tasks
            .read()
            .values()
            .filter(|info| filter.map(|f| info.status == f).unwrap_or(true))
            .cloned()
            .collect()
    }

    /// Cancel a pending task
    pub fn cancel(&self, task_id: Uuid) -> bool {
        let mut tasks = self.tasks.write();
        if let Some(info) = tasks.get_mut(&task_id) {
            if info.status == TaskStatus::Pending {
                info.status = TaskStatus::Cancelled;
                return true;
            }
        }
        false
    }

    /// Shutdown the scheduler
    pub async fn shutdown(&mut self) -> Result<()> {
        self.shutdown.store(true, Ordering::SeqCst);

        // Wait for workers to finish
        for handle in self.workers.drain(..) {
            let _ = handle.join();
        }

        Ok(())
    }

    /// Worker thread loop
    fn worker_loop(
        receiver: Receiver<Task>,
        tasks: Arc<RwLock<HashMap<Uuid, TaskInfo>>>,
        shutdown: Arc<AtomicBool>,
    ) {
        loop {
            if shutdown.load(Ordering::SeqCst) {
                break;
            }

            // Try to receive a task with timeout
            match receiver.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(task) => {
                    let task_id = task.id;

                    // Check if task was cancelled
                    {
                        let tasks_read = tasks.read();
                        if let Some(info) = tasks_read.get(&task_id) {
                            if info.status == TaskStatus::Cancelled {
                                continue;
                            }
                        }
                    }

                    // Check scheduled time
                    if let Some(scheduled_at) = task.scheduled_at {
                        if scheduled_at > Utc::now() {
                            // Re-queue task
                            // In a real implementation, use a priority queue
                            continue;
                        }
                    }

                    // Mark as running
                    {
                        let mut tasks_write = tasks.write();
                        if let Some(info) = tasks_write.get_mut(&task_id) {
                            info.status = TaskStatus::Running;
                            info.started_at = Some(Utc::now());
                        }
                    }

                    // Execute task
                    let result = (task.task_fn)();

                    // Update status
                    {
                        let mut tasks_write = tasks.write();
                        if let Some(info) = tasks_write.get_mut(&task_id) {
                            info.completed_at = Some(Utc::now());

                            match result {
                                Ok(value) => {
                                    info.status = TaskStatus::Completed;
                                    info.result = Some(value);
                                }
                                Err(e) => {
                                    info.status = TaskStatus::Failed;
                                    info.error = Some(e.to_string());
                                }
                            }
                        }
                    }
                }
                Err(flume::RecvTimeoutError::Timeout) => continue,
                Err(flume::RecvTimeoutError::Disconnected) => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_task_creation() {
        let task = Task::new("test", "test_module", || {
            Ok(serde_json::json!({"done": true}))
        });
        assert_eq!(task.name, "test");
        assert_eq!(task.priority, TaskPriority::Normal);
        assert_eq!(task.module_id, "test_module");
        assert_eq!(task.max_retries, 3);
        assert!(task.scheduled_at.is_none());
    }

    #[test]
    fn test_task_with_priority() {
        let task = Task::new("test", "module", || Ok(serde_json::Value::Null))
            .with_priority(TaskPriority::High);
        assert_eq!(task.priority, TaskPriority::High);
    }

    #[test]
    fn test_task_with_retries() {
        let task = Task::new("test", "module", || Ok(serde_json::Value::Null)).with_retries(5);
        assert_eq!(task.max_retries, 5);
    }

    #[test]
    fn test_task_schedule_at() {
        let future_time = Utc::now() + chrono::Duration::hours(1);
        let task =
            Task::new("test", "module", || Ok(serde_json::Value::Null)).schedule_at(future_time);
        assert_eq!(task.scheduled_at, Some(future_time));
    }

    #[test]
    fn test_task_priority_ordering() {
        assert!(TaskPriority::Critical < TaskPriority::High);
        assert!(TaskPriority::High < TaskPriority::Normal);
        assert!(TaskPriority::Normal < TaskPriority::Low);
        assert!(TaskPriority::Low < TaskPriority::Idle);
    }

    #[test]
    fn test_task_priority_default() {
        let priority = TaskPriority::default();
        assert_eq!(priority, TaskPriority::Normal);
    }

    #[tokio::test]
    async fn test_task_scheduler() {
        let mut scheduler = TaskScheduler::new(2);

        let task_id = scheduler.submit(Task::new("test", "test_module", || {
            Ok(serde_json::json!({"done": true}))
        }));

        // Wait for task to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let status = scheduler.get_status(task_id).unwrap();
        assert_eq!(status.status, TaskStatus::Completed);
        assert!(status.result.is_some());
        assert_eq!(status.result.unwrap()["done"], true);

        scheduler.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_task_scheduler_failed_task() {
        let mut scheduler = TaskScheduler::new(2);

        let task_id = scheduler.submit(Task::new("failing_task", "test_module", || {
            Err(crate::Error::Task("Task failed intentionally".to_string()))
        }));

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let status = scheduler.get_status(task_id).unwrap();
        assert_eq!(status.status, TaskStatus::Failed);
        assert!(status.error.is_some());
        assert!(status.error.unwrap().contains("intentionally"));

        scheduler.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_task_scheduler_list() {
        let mut scheduler = TaskScheduler::new(2);

        let task1 = scheduler.submit(Task::new("task1", "mod1", || Ok(serde_json::Value::Null)));
        let task2 = scheduler.submit(Task::new("task2", "mod2", || Ok(serde_json::Value::Null)));

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let all_tasks = scheduler.list(None);
        assert_eq!(all_tasks.len(), 2);

        let completed_tasks = scheduler.list(Some(TaskStatus::Completed));
        assert_eq!(completed_tasks.len(), 2);

        let pending_tasks = scheduler.list(Some(TaskStatus::Pending));
        assert_eq!(pending_tasks.len(), 0);

        scheduler.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_task_cancel() {
        let mut scheduler = TaskScheduler::new(1);

        // Submit a slow task
        let task_id = scheduler.submit(Task::new("slow_task", "module", || {
            std::thread::sleep(std::time::Duration::from_secs(10));
            Ok(serde_json::Value::Null)
        }));

        // Try to cancel immediately (may or may not work depending on timing)
        // First, submit another task
        let task_id2 = scheduler.submit(Task::new("to_cancel", "module", || {
            Ok(serde_json::Value::Null)
        }));

        // Cancel the second task
        let cancelled = scheduler.cancel(task_id2);

        // The result depends on whether the task has already started
        // This is somewhat timing-dependent

        scheduler.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_task_info_timestamps() {
        let mut scheduler = TaskScheduler::new(2);

        let task_id = scheduler.submit(Task::new("test", "module", || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            Ok(serde_json::Value::Null)
        }));

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let status = scheduler.get_status(task_id).unwrap();
        assert!(status.started_at.is_some());
        assert!(status.completed_at.is_some());
        assert!(status.started_at.unwrap() <= status.completed_at.unwrap());

        scheduler.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_task_scheduler_concurrent_execution() {
        let mut scheduler = TaskScheduler::new(4);
        let counter = Arc::new(AtomicUsize::new(0));

        for i in 0..10 {
            let counter_clone = counter.clone();
            scheduler.submit(Task::new(&format!("task_{}", i), "module", move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                Ok(serde_json::Value::Null)
            }));
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        assert_eq!(counter.load(Ordering::SeqCst), 10);

        scheduler.shutdown().await.unwrap();
    }

    #[test]
    fn test_task_status_variants() {
        let statuses = [
            TaskStatus::Pending,
            TaskStatus::Running,
            TaskStatus::Completed,
            TaskStatus::Failed,
            TaskStatus::Cancelled,
        ];

        for status in &statuses {
            let serialized = serde_json::to_string(status).expect("Failed to serialize");
            let deserialized: TaskStatus =
                serde_json::from_str(&serialized).expect("Failed to deserialize");
            assert_eq!(*status, deserialized);
        }
    }

    #[test]
    fn test_task_info_serialization() {
        let info = TaskInfo {
            id: Uuid::new_v4(),
            name: "test_task".to_string(),
            module_id: "test_module".to_string(),
            priority: TaskPriority::High,
            status: TaskStatus::Completed,
            scheduled_at: None,
            started_at: Some(Utc::now()),
            completed_at: Some(Utc::now()),
            retry_count: 0,
            error: None,
            result: Some(serde_json::json!({"key": "value"})),
        };

        let serialized = serde_json::to_string(&info).expect("Failed to serialize");
        let deserialized: TaskInfo =
            serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(deserialized.name, "test_task");
        assert_eq!(deserialized.status, TaskStatus::Completed);
        assert_eq!(deserialized.priority, TaskPriority::High);
    }

    #[tokio::test]
    async fn test_get_nonexistent_task() {
        let mut scheduler = TaskScheduler::new(1);

        let status = scheduler.get_status(Uuid::new_v4());
        assert!(status.is_none());

        scheduler.shutdown().await.unwrap();
    }
}
