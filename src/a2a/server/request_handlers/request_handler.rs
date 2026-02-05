//! A2A request handler interface
//! 
//! This module defines the RequestHandler trait that matches the Python implementation
//! in a2a-python/src/a2a/server/request_handlers/request_handler.py

use async_trait::async_trait;
use futures::stream::BoxStream;

use crate::a2a::models::*;
use crate::a2a::core_types::{Message, Role, TaskState, TaskStatus, Part, PartRoot};
use crate::a2a::server::context::ServerCallContext;
use crate::a2a::error::A2AError;

/// A2A request handler interface
/// 
/// This trait defines the methods that an A2A server implementation must
/// provide to handle incoming JSON-RPC requests.
#[async_trait]
pub trait RequestHandler: Send + Sync {
    /// Handles the 'tasks/get' method
    /// 
    /// Retrieves the state and history of a specific task.
    async fn on_get_task(
        &self,
        params: TaskQueryParams,
        context: Option<&ServerCallContext>,
    ) -> Result<Option<Task>, A2AError>;

    /// Handles the 'tasks/cancel' method
    /// 
    /// Requests the agent to cancel an ongoing task.
    async fn on_cancel_task(
        &self,
        params: TaskIdParams,
        context: Option<&ServerCallContext>,
    ) -> Result<Option<Task>, A2AError>;

    /// Handles the 'message/send' method (non-streaming)
    /// 
    /// Sends a message to the agent to create, continue, or restart a task,
    /// and waits for the final result (Task or Message).
    async fn on_message_send(
        &self,
        params: MessageSendParams,
        context: Option<&ServerCallContext>,
    ) -> Result<MessageSendResult, A2AError>;

    /// Handles the 'message/stream' method (streaming)
    /// 
    /// Sends a message to the agent and yields stream events as they are
    /// produced (Task updates, Message chunks, Artifact updates).
    async fn on_message_send_stream(
        &self,
        _params: MessageSendParams,
        _context: Option<&ServerCallContext>,
    ) -> Result<BoxStream<'static, Result<Event, A2AError>>, A2AError> {
        // Default implementation raises UnsupportedOperationError
        Err(A2AError::unsupported_operation("Streaming is not supported"))
    }

    /// Handles the 'tasks/pushNotificationConfig/set' method
    /// 
    /// Sets or updates the push notification configuration for a task.
    async fn on_set_task_push_notification_config(
        &self,
        params: TaskPushNotificationConfig,
        context: Option<&ServerCallContext>,
    ) -> Result<TaskPushNotificationConfig, A2AError>;

    /// Handles the 'tasks/pushNotificationConfig/get' method
    /// 
    /// Retrieves the current push notification configuration for a task.
    async fn on_get_task_push_notification_config(
        &self,
        params: TaskPushNotificationConfigQueryParams,
        context: Option<&ServerCallContext>,
    ) -> Result<TaskPushNotificationConfig, A2AError>;

    /// Handles the 'tasks/resubscribe' method
    /// 
    /// Allows a client to re-subscribe to a running streaming task's event stream.
    async fn on_resubscribe_to_task(
        &self,
        _params: TaskIdParams,
        _context: Option<&ServerCallContext>,
    ) -> Result<BoxStream<'static, Result<Event, A2AError>>, A2AError> {
        // Default implementation raises UnsupportedOperationError
        Err(A2AError::unsupported_operation("Resubscription is not supported"))
    }

    /// Handles the 'tasks/pushNotificationConfig/list' method
    /// 
    /// Retrieves the current push notification configurations for a task.
    async fn on_list_task_push_notification_config(
        &self,
        params: TaskIdParams,
        context: Option<&ServerCallContext>,
    ) -> Result<Vec<TaskPushNotificationConfig>, A2AError>;

    /// Handles the 'tasks/pushNotificationConfig/delete' method
    /// 
    /// Deletes a push notification configuration associated with a task.
    async fn on_delete_task_push_notification_config(
        &self,
        params: DeleteTaskPushNotificationConfigParams,
        context: Option<&ServerCallContext>,
    ) -> Result<(), A2AError>;
}

/// Result type for message send operations
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum MessageSendResult {
    Task(Task),
    Message(Message),
}

