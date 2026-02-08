//! REST request handler adapter
//!
//! Intended to be semantically equivalent to the Python RESTHandler implementation:
//! - Uses RequestHandler as the business logic source
//! - Streaming yields JSON strings per event (NOT SSE "data:" framing here)
//! - Capability validation matches Python decorators:
//!     * message/stream + tasks/resubscribe require streaming capability
//!     * set_push_notification requires push_notifications capability
//!     * get_push_notification DOES NOT gate on push capability (matches Python)
//! - tasks/get + tasks/cancel map None -> TaskNotFoundError (matches Python raising ServerError(TaskNotFoundError()))
//! - list_push_notifications and list_tasks are NOT implemented (matches Python raising NotImplementedError)

use std::pin::Pin;
use std::sync::Arc;

use futures::{Stream, StreamExt};
use serde::Serialize;
use serde_json::{json, Value};

use crate::a2a::error::{A2AError, TaskNotFoundError};
use crate::a2a::models::*;
use crate::a2a::server::context::ServerCallContext;
use crate::a2a::server::request_handlers::{
    Event, MessageSendResult, RequestHandler, TaskPushNotificationConfigQueryParams,
};

/// REST error envelope (matches Python "ServerError" concept at transport boundary)
#[derive(Debug, Clone, Serialize)]
pub struct RestErrorResponse {
    pub code: i32,
    pub message: String,
}

/// REST Handler (Python-equivalent semantics)
pub struct RestHandler {
    agent_card: AgentCard,
    request_handler: Arc<dyn RequestHandler>,
}

impl RestHandler {
    pub fn new(agent_card: AgentCard, request_handler: Arc<dyn RequestHandler>) -> Self {
        Self {
            agent_card,
            request_handler,
        }
    }

    // ------------------------
    // Python: on_message_send
    // returns dict(Task or Message) from "task_or_message"
    // ------------------------
    pub async fn on_message_send(
        &self,
        params: MessageSendParams,
        context: &ServerCallContext,
    ) -> Result<Value, RestErrorResponse> {
        let result = self
            .request_handler
            .on_message_send(params, Some(context))
            .await;

        match result {
            Ok(msr) => self.message_send_result_to_json(msr),
            Err(e) => Err(self.error_from_a2a(e)),
        }
    }

