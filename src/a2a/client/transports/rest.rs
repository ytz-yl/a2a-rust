//! REST transport implementation for A2A Rust client
//!
//! This module provides a REST transport that communicates with A2A agents
//! using RESTful HTTP APIs, matching the server's REST handler interface.

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::time::Duration;

use crate::a2a::client::client_trait::{ClientCallContext, ClientTransport, ClientEvent, ClientCallInterceptor};
use crate::a2a::client::card_resolver::A2ACardResolver;
use crate::a2a::client::config::ClientConfig;
use crate::a2a::core_types::{Message, TaskStatus};
use crate::a2a::error::A2AError;
use crate::a2a::extensions::common::HTTP_EXTENSION_HEADER;
use crate::a2a::models::*;

/// REST transport for A2A client
///
/// This transport communicates with A2A agents using RESTful HTTP APIs
/// with JSON request/response bodies and optional SSE for streaming.
pub struct RestTransport {
    /// The base URL endpoint for the agent
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

impl RestTransport {
    /// Create a new REST transport
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
    
    /// Create a new REST transport with custom configuration
    pub fn new_with_config(
        url: String,
        agent_card: Option<AgentCard>,
        config: ClientConfig,
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
            if let Ok(header_value) = HeaderValue::from_str(&extension_header) {
                headers.insert(HTTP_EXTENSION_HEADER, header_value);
            }
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
    
    /// Send a REST request and get the response
    async fn send_rest_request(
        &self,
        method: &str,
        endpoint: &str,
        body: Option<Value>,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Value, A2AError> {
        let full_url = format!("{}{}", self.url, endpoint);
        
        // Get HTTP args from context
        let http_kwargs = context
            .map(|ctx| ctx.http_kwargs.clone())
            .unwrap_or_default();
        
        // Apply interceptors if we have a body
        let (body, mut http_kwargs) = if let Some(body_value) = body {
            self.apply_interceptors(method, body_value, http_kwargs, context).await?
        } else {
            (Value::Null, http_kwargs)
        };
        
        // Build headers
        let headers = self.build_headers(extensions.as_ref(), &http_kwargs);
        
        // Remove headers from http_kwargs since they're handled separately
        http_kwargs.remove("headers");
        
        // Extract request options
        let timeout = http_kwargs.get("timeout")
            .and_then(|v| v.as_u64())
            .map(Duration::from_secs);
        
        // Build request
        let mut request_builder = match method {
            "GET" => self.client.get(&full_url),
            "POST" => self.client.post(&full_url),
            "PUT" => self.client.put(&full_url),
            "DELETE" => self.client.delete(&full_url),
            _ => return Err(A2AError::invalid_request("Unsupported HTTP method")),
        };
        
        request_builder = request_builder.headers(headers);
        
        if method == "POST" || method == "PUT" {
            request_builder = request_builder.json(&body);
        }
        
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
            let status = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            
            // Handle 404 as task not found
            if status == 404 && endpoint.contains("/tasks/") {
                return Err(A2AError::TaskNotFound(crate::a2a::error::TaskNotFoundError::default()));
            }
            
            return Err(A2AError::http_error(status, format!("HTTP error {}: {}", status, error_text)));
        }
        
        // Parse response
        let response_value: Value = response
            .json()
            .await
            .map_err(|e| A2AError::json_error(format!("Failed to parse JSON response: {}", e)))?;
        
        Ok(response_value)
    }
    
    /// Send a streaming REST request with SSE support
    async fn send_streaming_request(
        &self,
        endpoint: &str,
        body: Value,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<TaskOrMessage, A2AError>> + Send + '_>>, A2AError> {
        let full_url = format!("{}{}", self.url, endpoint);
        
        // Get HTTP args from context
        let http_kwargs = context
            .map(|ctx| ctx.http_kwargs.clone())
            .unwrap_or_default();
        
        // Apply interceptors
        let (body, mut http_kwargs) = self.apply_interceptors("message/stream", body, http_kwargs, context).await?;
        
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
        let mut request_builder = self.client.post(&full_url).headers(headers).json(&body);
        
        if let Some(timeout_duration) = timeout {
            request_builder = request_builder.timeout(timeout_duration);
        }
        
        let response = request_builder
            .send()
            .await
            .map_err(|e| A2AError::transport_error(format!("HTTP request failed: {}", e)))?;
        
        // Check response status
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(A2AError::http_error(status, format!("HTTP error {}: {}", status, error_text)));
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
            
            // Try to parse as a single TaskOrMessage
            let result = if let Ok(task_or_message) = serde_json::from_value::<TaskOrMessage>(response_value.clone()) {
                task_or_message
            } else if let Ok(task) = serde_json::from_value::<Task>(response_value.clone()) {
                TaskOrMessage::Task(task)
            } else if let Ok(message) = serde_json::from_value::<Message>(response_value) {
                TaskOrMessage::Message(message)
            } else {
                return Err(A2AError::json_error("Failed to parse response as Task or Message".to_string()));
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
        
        // Try to parse as TaskOrMessage
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
        
        // Try to parse as SendStreamingMessageResponse
        if let Ok(response) = serde_json::from_value::<SendStreamingMessageResponse>(json_value.clone()) {
            match response {
                SendStreamingMessageResponse::Success(success_response) => {
                    match success_response.result {
                        SendStreamingMessageResult::Task(task) => Ok(Some(TaskOrMessage::Task(task))),
                        SendStreamingMessageResult::Message(message) => Ok(Some(TaskOrMessage::Message(message))),
                        SendStreamingMessageResult::TaskStatusUpdateEvent(update) => Ok(Some(TaskOrMessage::TaskUpdate(update))),
                        SendStreamingMessageResult::TaskArtifactUpdateEvent(update) => Ok(Some(TaskOrMessage::TaskArtifactUpdateEvent(update))),
                    }
                }
                SendStreamingMessageResponse::Error(error_response) => {
                    Err(A2AError::jsonrpc_error(error_response.error.code, error_response.error.message))
                }
            }
        } else {
            Err(A2AError::json_error(format!("Failed to parse SSE data as TaskOrMessage. JSON: {}", json_value)))
        }
    }
}

#[async_trait]
impl ClientTransport for RestTransport {
    async fn send_message(
        &self,
        params: MessageSendParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<TaskOrMessage, A2AError> {
        let body = serde_json::to_value(params)
            .map_err(|e| A2AError::json_error(format!("Failed to serialize params: {}", e)))?;
        
        let result = self.send_rest_request("POST", "/message/send", Some(body), context, extensions).await?;
        
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
        let body = serde_json::to_value(params)
            .map_err(|e| A2AError::json_error(format!("Failed to serialize params: {}", e)))?;
        
        self.send_streaming_request("/message/stream", body, context, extensions).await
    }
    
    async fn get_task(
        &self,
        request: TaskQueryParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Task, A2AError> {
        let endpoint = format!("/tasks/{}", request.id);
        
        // Add query parameters if needed
        let mut full_endpoint = endpoint;
        if let Some(history_length) = request.history_length {
            full_endpoint = format!("{}?history_length={}", full_endpoint, history_length);
        }
        
        let result = self.send_rest_request("GET", &full_endpoint, None, context, extensions).await?;
        
        serde_json::from_value(result)
            .map_err(|e| A2AError::json_error(format!("Failed to parse Task response: {}", e)))
    }
    
    async fn cancel_task(
        &self,
        request: TaskIdParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Task, A2AError> {
        let endpoint = format!("/tasks/{}/cancel", request.id);
        
        let result = self.send_rest_request("POST", &endpoint, None, context, extensions).await?;
        
        serde_json::from_value(result)
            .map_err(|e| A2AError::json_error(format!("Failed to parse Task response: {}", e)))
    }
    
    async fn set_task_callback(
        &self,
        request: TaskPushNotificationConfig,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        let endpoint = format!("/tasks/{}/push_notifications", request.task_id);
        
        let body = serde_json::to_value(request)
            .map_err(|e| A2AError::json_error(format!("Failed to serialize params: {}", e)))?;
        
        let result = self.send_rest_request("POST", &endpoint, Some(body), context, extensions).await?;
        
        serde_json::from_value(result)
            .map_err(|e| A2AError::json_error(format!("Failed to parse TaskPushNotificationConfig response: {}", e)))
    }
    
    async fn get_task_callback(
        &self,
        request: GetTaskPushNotificationConfigParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        let endpoint = if let Some(config_id) = request.push_notification_config_id {
            format!("/tasks/{}/push_notifications/{}", request.id, config_id)
        } else {
            format!("/tasks/{}/push_notifications", request.id)
        };
        
        let result = self.send_rest_request("GET", &endpoint, None, context, extensions).await?;
        
        serde_json::from_value(result)
            .map_err(|e| A2AError::json_error(format!("Failed to parse TaskPushNotificationConfig response: {}", e)))
    }
    
    async fn resubscribe<'a>(
        &'a self,
        request: TaskIdParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ClientEvent, A2AError>> + Send + 'a>>, A2AError> {
        let endpoint = format!("/tasks/{}/resubscribe", request.id);
        
        let body = serde_json::to_value(request)
            .map_err(|e| A2AError::json_error(format!("Failed to serialize params: {}", e)))?;
        
        let task_stream = self.send_streaming_request(&endpoint, body, context, extensions).await?;
        
        let mapped_stream = task_stream.map(|result| {
            match result {
                Ok(TaskOrMessage::Task(task)) => Ok((task, None)),
                Ok(TaskOrMessage::TaskUpdate(task_update)) => {
                    // For task updates, we need to construct a task
                    let task = Task::new(
                        task_update.context_id.clone(),
                        task_update.status.clone()
                    ).with_task_id(task_update.task_id.clone());
                    Ok((task, Some(crate::a2a::client::client_trait::TaskUpdateEvent::Status(task_update))))
                }
                Ok(TaskOrMessage::TaskArtifactUpdateEvent(artifact_update)) => {
                    // For artifact updates, we need to construct a task
                    let status = TaskStatus::new(crate::a2a::core_types::TaskState::Working);
                    let task = Task::new(
                        artifact_update.context_id.clone(),
                        status
                    ).with_task_id(artifact_update.task_id.clone());
                    Ok((task, Some(crate::a2a::client::client_trait::TaskUpdateEvent::Artifact(artifact_update))))
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
            let result = self.send_rest_request("GET", "/agent/card/extended", None, context, extensions).await?;
            
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
impl Clone for RestTransport {
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
    fn test_rest_transport_creation() {
        let transport = RestTransport::new("http://localhost:8080".to_string(), None);
        assert!(transport.is_ok());
    }

    #[test]
    fn test_rest_transport_with_card() {
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
        
        let transport = RestTransport::new("http://localhost:8080".to_string(), Some(card));
        assert!(transport.is_ok());
    }
}