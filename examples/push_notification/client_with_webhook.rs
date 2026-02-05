//! A2A Client with Webhook Receiver
//! 
//! This client sends a message to the A2A server with a push notification configuration,
//! and then waits to receive the notification on its own HTTP port.

use a2a_rust::a2a::models::{
    MessageSendParams, MessageSendConfiguration, 
    PushNotificationConfig, Task
};
use a2a_rust::a2a::core_types::{Role, Part, Message};
use axum::{
    routing::post,
    Json, Router,
};
use std::net::SocketAddr;
use url::Url;
use reqwest;

#[tokio::main]
async fn main() {
    // 1. Start the Webhook Receiver in the background
    let webhook_addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let app = Router::new().route("/webhook", post(handle_webhook));
    
    println!("Webhook receiver listening at http://{}", webhook_addr);
    let listener = tokio::net::TcpListener::bind(webhook_addr).await.unwrap();
    
    // Spawn the webhook server
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give the server a moment to start
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // 2. Prepare the A2A Message with Push Configuration
    let message = Message::new(
        Role::User,
        vec![Part::text("Hello, please notify me when you are done.".to_string())],
    );

    let push_config = PushNotificationConfig::new(
        Url::parse("http://127.0.0.1:3000/webhook").unwrap()
    ).with_token("secret-client-token".to_string());

    let config = MessageSendConfiguration::new()
        .with_push_notification_config(push_config);

    let params = MessageSendParams::new(message)
        .with_configuration(config);

    // 3. Send the request to the A2A Server
    println!("Sending message to A2A Server (http://localhost:8080)...");
    let client = reqwest::Client::new();
    let response = client.post("http://localhost:8080/message/send")
        .json(&params)
        .send()
        .await
        .expect("Failed to send request to A2A server");

    if response.status().is_success() {
        let body = response.text().await.unwrap();
        // Parse as Task directly since message/send returns a Task
        match serde_json::from_str::<serde_json::Value>(&body) {
            Ok(value) => {
                if let Some(kind) = value.get("kind").and_then(|k| k.as_str()) {
                    if kind == "task" {
                        if let Ok(task) = serde_json::from_value::<Task>(value) {
                            println!("Task created successfully! ID: {}", task.id);
                            println!("Waiting for push notifications...");
                        }
                    }
                } else {
                    println!("Response: {}", serde_json::to_string_pretty(&value).unwrap());
                }
            }
            Err(e) => {
                println!("Warning: Failed to parse response: {}", e);
                println!("Response body: {}", body);
            }
        }
    } else {
        println!("Error from server: {}", response.status());
    }

    // Keep the client running to receive all webhook notifications
    // Wait long enough to receive both the initial notification and the completion notification
    println!("\nKeeping webhook server running to receive notifications...");
    println!("Press Ctrl+C to exit\n");
    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
    println!("\nClient demo finished.");
}

/// Handle incoming push notifications from the A2A Server
async fn handle_webhook(
    Json(payload): Json<serde_json::Value>,
) {
    println!("\n[WEBHOOK RECEIVED]");
    println!("Data: {}", serde_json::to_string_pretty(&payload).unwrap());
    
    if let Some(status) = payload.get("status") {
        if let Some(state) = status.get("state") {
            println!("Current Task State: {}", state);
        }
    }
}
