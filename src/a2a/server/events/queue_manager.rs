//! Queue manager for managing multiple event queues
//! 
//! This module provides the QueueManager trait and related functionality
//! for managing multiple event queues in the A2A server.

use crate::a2a::error::A2AError;
use crate::a2a::server::events::EventQueue;
use async_trait::async_trait;
use std::sync::Arc;

/// Trait for managing event queues
#[async_trait]
pub trait QueueManager: Send + Sync {
    /// Create a new event queue with the given identifier
    async fn create_queue(&self, id: &str) -> Result<Arc<dyn EventQueue>, A2AError>;

    /// Create a new event queue or tap into an existing one
    async fn create_or_tap(&self, id: &str) -> Result<Arc<dyn EventQueue>, A2AError>;

    /// Tap into an existing event queue
    async fn tap(&self, id: &str) -> Result<Option<Arc<dyn EventQueue>>, A2AError>;

    /// Close an event queue
    async fn close(&self, id: &str) -> Result<(), A2AError>;

    /// Close all event queues
    async fn close_all(&self) -> Result<(), A2AError>;

    /// Get the number of active queues
    fn queue_count(&self) -> usize;

    /// Check if a queue exists
    fn has_queue(&self, id: &str) -> bool;
}

/// Configuration for queue manager
#[derive(Debug, Clone)]
pub struct QueueManagerConfig {
    /// Maximum number of queues to manage
    pub max_queues: usize,
    /// Default configuration for new queues
    pub default_queue_config: crate::a2a::server::events::QueueConfig,
    /// Whether to automatically clean up empty queues
    pub auto_cleanup: bool,
}

impl Default for QueueManagerConfig {
    fn default() -> Self {
        Self {
            max_queues: 1000,
            default_queue_config: crate::a2a::server::events::QueueConfig::default(),
            auto_cleanup: true,
        }
    }
}

/// Error specific to queue manager operations
#[derive(Debug, thiserror::Error)]
pub enum QueueManagerError {
    #[error("Queue not found: {id}")]
    QueueNotFound { id: String },

    #[error("Queue already exists: {id}")]
    QueueExists { id: String },

    #[error("Maximum number of queues reached: {max}")]
    MaxQueuesReached { max: usize },

    #[error("Invalid queue ID: {id}")]
    InvalidQueueId { id: String },
}

impl From<QueueManagerError> for A2AError {
    fn from(err: QueueManagerError) -> Self {
        A2AError::internal(&err.to_string())
    }
}

/// Validate queue ID
pub fn validate_queue_id(id: &str) -> Result<(), QueueManagerError> {
    if id.is_empty() {
        return Err(QueueManagerError::InvalidQueueId { id: id.to_string() });
    }
    
    // Check for invalid characters (basic validation)
    if id.contains('/') || id.contains('\\') || id.contains('\0') {
        return Err(QueueManagerError::InvalidQueueId { id: id.to_string() });
    }
    
    // Check length limits
    if id.len() > 255 {
        return Err(QueueManagerError::InvalidQueueId { id: id.to_string() });
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_queue_id() {
        // Valid IDs
        assert!(validate_queue_id("task-123").is_ok());
        assert!(validate_queue_id("queue_456").is_ok());
        assert!(validate_queue_id("valid-id-123").is_ok());

        // Invalid IDs
        assert!(validate_queue_id("").is_err());
        assert!(validate_queue_id("invalid/id").is_err());
        assert!(validate_queue_id("invalid\\id").is_err());
        assert!(validate_queue_id("invalid\0id").is_err());
        
        // Too long ID
        let long_id = "a".repeat(256);
        assert!(validate_queue_id(&long_id).is_err());
    }

    #[test]
    fn test_queue_manager_config() {
        let config = QueueManagerConfig::default();
        assert_eq!(config.max_queues, 1000);
        assert!(config.auto_cleanup);
    }

    #[test]
    fn test_queue_manager_error() {
        let error = QueueManagerError::QueueNotFound { id: "test".to_string() };
        let a2a_error: A2AError = error.into();
        assert!(a2a_error.message().contains("Queue not found"));
    }
}
