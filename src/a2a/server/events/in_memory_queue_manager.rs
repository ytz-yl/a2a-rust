//! In-memory implementation of QueueManager
//! 
//! This module provides a concrete implementation of QueueManager that stores
//! event queues in memory using hash maps and synchronization primitives.

use crate::a2a::error::A2AError;
use crate::a2a::server::events::{
    EventQueue, QueueManager, QueueManagerConfig, QueueManagerError, 
    InMemoryEventQueue, validate_queue_id
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// In-memory implementation of QueueManager
pub struct InMemoryQueueManager {
    /// Map of queue ID to event queue
    queues: Arc<RwLock<HashMap<String, Arc<dyn EventQueue>>>>,
    /// Configuration for the queue manager
    config: QueueManagerConfig,
    /// Last cleanup time
    last_cleanup: Arc<RwLock<Instant>>,
}

impl InMemoryQueueManager {
    /// Create a new in-memory queue manager with default configuration
    pub fn new() -> Result<Self, A2AError> {
        Self::with_config(QueueManagerConfig::default())
    }

    /// Create a new in-memory queue manager with custom configuration
    pub fn with_config(config: QueueManagerConfig) -> Result<Self, A2AError> {
        Ok(Self {
            queues: Arc::new(RwLock::new(HashMap::new())),
            config,
            last_cleanup: Arc::new(RwLock::new(Instant::now())),
        })
    }

    /// Internal method to cleanup empty queues if auto_cleanup is enabled
    async fn cleanup_if_needed(&self) -> Result<(), A2AError> {
        if !self.config.auto_cleanup {
            return Ok(());
        }

        let should_cleanup = {
            let last_cleanup = self.last_cleanup.read().unwrap();
            last_cleanup.elapsed() > Duration::from_secs(60) // Cleanup every minute
        };

        if should_cleanup {
            self.cleanup_empty_queues().await?;
        }

        Ok(())
    }

    /// Remove empty queues from the manager
    async fn cleanup_empty_queues(&self) -> Result<(), A2AError> {
        let mut queues = self.queues.write().unwrap();
        let mut to_remove = Vec::new();

        for (id, queue) in queues.iter() {
            if queue.size() == 0 && queue.is_closed() {
                to_remove.push(id.clone());
            }
        }

        for id in to_remove {
            queues.remove(&id);
            tracing::debug!("Cleaned up empty queue: {}", id);
        }

        {
            let mut last_cleanup = self.last_cleanup.write().unwrap();
            *last_cleanup = Instant::now();
        }

        tracing::debug!("Cleanup completed, {} queues remaining", queues.len());
        Ok(())
    }

    /// Internal method to create a new queue
    async fn create_queue_internal(&self, id: &str) -> Result<Arc<dyn EventQueue>, A2AError> {
        validate_queue_id(id)?;

        let queue = InMemoryEventQueue::with_config(self.config.default_queue_config.clone())?;
        let queue_arc: Arc<dyn EventQueue> = Arc::new(queue);

        {
            let mut queues = self.queues.write().unwrap();
            if queues.len() >= self.config.max_queues {
                return Err(QueueManagerError::MaxQueuesReached { 
                    max: self.config.max_queues 
                }.into());
            }

            if queues.contains_key(id) {
                return Err(QueueManagerError::QueueExists { 
                    id: id.to_string() 
                }.into());
            }

            queues.insert(id.to_string(), queue_arc.clone());
        }

        tracing::debug!("Created new queue: {}", id);
        Ok(queue_arc)
    }
}

#[async_trait]
impl QueueManager for InMemoryQueueManager {
    async fn create_queue(&self, id: &str) -> Result<Arc<dyn EventQueue>, A2AError> {
        self.cleanup_if_needed().await?;
        self.create_queue_internal(id).await
    }

    async fn create_or_tap(&self, id: &str) -> Result<Arc<dyn EventQueue>, A2AError> {
        self.cleanup_if_needed().await?;
        validate_queue_id(id)?;

        // Try to get existing queue
        {
            let queues = self.queues.read().unwrap();
            if let Some(queue) = queues.get(id) {
                tracing::debug!("Tapping into existing queue: {}", id);
                return Ok(queue.tap());
            }
        }

        // Create new queue if it doesn't exist
        self.create_queue_internal(id).await
    }

    async fn tap(&self, id: &str) -> Result<Option<Arc<dyn EventQueue>>, A2AError> {
        validate_queue_id(id)?;

        let queues = self.queues.read().unwrap();
        if let Some(queue) = queues.get(id) {
            tracing::debug!("Tapping into existing queue: {}", id);
            Ok(Some(queue.tap()))
        } else {
            tracing::debug!("Queue not found for tapping: {}", id);
            Ok(None)
        }
    }

    async fn close(&self, id: &str) -> Result<(), A2AError> {
        validate_queue_id(id)?;

        let queue = {
            let mut queues = self.queues.write().unwrap();
            queues.remove(id)
        };

        if let Some(queue) = queue {
            queue.close(false).await?;
            tracing::debug!("Closed queue: {}", id);
            Ok(())
        } else {
            Err(QueueManagerError::QueueNotFound { 
                id: id.to_string() 
            }.into())
        }
    }

    async fn close_all(&self) -> Result<(), A2AError> {
        let queues = {
            let mut queues_guard = self.queues.write().unwrap();
            std::mem::take(&mut *queues_guard)
        };

        let mut errors = Vec::new();
        for (id, queue) in queues {
            if let Err(e) = queue.close(false).await {
                errors.push((id, e));
            }
        }

        if errors.is_empty() {
            tracing::debug!("Closed all queues");
            Ok(())
        } else {
            let error_msg = format!("Failed to close {} queues", errors.len());
            tracing::error!("{}: {:?}", error_msg, errors);
            Err(A2AError::internal(&error_msg))
        }
    }

    fn queue_count(&self) -> usize {
        let queues = self.queues.read().unwrap();
        queues.len()
    }

    fn has_queue(&self, id: &str) -> bool {
        let queues = self.queues.read().unwrap();
        queues.contains_key(id)
    }
}

impl Default for InMemoryQueueManager {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a2a::server::events::Event;
    use crate::a2a::core_types::*;

    #[tokio::test]
    async fn test_create_queue() {
        let manager = InMemoryQueueManager::new().unwrap();
        
        let queue = manager.create_queue("test-queue").await.unwrap();
        assert_eq!(manager.queue_count(), 1);
        assert!(manager.has_queue("test-queue"));

        // Test that we can use the queue
        let event = Event::Message(Message::new(
            Role::User,
            vec![Part::text("Hello".to_string())],
        ));
        queue.enqueue_event(event).await.unwrap();
        assert_eq!(queue.size(), 1);
    }

    #[tokio::test]
    async fn test_create_or_tap() {
        let manager = InMemoryQueueManager::new().unwrap();
        
        // Create a new queue
        let queue1 = manager.create_or_tap("test-queue").await.unwrap();
        assert_eq!(manager.queue_count(), 1);
        
        // Tap into existing queue
        let queue2 = manager.create_or_tap("test-queue").await.unwrap();
        assert_eq!(manager.queue_count(), 1); // Should still be 1
        
        // Test that both queues receive events
        let event = Event::Message(Message::new(
            Role::User,
            vec![Part::text("Hello".to_string())],
        ));
        queue1.enqueue_event(event.clone()).await.unwrap();
        
        let event_from_queue2 = queue2.dequeue_event(false).await.unwrap();
        match event_from_queue2 {
            Event::Message(msg) => {
                assert_eq!(msg.role, Role::User);
            }
            _ => panic!("Expected Message event"),
        }
    }

    #[tokio::test]
    async fn test_close_queue() {
        let manager = InMemoryQueueManager::new().unwrap();
        
        let queue = manager.create_queue("test-queue").await.unwrap();
        assert_eq!(manager.queue_count(), 1);
        
        manager.close("test-queue").await.unwrap();
        assert_eq!(manager.queue_count(), 0);
        assert!(!manager.has_queue("test-queue"));
        assert!(queue.is_closed());
    }

    #[tokio::test]
    async fn test_close_all() {
        let manager = InMemoryQueueManager::new().unwrap();
        
        manager.create_queue("queue1").await.unwrap();
        manager.create_queue("queue2").await.unwrap();
        manager.create_queue("queue3").await.unwrap();
        
        assert_eq!(manager.queue_count(), 3);
        
        manager.close_all().await.unwrap();
        assert_eq!(manager.queue_count(), 0);
    }

    #[tokio::test]
    async fn test_invalid_queue_id() {
        let manager = InMemoryQueueManager::new().unwrap();
        
        // Test empty ID
        let result = manager.create_queue("").await;
        assert!(result.is_err());
        
        // Test ID with invalid characters
        let result = manager.create_queue("invalid/id").await;
        assert!(result.is_err());
        
        // Test too long ID
        let long_id = "a".repeat(256);
        let result = manager.create_queue(&long_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_max_queues_limit() {
        let config = QueueManagerConfig {
            max_queues: 2,
            ..Default::default()
        };
        let manager = InMemoryQueueManager::with_config(config).unwrap();
        
        manager.create_queue("queue1").await.unwrap();
        manager.create_queue("queue2").await.unwrap();
        
        // Should fail when trying to create a third queue
        let result = manager.create_queue("queue3").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_queue_exists_error() {
        let manager = InMemoryQueueManager::new().unwrap();
        
        manager.create_queue("test-queue").await.unwrap();
        
        // Should fail when trying to create a queue with the same ID
        let result = manager.create_queue("test-queue").await;
        assert!(result.is_err());
    }
}