    // -----------------------------
    // Python: on_message_send_stream
    // @validate(streaming)
    // yields JSON per event
    // -----------------------------
    pub async fn on_message_send_stream(
        &self,
        params: MessageSendParams,
        context: &ServerCallContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, RestErrorResponse>> + Send>>, RestErrorResponse>
    {
        self.ensure_streaming_supported()?;

        let event_stream = self
            .request_handler
            .on_message_send_stream(params, Some(context))
            .await
            .map_err(|e| self.error_from_a2a(e))?;

        Ok(Box::pin(self.events_to_json_stream(event_stream)))
    }

    // ------------------------
    // Python: on_cancel_task
    // returns task dict or raises TaskNotFoundError
    // ------------------------
    pub async fn on_cancel_task(
        &self,
        params: TaskIdParams,
        context: &ServerCallContext,
    ) -> Result<Value, RestErrorResponse> {
        let result = self
            .request_handler
            .on_cancel_task(params, Some(context))
            .await;

        match result {
            Ok(Some(task)) => self.wrap_json(Ok(task)),
            Ok(None) => Err(self.task_not_found()),
            Err(e) => Err(self.error_from_a2a(e)),
        }
    }

    // -------------------------------
    // Python: on_resubscribe_to_task
    // @validate(streaming)
    // yields JSON per event
    // -------------------------------
    pub async fn on_resubscribe_to_task(
        &self,
        params: TaskIdParams,
        context: &ServerCallContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, RestErrorResponse>> + Send>>, RestErrorResponse>
    {
        self.ensure_streaming_supported()?;

        let event_stream = self
            .request_handler
            .on_resubscribe_to_task(params, Some(context))
            .await
            .map_err(|e| self.error_from_a2a(e))?;

        Ok(Box::pin(self.events_to_json_stream(event_stream)))
    }

    // ------------------------------------
    // Python: get_push_notification
    // NOTE: Python does NOT validate push_notifications capability here.
    // ------------------------------------
    pub async fn get_push_notification(
        &self,
        params: TaskPushNotificationConfigQueryParams,
        context: &ServerCallContext,
    ) -> Result<Value, RestErrorResponse> {
        let result = self
            .request_handler
            .on_get_task_push_notification_config(params, Some(context))
            .await;

        self.wrap_json(result)
    }

    // ------------------------------------
    // Python: set_push_notification
    // @validate(push_notifications)
    // ------------------------------------
    pub async fn set_push_notification(
        &self,
        params: TaskPushNotificationConfig,
        context: &ServerCallContext,
    ) -> Result<Value, RestErrorResponse> {
        self.ensure_push_supported()?;

        let result = self
            .request_handler
            .on_set_task_push_notification_config(params, Some(context))
            .await;

        self.wrap_json(result)
    }

    // ------------------------
    // Python: on_get_task
    // returns task dict or raises TaskNotFoundError
    // ------------------------
    pub async fn on_get_task(
        &self,
        params: TaskQueryParams,
        context: &ServerCallContext,
    ) -> Result<Value, RestErrorResponse> {
        let result = self
            .request_handler
            .on_get_task(params, Some(context))
            .await;

        match result {
            Ok(Some(task)) => self.wrap_json(Ok(task)),
            Ok(None) => Err(self.task_not_found()),
            Err(e) => Err(self.error_from_a2a(e)),
        }
    }

    // ------------------------
    // Python: list_push_notifications
    // raises NotImplementedError
    // ------------------------
    pub async fn list_push_notifications(
        &self,
        _params: TaskIdParams,
        _context: &ServerCallContext,
    ) -> Result<Value, RestErrorResponse> {
        Err(self.not_implemented("list notifications not implemented"))
    }

    // ------------------------
    // Python: list_tasks
    // raises NotImplementedError
    // ------------------------
    pub async fn list_tasks(&self, _context: &ServerCallContext) -> Result<Value, RestErrorResponse> {
        Err(self.not_implemented("list tasks not implemented"))
    }

    // ========================
    // Helpers (Python-equivalent)
    // ========================

    fn ensure_streaming_supported(&self) -> Result<(), RestErrorResponse> {
        if !self.agent_card.capabilities.streaming.unwrap_or(false) {
            let err = A2AError::unsupported_operation("Streaming is not supported by the agent");
            return Err(RestErrorResponse {
                code: err.code(),
                message: err.message().to_string(),
            });
        }
        Ok(())
    }

    fn ensure_push_supported(&self) -> Result<(), RestErrorResponse> {
        if !self.agent_card.capabilities.push_notifications.unwrap_or(false) {
            let err = A2AError::push_notification_not_supported();
            return Err(RestErrorResponse {
                code: err.code(),
                message: err.message().to_string(),
            });
        }
        Ok(())
    }

    fn task_not_found(&self) -> RestErrorResponse {
        // Match Python raising ServerError(error=TaskNotFoundError())
        let err = A2AError::TaskNotFound(TaskNotFoundError::default());
        self.error_from_a2a(err)
    }

    fn not_implemented(&self, msg: &str) -> RestErrorResponse {
        let err = A2AError::unsupported_operation(msg);
        RestErrorResponse {
            code: err.code(),
            message: err.message().to_string(),
        }
    }

    fn wrap_json<T: Serialize>(&self, result: Result<T, A2AError>) -> Result<Value, RestErrorResponse> {
        result
            .and_then(|val| {
                serde_json::to_value(&val)
                    .map_err(|e| A2AError::internal(&format!("Failed to serialize response: {}", e)))
            })
            .map_err(|err| self.error_from_a2a(err))
    }

    fn error_from_a2a(&self, err: A2AError) -> RestErrorResponse {
        RestErrorResponse {
            code: err.code(),
            message: err.message().to_string(),
        }
    }

    /// Convert MessageSendResult -> JSON.
    ///
    /// NOTE: This assumes MessageSendResult has variants `Task(Task)` and `Message(Message)`.
    /// If your enum uses different variant names, adjust the match arms accordingly.
    fn message_send_result_to_json(&self, msr: MessageSendResult) -> Result<Value, RestErrorResponse> {
        match msr {
            MessageSendResult::Task(task) => serde_json::to_value(task).map_err(|e| {
                let err = A2AError::internal(&format!("Failed to serialize Task: {}", e));
                self.error_from_a2a(err)
            }),
            MessageSendResult::Message(message) => serde_json::to_value(message).map_err(|e| {
                let err = A2AError::internal(&format!("Failed to serialize Message: {}", e));
                self.error_from_a2a(err)
            }),

            // 如果你们 MessageSendResult 还有其他分支（例如带 envelope 的 oneof），
            // 你可以在这里按 REST schema 包一层，比如：
            // MessageSendResult::Task(task) => Ok(json!({"task": task})),
            // MessageSendResult::Message(msg) => Ok(json!({"message": msg})),
            _ => Ok(json!({
                "error": "Unsupported MessageSendResult variant for REST serialization"
            })),
        }
    }

    /// Streaming: emit SSE events ("data: {json}\n\n")
    fn events_to_json_stream(
        &self,
        event_stream: Pin<Box<dyn Stream<Item = Result<Event, A2AError>> + Send>>,
    ) -> impl Stream<Item = Result<String, RestErrorResponse>> {
        event_stream.map(|event_result| match event_result {
            Ok(event) => {
                let result = match event {
                    Event::TaskStatusUpdate(update) => SendStreamingMessageResult::TaskStatusUpdateEvent(update),
                    Event::TaskArtifactUpdate(update) => SendStreamingMessageResult::TaskArtifactUpdateEvent(update),
                    Event::Message(message) => SendStreamingMessageResult::Message(message),
                    Event::Task(task) => SendStreamingMessageResult::Task(task),
                };

                let response = SendStreamingMessageResponse::success(None, result);

                match serde_json::to_string(&response) {
                    Ok(json_str) => Ok(format!("data: {}\n\n", json_str)),
                    Err(e) => {
                        let err = A2AError::internal(&format!(
                            "Failed to serialize streaming response to JSON: {}",
                            e
                        ));
                        Err(RestErrorResponse {
                            code: err.code(),
                            message: err.message().to_string(),
                        })
                    }
                }
            }
            Err(e) => Err(RestErrorResponse {
                code: e.code(),
                message: e.message().to_string(),
            }),
        })
    }
}
