//! A2A Server Example with SSE Support
//! 
//! This example demonstrates how to create an A2A server with Server-Sent Events (SSE)
//! streaming support using the a2a-rust library.

use a2a_rust::a2a::{
    models::*,
    core_types::{Message, Part, Role, TaskStatus, TaskState},
    server::{
        apps::jsonrpc::{A2AServerBuilder, ServerConfig},
        context::DefaultServerCallContextBuilder,
        request_handlers::{RequestHandler, MessageSendResult, TaskPushNotificationConfigQueryParams, Event},
    },
};
use futures::Stream;
use std::net::SocketAddr;
use std::sync::Arc;
use std::pin::Pin;
use tokio;
use tokio::time::Duration;

/// Simple request handler that echoes back messages
struct EchoHandler;

impl EchoHandler {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl RequestHandler for EchoHandler {
    async fn on_get_task(
        &self,
        _params: TaskQueryParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<Option<Task>, a2a_rust::a2a::error::A2AError> {
        // For this simple example, we don't support task retrieval
        Ok(None)
    }

    async fn on_cancel_task(
        &self,
        _params: TaskIdParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<Option<Task>, a2a_rust::a2a::error::A2AError> {
        // For this simple example, we don't support task cancellation
        Ok(None)
    }

    async fn on_message_send(
        &self,
        params: MessageSendParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<MessageSendResult, a2a_rust::a2a::error::A2AError> {
        // Echo back the message
        let mut response_parts = Vec::new();
        
        // Add a text part indicating this is an echo
        response_parts.push(Part::text("Echo from Rust server: ".to_string()));
        
        // Add all parts from the incoming message
        for part in &params.message.parts {
            response_parts.push(part.clone());
        }
        
        let response_message = Message::new(Role::Agent, response_parts)
            .with_context_id(params.message.context_id.clone().unwrap_or_default())
            .with_task_id(params.message.task_id.clone().unwrap_or_default());

        Ok(MessageSendResult::Message(response_message))
    }

    async fn on_message_send_stream(
        &self,
        params: MessageSendParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Event, a2a_rust::a2a::error::A2AError>> + Send>>, a2a_rust::a2a::error::A2AError> {
        // Create a streaming response that demonstrates SSE functionality
        let context_id = params.message.context_id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let task_id = params.message.task_id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        
        // Create the initial task
        let status_message = Message::new(
            Role::Agent,
            vec![Part::text("Processing your message...".to_string())]
        );
        let initial_task = Task::new(
            context_id.clone(),
            TaskStatus::new(TaskState::Working).with_message(status_message),
        ).with_task_id(task_id.clone());
        
        // Create a stream of events
        let stream = async_stream::stream! {
            // Event 1: Initial task status
            yield Ok(Event::Task(initial_task));
            
            // Add a small delay to simulate processing
            tokio::time::sleep(Duration::from_millis(500)).await;
            
            // Event 2: Task status update
            let update_message = Message::new(
                Role::Agent,
                vec![Part::text("Still processing...".to_string())]
            );
            let update_status = TaskStatusUpdateEvent::new(
                task_id.clone(),
                context_id.clone(),
                TaskStatus::new(TaskState::Working).with_message(update_message),
                false, // not final
            );
            yield Ok(Event::TaskStatusUpdate(update_status));
            
            // Add another delay
            tokio::time::sleep(Duration::from_millis(500)).await;
            
            // Event 3: Create and echo the response message
            let mut response_parts = Vec::new();
            response_parts.push(Part::text("🔄 Streaming echo from Rust server: ".to_string()));
            
            // Add all parts from the incoming message
            for part in &params.message.parts {
                response_parts.push(part.clone());
            }
            
            let response_message = Message::new(Role::Agent, response_parts)
                .with_context_id(context_id.clone())
                .with_task_id(task_id.clone());
            
            yield Ok(Event::Message(response_message));
            
            // Final delay
            tokio::time::sleep(Duration::from_millis(300)).await;
            
            // Event 4: Final task status (completed)
            let final_message = Message::new(
                Role::Agent,
                vec![Part::text("Message processed successfully!".to_string())]
            );
            let final_status = TaskStatusUpdateEvent::new(
                task_id.clone(),
                context_id,
                TaskStatus::new(TaskState::Completed).with_message(final_message),
                true, // final
            );
            yield Ok(Event::TaskStatusUpdate(final_status));
        };
        
        Ok(Box::pin(stream))
    }

    async fn on_set_task_push_notification_config(
        &self,
        _params: TaskPushNotificationConfig,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<TaskPushNotificationConfig, a2a_rust::a2a::error::A2AError> {
        Err(a2a_rust::a2a::error::A2AError::unsupported_operation("Push notifications not supported"))
    }

    async fn on_get_task_push_notification_config(
        &self,
        _params: TaskPushNotificationConfigQueryParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<TaskPushNotificationConfig, a2a_rust::a2a::error::A2AError> {
        Err(a2a_rust::a2a::error::A2AError::unsupported_operation("Push notifications not supported"))
    }

    async fn on_list_task_push_notification_config(
        &self,
        _params: TaskIdParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<Vec<TaskPushNotificationConfig>, a2a_rust::a2a::error::A2AError> {
        Ok(vec![])
    }

    async fn on_delete_task_push_notification_config(
        &self,
        _params: DeleteTaskPushNotificationConfigParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<(), a2a_rust::a2a::error::A2AError> {
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create agent card with streaming capabilities
    let agent_card = AgentCard::new(
        "Echo Server with SSE".to_string(),
        "A simple echo server implemented in Rust with Server-Sent Events streaming support".to_string(),
        "http://localhost:8080".to_string(),
        "1.0.0".to_string(),
        vec!["text/plain".to_string()],
        vec!["text/plain".to_string()],
        AgentCapabilities::new().with_streaming(true),
        vec![],
    );

    // Create request handler
    let request_handler = Arc::new(EchoHandler::new());

    // Create context builder
    let context_builder = Arc::new(DefaultServerCallContextBuilder);

    // Configure server
    let config = ServerConfig {
        bind_addr: "127.0.0.1:8080".parse::<SocketAddr>()?,
        ..Default::default()
    };

    // Build and start server
    let server = A2AServerBuilder::new()
        .with_agent_card(agent_card)
        .with_request_handler(request_handler)
        .with_context_builder(context_builder)
        .with_config(config)
        .build()?;

    println!("🚀 Starting A2A Echo Server with SSE on http://127.0.0.1:8080");
    println!("📋 Agent Card available at: http://127.0.0.1:8080/.well-known/agent.json");
    println!("🔌 JSON-RPC endpoint at: http://127.0.0.1:8080/rpc");
    println!("🌊 SSE streaming enabled for message/stream requests");
    println!("✨ Server is ready to accept connections!");

    // Start the server
    server.serve().await?;

    Ok(())
}
