//! JSON-RPC request handler implementation
//! 
//! This module provides the JSONRPCHandler which maps incoming JSON-RPC requests
//! to the appropriate request handler methods and formats responses.

use crate::a2a::models::*;
use crate::a2a::server::context::ServerCallContext;
use crate::a2a::server::request_handlers::RequestHandler;
use crate::a2a::jsonrpc::*;
use serde_json::Value;
use std::sync::Arc;
use futures::{Stream, StreamExt};
use std::pin::Pin;

/// JSON-RPC Handler
/// 
/// Maps incoming JSON-RPC requests to the appropriate request handler methods
/// and formats responses according to the A2A specification.
pub struct JSONRPCHandler {
    agent_card: AgentCard,
    #[allow(dead_code)]
    request_handler: Arc<dyn RequestHandler>,
}

impl JSONRPCHandler {
    /// Create a new JSON-RPC handler
    /// 
    /// # Arguments
    /// * `agent_card` - The AgentCard describing the agent's capabilities
    /// * `request_handler` - The underlying request handler to delegate requests to
    pub fn new(
        agent_card: AgentCard,
        request_handler: Arc<dyn RequestHandler>,
    ) -> Self {
        Self {
            agent_card,
            request_handler,
        }
    }

    /// Convert JSONRPCId to serde_json::Value
    fn id_to_value(id: &Option<crate::a2a::jsonrpc::JSONRPCId>) -> Value {
        match id {
            Some(crate::a2a::jsonrpc::JSONRPCId::String(s)) => Value::String(s.clone()),
            Some(crate::a2a::jsonrpc::JSONRPCId::Number(n)) => Value::Number(serde_json::Number::from(*n)),
            Some(crate::a2a::jsonrpc::JSONRPCId::Null) => Value::Null,
            None => Value::Null,
        }
    }

    /// Handle a JSON-RPC request
    /// 
    /// # Arguments
    /// * `request` - The JSON-RPC request as a serde_json::Value
    /// * `context` - The server call context
    /// 
    /// # Returns
    /// The JSON-RPC response as a serde_json::Value
    pub async fn handle_request(
        &self,
        request: Value,
        context: &ServerCallContext,
    ) -> Result<Value, JSONRPCError> {
        // Parse the JSON-RPC request
        let jsonrpc_request = self.parse_request(request)?;
        
        // Route based on method
        match jsonrpc_request.method.as_str() {
            "message/send" => self.handle_message_send(jsonrpc_request, context).await,
            "message/stream" => self.handle_message_stream(jsonrpc_request, context).await,
            "tasks/get" => self.handle_get_task(jsonrpc_request, context).await,
            "tasks/cancel" => self.handle_cancel_task(jsonrpc_request, context).await,
            "tasks/pushNotificationConfig/set" => self.handle_set_push_notification_config(jsonrpc_request, context).await,
            "tasks/pushNotificationConfig/get" => self.handle_get_push_notification_config(jsonrpc_request, context).await,
            "tasks/pushNotificationConfig/list" => self.handle_list_push_notification_config(jsonrpc_request, context).await,
            "tasks/pushNotificationConfig/delete" => self.handle_delete_push_notification_config(jsonrpc_request, context).await,
            "tasks/resubscribe" => self.handle_resubscribe_task(jsonrpc_request, context).await,
            "agent/authenticatedExtendedCard" => self.handle_get_authenticated_extended_card(jsonrpc_request, context).await,
            _ => Err(JSONRPCError::new(
                standard_error_codes::METHOD_NOT_FOUND,
                format!("Method '{}' not found", jsonrpc_request.method),
            )),
        }
    }

