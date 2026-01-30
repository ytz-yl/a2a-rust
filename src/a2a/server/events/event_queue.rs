//! Event queue trait and types for A2A server
//! 
//! This module defines the Event trait and EventQueue trait that form the foundation
//! of the asynchronous event system in the A2A server.

use crate::a2a::error::A2AError;
use crate::a2a::core_types::Message;
use crate::{Task, TaskStatusUpdateEvent, TaskArtifactUpdateEvent};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use futures::Stream;

/// Events that can be enqueued and processed by the event queue
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Event {
    /// A message event
    Message(Message),
    /// A task event
    Task(Task),
    /// A task status update event
    TaskStatusUpdate(TaskStatusUpdateEvent),
    /// A task artifact update event
    TaskArtifactUpdate(TaskArtifactUpdateEvent),
}


/// Trait for event queues that handle asynchronous event processing
#[async_trait]
pub trait EventQueue: Send + Sync {
    /// Enqueue an event to this queue and all its children
    async fn enqueue_event(&self, event: Event) -> Result<(), A2AError>;

    /// Dequeue an event from the queue
    /// 
    /// # Arguments
    /// * `no_wait` - If true, return immediately with an error if the queue is empty
    /// 
    /// # Returns
    /// * `Ok(Event)` - The next event from the queue
    /// * `Err(A2AError)` - If the queue is closed or empty (when no_wait=true)
    async fn dequeue_event(&self, no_wait: bool) -> Result<Event, A2AError>;

    /// Create a child queue that receives all future events from this queue
    fn tap(&self) -> Arc<dyn EventQueue>;

    /// Close the queue for future enqueue operations
    /// 
    /// # Arguments
    /// * `immediate` - If true, clear all pending events immediately
    async fn close(&self, immediate: bool) -> Result<(), A2AError>;

    /// Check if the queue is closed
    fn is_closed(&self) -> bool;

    /// Get the current size of the queue
    fn size(&self) -> usize;

    /// Signal that a dequeued event has been processed
    fn task_done(&self);
}

/// Stream implementation for EventQueue
pub struct EventQueueStream {
    queue: Arc<dyn EventQueue>,
}

impl EventQueueStream {
    /// Create a new stream from an event queue
    pub fn new(queue: Arc<dyn EventQueue>) -> Self {
        Self { queue }
    }
}

impl Stream for EventQueueStream {
    type Item = Result<Event, A2AError>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // For now, we'll use a blocking approach in a spawn_blocking task
        // In a real implementation, this would be more sophisticated
        if self.queue.is_closed() && self.queue.size() == 0 {
            Poll::Ready(None)
        } else {
            // This is a simplified implementation
            // A proper implementation would use async notification mechanisms
            Poll::Pending
        }
    }
}

/// Default maximum queue size to prevent memory exhaustion
pub const DEFAULT_MAX_QUEUE_SIZE: usize = 1024;

/// Error specific to event queue operations
#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("Queue is closed")]
    Closed,
    
    #[error("Queue is full")]
    Full,
    
    #[error("Queue is empty")]
    Empty,
    
    #[error("Invalid queue size: {size}")]
    InvalidSize { size: usize },
}

impl From<QueueError> for A2AError {
    fn from(err: QueueError) -> Self {
        A2AError::internal(&err.to_string())
    }
}

/// Configuration for event queues
#[derive(Debug, Clone)]
pub struct QueueConfig {
    /// Maximum number of events in the queue
    pub max_size: usize,
    /// Whether to block when the queue is full
    pub block_when_full: bool,
    /// Timeout for blocking operations (in milliseconds)
    pub blocking_timeout_ms: Option<u64>,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_size: DEFAULT_MAX_QUEUE_SIZE,
            block_when_full: true,
            blocking_timeout_ms: Some(5000), // 5 seconds
        }
    }
}

impl QueueConfig {
    /// Create a new queue config with custom max size
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            max_size,
            ..Default::default()
        }
    }

    /// Create a non-blocking queue config
    pub fn non_blocking() -> Self {
        Self {
            block_when_full: false,
            ..Default::default()
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), QueueError> {
        if self.max_size == 0 {
            return Err(QueueError::InvalidSize { size: self.max_size });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a2a::core_types::*;

    #[test]
    fn test_queue_config_validation() {
        let config = QueueConfig::default();
        assert!(config.validate().is_ok());

        let config = QueueConfig { max_size: 0, ..Default::default() };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_event_serialization() {
        let message = Message::new(
            Role::User,
            vec![Part::text("Hello".to_string())],
        );
        let event = Event::Message(message);
        
        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: Event = serde_json::from_str(&serialized).unwrap();
        
        match deserialized {
            Event::Message(msg) => {
                assert_eq!(msg.role, Role::User);
                assert_eq!(msg.parts.len(), 1);
            }
            _ => panic!("Expected Message event"),
        }
    }

    #[test]
    fn test_task_status_update_event() {
        let event = TaskStatusUpdateEvent {
            task_id: "task-123".to_string(),
            context_id: "ctx-456".to_string(),
            status: TaskStatus::new(TaskState::Working),
            r#final: false,
            metadata: None,
            kind: "status-update".to_string(),
        };

        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: TaskStatusUpdateEvent = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(deserialized.task_id, "task-123");
        assert_eq!(deserialized.context_id, "ctx-456");
        assert_eq!(deserialized.status.state, TaskState::Working);
        assert_eq!(deserialized.r#final, false);
        assert_eq!(deserialized.kind, "status-update");
    }
}
