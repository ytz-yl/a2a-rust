//! In-memory implementation of EventQueue
//! 
//! This module provides a concrete implementation of EventQueue that stores
//! events in memory using async channels and synchronization primitives.

use crate::a2a::error::A2AError;
use crate::a2a::server::events::{Event, EventQueue, QueueConfig, QueueError};
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Notify, Mutex};
use tokio::time::timeout;

/// In-memory implementation of EventQueue
pub struct InMemoryEventQueue {
    /// The actual queue storage
    queue: Arc<Mutex<VecDeque<Event>>>,
    /// Maximum queue size
    max_size: usize,
    /// Whether the queue is closed
    is_closed: Arc<AtomicBool>,
    /// Notify waiting consumers
    notifier: Arc<Notify>,
    /// Child queues that receive all events
    children: Arc<Mutex<Vec<Arc<dyn EventQueue>>>>,
    /// Broadcast channel for event distribution
    event_sender: broadcast::Sender<Event>,
    /// Current queue size for atomic access
    current_size: Arc<AtomicUsize>,
}

impl InMemoryEventQueue {
    /// Create a new in-memory event queue with default configuration
    pub fn new() -> Result<Self, A2AError> {
        Self::with_config(QueueConfig::default())
    }

    /// Create a new in-memory event queue with custom configuration
    pub fn with_config(config: QueueConfig) -> Result<Self, A2AError> {
        config.validate()?;
        
        let (event_sender, _) = broadcast::channel(config.max_size);
        
        Ok(Self {
            queue: Arc::new(Mutex::new(VecDeque::with_capacity(config.max_size))),
            max_size: config.max_size,
            is_closed: Arc::new(AtomicBool::new(false)),
            notifier: Arc::new(Notify::new()),
            children: Arc::new(Mutex::new(Vec::new())),
            event_sender,
            current_size: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// Internal method to add an event to the queue
    async fn push_internal(&self, event: Event) -> Result<(), A2AError> {
        if self.is_closed.load(Ordering::Relaxed) {
            return Err(QueueError::Closed.into());
        }

        {
            let mut queue = self.queue.lock().await;
            if queue.len() >= self.max_size {
                return Err(QueueError::Full.into());
            }

            queue.push_back(event.clone());
        }

        self.current_size.fetch_add(1, Ordering::Relaxed);
        self.notifier.notify_one();

        // Send to child queues
        if let Err(e) = self.event_sender.send(event) {
            // This happens when there are no receivers, which is fine
            tracing::debug!("No child queues to receive event: {}", e);
        }

        Ok(())
    }

    /// Internal method to remove an event from the queue
    async fn pop_internal(&self, no_wait: bool) -> Result<Event, A2AError> {
        loop {
            {
                let mut queue = self.queue.lock().await;
                if let Some(event) = queue.pop_front() {
                    self.current_size.fetch_sub(1, Ordering::Relaxed);
                    return Ok(event);
                }

                if self.is_closed.load(Ordering::Relaxed) {
                    return Err(QueueError::Closed.into());
                }
            }

            if no_wait {
                return Err(QueueError::Empty.into());
            }

            // Wait for notification
            if let Err(_) = timeout(Duration::from_millis(100), self.notifier.notified()).await {
                // Timeout, continue loop to check if closed
                continue;
            }
        }
    }

    /// Clear all events from the queue
    async fn clear_events(&self) -> Result<(), A2AError> {
        let mut queue = self.queue.lock().await;

        let cleared_count = queue.len();
        queue.clear();
        self.current_size.store(0, Ordering::Relaxed);

        tracing::debug!("Cleared {} events from queue", cleared_count);
        Ok(())
    }
}

#[async_trait]
impl EventQueue for InMemoryEventQueue {
    async fn enqueue_event(&self, event: Event) -> Result<(), A2AError> {
        self.push_internal(event).await
    }

    async fn dequeue_event(&self, no_wait: bool) -> Result<Event, A2AError> {
        self.pop_internal(no_wait).await
    }

    fn tap(&self) -> Arc<dyn EventQueue> {
        let child = Arc::new(InMemoryEventQueueChild::new(self.event_sender.subscribe()));
        
        // We can't use blocking_lock in an async context
        // Instead, we'll use a different approach - spawn a task to add the child
        // For now, we'll create a simple implementation that doesn't track children
        // in the parent, since the broadcast channel already handles the distribution
        
        child
    }

    async fn close(&self, immediate: bool) -> Result<(), A2AError> {
        if self.is_closed.load(Ordering::Relaxed) {
            return Ok(());
        }

        self.is_closed.store(true, Ordering::Relaxed);

        if immediate {
            self.clear_events().await?;
        }

        // Notify all waiting consumers
        self.notifier.notify_waiters();

        // Close child queues
        let children = {
            let mut children_guard = self.children.lock().await;
            std::mem::take(&mut *children_guard)
        };

        for child in children {
            child.close(immediate).await?;
        }

        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.is_closed.load(Ordering::Relaxed)
    }

    fn size(&self) -> usize {
        self.current_size.load(Ordering::Relaxed)
    }

    fn task_done(&self) {
        // In this implementation, we don't need to track task completion
        // since we're using a simple deque without worker tracking
    }
}

/// Child queue that receives events from a parent queue via broadcast channel
pub struct InMemoryEventQueueChild {
    /// Receiver for events from parent
    event_receiver: Arc<Mutex<broadcast::Receiver<Event>>>,
    /// Whether this child queue is closed
    is_closed: Arc<AtomicBool>,
}

impl InMemoryEventQueueChild {
    /// Create a new child queue with the given receiver
    fn new(event_receiver: broadcast::Receiver<Event>) -> Self {
        Self {
            event_receiver: Arc::new(Mutex::new(event_receiver)),
            is_closed: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[async_trait]
impl EventQueue for InMemoryEventQueueChild {
    async fn enqueue_event(&self, _event: Event) -> Result<(), A2AError> {
        // Child queues cannot be enqueued directly
        Err(A2AError::unsupported_operation("Child queues cannot be enqueued directly"))
    }

    async fn dequeue_event(&self, no_wait: bool) -> Result<Event, A2AError> {
        if self.is_closed.load(Ordering::Relaxed) {
            return Err(QueueError::Closed.into());
        }

        let mut receiver = self.event_receiver.lock().await;

        if no_wait {
            match receiver.try_recv() {
                Ok(event) => Ok(event),
                Err(broadcast::error::TryRecvError::Empty) => Err(QueueError::Empty.into()),
                Err(broadcast::error::TryRecvError::Lagged(skipped)) => {
                    tracing::warn!("Child queue lagged behind, skipped {} events", skipped);
                    match receiver.try_recv() {
                        Ok(event) => Ok(event),
                        Err(_) => Err(QueueError::Empty.into()),
                    }
                }
                Err(broadcast::error::TryRecvError::Closed) => Err(QueueError::Closed.into()),
            }
        } else {
            match receiver.recv().await {
                Ok(event) => Ok(event),
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!("Child queue lagged behind, skipped {} events", skipped);
                    // Try again to get the next event
                    match receiver.recv().await {
                        Ok(event) => Ok(event),
                        Err(_) => Err(QueueError::Closed.into()),
                    }
                }
                Err(broadcast::error::RecvError::Closed) => Err(QueueError::Closed.into()),
            }
        }
    }

    fn tap(&self) -> Arc<dyn EventQueue> {
        // Child queues don't support tapping, but we need to create a new receiver
        // We can't use blocking_lock in async context, so we'll create a new receiver
        // by cloning the existing receiver's subscription
        // This is a limitation of the sync interface - in practice, tapping child queues
        // should be done in async context
        let receiver = self.event_receiver.clone();
        // We need to get a new subscription, but we can't do this synchronously
        // For now, we'll return an error or a dummy implementation
        // In a real implementation, this method should be async
        Arc::new(InMemoryEventQueueChild::new(receiver.try_lock().map(|r| r.resubscribe()).unwrap_or_else(|_| {
            // Create a new receiver if we can't lock the existing one
            // This is a fallback that shouldn't happen in normal usage
            let (_, new_rx) = broadcast::channel(100);
            new_rx
        })))
    }

    async fn close(&self, _immediate: bool) -> Result<(), A2AError> {
        self.is_closed.store(true, Ordering::Relaxed);
        // Drop the receiver to close the subscription
        // This is handled by the broadcast channel automatically
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.is_closed.load(Ordering::Relaxed)
    }

    fn size(&self) -> usize {
        // Child queues don't have a size concept since they're just receivers
        0
    }

    fn task_done(&self) {
        // No-op for child queues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a2a::core_types::*;

    #[tokio::test]
    async fn test_basic_queue_operations() {
        let queue = InMemoryEventQueue::new().unwrap();
        
        let event = Event::Message(Message::new(
            Role::User,
            vec![Part::text("Hello".to_string())],
        ));

        // Test enqueue
        queue.enqueue_event(event.clone()).await.unwrap();
        assert_eq!(queue.size(), 1);

        // Test dequeue
        let dequeued = queue.dequeue_event(false).await.unwrap();
        assert_eq!(queue.size(), 0);
        
        match dequeued {
            Event::Message(msg) => {
                assert_eq!(msg.role, Role::User);
            }
            _ => panic!("Expected Message event"),
        }
    }

    #[tokio::test]
    async fn test_queue_tapping() {
        let parent = InMemoryEventQueue::new().unwrap();
        let child = parent.tap();

        let event = Event::Message(Message::new(
            Role::User,
            vec![Part::text("Hello".to_string())],
        ));

        // Enqueue to parent
        parent.enqueue_event(event.clone()).await.unwrap();

        // Should be available in child
        let child_event = child.dequeue_event(false).await.unwrap();
        
        match child_event {
            Event::Message(msg) => {
                assert_eq!(msg.role, Role::User);
            }
            _ => panic!("Expected Message event"),
        }
    }

    #[tokio::test]
    async fn test_queue_close() {
        let queue = InMemoryEventQueue::new().unwrap();
        
        let event = Event::Message(Message::new(
            Role::User,
            vec![Part::text("Hello".to_string())],
        ));

        queue.enqueue_event(event).await.unwrap();
        queue.close(false).await.unwrap();

        assert!(queue.is_closed());

        // Should still be able to dequeue existing events
        let _dequeued = queue.dequeue_event(false).await.unwrap();

        // But not able to dequeue new ones
        let result = queue.dequeue_event(false).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_queue_size_limit() {
        let config = QueueConfig::with_max_size(2);
        let queue = InMemoryEventQueue::with_config(config).unwrap();

        let event = Event::Message(Message::new(
            Role::User,
            vec![Part::text("Hello".to_string())],
        ));

        // Fill the queue
        queue.enqueue_event(event.clone()).await.unwrap();
        queue.enqueue_event(event.clone()).await.unwrap();
        assert_eq!(queue.size(), 2);

        // Should fail when trying to add more
        let result = queue.enqueue_event(event).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_no_wait_dequeue() {
        let queue = InMemoryEventQueue::new().unwrap();

        // Should return error when empty and no_wait=true
        let result = queue.dequeue_event(true).await;
        assert!(result.is_err());
    }
}
