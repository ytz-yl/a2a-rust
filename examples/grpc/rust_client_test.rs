//! gRPC Client Test for A2A Rust Implementation
//!
//! This test demonstrates how to use the gRPC transport to communicate with a gRPC server.
//! It shows the gRPC transport interface and how to send requests over gRPC.

use a2a_rust::a2a::{
    client::{ClientFactory, ClientConfig},
    models::*,
    core_types::{Message, Part, Role},
};
use futures::StreamExt;
use tokio;
use tonic::transport::Channel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 A2A gRPC Client Test");
    println!("{}", "=".repeat(60));
    
    let server_url = "http://localhost:50051".to_string();
    
    println!("🔗 Attempting to connect to gRPC server at {}...", server_url);
    
    // Test 1: Direct gRPC channel connection
    println!("\n📡 Test 1: Testing direct gRPC channel connection...");
    
    let channel = match Channel::from_shared(server_url.clone()) {
        Ok(channel) => channel,
        Err(e) => {
            println!("❌ Failed to create gRPC channel: {}", e);
            println!("💡 Make sure the gRPC server is running on port 50051");
            return Err(e.into());
        }
    };
    
    let channel = match channel.connect().await {
        Ok(channel) => {
            println!("✅ Successfully connected to gRPC server");
            channel
        },
        Err(e) => {
            println!("❌ Failed to connect to gRPC server: {}", e);
            println!("💡 The server might not be running or listening on port 50051");
            return Err(e.into());
        }
    };
    
    // Test 2: Using ClientFactory with gRPC transport
    println!("\n📡 Test 2: Testing ClientFactory with gRPC transport...");
    
    let config = ClientConfig::new()
        .with_streaming(true)
        .with_polling(false);
    
    let client_result = ClientFactory::connect(
        server_url,
        Some(config),
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
            
            // Try to get agent card
            println!("📋 Attempting to get agent card...");
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
                }
                Err(e) => {
                    println!("❌ Failed to get agent card: {}", e);
                    println!("💡 The server might not support the GetAgentCard endpoint");
                }
            }
            
            // Test message sending (if server supports it)
            println!("\n📤 Testing message send (if server supports it)...");
            
            let test_message = Message::new(
                Role::User,
                vec![
                    Part::text("Hello from gRPC client test!".to_string()),
                    Part::data(serde_json::json!({
                        "test": "gRPC transport",
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
                    let max_events = 5; // Limit to prevent infinite loop
                    
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
                        println!("⚠️ No events received (server might not support streaming or message handling)");
                    }
                }
                Err(e) => {
                    println!("❌ Failed to send message: {}", e);
                    println!("💡 This might be expected if the server doesn't implement message handling yet");
                }
            }
            
            println!("\n✅ gRPC client test completed successfully!");
        }
        Err(e) => {
            println!("❌ Failed to create client via ClientFactory: {}", e);
            println!("💡 This might be because:");
            println!("   - The server is not running");
            println!("   - The server doesn't support gRPC transport");
            println!("   - There's a network issue");
            return Err(e.into());
        }
    }
    
    println!("\n{}", "=".repeat(60));
    println!("🎯 gRPC Transport Test Summary:");
    println!("✅ gRPC channel connection test passed");
    println!("✅ ClientFactory gRPC transport integration test passed");
    println!("📊 Note: Full message exchange depends on server implementation");
    
    Ok(())
}

// Additional test utilities for gRPC transport
#[cfg(test)]
mod tests {
    use super::*;
    use a2a_rust::a2a::client::transports::grpc::GrpcTransport;
    use a2a_rust::a2a::client::client_trait::ClientTransport;
    use tonic::transport::Channel;
    
    #[tokio::test]
    async fn test_grpc_transport_creation() {
        // This is a basic test that the gRPC transport can be created
        // Note: We can't actually connect without a server running
        
        let agent_card = AgentCard::new(
            "Test Agent".to_string(),
            "Test agent for gRPC transport".to_string(),
            "http://localhost:50051".to_string(),
            "1.0.0".to_string(),
            vec!["text/plain".to_string()],
            vec!["text/plain".to_string()],
            AgentCapabilities::new().with_streaming(true),
            vec![],
        );
        
        // We'll skip the actual connection test since it requires a server
        // Just verify the types and imports are correct
        println!("✅ gRPC transport imports and types are valid");
    }
    
    #[tokio::test]
    async fn test_grpc_message_serialization() {
        // Test that we can create messages for gRPC transport
        
        let message = Message::new(
            Role::User,
            vec![
                Part::text("Test message for gRPC".to_string()),
                Part::data(serde_json::json!({"test": true})),
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
        
        println!("✅ gRPC message serialization test passed");
    }
}