    /// Parse a JSON-RPC request
    pub fn parse_request(&self, request: Value) -> Result<JSONRPCRequest, JSONRPCError> {
        // Check for required JSON-RPC 2.0 fields
        if !request.get("jsonrpc").and_then(|v| v.as_str()).map(|s| s == "2.0").unwrap_or(false) {
            return Err(JSONRPCError::new(
                standard_error_codes::INVALID_REQUEST,
                "Invalid or missing 'jsonrpc' version field".to_string(),
            ));
        }

        let method = request
            .get("method")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                JSONRPCError::new(
                    standard_error_codes::INVALID_REQUEST,
                    "Missing 'method' field".to_string(),
                )
            })?
            .to_string();

        let params = request.get("params").cloned().unwrap_or(Value::Null);
        let id = request.get("id").cloned();

        Ok(JSONRPCRequest {
            jsonrpc: "2.0".to_string(),
            method,
            params: Some(params),
            id: id.and_then(|id| {
                match id {
                    Value::String(s) => Some(crate::a2a::jsonrpc::JSONRPCId::String(s)),
                    Value::Number(n) => n.as_i64().map(crate::a2a::jsonrpc::JSONRPCId::Number),
                    Value::Null => Some(crate::a2a::jsonrpc::JSONRPCId::Null),
                    _ => None,
                }
            }),
        })
    }

    /// Handle message/send requests
    async fn handle_message_send(
        &self,
        request: JSONRPCRequest,
        context: &ServerCallContext,
    ) -> Result<Value, JSONRPCError> {
        // Parse the params
        let params = request.params.as_ref().ok_or_else(|| {
            JSONRPCError::new(
                standard_error_codes::INVALID_PARAMS,
                "Missing params field".to_string(),
            )
        })?;

        // Deserialize MessageSendParams
        let message_send_params: MessageSendParams = serde_json::from_value(params.clone())
            .map_err(|e| {
                JSONRPCError::new(
                    standard_error_codes::INVALID_PARAMS,
                    format!("Invalid params: {}", e),
                )
            })?;

        // Call the request handler
        let result = self.request_handler
            .on_message_send(message_send_params, Some(context))
            .await
            .map_err(|e| {
                JSONRPCError::new(
                    standard_error_codes::INTERNAL_ERROR,
                    format!("Handler error: {}", e),
                )
            })?;

        // Convert the result to the expected format
        let result_value = match result {
            crate::a2a::server::request_handlers::request_handler::MessageSendResult::Task(task) => {
                serde_json::to_value(task).map_err(|e| {
                    JSONRPCError::new(
                        standard_error_codes::INTERNAL_ERROR,
                        format!("Failed to serialize task: {}", e),
                    )
                })?
            }
            crate::a2a::server::request_handlers::request_handler::MessageSendResult::Message(message) => {
                serde_json::to_value(message).map_err(|e| {
                    JSONRPCError::new(
                        standard_error_codes::INTERNAL_ERROR,
                        format!("Failed to serialize message: {}", e),
                    )
                })?
            }
        };

        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "result": result_value,
            "id": Self::id_to_value(&request.id)
        });
        Ok(response)
    }

    /// Handle message/stream requests
    async fn handle_message_stream(
        &self,
        request: JSONRPCRequest,
        context: &ServerCallContext,
    ) -> Result<Value, JSONRPCError> {
        // Check if streaming is supported
        if !self.agent_card.capabilities.streaming.unwrap_or(false) {
            return Err(JSONRPCError::new(
                standard_error_codes::INVALID_REQUEST,
                "Streaming is not supported by this agent".to_string(),
            ));
        }

        // Parse the params
        let params = request.params.as_ref().ok_or_else(|| {
            JSONRPCError::new(
                standard_error_codes::INVALID_PARAMS,
                "Missing params field".to_string(),
            )
        })?;

        // Deserialize MessageSendParams
        let message_send_params: MessageSendParams = serde_json::from_value(params.clone())
            .map_err(|e| {
                JSONRPCError::new(
                    standard_error_codes::INVALID_PARAMS,
                    format!("Invalid params: {}", e),
                )
            })?;

        // Call the request handler's streaming method
        let event_stream = self.request_handler
            .on_message_send_stream(message_send_params, Some(context))
            .await
            .map_err(|e| {
                JSONRPCError::new(
                    standard_error_codes::INTERNAL_ERROR,
                    format!("Handler error: {}", e),
                )
            })?;

        // Convert the event stream to SSE format and return as JSON-RPC response
        // This is a simplified implementation that converts the stream to a JSON array
        // In a real web framework, this should be handled as proper SSE streaming
        let events = self.collect_events_from_stream(event_stream).await?;
        
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "result": {
                "events": events,
                "stream": "completed"
            },
            "id": Self::id_to_value(&request.id)
        });
        
        Ok(response)
    }

    /// Handle message/stream requests with proper SSE stream
    /// This method returns a stream that can be used for Server-Sent Events
    pub async fn handle_message_stream_sse(
        &self,
        request: JSONRPCRequest,
        context: &ServerCallContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, JSONRPCError>> + Send>>, JSONRPCError> {
        // Check if streaming is supported
        if !self.agent_card.capabilities.streaming.unwrap_or(false) {
            return Err(JSONRPCError::new(
                standard_error_codes::INVALID_REQUEST,
                "Streaming is not supported by this agent".to_string(),
            ));
        }

        // Parse the params
        let params = request.params.as_ref().ok_or_else(|| {
            JSONRPCError::new(
                standard_error_codes::INVALID_PARAMS,
                "Missing params field".to_string(),
            )
        })?;

        // Deserialize MessageSendParams
        let message_send_params: MessageSendParams = serde_json::from_value(params.clone())
            .map_err(|e| {
                JSONRPCError::new(
                    standard_error_codes::INVALID_PARAMS,
                    format!("Invalid params: {}", e),
                )
            })?;

        // Call the request handler's streaming method
        let event_stream = self.request_handler
            .on_message_send_stream(message_send_params, Some(context))
            .await
            .map_err(|e| {
                JSONRPCError::new(
                    standard_error_codes::INTERNAL_ERROR,
                    format!("Handler error: {}", e),
                )
            })?;

        // Get the request ID as serde_json::Value
        let request_id = request.id.as_ref().map(|id| {
            match id {
                crate::a2a::jsonrpc::JSONRPCId::String(s) => Value::String(s.clone()),
                crate::a2a::jsonrpc::JSONRPCId::Number(n) => Value::Number(serde_json::Number::from(*n)),
                crate::a2a::jsonrpc::JSONRPCId::Null => Value::Null,
            }
        });

        // Convert the event stream to SSE format
        Ok(Box::pin(self.events_to_sse_stream(event_stream, request_id)))
    }

    /// Collect events from a stream into a JSON array
    /// This is a helper method for the non-streaming implementation
    async fn collect_events_from_stream(
        &self,
        mut event_stream: Pin<Box<dyn Stream<Item = Result<crate::a2a::server::request_handlers::request_handler::Event, crate::a2a::error::A2AError>> + Send>>,
    ) -> Result<Vec<Value>, JSONRPCError> {
        let mut events = Vec::new();
        
        while let Some(event_result) = event_stream.next().await {
            match event_result {
                Ok(event) => {
                    let event_value = match event {
                        crate::a2a::server::request_handlers::request_handler::Event::TaskStatusUpdate(update) => {
                            serde_json::to_value(update).map_err(|e| {
                                JSONRPCError::new(
                                    standard_error_codes::INTERNAL_ERROR,
                                    format!("Failed to serialize task status update: {}", e),
                                )
                            })?
                        }
                        crate::a2a::server::request_handlers::request_handler::Event::TaskArtifactUpdate(update) => {
                            serde_json::to_value(update).map_err(|e| {
                                JSONRPCError::new(
                                    standard_error_codes::INTERNAL_ERROR,
                                    format!("Failed to serialize task artifact update: {}", e),
                                )
                            })?
                        }
                        crate::a2a::server::request_handlers::request_handler::Event::Message(message) => {
                            serde_json::to_value(message).map_err(|e| {
                                JSONRPCError::new(
                                    standard_error_codes::INTERNAL_ERROR,
                                    format!("Failed to serialize message: {}", e),
                                )
                            })?
                        }
                        crate::a2a::server::request_handlers::request_handler::Event::Task(task) => {
                            serde_json::to_value(task).map_err(|e| {
                                JSONRPCError::new(
                                    standard_error_codes::INTERNAL_ERROR,
                                    format!("Failed to serialize task: {}", e),
                                )
                            })?
                        }
                    };
                    events.push(event_value);
                }
                Err(e) => {
                    return Err(JSONRPCError::new(
                        standard_error_codes::INTERNAL_ERROR,
                        format!("Event stream error: {}", e),
                    ));
                }
            }
        }
        
        Ok(events)
    }

    /// Convert events to SSE (Server-Sent Events) format stream
    fn events_to_sse_stream(
        &self,
        event_stream: Pin<Box<dyn Stream<Item = Result<crate::a2a::server::request_handlers::request_handler::Event, crate::a2a::error::A2AError>> + Send>>,
        request_id: Option<serde_json::Value>,
    ) -> impl Stream<Item = Result<String, crate::a2a::jsonrpc::JSONRPCError>> {
        event_stream.map(move |event_result| {
            match event_result {
                Ok(event) => {
                    // Convert the event to SendStreamingMessageResult
                    let result = match event {
                        crate::a2a::server::request_handlers::request_handler::Event::TaskStatusUpdate(update) => {
                            crate::a2a::models::SendStreamingMessageResult::TaskStatusUpdateEvent(update)
                        }
                        crate::a2a::server::request_handlers::request_handler::Event::TaskArtifactUpdate(update) => {
                            crate::a2a::models::SendStreamingMessageResult::TaskArtifactUpdateEvent(update)
                        }
                        crate::a2a::server::request_handlers::request_handler::Event::Message(message) => {
                            crate::a2a::models::SendStreamingMessageResult::Message(message)
                        }
                        crate::a2a::server::request_handlers::request_handler::Event::Task(task) => {
                            crate::a2a::models::SendStreamingMessageResult::Task(task)
                        }
                    };

                    // Create the streaming response
                    let response = crate::a2a::models::SendStreamingMessageResponse::success(
                        request_id.clone(),
                        result,
                    );
                    
                    match serde_json::to_value(&response) {
                        Ok(json) => {
                            // Format as SSE: data: {json}\n\n
                            Ok(format!("data: {}\n\n", json.to_string()))
                        }
                        Err(e) => Err(crate::a2a::jsonrpc::JSONRPCError::new(
                            standard_error_codes::INTERNAL_ERROR,
                            format!("Failed to serialize streaming response to JSON: {}", e),
                        )),
                    }
                }
                Err(e) => Err(crate::a2a::jsonrpc::JSONRPCError::new(
                    standard_error_codes::INTERNAL_ERROR,
                    format!("Event stream error: {}", e),
                )),
            }
        })
    }

    /// Handle tasks/get requests
    async fn handle_get_task(
        &self,
        request: JSONRPCRequest,
        _context: &ServerCallContext,
    ) -> Result<Value, JSONRPCError> {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "result": "tasks/get handled",
            "id": Self::id_to_value(&request.id)
        });
        Ok(response)
    }

    /// Handle tasks/cancel requests
    async fn handle_cancel_task(
        &self,
        request: JSONRPCRequest,
        _context: &ServerCallContext,
    ) -> Result<Value, JSONRPCError> {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "result": "tasks/cancel handled",
            "id": Self::id_to_value(&request.id)
        });
        Ok(response)
    }

    /// Handle tasks/pushNotificationConfig/set requests
    async fn handle_set_push_notification_config(
        &self,
        request: JSONRPCRequest,
        _context: &ServerCallContext,
    ) -> Result<Value, JSONRPCError> {
        // Check if push notifications are supported
        if !self.agent_card.capabilities.push_notifications.unwrap_or(false) {
            return Err(JSONRPCError::new(
                standard_error_codes::INVALID_REQUEST,
                "Push notifications are not supported by this agent".to_string(),
            ));
        }

        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "result": "tasks/pushNotificationConfig/set handled",
            "id": Self::id_to_value(&request.id)
        });
        Ok(response)
    }

    /// Handle tasks/pushNotificationConfig/get requests
    async fn handle_get_push_notification_config(
        &self,
        request: JSONRPCRequest,
        _context: &ServerCallContext,
    ) -> Result<Value, JSONRPCError> {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "result": "tasks/pushNotificationConfig/get handled",
            "id": Self::id_to_value(&request.id)
        });
        Ok(response)
    }

    /// Handle tasks/pushNotificationConfig/list requests
    async fn handle_list_push_notification_config(
        &self,
        request: JSONRPCRequest,
        _context: &ServerCallContext,
    ) -> Result<Value, JSONRPCError> {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "result": "tasks/pushNotificationConfig/list handled",
            "id": Self::id_to_value(&request.id)
        });
        Ok(response)
    }

    /// Handle tasks/pushNotificationConfig/delete requests
    async fn handle_delete_push_notification_config(
        &self,
        request: JSONRPCRequest,
        _context: &ServerCallContext,
    ) -> Result<Value, JSONRPCError> {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "result": "tasks/pushNotificationConfig/delete handled",
            "id": Self::id_to_value(&request.id)
        });
        Ok(response)
    }

    /// Handle tasks/resubscribe requests
    async fn handle_resubscribe_task(
        &self,
        request: JSONRPCRequest,
        _context: &ServerCallContext,
    ) -> Result<Value, JSONRPCError> {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "result": "tasks/resubscribe handled",
            "id": Self::id_to_value(&request.id)
        });
        Ok(response)
    }

    /// Handle agent/authenticatedExtendedCard requests
    async fn handle_get_authenticated_extended_card(
        &self,
        request: JSONRPCRequest,
        _context: &ServerCallContext,
    ) -> Result<Value, JSONRPCError> {
        // Check if authenticated extended card is supported
        if !self.agent_card.supports_authenticated_extended_card.unwrap_or(false) {
            return Err(JSONRPCError::new(
                standard_error_codes::INVALID_REQUEST,
                "Authenticated extended card is not supported by this agent".to_string(),
            ));
        }

        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "result": "agent/authenticatedExtendedCard handled",
            "id": Self::id_to_value(&request.id)
        });
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a2a::server::request_handlers::request_handler::MockRequestHandler;
    

    #[tokio::test]
    async fn test_parse_valid_request() {
        let handler = create_test_handler();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "test",
            "params": {},
            "id": 1
        });

        let result = handler.parse_request(request).unwrap();
        assert_eq!(result.method, "test");
        assert_eq!(result.jsonrpc, "2.0");
    }

    #[tokio::test]
    async fn test_parse_invalid_request_missing_jsonrpc() {
        let handler = create_test_handler();
        let request = serde_json::json!({
            "method": "test",
            "params": {},
            "id": 1
        });

        let result = handler.parse_request(request);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parse_invalid_request_missing_method() {
        let handler = create_test_handler();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "params": {},
            "id": 1
        });

        let result = handler.parse_request(request);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_unknown_method() {
        let handler = create_test_handler();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "unknown_method",
            "params": {},
            "id": 1
        });

        let context = ServerCallContext::new();
        let result = handler.handle_request(request, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_message_send() {
        let handler = create_test_handler();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "message/send",
            "params": {
                "message": {
                    "kind": "message",
                    "messageId": "test-msg-123",
                    "role": "user",
                    "parts": [
                        {
                            "kind": "text",
                            "text": "Hello, world!"
                        }
                    ]
                }
            },
            "id": 1
        });

        let context = ServerCallContext::new();
        let result = handler.handle_request(request, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_message_stream() {
        let agent_card = AgentCard::new(
            "Test Agent".to_string(),
            "A test agent".to_string(),
            "http://localhost:8080".to_string(),
            "1.0.0".to_string(),
            vec!["text/plain".to_string()],
            vec!["text/plain".to_string()],
            AgentCapabilities::new().with_streaming(true),
            vec![],
        );

        let request_handler = Arc::new(MockRequestHandler::new());
        let handler = JSONRPCHandler::new(agent_card, request_handler);

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "message/stream",
            "params": {
                "message": {
                    "kind": "message",
                    "messageId": "test-msg-123",
                    "role": "user",
                    "parts": [
                        {
                            "kind": "text",
                            "text": "Hello, streaming!"
                        }
                    ]
                }
            },
            "id": 1
        });

        let context = ServerCallContext::new();
        let result = handler.handle_request(request, &context).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        let result_obj = response.get("result").unwrap();
        let events = result_obj.get("events").unwrap().as_array().unwrap();
        
        // Should have 3 events: working status, message response, completed status
        assert_eq!(events.len(), 3);
        
        // Check that the first event is a task status update
        let first_event = &events[0];
        assert_eq!(first_event.get("kind").unwrap().as_str().unwrap(), "status-update");
        assert_eq!(first_event.get("final").unwrap().as_bool().unwrap(), false);
        
        // Check that the second event is a message
        let second_event = &events[1];
        assert_eq!(second_event.get("kind").unwrap().as_str().unwrap(), "message");
        assert_eq!(second_event.get("role").unwrap().as_str().unwrap(), "agent");
        
        // Check that the third event is a completed task status update
        let third_event = &events[2];
        assert_eq!(third_event.get("kind").unwrap().as_str().unwrap(), "status-update");
        assert_eq!(third_event.get("final").unwrap().as_bool().unwrap(), true);
    }

    #[tokio::test]
    async fn test_handle_message_stream_not_supported() {
        let agent_card = AgentCard::new(
            "Test Agent".to_string(),
            "A test agent".to_string(),
            "http://localhost:8080".to_string(),
            "1.0.0".to_string(),
            vec!["text/plain".to_string()],
            vec!["text/plain".to_string()],
            AgentCapabilities::new().with_streaming(false), // Streaming disabled
            vec![],
        );

        let request_handler = Arc::new(MockRequestHandler::new());
        let handler = JSONRPCHandler::new(agent_card, request_handler);

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "message/stream",
            "params": {
                "message": {
                    "kind": "message",
                    "messageId": "test-msg-123",
                    "role": "user",
                    "parts": []
                }
            },
            "id": 1
        });

        let context = ServerCallContext::new();
        let result = handler.handle_request(request, &context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(error.code, -32600); // INVALID_REQUEST
        assert!(error.message.contains("Streaming is not supported"));
    }

    fn create_test_handler() -> JSONRPCHandler {
        let agent_card = AgentCard::new(
            "Test Agent".to_string(),
            "A test agent".to_string(),
            "http://localhost:8080".to_string(),
            "1.0.0".to_string(),
            vec!["text/plain".to_string()],
            vec!["text/plain".to_string()],
            AgentCapabilities::new(),
            vec![],
        );

        let request_handler = Arc::new(MockRequestHandler::new());
        JSONRPCHandler::new(agent_card, request_handler)
    }
}
