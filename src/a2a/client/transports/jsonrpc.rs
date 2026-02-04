//! JSON-RPC transport implementation for A2A Rust client
//! 
//! This module provides a JSON-RPC transport that mirrors the functionality
//! of a2a-python's JsonRpcTransport.

use crate::a2a::client::client_trait::{ClientCallContext, ClientTransport, ClientEvent, ClientCallInterceptor};
use crate::a2a::client::card_resolver::A2ACardResolver;
use crate::a2a::models::*;
use crate::a2a::core_types::*;
use crate::a2a::error::A2AError;
use crate::a2a::jsonrpc::{JSONRPCResponse, JSONRPCError, JSONRPCSuccessResponse, JSONRPCErrorResponse};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::time::Duration;

/// Create a JSON-RPC 2.0 request
fn create_jsonrpc_request(method: &str, params: Value) -> Result<Value, A2AError> {
    Ok(serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": uuid::Uuid::new_v4().to_string()
    }))
}

/// Parse a JSON-RPC response
fn parse_jsonrpc_response(value: Value) -> Result<JSONRPCResponse, A2AError> {
    if let Some(error) = value.get("error") {
        let error: JSONRPCError = serde_json::from_value(error.clone())
            .map_err(|e| A2AError::json_error(format!("Failed to parse JSON-RPC error: {}", e)))?;
        Ok(JSONRPCResponse::Error(JSONRPCErrorResponse {
            id: None,
            jsonrpc: "2.0".to_string(),
            error,
        }))
    } else if let Some(result) = value.get("result") {
        Ok(JSONRPCResponse::Success(JSONRPCSuccessResponse {
            result: result.clone(),
            id: None,
            jsonrpc: "2.0".to_string(),
        }))
    } else {
        Err(A2AError::json_error("Invalid JSON-RPC response: missing result or error".to_string()))
    }
}

/// JSON-RPC transport for A2A client
/// 
/// This transport communicates with A2A agents using JSON-RPC 2.0 over HTTP/HTTPS
/// and supports both regular requests and SSE-based streaming.
pub struct JsonRpcTransport {
    /// The URL endpoint for the agent
    url: String,
    
    /// HTTP client for making requests
    client: reqwest::Client,
    
    /// Agent card (optional)
    agent_card: Option<AgentCard>,
    
    /// List of interceptors for requests
    interceptors: Vec<Box<dyn ClientCallInterceptor>>,
    
    /// Extensions to include in requests
    extensions: Vec<String>,
    
    /// Whether we need to fetch the extended card
    needs_extended_card: bool,
}