/// Parameters for querying push notification configuration
#[derive(Debug, Clone)]
pub struct TaskPushNotificationConfigQueryParams {
    pub task_id: String,
    pub push_notification_config_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Event types for streaming operations
#[derive(Debug, Clone)]
pub enum Event {
    TaskStatusUpdate(TaskStatusUpdateEvent),
    TaskArtifactUpdate(TaskArtifactUpdateEvent),
    Message(Message),
    Task(Task),
}

/// Mock request handler for testing
pub struct MockRequestHandler;

impl MockRequestHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RequestHandler for MockRequestHandler {
    async fn on_get_task(
        &self,
        _params: TaskQueryParams,
        _context: Option<&ServerCallContext>,
    ) -> Result<Option<Task>, A2AError> {
        Ok(None)
    }

    async fn on_cancel_task(
        &self,
        _params: TaskIdParams,
        _context: Option<&ServerCallContext>,
    ) -> Result<Option<Task>, A2AError> {
        Ok(None)
    }

    async fn on_message_send(
        &self,
        params: MessageSendParams,
        _context: Option<&ServerCallContext>,
    ) -> Result<MessageSendResult, A2AError> {
        // Return the message back as a mock response
        Ok(MessageSendResult::Message(params.message))
    }

    async fn on_message_send_stream(
        &self,
        params: MessageSendParams,
        _context: Option<&ServerCallContext>,
    ) -> Result<BoxStream<'static, Result<Event, A2AError>>, A2AError> {
        use futures::stream;
        
        // Create a simple mock stream that returns a few events
        let message = params.message.clone();
        let stream = stream::iter(vec![
            // Task status update - working
            Ok(Event::TaskStatusUpdate(TaskStatusUpdateEvent {
                task_id: "mock-task-123".to_string(),
                context_id: message.context_id.clone().unwrap_or_else(|| "mock-context".to_string()),
                status: TaskStatus::new(TaskState::Working),
                r#final: false,
                metadata: None,
                kind: "status-update".to_string(),
            })),
            // Message response
            Ok(Event::Message(Message {
                message_id: format!("response-{}", message.message_id),
                context_id: message.context_id.clone(),
                task_id: Some("mock-task-123".to_string()),
                role: Role::Agent,
                parts: vec![
                    Part::text(format!("Mock response to: {}", 
                        if let Some(PartRoot::Text(text_part)) = message.parts.first().map(|p| p.root()) {
                            text_part.text.clone()
                        } else {
                            "your message".to_string()
                        }
                    ))
                ],
                metadata: None,
                extensions: None,
                reference_task_ids: None,
                kind: "message".to_string(),
            })),
            // Task status update - completed
            Ok(Event::TaskStatusUpdate(TaskStatusUpdateEvent {
                task_id: "mock-task-123".to_string(),
                context_id: message.context_id.clone().unwrap_or_else(|| "mock-context".to_string()),
                status: TaskStatus::new(TaskState::Completed),
                r#final: true,
                metadata: None,
                kind: "status-update".to_string(),
            })),
        ]);
        
        Ok(Box::pin(stream))
    }

    async fn on_set_task_push_notification_config(
        &self,
        _params: TaskPushNotificationConfig,
        _context: Option<&ServerCallContext>,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        Err(A2AError::unsupported_operation("Not implemented"))
    }

    async fn on_get_task_push_notification_config(
        &self,
        _params: TaskPushNotificationConfigQueryParams,
        _context: Option<&ServerCallContext>,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        Err(A2AError::unsupported_operation("Not implemented"))
    }

    async fn on_list_task_push_notification_config(
        &self,
        _params: TaskIdParams,
        _context: Option<&ServerCallContext>,
    ) -> Result<Vec<TaskPushNotificationConfig>, A2AError> {
        Ok(vec![])
    }

    async fn on_delete_task_push_notification_config(
        &self,
        _params: DeleteTaskPushNotificationConfigParams,
        _context: Option<&ServerCallContext>,
    ) -> Result<(), A2AError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    

    #[tokio::test]
    async fn test_mock_request_handler() {
        let handler = MockRequestHandler;
        
        let params = TaskQueryParams {
            id: "test-task".to_string(),
            history_length: None,
            metadata: None,
        };

        let result = handler.on_get_task(params, None).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
