//! Core client trait and types for A2A Rust client
//! 
//! This module defines the main client interface that mirrors the functionality
//! of a2a-python's Client abstract base class.

use crate::a2a::models::*;
use crate::a2a::core_types::*;
use crate::a2a::client::config::ClientConfig;
use serde::{Deserialize, Serialize};

/// Task update events that can occur during task execution
/// Matches Python's UpdateEvent = TaskStatusUpdateEvent | TaskArtifactUpdateEvent | None
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TaskUpdateEvent {
    Status(TaskStatusUpdateEvent),
    Artifact(TaskArtifactUpdateEvent),
}
use async_stream::stream;
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;

/// Type alias for client events - either a task with optional update, or a message
pub type ClientEvent = (Task, Option<TaskUpdateEvent>);

/// Type alias for event consuming callback
pub type Consumer = Box<dyn Fn(ClientEventOrMessage, AgentCard) + Send + Sync>;

/// Type that can be either a ClientEvent or a Message
#[derive(Debug, Clone)]
pub enum ClientEventOrMessage {
    Event(ClientEvent),
    Message(Message),
}

/// Context for client calls, similar to Python's ClientCallContext
#[derive(Debug, Clone)]
pub struct ClientCallContext {
    /// Additional metadata for the call
    pub metadata: HashMap<String, Value>,
    
    /// HTTP-specific arguments
    pub http_kwargs: HashMap<String, Value>,
}

impl Default for ClientCallContext {
    fn default() -> Self {
        Self {
            metadata: HashMap::new(),
            http_kwargs: HashMap::new(),
        }
    }
}

impl ClientCallContext {
    /// Create a new client call context
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Add metadata to the context
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    
    /// Add HTTP arguments to the context
    pub fn with_http_kwargs(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.http_kwargs.insert(key.into(), value.into());
        self
    }
}

/// Trait for intercepting client calls, similar to Python's ClientCallInterceptor
#[async_trait]
pub trait ClientCallInterceptor: Send + Sync {
    /// Intercept a client call before it's sent
    async fn intercept(
        &self,
        method_name: &str,
        request_payload: Value,
        http_kwargs: HashMap<String, Value>,
        agent_card: &AgentCard,
        context: Option<&ClientCallContext>,
    ) -> Result<(Value, HashMap<String, Value>), crate::a2a::error::A2AError>;
}