impl JsonRpcTransport {
    /// Create a new JSON-RPC transport
    pub fn new(
        url: String,
        agent_card: Option<AgentCard>,
    ) -> Result<Self, A2AError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| A2AError::transport_error(format!("Failed to create HTTP client: {}", e)))?;
        
        let needs_extended_card = agent_card
            .as_ref()
            .map(|card| card.supports_authenticated_extended_card.unwrap_or(false))
            .unwrap_or(true);
        
        Ok(Self {
            url,
            client,
            agent_card,
            interceptors: Vec::new(),
            extensions: Vec::new(),
            needs_extended_card,
        })
    }
    
    /// Create a new JSON-RPC transport with custom configuration
    pub fn new_with_config(
        url: String,
        agent_card: Option<AgentCard>,
        config: crate::a2a::client::config::ClientConfig,
    ) -> Result<Self, A2AError> {
        // Use the timeout from config, or default to 30 seconds
        let timeout_duration = config.timeout.unwrap_or(Duration::from_secs(30));
        
        let client = reqwest::Client::builder()
            .timeout(timeout_duration)
            .build()
            .map_err(|e| A2AError::transport_error(format!("Failed to create HTTP client: {}", e)))?;
        
        let needs_extended_card = agent_card
            .as_ref()
            .map(|card| card.supports_authenticated_extended_card.unwrap_or(false))
            .unwrap_or(true);
        
        Ok(Self {
            url,
            client,
            agent_card,
            interceptors: Vec::new(),
            extensions: config.extensions,
            needs_extended_card,
        })
    }
    
    /// Create a transport with custom HTTP client
    pub fn with_client(
        url: String,
        client: reqwest::Client,
        agent_card: Option<AgentCard>,
    ) -> Self {
        let needs_extended_card = agent_card
            .as_ref()
            .map(|card| card.supports_authenticated_extended_card.unwrap_or(false))
            .unwrap_or(true);
        
        Self {
            url,
            client,
            agent_card,
            interceptors: Vec::new(),
            extensions: Vec::new(),
            needs_extended_card,
        }
    }
    
    /// Add interceptors to the transport
    pub fn with_interceptors(mut self, interceptors: Vec<Box<dyn ClientCallInterceptor>>) -> Self {
        self.interceptors = interceptors;
        self
    }
    
    /// Set extensions for the transport
    pub fn with_extensions(mut self, extensions: Vec<String>) -> Self {
        self.extensions = extensions;
        self
    }
    
    /// Apply interceptors to a request
    async fn apply_interceptors(
        &self,
        method_name: &str,
        mut request_payload: Value,
        mut http_kwargs: HashMap<String, Value>,
        context: Option<&ClientCallContext>,
    ) -> Result<(Value, HashMap<String, Value>), A2AError> {
        // Extract agent card for interceptors
        let agent_card = self.agent_card.as_ref()
            .ok_or_else(|| A2AError::invalid_request("No agent card available for interceptors"))?;
        
        for interceptor in &self.interceptors {
            let (new_payload, new_kwargs) = interceptor.intercept(
                method_name,
                request_payload,
                http_kwargs,
                agent_card,
                context,
            ).await?;
            request_payload = new_payload;
            http_kwargs = new_kwargs;
        }
        
        Ok((request_payload, http_kwargs))
    }
    
    /// Build HTTP headers for a request
    fn build_headers(&self, extensions: Option<&Vec<String>>, http_kwargs: &HashMap<String, Value>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        
        // Default headers
        headers.insert("Content-Type", "application/json".parse().unwrap());
        headers.insert("Accept", "application/json".parse().unwrap());
        
        // Add extension header if needed
        let extension_list = extensions.unwrap_or(&self.extensions);
        if !extension_list.is_empty() {
            let extension_header = extension_list.join(",");
            headers.insert("A2A-Extensions", extension_header.parse().unwrap());
        }
        
        // Add custom headers from http_kwargs
        if let Some(headers_map) = http_kwargs.get("headers").and_then(|v| v.as_object()) {
            for (key, value) in headers_map {
                if let Some(value_str) = value.as_str() {
                    if let Ok(header_name) = HeaderName::from_bytes(key.as_bytes()) {
                        if let Ok(header_value) = HeaderValue::from_str(value_str) {
                            headers.insert(header_name, header_value);
                        }
                    }
                }
            }
        }
        
        headers
    }
    
    /// Send a JSON-RPC request and get the response
    async fn send_jsonrpc_request(
        &self,
        method: &str,
        params: Value,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Value, A2AError> {
        let request = create_jsonrpc_request(method, params)?;
        
        // Get HTTP args from context
        let http_kwargs = context
            .and_then(|ctx| ctx.http_kwargs.get("http_kwargs"))
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| Some((k.clone(), v.clone())))
                    .collect()
            })
            .unwrap_or_default();
        
        // Apply interceptors
        let (payload, mut http_kwargs) = self.apply_interceptors(method, request, http_kwargs, context).await?;
        
        // Build headers
        let headers = self.build_headers(extensions.as_ref(), &http_kwargs);
        
        // Remove headers from http_kwargs since they're handled separately
        http_kwargs.remove("headers");
        
        // Extract request options
        let timeout = http_kwargs.get("timeout")
            .and_then(|v| v.as_u64())
            .map(Duration::from_secs);
        
        // Build request
        let mut request_builder = self.client.post(&self.url).headers(headers).json(&payload);
        
        if let Some(timeout_duration) = timeout {
            request_builder = request_builder.timeout(timeout_duration);
        }
        
        // Send request
        let response = request_builder
            .send()
            .await
            .map_err(|e| A2AError::transport_error(format!("HTTP request failed: {}", e)))?;
        
        // Check response status
        if !response.status().is_success() {
            return Err(A2AError::http_error(
                response.status().as_u16(),
                format!("HTTP error: {}", response.status()),
            ));
        }
        
        // Parse response
        let response_value: Value = response
            .json()
            .await
            .map_err(|e| A2AError::json_error(format!("Failed to parse JSON response: {}", e)))?;
        
        // Parse JSON-RPC response
        let jsonrpc_response = parse_jsonrpc_response(response_value)?;
        
        match jsonrpc_response {
            JSONRPCResponse::Success(success_response) => Ok(success_response.result),
            JSONRPCResponse::Error(error_response) => {
                Err(A2AError::jsonrpc_error(error_response.error.code, error_response.error.message))
            }
        }
    }
    
    /// Send a streaming JSON-RPC request with SSE support
    async fn send_streaming_request(
        &self,
        method: &str,
        params: Value,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<TaskOrMessage, A2AError>> + Send + '_>>, A2AError> {
        let request = create_jsonrpc_request(method, params)?;
        
        // Get HTTP args from context
        let http_kwargs = context
            .and_then(|ctx| ctx.http_kwargs.get("http_kwargs"))
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| Some((k.clone(), v.clone())))
                    .collect()
            })
            .unwrap_or_default();
        
        // Apply interceptors
        let (payload, mut http_kwargs) = self.apply_interceptors(method, request, http_kwargs, context).await?;
        
        // Build headers for SSE
        let mut headers = self.build_headers(extensions.as_ref(), &http_kwargs);
        
        // Override Accept header for SSE
        headers.insert("Accept", "text/event-stream".parse().unwrap());
        
        // Remove headers from http_kwargs since they're handled separately
        http_kwargs.remove("headers");
        
        // Extract request options
        let timeout = http_kwargs.get("timeout")
            .and_then(|v| v.as_u64())
            .map(Duration::from_secs);
        
        // Send the streaming POST request
        let mut request_builder = self.client.post(&self.url).headers(headers).json(&payload);
        
        if let Some(timeout_duration) = timeout {
            request_builder = request_builder.timeout(timeout_duration);
        }
        
        let response = request_builder
            .send()
            .await
            .map_err(|e| A2AError::transport_error(format!("HTTP request failed: {}", e)))?;
        
        // Check response status
        if !response.status().is_success() {
            return Err(A2AError::http_error(
                response.status().as_u16(),
                format!("HTTP error: {}", response.status()),
            ));
        }
        
        // Check if response is SSE
        let content_type = response.headers().get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        
        if !content_type.contains("text/event-stream") {
            // If not SSE, fallback to regular JSON response
            let response_value: Value = response
                .json()
                .await
                .map_err(|e| A2AError::json_error(format!("Failed to parse JSON response: {}", e)))?;
            
            let jsonrpc_response = parse_jsonrpc_response(response_value)?;
            
            let result = match jsonrpc_response {
                JSONRPCResponse::Success(success_response) => {
                    // Try to parse the result as TaskOrMessage
                    if let Ok(task_or_message) = serde_json::from_value::<TaskOrMessage>(success_response.result.clone()) {
                        task_or_message
                    } else if let Ok(task) = serde_json::from_value::<Task>(success_response.result.clone()) {
                        TaskOrMessage::Task(task)
                    } else if let Ok(message) = serde_json::from_value::<Message>(success_response.result) {
                        TaskOrMessage::Message(message)
                    } else {
                        return Err(A2AError::json_error("Failed to parse response as Task or Message".to_string()));
                    }
                }
                JSONRPCResponse::Error(error_response) => {
                    return Err(A2AError::jsonrpc_error(error_response.error.code, error_response.error.message));
                }
            };
            
            // Return a single-item stream for non-streaming response
            let single_item_stream = async_stream::stream! {
                yield Ok(result);
            };
            
            return Ok(Box::pin(single_item_stream));
        }
        
        // Handle SSE response using a proper async stream
        let byte_stream = response.bytes_stream();
        let stream = async_stream::stream! {
            let mut buffer = String::new();
            use futures::StreamExt;
            
            futures::pin_mut!(byte_stream);
            
            while let Some(chunk_result) = byte_stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        let chunk_str = String::from_utf8_lossy(&chunk);
                        buffer.push_str(&chunk_str);
                        
                        // Process complete SSE messages
                        while let Some(double_newline_pos) = buffer.find("\n\n") {
                            let message_end = double_newline_pos;
                            let message = &buffer[..message_end];
                            let remaining_buffer = buffer[message_end + 2..].to_string();
                            
                            if !message.trim().is_empty() {
                                match self.parse_sse_message(message.trim()) {
                                    Ok(Some(task_or_message)) => {
                                        yield Ok(task_or_message);
                                    }
                                    Ok(None) => {
                                        // Continue, this might be a comment or empty event
                                    }
                                    Err(e) => {
                                        yield Err(e);
                                    }
                                }
                            }
                            
                            // Update buffer with remaining content
                            buffer = remaining_buffer;
                        }
                    }
                    Err(e) => {
                        yield Err(A2AError::transport_error(format!("Stream error: {}", e)));
                        break;
                    }
                }
            }
            
            // Process any remaining content in buffer
            if !buffer.trim().is_empty() {
                match self.parse_sse_message(buffer.trim()) {
                    Ok(Some(task_or_message)) => {
                        yield Ok(task_or_message);
                    }
                    Ok(None) => {
                        // Ignore final empty content
                    }
                    Err(e) => {
                        yield Err(e);
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
    
    /// Parse a single SSE message and convert to TaskOrMessage
    fn parse_sse_message(&self, message: &str) -> Result<Option<TaskOrMessage>, A2AError> {
        let mut data_lines = Vec::new();
        let mut _event_type = None;
        
        // Parse SSE fields
        for line in message.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(':') {
                // Skip empty lines and comments
                continue;
            }
            if line.starts_with("data:") {
                // Handle both "data:" and "data: " formats
                if line.len() > 5 {
                    let data_content = line[5..].trim_start();
                    data_lines.push(data_content);
                } else {
                    data_lines.push("");
                }
            } else if line.starts_with("event:") {
                let event_content = line[6..].trim_start();
                _event_type = Some(event_content);
            }
        }
        
        if data_lines.is_empty() {
            return Ok(None);
        }
        
        // Combine data lines (SSE spec says to join with newline)
        let data = data_lines.join("\n");
        
        // Skip empty data
        if data.trim().is_empty() {
            return Ok(None);
        }
        
        // Parse JSON data
        let json_value: Value = serde_json::from_str(&data)
            .map_err(|e| A2AError::json_error(format!("Failed to parse SSE data as JSON: {} (data: {})", e, data)))?;
        
        // Check if this is a JSON-RPC streaming response
        if let Some(result) = json_value.get("result") {
            // Try to parse as SendStreamingMessageResult
            if let Ok(streaming_result) = serde_json::from_value::<SendStreamingMessageResult>(result.clone()) {
                return Ok(Some(self.convert_streaming_result(streaming_result)?));
            }
        }
        
        // Try to parse directly as TaskOrMessage
        if let Ok(task_or_message) = serde_json::from_value::<TaskOrMessage>(json_value.clone()) {
            return Ok(Some(task_or_message));
        }
        
        // Try to parse as Task
        if let Ok(task) = serde_json::from_value::<Task>(json_value.clone()) {
            return Ok(Some(TaskOrMessage::Task(task)));
        }
        
        // Try to parse as Message
        if let Ok(message) = serde_json::from_value::<Message>(json_value.clone()) {
            return Ok(Some(TaskOrMessage::Message(message)));
        }
        
        // Try to parse as TaskStatusUpdateEvent
        if let Ok(task_update) = serde_json::from_value::<TaskStatusUpdateEvent>(json_value.clone()) {
            return Ok(Some(TaskOrMessage::TaskUpdate(task_update)));
        }
        
        // Try to parse as TaskArtifactUpdateEvent
        if let Ok(artifact_update) = serde_json::from_value::<TaskArtifactUpdateEvent>(json_value.clone()) {
            return Ok(Some(TaskOrMessage::TaskArtifactUpdateEvent(artifact_update)));
        }
        
        Err(A2AError::json_error(format!("Failed to parse SSE data as TaskOrMessage. JSON: {}", json_value)))
    }
    
    /// Convert SendStreamingMessageResult to TaskOrMessage
    fn convert_streaming_result(&self, result: SendStreamingMessageResult) -> Result<TaskOrMessage, A2AError> {
        match result {
            SendStreamingMessageResult::Task(task) => Ok(TaskOrMessage::Task(task)),
            SendStreamingMessageResult::TaskStatusUpdateEvent(update) => {
                // Convert task status update to a TaskStatusUpdateEvent
                Ok(TaskOrMessage::TaskUpdate(update))
            }
            SendStreamingMessageResult::TaskArtifactUpdateEvent(update) => {
                // Convert artifact update to a TaskArtifactUpdateEvent
                Ok(TaskOrMessage::TaskArtifactUpdateEvent(update))
            }
            SendStreamingMessageResult::Message(message) => Ok(TaskOrMessage::Message(message)),
        }
    }
}

#[async_trait]
impl ClientTransport for JsonRpcTransport {
    async fn send_message(
        &self,
        params: MessageSendParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<TaskOrMessage, A2AError> {
        let params_value = serde_json::to_value(params)
            .map_err(|e| A2AError::json_error(format!("Failed to serialize params: {}", e)))?;
        
        let result = self.send_jsonrpc_request("message/send", params_value, context, extensions).await?;
        
        // Try to parse as TaskOrMessage
        if let Ok(task_or_message) = serde_json::from_value::<TaskOrMessage>(result.clone()) {
            Ok(task_or_message)
        } else if let Ok(task) = serde_json::from_value::<Task>(result.clone()) {
            Ok(TaskOrMessage::Task(task))
        } else if let Ok(message) = serde_json::from_value::<Message>(result) {
            Ok(TaskOrMessage::Message(message))
        } else {
            Err(A2AError::json_error("Failed to parse response as Task or Message".to_string()))
        }
    }
    
    async fn send_message_streaming<'a>(
        &'a self,
        params: MessageSendParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<TaskOrMessage, A2AError>> + Send + 'a>>, A2AError> {
        let params_value = serde_json::to_value(params)
            .map_err(|e| A2AError::json_error(format!("Failed to serialize params: {}", e)))?;
        
        self.send_streaming_request("message/stream", params_value, context, extensions).await
    }
    
    async fn get_task(
        &self,
        request: TaskQueryParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Task, A2AError> {
        let params_value = serde_json::to_value(request)
            .map_err(|e| A2AError::json_error(format!("Failed to serialize params: {}", e)))?;
        
        let result = self.send_jsonrpc_request("tasks/get", params_value, context, extensions).await?;
        
        serde_json::from_value(result)
            .map_err(|e| A2AError::json_error(format!("Failed to parse Task response: {}", e)))
    }
    
    async fn cancel_task(
        &self,
        request: TaskIdParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Task, A2AError> {
        let params_value = serde_json::to_value(request)
            .map_err(|e| A2AError::json_error(format!("Failed to serialize params: {}", e)))?;
        
        let result = self.send_jsonrpc_request("tasks/cancel", params_value, context, extensions).await?;
        
        serde_json::from_value(result)
            .map_err(|e| A2AError::json_error(format!("Failed to parse Task response: {}", e)))
    }
    
    async fn set_task_callback(
        &self,
        request: TaskPushNotificationConfig,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        let params_value = serde_json::to_value(request)
            .map_err(|e| A2AError::json_error(format!("Failed to serialize params: {}", e)))?;
        
        let result = self.send_jsonrpc_request("tasks/pushNotificationConfig/set", params_value, context, extensions).await?;
        
        serde_json::from_value(result)
            .map_err(|e| A2AError::json_error(format!("Failed to parse TaskPushNotificationConfig response: {}", e)))
    }
    
    async fn get_task_callback(
        &self,
        request: GetTaskPushNotificationConfigParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        let params_value = serde_json::to_value(request)
            .map_err(|e| A2AError::json_error(format!("Failed to serialize params: {}", e)))?;
        
        let result = self.send_jsonrpc_request("tasks/pushNotificationConfig/get", params_value, context, extensions).await?;
        
        serde_json::from_value(result)
            .map_err(|e| A2AError::json_error(format!("Failed to parse TaskPushNotificationConfig response: {}", e)))
    }
    
    async fn resubscribe<'a>(
        &'a self,
        request: TaskIdParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ClientEvent, A2AError>> + Send + 'a>>, A2AError> {
        let params_value = serde_json::to_value(request)
            .map_err(|e| A2AError::json_error(format!("Failed to serialize params: {}", e)))?;
        
        let task_stream = self.send_streaming_request("tasks/resubscribe", params_value, context, extensions).await?;
        
        let mapped_stream = task_stream.map(|result| {
            match result {
                Ok(TaskOrMessage::Task(task)) => Ok((task, None)),
                Ok(TaskOrMessage::TaskUpdate(_task_update)) => {
                    // For task updates, we need to construct a task
                    // This is a simplified implementation
                    Err(A2AError::unsupported_operation("Task updates not fully implemented in resubscribe"))
                }
                Ok(TaskOrMessage::TaskArtifactUpdateEvent(_artifact_update)) => {
                    // For artifact updates, we need to construct a task
                    // This is a simplified implementation
                    Err(A2AError::unsupported_operation("Task artifact updates not fully implemented in resubscribe"))
                }
                Ok(TaskOrMessage::Message(_)) => {
                    Err(A2AError::invalid_response("Unexpected message in resubscribe stream"))
                }
                Err(e) => Err(e),
            }
        });
        
        Ok(Box::pin(mapped_stream))
    }
    
    async fn get_card(
        &self,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<AgentCard, A2AError> {
        // If we already have an agent card and don't need extended card, return it
        if let Some(ref card) = self.agent_card {
            if !self.needs_extended_card {
                return Ok(card.clone());
            }
        }
        
        // Try to get card from agent
        let resolver = A2ACardResolver::new(self.url.clone());
        let mut card = resolver.get_agent_card().await?;
        
        // If we need extended card and it's supported, fetch it
        if self.needs_extended_card && card.supports_authenticated_extended_card.unwrap_or(false) {
            let result = self.send_jsonrpc_request("agent/authenticatedExtendedCard", Value::Null, context, extensions).await?;
            
            let extended_card: AgentCard = serde_json::from_value(result)
                .map_err(|e| A2AError::json_error(format!("Failed to parse extended AgentCard: {}", e)))?;
            
            card = extended_card;
        }
        
        Ok(card)
    }
    
    async fn close(&self) -> Result<(), A2AError> {
        // reqwest::Client doesn't need explicit closing
        // This is a placeholder for any cleanup that might be needed
        Ok(())
    }
}

// Clone implementation for the transport
impl Clone for JsonRpcTransport {
    fn clone(&self) -> Self {
        Self {
            url: self.url.clone(),
            client: self.client.clone(),
            agent_card: self.agent_card.clone(),
            interceptors: Vec::new(), // Note: interceptors are not cloned as they're trait objects
            extensions: self.extensions.clone(),
            needs_extended_card: self.needs_extended_card,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jsonrpc_transport_creation() {
        let transport = JsonRpcTransport::new("http://localhost:8080".to_string(), None);
        assert!(transport.is_ok());
    }

    #[test]
    fn test_jsonrpc_transport_with_card() {
        let card = AgentCard::new(
            "Test".to_string(),
            "Test agent".to_string(),
            "http://localhost:8080".to_string(),
            "1.0.0".to_string(),
            vec!["text/plain".to_string()],
            vec!["text/plain".to_string()],
            AgentCapabilities::new(),
            vec![],
        );
        
        let transport = JsonRpcTransport::new("http://localhost:8080".to_string(), Some(card));
        assert!(transport.is_ok());
    }
}
