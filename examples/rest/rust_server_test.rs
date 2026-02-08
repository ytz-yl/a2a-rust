//! REST Server Test for A2A Rust Implementation
//!
//! This test demonstrates a REST server implementation for A2A protocol.
//! It shows how to set up a REST server using the RestHandler and integrate it
//! with the existing request handler infrastructure.

use a2a_rust::a2a::{
    models::*,
    core_types::{Message, Part, Role, TaskStatus, TaskState},
    server::{
        context::DefaultServerCallContextBuilder,
        request_handlers::{RequestHandler, MessageSendResult, TaskPushNotificationConfigQueryParams, Event},
        apps::rest::RestHandler,
    },
};
use futures::Stream;
use std::net::SocketAddr;
use std::sync::Arc;
use std::pin::Pin;
use tokio;
use axum::{
    Router,
    routing::{get, post},
    extract::State,
    http::{StatusCode, HeaderMap},
    response::{Response, IntoResponse},
};
use axum::body::Body;
use serde_json::json;
use uuid::Uuid;
use tracing_subscriber;
use std::collections::HashMap;

/// Simple REST test handler
struct RestTestHandler;

impl RestTestHandler {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl RequestHandler for RestTestHandler {
    async fn on_get_task(
        &self,
        params: TaskQueryParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<Option<Task>, a2a_rust::a2a::error::A2AError> {
        println!("REST Test Handler: Get task {}", params.id);
        
        // For test purposes, return a mock task
        if params.id == "test-task-123" {
            let task = Task::new(
                "test-context".to_string(),
                TaskStatus::new(TaskState::Completed),
            ).with_task_id("test-task-123".to_string());
            
            Ok(Some(task))
        } else {
            Ok(None)
        }
    }

    async fn on_cancel_task(
        &self,
        params: TaskIdParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<Option<Task>, a2a_rust::a2a::error::A2AError> {
        println!("REST Test Handler: Cancel task {}", params.id);
        
        // For test purposes, return a mock cancelled task
        if params.id == "test-task-123" {
            let task = Task::new(
                "test-context".to_string(),
                TaskStatus::new(TaskState::Cancelled),
            ).with_task_id("test-task-123".to_string());
            
            Ok(Some(task))
        } else {
            Ok(None)
        }
    }

    async fn on_message_send(
        &self,
        params: MessageSendParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<MessageSendResult, a2a_rust::a2a::error::A2AError> {
        println!("REST Test Handler: Message send with {} parts", params.message.parts.len());
        
        let mut response_parts = Vec::new();
        response_parts.push(Part::text("REST Echo: ".to_string()));
        
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
        println!("REST Test Handler: Message stream with {} parts", params.message.parts.len());
        
        let context_id = params.message.context_id.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
        let task_id = params.message.task_id.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
        
        let status_message = Message::new(
            Role::Agent,
            vec![Part::text("Processing REST streaming message...".to_string())]
        );
        let initial_task = Task::new(
            context_id.clone(),
            TaskStatus::new(TaskState::Working).with_message(status_message),
        ).with_task_id(task_id.clone());
        
        let stream = async_stream::stream! {
            yield Ok(Event::Task(initial_task));
            
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            let response_message = Message::new(
                Role::Agent,
                vec![Part::text("REST Streaming Response".to_string())]
            )
            .with_context_id(context_id.clone())
            .with_task_id(task_id.clone());
            
            yield Ok(Event::Message(response_message));
            
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            let final_status = TaskStatusUpdateEvent::new(
                task_id.clone(),
                context_id,
                TaskStatus::new(TaskState::Completed).with_message(
                    Message::new(Role::Agent, vec![Part::text("REST stream completed".to_string())])
                ),
                true,
            );
            yield Ok(Event::TaskStatusUpdate(final_status));
        };
        
        Ok(Box::pin(stream))
    }

    async fn on_set_task_push_notification_config(
        &self,
        params: TaskPushNotificationConfig,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<TaskPushNotificationConfig, a2a_rust::a2a::error::A2AError> {
        println!("REST Test Handler: Set push notification config for task {}", params.task_id);
        Ok(params)
    }

    async fn on_get_task_push_notification_config(
        &self,
        params: TaskPushNotificationConfigQueryParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<TaskPushNotificationConfig, a2a_rust::a2a::error::A2AError> {
        println!("REST Test Handler: Get push notification config for task {}", params.id);
        
        // Return a mock config
        let config = TaskPushNotificationConfig {
            task_id: params.id,
            push_notification_config: PushNotificationConfig {
                id: Some("test-config".to_string()),
                url: url::Url::parse("https://example.com/webhook").unwrap(),
                token: Some("test-token".to_string()),
                authentication: None,
            },
        };
        
        Ok(config)
    }

    async fn on_list_task_push_notification_config(
        &self,
        params: TaskIdParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<Vec<TaskPushNotificationConfig>, a2a_rust::a2a::error::A2AError> {
        println!("REST Test Handler: List push notification configs for task {}", params.id);
        Ok(vec![])
    }

    async fn on_delete_task_push_notification_config(
        &self,
        params: DeleteTaskPushNotificationConfigParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<(), a2a_rust::a2a::error::A2AError> {
        println!("REST Test Handler: Delete push notification config {} for task {}", 
                params.push_notification_config_id, params.id);
        Ok(())
    }

    async fn on_resubscribe_to_task(
        &self,
        params: TaskIdParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Event, a2a_rust::a2a::error::A2AError>> + Send>>, a2a_rust::a2a::error::A2AError> {
        println!("REST Test Handler: Resubscribe to task {}", params.id);
        
        let stream = async_stream::stream! {
            let task = Task::new(
                "test-context".to_string(),
                TaskStatus::new(TaskState::Working)
            ).with_task_id(params.id.clone());
            
            yield Ok(Event::Task(task));
            
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            let status_update = TaskStatusUpdateEvent::new(
                params.id.clone(),
                "test-context".to_string(),
                TaskStatus::new(TaskState::Completed),
                true,
            );
            
            yield Ok(Event::TaskStatusUpdate(status_update));
        };
        
        Ok(Box::pin(stream))
    }
}

/// Server state to share between handlers
#[derive(Clone)]
struct ServerState {
    agent_card: AgentCard,
    handler: Arc<RestTestHandler>,
    context_builder: Arc<DefaultServerCallContextBuilder>,
    rest_handler: Arc<RestHandler>,
}

/// Create router for REST endpoints
fn create_router(state: ServerState) -> Router {
    Router::new()
        // Agent card endpoint
        .route("/agent/card", get(get_agent_card))
        // Message endpoints
        .route("/message/send", post(message_send))
        .route("/message/stream", post(message_stream))
        // Task endpoints
        .route("/tasks/{task_id}", get(get_task))
        .route("/tasks/{task_id}/cancel", post(cancel_task))
        .route("/tasks/{task_id}/resubscribe", post(resubscribe_task))
        // Push notification endpoints
        .route("/tasks/{task_id}/push_notifications", post(set_push_notification))
        .route("/tasks/{task_id}/push_notifications", get(get_push_notification))
        .route("/tasks/{task_id}/push_notifications/{config_id}", get(get_push_notification_by_id))
        .route("/tasks/{task_id}/push_notifications/{config_id}", post(delete_push_notification))
        .with_state(state)
}

/// Get agent card handler
async fn get_agent_card(
    State(state): State<ServerState>,
) -> impl IntoResponse {
    let context = state.context_builder.build_context(&HashMap::new());
    
    match state.rest_handler.get_agent_card(&context).await {
        Ok(card) => (
            StatusCode::OK,
            [("content-type", "application/json")],
            serde_json::to_string(&card).unwrap(),
        ),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [("content-type", "application/json")],
            json!({"error": err.message}).to_string(),
        ),
    }
}

/// Message send handler
async fn message_send(
    State(state): State<ServerState>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    let context = state.context_builder.build_context(&headers_to_map(&headers));
    
    match serde_json::from_str::<MessageSendParams>(&body) {
        Ok(params) => {
            match state.rest_handler.on_message_send(params, &context).await {
                Ok(result) => (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    result.to_string(),
                ),
                Err(err) => (
                    StatusCode::BAD_REQUEST,
                    [("content-type", "application/json")],
                    json!({"error": {"code": err.code, "message": err.message}}).to_string(),
                ),
            }
        }
        Err(err) => (
            StatusCode::BAD_REQUEST,
            [("content-type", "application/json")],
            json!({"error": {"code": -32700, "message": format!("Parse error: {}", err)}}).to_string(),
        ),
    }
}

/// Message stream handler
async fn message_stream(
    State(state): State<ServerState>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let context = state.context_builder.build_context(&headers_to_map(&headers));
    
    match serde_json::from_str::<MessageSendParams>(&body) {
        Ok(params) => {
            match state.rest_handler.on_message_send_stream(params, &context).await {
                Ok(stream) => {
                    // For streaming response, we need to set appropriate headers
                    let stream = stream.map(|result| match result {
                        Ok(json_str) => Ok::<_, std::io::Error>(axum::body::Bytes::from(json_str + "\n")),
                        Err(err) => Ok(axum::body::Bytes::from(
                            json!({"error": {"code": err.code, "message": err.message}}).to_string() + "\n"
                        )),
                    });
                    
                    Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "text/event-stream")
                        .header("cache-control", "no-cache")
                        .header("connection", "keep-alive")
                        .body(Body::from_stream(stream))
                        .unwrap()
                }
                Err(err) => (
                    StatusCode::BAD_REQUEST,
                    [("content-type", "application/json")],
                    json!({"error": {"code": err.code, "message": err.message}}).to_string(),
                ).into_response(),
            }
        }
        Err(err) => (
            StatusCode::BAD_REQUEST,
            [("content-type", "application/json")],
            json!({"error": {"code": -32700, "message": format!("Parse error: {}", err)}}).to_string(),
        ).into_response(),
    }
}

/// Get task handler
async fn get_task(
    State(state): State<ServerState>,
    headers: HeaderMap,
    axum::extract::Path(task_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let context = state.context_builder.build_context(&headers_to_map(&headers));
    
    let params = TaskQueryParams {
        id: task_id,
        history_length: None,
        metadata: None,
    };
    
    match state.rest_handler.on_get_task(params, &context).await {
        Ok(result) => (
            StatusCode::OK,
            [("content-type", "application/json")],
            result.to_string(),
        ),
        Err(err) => {
            let status = if err.code == 404 {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::BAD_REQUEST
            };
            
            (
                status,
                [("content-type", "application/json")],
                json!({"error": {"code": err.code, "message": err.message}}).to_string(),
            )
        }
    }
}

/// Cancel task handler
async fn cancel_task(
    State(state): State<ServerState>,
    headers: HeaderMap,
    axum::extract::Path(task_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let context = state.context_builder.build_context(&headers_to_map(&headers));
    
    let params = TaskIdParams {
        id: task_id,
        metadata: None,
    };
    
    match state.rest_handler.on_cancel_task(params, &context).await {
        Ok(result) => (
            StatusCode::OK,
            [("content-type", "application/json")],
            result.to_string(),
        ),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            [("content-type", "application/json")],
            json!({"error": {"code": err.code, "message": err.message}}).to_string(),
        ),
    }
}

/// Resubscribe task handler
async fn resubscribe_task(
    State(state): State<ServerState>,
    headers: HeaderMap,
    axum::extract::Path(task_id): axum::extract::Path<String>,
) -> Response {
    let context = state.context_builder.build_context(&headers_to_map(&headers));
    
    let params = TaskIdParams {
        id: task_id,
        metadata: None,
    };
    
    match state.rest_handler.on_resubscribe_to_task(params, &context).await {
        Ok(stream) => {
            // For streaming response
            let stream = stream.map(|result| match result {
                Ok(json_str) => Ok::<_, std::io::Error>(axum::body::Bytes::from(json_str + "\n")),
                Err(err) => Ok(axum::body::Bytes::from(
                    json!({"error": {"code": err.code, "message": err.message}}).to_string() + "\n"
                )),
            });
            
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/event-stream")
                .header("cache-control", "no-cache")
                .header("connection", "keep-alive")
                .body(Body::from_stream(stream))
                .unwrap()
        }
        Err(err) => (
            StatusCode::BAD_REQUEST,
            [("content-type", "application/json")],
            json!({"error": {"code": err.code, "message": err.message}}).to_string(),
        ).into_response(),
    }
}

/// Set push notification handler
async fn set_push_notification(
    State(state): State<ServerState>,
    headers: HeaderMap,
    axum::extract::Path(task_id): axum::extract::Path<String>,
    body: String,
) -> impl IntoResponse {
    let context = state.context_builder.build_context(&headers_to_map(&headers));
    
    match serde_json::from_str::<TaskPushNotificationConfig>(&body) {
        Ok(mut config) => {
            config.task_id = task_id;
            match state.rest_handler.set_push_notification(config, &context).await {
                Ok(result) => (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    result.to_string(),
                ),
                Err(err) => (
                    StatusCode::BAD_REQUEST,
                    [("content-type", "application/json")],
                    json!({"error": {"code": err.code, "message": err.message}}).to_string(),
                ),
            }
        }
        Err(err) => (
            StatusCode::BAD_REQUEST,
            [("content-type", "application/json")],
            json!({"error": {"code": -32700, "message": format!("Parse error: {}", err)}}).to_string(),
        ),
    }
}

/// Get push notification handler
async fn get_push_notification(
    State(state): State<ServerState>,
    headers: HeaderMap,
    axum::extract::Path(task_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let context = state.context_builder.build_context(&headers_to_map(&headers));
    
    let params = TaskPushNotificationConfigQueryParams {
        id: task_id,
        push_notification_config_id: None,
        metadata: None,
    };
    
    match state.rest_handler.get_push_notification(params, &context).await {
        Ok(result) => (
            StatusCode::OK,
            [("content-type", "application/json")],
            result.to_string(),
        ),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            [("content-type", "application/json")],
            json!({"error": {"code": err.code, "message": err.message}}).to_string(),
        ),
    }
}

/// Get push notification by ID handler
async fn get_push_notification_by_id(
    State(state): State<ServerState>,
    headers: HeaderMap,
    axum::extract::Path((task_id, config_id)): axum::extract::Path<(String, String)>,
) -> impl IntoResponse {
    let context = state.context_builder.build_context(&headers_to_map(&headers));
    
    let params = TaskPushNotificationConfigQueryParams {
        id: task_id,
        push_notification_config_id: Some(config_id),
        metadata: None,
    };
    
    match state.rest_handler.get_push_notification(params, &context).await {
        Ok(result) => (
            StatusCode::OK,
            [("content-type", "application/json")],
            result.to_string(),
        ),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            [("content-type", "application/json")],
            json!({"error": {"code": err.code, "message": err.message}}).to_string(),
        ),
    }
}

/// Delete push notification handler
async fn delete_push_notification(
    State(state): State<ServerState>,
    headers: HeaderMap,
    axum::extract::Path((task_id, config_id)): axum::extract::Path<(String, String)>,
) -> impl IntoResponse {
    // Note: This endpoint is not implemented in RestHandler yet
    // We'll return a placeholder response
    let response = json!({
        "status": "success",
        "message": format!("Push notification config {} for task {} deleted", config_id, task_id)
    });
    
    (
        StatusCode::OK,
        [("content-type", "application/json")],
        response.to_string(),
    )
}

/// Helper to convert headers to map
fn headers_to_map(headers: &HeaderMap) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (name, value) in headers.iter() {
        if let Ok(value_str) = value.to_str() {
            map.insert(name.to_string(), value_str.to_string());
        }
    }
    map
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create agent card
    let agent_card = AgentCard::new(
        "REST Test Server".to_string(),
        "A REST test server for A2A protocol".to_string(),
        "http://localhost:8081".to_string(),
        "1.0.0".to_string(),
        vec!["text/plain".to_string()],
        vec!["text/plain".to_string()],
        AgentCapabilities::new().with_streaming(true).with_push_notifications(true),
        vec![],
    );

    // Create handler and context builder
    let handler = Arc::new(RestTestHandler::new());
    let context_builder = Arc::new(DefaultServerCallContextBuilder);
    
    // Create RestHandler
    let rest_handler = Arc::new(RestHandler::new(agent_card.clone(), handler.clone()));

    // Create server state
    let state = ServerState {
        agent_card: agent_card.clone(),
        handler,
        context_builder,
        rest_handler,
    };

    // Create router
    let router = create_router(state);

    // Start server
    let addr = SocketAddr::from(([127, 0, 0, 1], 8081));
    
    println!("🚀 Starting REST Test Server on http://{}", addr);
    println!("📡 Endpoints:");
    println!("   - Agent card: GET http://{}/agent/card", addr);
    println!("   - Send message: POST http://{}/message/send", addr);
    println!("   - Stream message: POST http://{}/message/stream", addr);
    println!("   - Get task: GET http://{}/tasks/{{task_id}}", addr);
    println!("   - Cancel task: POST http://{}/tasks/{{task_id}}/cancel", addr);
    println!("   - Resubscribe: POST http://{}/tasks/{{task_id}}/resubscribe", addr);
    println!("   - Push notifications: POST/GET http://{}/tasks/{{task_id}}/push_notifications", addr);
    println!("✨ Server is ready to accept connections!");

    // Run server
    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await?;

    Ok(())
}