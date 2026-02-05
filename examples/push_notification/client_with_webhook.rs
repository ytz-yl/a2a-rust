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
    // Use environment variable or default to 3000
    let webhook_port: u16 = std::env::var("WEBHOOK_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    // Use environment variable or default to 8080 for A2A Server
    let server_port: u16 = std::env::var("A2A_SERVER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    
    let webhook_addr = SocketAddr::from(([127, 0, 0, 1], webhook_port));
    let app = Router::new().route("/webhook", post(handle_webhook));
    
    println!("Webhook receiver listening at http://{}", webhook_addr);
    let listener = match tokio::net::TcpListener::bind(webhook_addr).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("Failed to bind to port {}: {}", webhook_port, e);
            eprintln!("Please ensure the port is not already in use or set WEBHOOK_PORT environment variable to a different port.");
            std::process::exit(1);
        }
    };
    
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

    let webhook_url = format!("http://127.0.0.1:{}/webhook", webhook_port);
    let push_config = PushNotificationConfig::new(
        Url::parse(&webhook_url).unwrap()
    ).with_token("secret-client-token".to_string());

    let config = MessageSendConfiguration::new()
        .with_push_notification_config(push_config);

    let params = MessageSendParams::new(message)
        .with_configuration(config);

    // 3. Send the request to the A2A Server
    let server_url = format!("http://localhost:{}/message/send", server_port);
    println!("Sending message to A2A Server ({})...", server_url);
    let client = reqwest::Client::new();
    let response = client.post(&server_url)
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
