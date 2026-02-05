//! A2A Server with Real Push Notification Support
//! 
//! This server listens for A2A messages and automatically sends push notifications
//! to the configured webhook URL when task status changes.

use a2a_rust::a2a::server::request_handlers::default_request_handler::DefaultRequestHandler;
use a2a_rust::a2a::server::request_handlers::request_handler::{RequestHandler, MessageSendResult};
use a2a_rust::a2a::server::tasks::push_notification_sender::{HttpPushNotificationSender, PushNotificationSender};
use a2a_rust::a2a::server::tasks::task_store::{InMemoryTaskStore, TaskStore};
use a2a_rust::a2a::server::tasks::push_notification_config_store::InMemoryPushNotificationConfigStore;
use a2a_rust::a2a::models::MessageSendParams;
use a2a_rust::a2a::core_types::{TaskState, TaskStatus};
use axum::{
    routing::post,
    Json, Router,
    extract::State,
};
use std::sync::Arc;
use std::net::SocketAddr;
use tracing_subscriber;

struct AppState {
    handler: DefaultRequestHandler,
    task_store: Arc<InMemoryTaskStore>,
    push_sender: Arc<dyn PushNotificationSender>,
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Use environment variable or default to 8080
    let server_port: u16 = std::env::var("A2A_SERVER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    // 1. Setup Server Components
    let task_store = Arc::new(InMemoryTaskStore::new());
    let config_store = Arc::new(InMemoryPushNotificationConfigStore::new());
    let push_sender = Arc::new(HttpPushNotificationSender::new(config_store.clone()));

    // Create the DefaultRequestHandler which coordinates storage and notifications
    let handler = DefaultRequestHandler::new(
        task_store.clone(),
        Some(config_store.clone()),
        Some(push_sender.clone()),
    );

    let app_state = Arc::new(AppState { 
        handler,
        task_store,
        push_sender,
    });

    // 2. Define Routes
    let app = Router::new()
        .route("/message/send", post(handle_message_send))
        .with_state(app_state);

    // 3. Start Server
    let addr = SocketAddr::from(([127, 0, 0, 1], server_port));
    println!("A2A Server running at http://{}", addr);
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("Failed to bind to port {}: {}", server_port, e);
            eprintln!("Please ensure the port is not already in use or set A2A_SERVER_PORT environment variable to a different port.");
            std::process::exit(1);
        }
    };
    axum::serve(listener, app).await.unwrap();
}

/// Handle incoming message/send requests
async fn handle_message_send(
    State(state): State<Arc<AppState>>,
    Json(params): Json<MessageSendParams>,
) -> Json<serde_json::Value> {
    println!("Received message from client: {:?}", params.message.message_id);

    // Process the message using the handler
    // This will automatically trigger a push notification if configuration is present
    let result = state.handler.on_message_send(params, None).await.unwrap();

    // Simulate some background work that updates the task status
    if let MessageSendResult::Task(task) = &result {
        let mut task_to_update = task.clone();
        let task_store = state.task_store.clone();
        let push_sender = state.push_sender.clone();
        
        tokio::spawn(async move {
            // Wait a bit to simulate processing
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            
            println!("Updating task {} to Completed...", task_to_update.id);
            
            // Update status
            task_to_update.status = TaskStatus::new(TaskState::Completed);
            
            // Save to store
            let _ = task_store.save(task_to_update.clone()).await;
            
            // Manually trigger push notification for the update
            let _ = push_sender.send_notification(&task_to_update).await;
            println!("Push notification sent for task update.");
        });
    }

    Json(serde_json::to_value(result).unwrap())
}
