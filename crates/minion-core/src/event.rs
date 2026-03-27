//! Event bus for inter-module communication

use chrono::{DateTime, Utc};
use flume::{Receiver, Sender};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Event types that can be emitted in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum Event {
    // System events
    SystemStartup,
    SystemShutdown,

    // Module events
    ModuleLoaded {
        module_id: String,
    },
    ModuleUnloaded {
        module_id: String,
    },
    ModuleError {
        module_id: String,
        error: String,
    },

    // Configuration events
    ConfigChanged {
        key: String,
        value: serde_json::Value,
    },

    // File events
    FileCreated {
        path: String,
    },
    FileModified {
        path: String,
    },
    FileDeleted {
        path: String,
    },

    // Content events
    ContentCreated {
        content_type: String,
        id: String,
        module_id: String,
    },
    ContentUpdated {
        content_type: String,
        id: String,
        module_id: String,
    },
    ContentDeleted {
        content_type: String,
        id: String,
        module_id: String,
    },

    // AI events
    AIRequestStarted {
        request_id: String,
        model: String,
    },
    AIRequestCompleted {
        request_id: String,
        tokens: u32,
    },
    AIRequestFailed {
        request_id: String,
        error: String,
    },

    // User events
    UserAction {
        action: String,
        payload: serde_json::Value,
    },

    // Custom events from plugins
    Custom {
        event_type: String,
        source: String,
        payload: serde_json::Value,
    },
}

/// Wrapper for events with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// Unique event ID
    pub id: Uuid,

    /// Event timestamp
    pub timestamp: DateTime<Utc>,

    /// Source module/component
    pub source: String,

    /// The event payload
    pub event: Event,

    /// Correlation ID for tracking related events
    pub correlation_id: Option<Uuid>,
}

impl EventEnvelope {
    pub fn new(source: &str, event: Event) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            source: source.to_string(),
            event,
            correlation_id: None,
        }
    }

    pub fn with_correlation(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }
}

/// Event handler callback type
pub type EventHandler = Arc<dyn Fn(&EventEnvelope) + Send + Sync>;

/// Map of event type patterns to lists of subscriber handlers
type SubscriberMap = HashMap<String, Vec<(SubscriptionId, EventHandler)>>;

/// Subscription handle for unsubscribing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(Uuid);

/// Event bus for publishing and subscribing to events
pub struct EventBus {
    /// Event channel sender
    sender: Sender<EventEnvelope>,

    /// Event channel receiver (for the dispatcher)
    receiver: Receiver<EventEnvelope>,

    /// Subscribers by event type pattern
    subscribers: Arc<RwLock<SubscriberMap>>,

    /// Global subscribers (receive all events)
    global_subscribers: Arc<RwLock<Vec<(SubscriptionId, EventHandler)>>>,

    /// Shutdown flag
    shutdown: Arc<RwLock<bool>>,
}

impl EventBus {
    /// Create a new event bus
    pub fn new() -> Self {
        let (sender, receiver) = flume::bounded(10000);

        Self {
            sender,
            receiver,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            global_subscribers: Arc::new(RwLock::new(Vec::new())),
            shutdown: Arc::new(RwLock::new(false)),
        }
    }

    /// Publish an event to the bus
    pub fn publish(&self, envelope: EventEnvelope) {
        if *self.shutdown.read() {
            return;
        }

        let _ = self.sender.send(envelope);
    }

    /// Publish an event with automatic envelope creation
    pub fn emit(&self, source: &str, event: Event) {
        self.publish(EventEnvelope::new(source, event));
    }

    /// Subscribe to events matching a pattern
    ///
    /// Patterns:
    /// - "*" matches all events
    /// - "ModuleLoaded" matches specific event type
    /// - "Module*" matches events starting with "Module"
    pub fn subscribe<F>(&self, pattern: &str, handler: F) -> SubscriptionId
    where
        F: Fn(&EventEnvelope) + Send + Sync + 'static,
    {
        let id = SubscriptionId(Uuid::new_v4());
        let handler = Arc::new(handler) as EventHandler;

        if pattern == "*" {
            self.global_subscribers.write().push((id, handler));
        } else {
            self.subscribers
                .write()
                .entry(pattern.to_string())
                .or_default()
                .push((id, handler));
        }

        id
    }