/// Main client trait that defines the interface for interacting with A2A agents
/// This mirrors the functionality of a2a-python's Client abstract base class
#[async_trait]
pub trait Client: Send + Sync {
    /// Send a message to the server and return a stream of events or a message
    async fn send_message<'life0, 'life1>(
        &'life0 self,
        request: Message,
        context: Option<&'life1 ClientCallContext>,
        request_metadata: Option<HashMap<String, Value>>,
        extensions: Option<Vec<String>>,
    ) -> Pin<Box<dyn Stream<Item = Result<ClientEventOrMessage, crate::a2a::error::A2AError>> + Send + 'life0>>
    where
        'life1: 'life0;
    
    
    /// Retrieve the current state and history of a specific task
    async fn get_task(
        &self,
        request: TaskQueryParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Task, crate::a2a::error::A2AError>;
    
    /// Request the agent to cancel a specific task
    async fn cancel_task(
        &self,
        request: TaskIdParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Task, crate::a2a::error::A2AError>;
    
    /// Set or update the push notification configuration for a specific task
    async fn set_task_callback(
        &self,
        request: TaskPushNotificationConfig,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<TaskPushNotificationConfig, crate::a2a::error::A2AError>;
    
    /// Retrieve the push notification configuration for a specific task
    async fn get_task_callback(
        &self,
        request: GetTaskPushNotificationConfigParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<TaskPushNotificationConfig, crate::a2a::error::A2AError>;
    
    /// Resubscribe to a task's event stream
    async fn resubscribe<'a>(
        &'a self,
        request: TaskIdParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Pin<Box<dyn Stream<Item = Result<ClientEvent, crate::a2a::error::A2AError>> + Send + 'a>>;
    
    /// Retrieve the agent's card
    async fn get_card(
        &self,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<AgentCard, crate::a2a::error::A2AError>;
    
    /// Add an event consumer to the client
    async fn add_event_consumer(&self, consumer: Consumer);
    
    /// Add request middleware to the client
    async fn add_request_middleware(&self, middleware: Box<dyn ClientCallInterceptor>);
    
    /// Process events via all registered consumers
    async fn consume(
        &self,
        event: Option<ClientEventOrMessage>,
        card: &AgentCard,
    ) -> Result<(), crate::a2a::error::A2AError>;
}

/// Base client implementation with common functionality
/// This mirrors a2a-python's BaseClient
pub struct BaseClient {
    card: AgentCard,
    config: ClientConfig,
    transport: Box<dyn ClientTransport>,
    consumers: Vec<Consumer>,
    #[allow(dead_code)] // TODO: Implement middleware functionality
    middleware: Vec<Box<dyn ClientCallInterceptor>>,
}

impl BaseClient {
    /// Create a new base client
    pub fn new(
        card: AgentCard,
        config: ClientConfig,
        transport: Box<dyn ClientTransport>,
        consumers: Vec<Consumer>,
        middleware: Vec<Box<dyn ClientCallInterceptor>>,
    ) -> Self {
        Self {
            card,
            config,
            transport,
            consumers,
            middleware,
        }
    }
    
    /// Get the agent card
    pub fn card(&self) -> &AgentCard {
        &self.card
    }
    
    /// Get the client configuration
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }
    
    /// Get the transport
    pub fn transport(&self) -> &dyn ClientTransport {
        &*self.transport
    }
}

#[async_trait]
impl Client for BaseClient {
    async fn send_message<'life0, 'life1>(
        &'life0 self,
        request: Message,
        context: Option<&'life1 ClientCallContext>,
        request_metadata: Option<HashMap<String, Value>>,
        extensions: Option<Vec<String>>,
    ) -> Pin<Box<dyn Stream<Item = Result<ClientEventOrMessage, crate::a2a::error::A2AError>> + Send + 'life0>>
    where
        'life1: 'life0,
    {
        // Create base configuration from client config
        let config = crate::a2a::models::MessageSendConfiguration {
            accepted_output_modes: if self.config.accepted_output_modes.is_empty() {
                None
            } else {
                Some(self.config.accepted_output_modes.clone())
            },
            blocking: Some(!self.config.polling),
            history_length: None,
            push_notification_config: self.config.push_notification_configs.first().cloned(),
        };
        
        let params = MessageSendParams {
            message: request,
            configuration: Some(config),
            metadata: request_metadata,
        };
        
        // Choose between streaming and non-streaming based on configuration
        if self.config.streaming {
            // Try streaming first
            match self.transport.send_message_streaming(params.clone(), context, extensions.clone()).await {
                Ok(stream) => {
                    // Convert TaskOrMessage stream to ClientEventOrMessage stream
                    let mapped_stream = stream.map(|result| {
                        match result {
                            Ok(task_or_message) => {
                                match task_or_message {
                                    TaskOrMessage::Message(message) => Ok(ClientEventOrMessage::Message(message)),
                                    TaskOrMessage::Task(task) => Ok(ClientEventOrMessage::Event((task, None))),
                                    TaskOrMessage::TaskUpdate(task_update) => {
                                        // Create a dummy task for the event
                                        let dummy_task = Task::new(
                                            task_update.context_id.clone(),
                                            task_update.status.clone()
                                        ).with_task_id(task_update.task_id.clone());
                                        Ok(ClientEventOrMessage::Event((dummy_task, Some(TaskUpdateEvent::Status(task_update)))))
                                    },
                                    TaskOrMessage::TaskArtifactUpdateEvent(artifact_update) => {
                                        // Create a dummy task for the event
                                        let dummy_status = TaskStatus::new(TaskState::Working);
                                        let dummy_task = Task::new(
                                            artifact_update.context_id.clone(),
                                            dummy_status
                                        ).with_task_id(artifact_update.task_id.clone());
                                        Ok(ClientEventOrMessage::Event((dummy_task, Some(TaskUpdateEvent::Artifact(artifact_update)))))
                                    },
                                }
                            }
                            Err(e) => Err(e),
                        }
                    });
                    Box::pin(mapped_stream)
                }
                Err(_) => {
                    // Fall back to non-streaming if streaming fails
                    Box::pin(stream! {
                        match self.transport.send_message(params, context, extensions).await {
                            Ok(task_or_message) => {
                                match task_or_message {
                                    TaskOrMessage::Message(message) => yield Ok(ClientEventOrMessage::Message(message)),
                                    TaskOrMessage::Task(task) => yield Ok(ClientEventOrMessage::Event((task, None))),
                                    TaskOrMessage::TaskUpdate(task_update) => {
                                        // Create a dummy task for the event
                                        let dummy_task = Task::new(
                                            task_update.context_id.clone(),
                                            task_update.status.clone()
                                        ).with_task_id(task_update.task_id.clone());
                                        yield Ok(ClientEventOrMessage::Event((dummy_task, Some(TaskUpdateEvent::Status(task_update)))));
                                    },
                                    TaskOrMessage::TaskArtifactUpdateEvent(artifact_update) => {
                                        // Create a dummy task for the event
                                        let dummy_status = TaskStatus::new(TaskState::Working);
                                        let dummy_task = Task::new(
                                            artifact_update.context_id.clone(),
                                            dummy_status
                                        ).with_task_id(artifact_update.task_id.clone());
                                        yield Ok(ClientEventOrMessage::Event((dummy_task, Some(TaskUpdateEvent::Artifact(artifact_update)))));
                                    },
                                }
                            }
                            Err(e) => yield Err(e),
                        }
                    })
                }
            }
        } else {
            // Non-streaming mode
            Box::pin(stream! {
                match self.transport.send_message(params, context, extensions).await {
                    Ok(task_or_message) => {
                        match task_or_message {
                            TaskOrMessage::Message(message) => yield Ok(ClientEventOrMessage::Message(message)),
                            TaskOrMessage::Task(task) => yield Ok(ClientEventOrMessage::Event((task, None))),
                            TaskOrMessage::TaskUpdate(task_update) => {
                                // Create a dummy task for the event
                                let dummy_task = Task::new(
                                    task_update.context_id.clone(),
                                    task_update.status.clone()
                                ).with_task_id(task_update.task_id.clone());
                                yield Ok(ClientEventOrMessage::Event((dummy_task, Some(TaskUpdateEvent::Status(task_update)))));
                            },
                            TaskOrMessage::TaskArtifactUpdateEvent(artifact_update) => {
                                // Create a dummy task for the event
                                let dummy_status = TaskStatus::new(TaskState::Working);
                                let dummy_task = Task::new(
                                    artifact_update.context_id.clone(),
                                    dummy_status
                                ).with_task_id(artifact_update.task_id.clone());
                                yield Ok(ClientEventOrMessage::Event((dummy_task, Some(TaskUpdateEvent::Artifact(artifact_update)))));
                            },
                        }
                    }
                    Err(e) => yield Err(e),
                }
            })
        }
    }
    
    async fn get_task(
        &self,
        request: TaskQueryParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Task, crate::a2a::error::A2AError> {
        self.transport.get_task(request, context, extensions).await
    }
    
    async fn cancel_task(
        &self,
        request: TaskIdParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Task, crate::a2a::error::A2AError> {
        self.transport.cancel_task(request, context, extensions).await
    }
    
    async fn set_task_callback(
        &self,
        request: TaskPushNotificationConfig,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<TaskPushNotificationConfig, crate::a2a::error::A2AError> {
        self.transport.set_task_callback(request, context, extensions).await
    }
    
    async fn get_task_callback(
        &self,
        request: GetTaskPushNotificationConfigParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<TaskPushNotificationConfig, crate::a2a::error::A2AError> {
        self.transport.get_task_callback(request, context, extensions).await
    }
    
    async fn resubscribe<'a>(
        &'a self,
        request: TaskIdParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Pin<Box<dyn Stream<Item = Result<ClientEvent, crate::a2a::error::A2AError>> + Send + 'a>> {
        if !self.config.streaming || !self.card.capabilities.streaming.unwrap_or(false) {
            return Box::pin(stream! {
                yield Err(crate::a2a::error::A2AError::unsupported_operation(
                    "client and/or server do not support resubscription"
                ));
            });
        }
        
        match self.transport.resubscribe(request, context, extensions).await {
            Ok(stream) => stream,
            Err(e) => Box::pin(stream! {
                yield Err(e);
            }),
        }
    }
    
    async fn get_card(
        &self,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<AgentCard, crate::a2a::error::A2AError> {
        let card = self.transport.get_card(context, extensions).await?;
        // In a real implementation, we would update the internal card
        // For now, just return the fetched card
        Ok(card)
    }
    
    async fn add_event_consumer(&self, _consumer: Consumer) {
        // In a real implementation, we would need interior mutability
        // For now, this is a placeholder
    }
    
    async fn add_request_middleware(&self, _middleware: Box<dyn ClientCallInterceptor>) {
        // In a real implementation, we would need interior mutability
        // For now, this is a placeholder
    }
    
    async fn consume(
        &self,
        event: Option<ClientEventOrMessage>,
        card: &AgentCard,
    ) -> Result<(), crate::a2a::error::A2AError> {
        if let Some(event_ref) = event {
            for consumer in &self.consumers {
                consumer(event_ref.clone(), card.clone());
            }
        }
        Ok(())
    }
}

/// Transport trait for different communication protocols
/// This mirrors a2a-python's ClientTransport
#[async_trait]
pub trait ClientTransport: Send + Sync {
    /// Send a non-streaming message
    async fn send_message(
        &self,
        params: MessageSendParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<TaskOrMessage, crate::a2a::error::A2AError>;
    
    /// Send a streaming message
    async fn send_message_streaming<'a>(
        &'a self,
        params: MessageSendParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<TaskOrMessage, crate::a2a::error::A2AError>> + Send + 'a>>, crate::a2a::error::A2AError>;
    
    /// Get a task
    async fn get_task(
        &self,
        request: TaskQueryParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Task, crate::a2a::error::A2AError>;
    
    /// Cancel a task
    async fn cancel_task(
        &self,
        request: TaskIdParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Task, crate::a2a::error::A2AError>;
    
    /// Set task callback
    async fn set_task_callback(
        &self,
        request: TaskPushNotificationConfig,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<TaskPushNotificationConfig, crate::a2a::error::A2AError>;
    
    /// Get task callback
    async fn get_task_callback(
        &self,
        request: GetTaskPushNotificationConfigParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<TaskPushNotificationConfig, crate::a2a::error::A2AError>;
    
    /// Resubscribe to task events
    async fn resubscribe<'a>(
        &'a self,
        request: TaskIdParams,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ClientEvent, crate::a2a::error::A2AError>> + Send + 'a>>, crate::a2a::error::A2AError>;
    
    /// Get agent card
    async fn get_card(
        &self,
        context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<AgentCard, crate::a2a::error::A2AError>;
    
    /// Close the transport
    async fn close(&self) -> Result<(), crate::a2a::error::A2AError>;
}
