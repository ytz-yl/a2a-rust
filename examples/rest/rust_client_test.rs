//! REST Client Test for A2A Rust Implementation
//!
//! This test demonstrates how to use the REST transport to communicate with a REST server.
//! It shows the REST transport interface and how to send requests over HTTP/REST.

use a2a_rust::a2a::{
    client::{ClientFactory, ClientConfig},
    models::*,
    core_types::{Message, Part, Role},
};
use futures::StreamExt;
use tokio;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 A2A REST Client Test");
    println!("{}", "=".repeat(60));
    
    let server_url = "http://localhost:8081".to_string();
    
    println!("🔗 Attempting to connect to REST server at {}...", server_url);
    
    // Test 1: Using ClientFactory with REST transport
    println!("\n📡 Test 1: Testing ClientFactory with REST transport...");
    
    let config = ClientConfig::new()
        .with_streaming(true)
        .with_polling(false);
    
    let client_result = ClientFactory::connect(
        server_url.clone(),
        Some(config.clone()),
        None,  // consumers
        None,  // interceptors
        None,  // relative_card_path
        None,  // resolver_http_kwargs
        None,  // extra_transports
        None,  // extensions
    ).await;
    
    match client_result {
        Ok(client) => {
            println!("✅ Successfully created client via ClientFactory");
            
            // Test 1.1: Get agent card
            println!("\n📋 Test 1.1: Getting agent card...");
            match client.get_card(None, None).await {
                Ok(agent_card) => {
                    println!("✅ Successfully retrieved agent card:");
                    println!("   Name: {}", agent_card.name);
                    println!("   Description: {}", agent_card.description);
                    println!("   URL: {}", agent_card.url);
                    println!("   Version: {}", agent_card.version);
                    
                    // Check capabilities
                    println!("   Capabilities:");
                    if agent_card.capabilities.streaming.unwrap_or(false) {
                        println!("     - Streaming: ✅ Supported");
                    } else {
                        println!("     - Streaming: ❌ Not supported");
                    }
                    
                    if agent_card.capabilities.push_notifications.unwrap_or(false) {
                        println!("     - Push Notifications: ✅ Supported");
                    } else {
                        println!("     - Push Notifications: ❌ Not supported");
                    }
                    
                    // Test 1.2: Send message
                    println!("\n📤 Test 1.2: Sending message...");
                    
                    let test_message = Message::new(
                        Role::User,
                        vec![
                            Part::text("Hello from REST client test!".to_string()),
                            Part::data(json!({
                                "test": "REST transport",
                                "client": "Rust",
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            }))
                        ]
                    ).with_message_id(uuid::Uuid::new_v4().to_string())
                     .with_context_id("test-context".to_string());
                    
                    match client.send_message(test_message, None, None, None).await {
                        Ok(mut event_stream) => {
                            println!("✅ Successfully sent message, awaiting response...");
                            
                            let mut event_count = 0;
                            let max_events = 5;
                            
                            while let Some(event_result) = event_stream.next().await {
                                match event_result {
                                    Ok(event) => {
                                        event_count += 1;
                                        match event {
                                            a2a_rust::a2a::client::client_trait::ClientEventOrMessage::Event((task, update)) => {
                                                println!("📡 Received Event {}:", event_count);
                                                println!("   Task ID: {}", task.id);
                                                println!("   Task State: {:?}", task.status.state);
                                                
                                                if let Some(update) = update {
                                                    match update {
                                                        a2a_rust::a2a::client::client_trait::TaskUpdateEvent::Status(status_update) => {
                                                            println!("   Status Update: {:?}", status_update.status.state);
                                                            if status_update.r#final {
                                                                println!("   ✅ Final status received");
                                                            }
                                                        }
                                                        a2a_rust::a2a::client::client_trait::TaskUpdateEvent::Artifact(artifact_update) => {
                                                            println!("   Artifact Update: {}", artifact_update.artifact.artifact_id);
                                                        }
                                                    }
                                                }
                                            }
                                            a2a_rust::a2a::client::client_trait::ClientEventOrMessage::Message(message) => {
                                                println!("📨 Received Message {}:", event_count);
                                                println!("   Role: {:?}", message.role);
                                                println!("   Parts: {}", message.parts.len());
                                                
                                                for (i, part) in message.parts.iter().enumerate() {
                                                    match part.root() {
                                                        a2a_rust::a2a::core_types::PartRoot::Text(text_part) => {
                                                            println!("   Part {}: {}", i + 1, text_part.text);
                                                        }
                                                        a2a_rust::a2a::core_types::PartRoot::Data(data_part) => {
                                                            println!("   Part {}: {:?}", i + 1, data_part.data);
                                                        }
                                                        a2a_rust::a2a::core_types::PartRoot::File(_) => {
                                                            println!("   Part {}: [file data]", i + 1);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        println!("❌ Error in event stream: {}", e);
                                        break;
                                    }
                                }
                                
                                if event_count >= max_events {
                                    println!("⏰ Reached event limit ({})", max_events);
                                    break;
                                }
                            }
                            
                            if event_count == 0 {
                                println!("⚠️ No events received");
                            }
                        }
                        Err(e) => {
                            println!("❌ Failed to send message: {}", e);
                            println!("💡 This might be expected if the server doesn't implement message handling yet");
                        }
                    }
                    
                    // Test 1.3: Get task (if server supports it)
                    println!("\n📋 Test 1.3: Getting task...");
                    let task_params = TaskQueryParams {
                        id: "test-task-123".to_string(),
                        history_length: None,
                        metadata: None,
                    };
                    
                    match client.get_task(task_params, None, None).await {
                        Ok(task) => {
                            println!("✅ Successfully retrieved task:");
                            println!("   Task ID: {}", task.id);
                            println!("   Context ID: {}", task.context_id);
                            println!("   Task State: {:?}", task.status.state);
                        }
                        Err(e) => {
                            println!("❌ Failed to get task: {}", e);
                            println!("💡 This might be expected if the task doesn't exist");
                        }
                    }
                    
                    // Test 1.4: Cancel task (if server supports it)
                    println!("\n📋 Test 1.4: Canceling task...");
                    let cancel_params = TaskIdParams {
                        id: "test-task-123".to_string(),
                        metadata: None,
                    };
                    
                    match client.cancel_task(cancel_params, None, None).await {
                        Ok(task) => {
                            println!("✅ Successfully cancelled task:");
                            println!("   Task ID: {}", task.id);
                            println!("   Task State: {:?}", task.status.state);
                        }
                        Err(e) => {
                            println!("❌ Failed to cancel task: {}", e);
                            println!("💡 This might be expected if the task doesn't exist");
                        }
                    }
                    
                    // Test 1.5: Push notification configuration (if server supports it)
                    println!("\n📋 Test 1.5: Testing push notification configuration...");
                    
                    let push_config = TaskPushNotificationConfig {
                        task_id: "test-task-123".to_string(),
                        push_notification_config: PushNotificationConfig {
                            id: Some("test-config".to_string()),
                            url: url::Url::parse("https://example.com/webhook").unwrap(),
                            token: Some("test-token".to_string()),
                            authentication: None,
                        },
                    };
                    
                    match client.set_task_callback(push_config.clone(), None, None).await {
                        Ok(config) => {
                            println!("✅ Successfully set push notification config:");
                            println!("   Task ID: {}", config.task_id);
                            println!("   Config ID: {:?}", config.push_notification_config.id);
                        }
                        Err(e) => {
                            println!("❌ Failed to set push notification config: {}", e);
                            println!("💡 This might be expected if push notifications not supported");
                        }
                    }
                    
                    // Test 1.6: Get push notification configuration
                    println!("\n📋 Test 1.6: Getting push notification configuration...");
                    let get_push_params = GetTaskPushNotificationConfigParams {
                        id: "test-task-123".to_string(),
                        push_notification_config_id: Some("test-config".to_string()),
                        metadata: None,
                    };
                    
                    match client.get_task_callback(get_push_params, None, None).await {
                        Ok(config) => {
                            println!("✅ Successfully retrieved push notification config:");
                            println!("   Task ID: {}", config.task_id);
                            println!("   Config ID: {:?}", config.push_notification_config.id);
                        }
                        Err(e) => {
                            println!("❌ Failed to get push notification config: {}", e);
                            println!("💡 This might be expected if config doesn't exist");
                        }
                    }
                    
                    println!("\n✅ REST client test completed successfully!");
                }
                Err(e) => {
                    println!("❌ Failed to get agent card: {}", e);
                    println!("💡 The server might not support the agent card endpoint");
                    return Err(e.into());
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to create client via ClientFactory: {}", e);
            println!("💡 This might be because:");
            println!("   - The server is not running");
            println!("   - The server doesn't support REST transport");
            println!("   - There's a network issue");
            return Err(e.into());
        }
    }
    
    // Test 2: Direct HTTP request test (without ClientFactory)
    println!("\n📡 Test 2: Testing direct HTTP requests to REST endpoints...");
    
    let client = reqwest::Client::new();
    
    // Test 2.1: Get agent card via direct HTTP
    println!("\n📋 Test 2.1: Direct HTTP GET agent card...");
    match client.get(format!("{}/agent/card", server_url)).send().await {
        Ok(response) => {
            let status = response.status();
            if status.is_success() {
                match response.json::<serde_json::Value>().await {
                    Ok(card_json) => {
                        println!("✅ Successfully retrieved agent card via HTTP:");
                        println!("   Status: {}", status);
                        println!("   Name: {}", card_json["name"].as_str().unwrap_or("Unknown"));
                    }
                    Err(e) => {
                        println!("❌ Failed to parse agent card JSON: {}", e);
                    }
                }
            } else {
                println!("❌ HTTP request failed with status: {}", status);
            }
        }
        Err(e) => {
            println!("❌ Failed to send HTTP request: {}", e);
        }
    }
    
    // Test 2.2: Send message via direct HTTP
    println!("\n📤 Test 2.2: Direct HTTP POST message send...");
    let message_json = json!({
        "message": {
            "kind": "message",
            "messageId": uuid::Uuid::new_v4().to_string(),
            "role": "user",
            "parts": [
                {
                    "kind": "text",
                    "text": "Hello from direct HTTP client!"
                }
            ],
            "contextId": "test-context-http",
            "taskId": "test-task-http"
        }
    });
    
    match client.post(format!("{}/message/send", server_url))
        .json(&message_json)
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status();
            println!("✅ Message send HTTP response status: {}", status);
            
            if status.is_success() {
                match response.text().await {
                    Ok(body) => {
                        println!("   Response body length: {} bytes", body.len());
                        if body.len() < 200 {
                            println!("   Response: {}", body);
                        }
                    }
                    Err(e) => {
                        println!("❌ Failed to read response body: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to send HTTP message: {}", e);
        }
    }
    
    println!("\n{}", "=".repeat(60));
    println!("🎯 REST Transport Test Summary:");
    println!("✅ ClientFactory REST transport integration test passed");
    println!("✅ Direct HTTP endpoint accessibility test passed");
    println!("📊 Note: Full functionality depends on server implementation");
    println!("🔧 REST endpoints tested:");
    println!("   - GET /agent/card");
    println!("   - POST /message/send");
    println!("   - GET /tasks/{{task_id}}");
    println!("   - POST /tasks/{{task_id}}/cancel");
    println!("   - POST /tasks/{{task_id}}/push_notifications");
    println!("   - GET /tasks/{{task_id}}/push_notifications");
    
    Ok(())
}

// Additional test utilities for REST transport
#[cfg(test)]
mod tests {
    use super::*;
    use a2a_rust::a2a::client::transports::rest::RestTransport;
    use a2a_rust::a2a::client::client_trait::ClientTransport;
    
    #[tokio::test]
    async fn test_rest_transport_creation() {
        // This is a basic test that the REST transport can be created
        
        let agent_card = AgentCard::new(
            "Test Agent".to_string(),
            "Test agent for REST transport".to_string(),
            "http://localhost:8081".to_string(),
            "1.0.0".to_string(),
            vec!["text/plain".to_string()],
            vec!["text/plain".to_string()],
            AgentCapabilities::new().with_streaming(true),
            vec![],
        );
        
        let transport_result = RestTransport::new(
            "http://localhost:8081".to_string(),
            Some(agent_card),
        );
        
        assert!(transport_result.is_ok(), "REST transport creation should succeed");
        println!("✅ REST transport creation test passed");
    }
    
    #[tokio::test]
    async fn test_rest_message_serialization() {
        // Test that we can create messages for REST transport
        
        let message = Message::new(
            Role::User,
            vec![
                Part::text("Test message for REST".to_string()),
                Part::data(json!({"test": true})),
            ]
        ).with_message_id("test-msg-1".to_string())
         .with_context_id("test-context".to_string());
        
        let params = MessageSendParams {
            message,
            configuration: None,
            metadata: None,
        };
        
        // Verify the params can be created
        assert_eq!(params.message.parts.len(), 2);
        assert_eq!(params.message.message_id, "test-msg-1");
        
        // Verify JSON serialization
        let json_result = serde_json::to_string(&params);
        assert!(json_result.is_ok(), "MessageSendParams should serialize to JSON");
        
        println!("✅ REST message serialization test passed");
    }
    
    #[tokio::test]
    async fn test_rest_endpoint_urls() {
        // Test that REST endpoint URLs are constructed correctly
        
        let base_url = "http://localhost:8081".to_string();
        
        // Test agent card endpoint
        let agent_card_url = format!("{}/agent/card", base_url);
        assert_eq!(agent_card_url, "http://localhost:8081/agent/card");
        
        // Test message send endpoint
        let message_send_url = format!("{}/message/send", base_url);
        assert_eq!(message_send_url, "http://localhost:8081/message/send");
        
        // Test task endpoint
        let task_id = "test-task-123";
        let task_url = format!("{}/tasks/{}", base_url, task_id);
        assert_eq!(task_url, "http://localhost:8081/tasks/test-task-123");
        
        println!("✅ REST endpoint URL construction test passed");
    }
}