    /// Unsubscribe from events
    pub fn unsubscribe(&self, id: SubscriptionId) {
        // Remove from global subscribers
        self.global_subscribers
            .write()
            .retain(|(sub_id, _)| *sub_id != id);

        // Remove from pattern subscribers
        let mut subs = self.subscribers.write();
        for handlers in subs.values_mut() {
            handlers.retain(|(sub_id, _)| *sub_id != id);
        }
    }

    /// Start the event dispatcher
    pub fn start_dispatcher(&self) -> tokio::task::JoinHandle<()> {
        let receiver = self.receiver.clone();
        let subscribers = self.subscribers.clone();
        let global_subscribers = self.global_subscribers.clone();
        let shutdown = self.shutdown.clone();

        tokio::spawn(async move {
            loop {
                if *shutdown.read() {
                    break;
                }

                match receiver.recv_async().await {
                    Ok(envelope) => {
                        // Notify global subscribers
                        for (_, handler) in global_subscribers.read().iter() {
                            handler(&envelope);
                        }

                        // Notify pattern subscribers
                        let event_type = Self::get_event_type(&envelope.event);
                        let subs = subscribers.read();

                        for (pattern, handlers) in subs.iter() {
                            if Self::matches_pattern(pattern, &event_type) {
                                for (_, handler) in handlers {
                                    handler(&envelope);
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        })
    }

    /// Shutdown the event bus
    pub fn shutdown(&self) {
        *self.shutdown.write() = true;
    }

    /// Get the event type name for pattern matching
    fn get_event_type(event: &Event) -> String {
        match event {
            Event::SystemStartup => "SystemStartup".to_string(),
            Event::SystemShutdown => "SystemShutdown".to_string(),
            Event::ModuleLoaded { .. } => "ModuleLoaded".to_string(),
            Event::ModuleUnloaded { .. } => "ModuleUnloaded".to_string(),
            Event::ModuleError { .. } => "ModuleError".to_string(),
            Event::ConfigChanged { .. } => "ConfigChanged".to_string(),
            Event::FileCreated { .. } => "FileCreated".to_string(),
            Event::FileModified { .. } => "FileModified".to_string(),
            Event::FileDeleted { .. } => "FileDeleted".to_string(),
            Event::ContentCreated { .. } => "ContentCreated".to_string(),
            Event::ContentUpdated { .. } => "ContentUpdated".to_string(),
            Event::ContentDeleted { .. } => "ContentDeleted".to_string(),
            Event::AIRequestStarted { .. } => "AIRequestStarted".to_string(),
            Event::AIRequestCompleted { .. } => "AIRequestCompleted".to_string(),
            Event::AIRequestFailed { .. } => "AIRequestFailed".to_string(),
            Event::UserAction { .. } => "UserAction".to_string(),
            Event::Custom { event_type, .. } => format!("Custom:{}", event_type),
        }
    }

    /// Check if an event type matches a pattern
    fn matches_pattern(pattern: &str, event_type: &str) -> bool {
        if let Some(prefix) = pattern.strip_suffix('*') {
            event_type.starts_with(prefix)
        } else {
            pattern == event_type
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_pattern_matching() {
        assert!(EventBus::matches_pattern("Module*", "ModuleLoaded"));
        assert!(EventBus::matches_pattern("Module*", "ModuleUnloaded"));
        assert!(!EventBus::matches_pattern("Module*", "FileCreated"));
        assert!(EventBus::matches_pattern("FileCreated", "FileCreated"));
        assert!(!EventBus::matches_pattern("FileCreated", "FileModified"));
    }

    #[test]
    fn test_pattern_matching_custom_events() {
        assert!(EventBus::matches_pattern("Custom:*", "Custom:MyEvent"));
        assert!(EventBus::matches_pattern("Custom:My*", "Custom:MyEvent"));
        assert!(!EventBus::matches_pattern(
            "Custom:Other*",
            "Custom:MyEvent"
        ));
    }

    #[test]
    fn test_event_envelope_creation() {
        let envelope = EventEnvelope::new("test_source", Event::SystemStartup);

        assert_eq!(envelope.source, "test_source");
        assert!(envelope.correlation_id.is_none());
        assert!(matches!(envelope.event, Event::SystemStartup));
    }

    #[test]
    fn test_event_envelope_with_correlation() {
        let correlation_id = Uuid::new_v4();
        let envelope = EventEnvelope::new("test_source", Event::SystemStartup)
            .with_correlation(correlation_id);

        assert_eq!(envelope.correlation_id, Some(correlation_id));
    }

    #[tokio::test]
    async fn test_event_subscription() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        bus.subscribe("ModuleLoaded", move |_| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        let dispatcher = bus.start_dispatcher();

        bus.emit(
            "test",
            Event::ModuleLoaded {
                module_id: "test".to_string(),
            },
        );
        bus.emit(
            "test",
            Event::FileCreated {
                path: "/test".to_string(),
            },
        );

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        bus.shutdown();

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_global_subscription() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        // Subscribe to all events
        bus.subscribe("*", move |_| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        let _dispatcher = bus.start_dispatcher();

        bus.emit("test", Event::SystemStartup);
        bus.emit(
            "test",
            Event::ModuleLoaded {
                module_id: "m1".to_string(),
            },
        );
        bus.emit(
            "test",
            Event::FileCreated {
                path: "/test".to_string(),
            },
        );

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        bus.shutdown();

        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_unsubscribe() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let sub_id = bus.subscribe("SystemStartup", move |_| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        let _dispatcher = bus.start_dispatcher();

        bus.emit("test", Event::SystemStartup);
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Unsubscribe
        bus.unsubscribe(sub_id);

        bus.emit("test", Event::SystemStartup);
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        bus.shutdown();

        // Should only have received one event
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_event_bus_default() {
        let bus = EventBus::default();
        // Should be able to emit without crashing
        bus.emit("test", Event::SystemStartup);
    }

    #[tokio::test]
    async fn test_wildcard_pattern_subscription() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        bus.subscribe("File*", move |_| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        let _dispatcher = bus.start_dispatcher();

        bus.emit(
            "test",
            Event::FileCreated {
                path: "/a".to_string(),
            },
        );
        bus.emit(
            "test",
            Event::FileModified {
                path: "/b".to_string(),
            },
        );
        bus.emit(
            "test",
            Event::FileDeleted {
                path: "/c".to_string(),
            },
        );
        bus.emit("test", Event::SystemStartup); // Should not match

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        bus.shutdown();

        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_get_event_type() {
        assert_eq!(
            EventBus::get_event_type(&Event::SystemStartup),
            "SystemStartup"
        );
        assert_eq!(
            EventBus::get_event_type(&Event::SystemShutdown),
            "SystemShutdown"
        );
        assert_eq!(
            EventBus::get_event_type(&Event::ModuleLoaded {
                module_id: "x".to_string()
            }),
            "ModuleLoaded"
        );
        assert_eq!(
            EventBus::get_event_type(&Event::Custom {
                event_type: "MyEvent".to_string(),
                source: "test".to_string(),
                payload: serde_json::Value::Null
            }),
            "Custom:MyEvent"
        );
    }

    #[test]
    fn test_event_serialization() {
        let event = Event::ConfigChanged {
            key: "theme".to_string(),
            value: serde_json::json!("dark"),
        };

        let serialized = serde_json::to_string(&event).expect("Failed to serialize");
        let deserialized: Event = serde_json::from_str(&serialized).expect("Failed to deserialize");

        match deserialized {
            Event::ConfigChanged { key, value } => {
                assert_eq!(key, "theme");
                assert_eq!(value, "dark");
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[tokio::test]
    async fn test_shutdown_prevents_new_events() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        bus.subscribe("*", move |_| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        let _dispatcher = bus.start_dispatcher();

        bus.emit("test", Event::SystemStartup);
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Shutdown
        bus.shutdown();

        // Event after shutdown should be dropped
        bus.emit("test", Event::SystemStartup);
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_event_envelope_serialization() {
        let envelope = EventEnvelope::new(
            "test",
            Event::UserAction {
                action: "click".to_string(),
                payload: serde_json::json!({"x": 100, "y": 200}),
            },
        );

        let serialized = serde_json::to_string(&envelope).expect("Failed to serialize");
        let deserialized: EventEnvelope =
            serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(deserialized.source, "test");
        match deserialized.event {
            Event::UserAction { action, payload } => {
                assert_eq!(action, "click");
                assert_eq!(payload["x"], 100);
            }
            _ => panic!("Wrong event type"),
        }
    }
}
