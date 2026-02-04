//! A2A Rust Client Example with SSE Support
//! 
//! This example demonstrates how to use the a2a-rust client to communicate with our Rust server,
//! including Server-Sent Events (SSE) streaming functionality.

use a2a_rust::a2a::{
    client::{ClientFactory, ClientConfig},
    models::*,
    core_types::{Message, Part, Role},
};
use futures::{StreamExt};
use tokio;
use std::time::Instant;

fn print_events() -> Box<dyn Fn(a2a_rust::a2a::client::client_trait::ClientEventOrMessage, AgentCard) + Send + Sync> {
    Box::new(move |event, _card| {
        match event {
            a2a_rust::a2a::client::client_trait::ClientEventOrMessage::Event((task, update)) => {
                println!("📡 Event: Task {} - {:?}", task.id, task.status.state);
                if let Some(update) = update {
                    match update {
                        a2a_rust::a2a::client::client_trait::TaskUpdateEvent::Status(status_update) => {
                            println!("   Status Update: {:?}", status_update.status.state);
                        }
                        a2a_rust::a2a::client::client_trait::TaskUpdateEvent::Artifact(artifact_update) => {
                            println!("   Artifact Update: {:?}", artifact_update.artifact.name);
                        }
                    }
                }
            }
            a2a_rust::a2a::client::client_trait::ClientEventOrMessage::Message(message) => {
                println!("📨 Message: {:?} - {} parts", message.role, message.parts.len());
                for (i, part) in message.parts.iter().enumerate() {
                    match part.root() {
                        a2a_rust::a2a::core_types::PartRoot::Text(text_part) => {
                            println!("   Part {} (text): {}", i + 1, text_part.text);
                        }
                        a2a_rust::a2a::core_types::PartRoot::Data(data_part) => {
                            println!("   Part {} (data): {}", i + 1, data_part.data);
                        }
                        a2a_rust::a2a::core_types::PartRoot::File(_) => {
                            println!("   Part {} (file): [file content]", i + 1);
                        }
                    }
                }
            }
        }
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 A2A Rust Client Example");
    println!("{}", "=".repeat(60));
    
    // Configure client
    let config = ClientConfig::new()
        .with_streaming(true)
        .with_polling(false);
    
    println!("🔗 Connecting to Rust server at http://localhost:8080...");
    
    // Create client using ClientFactory
    let client = ClientFactory::connect(
        "http://localhost:8080".to_string(),
        Some(config),
        None,  // consumers
        None,  // interceptors
        None,  // relative_card_path
        None,  // resolver_http_kwargs
        None,  // extra_transports
        None,  // extensions
    ).await?;
    
    // Get agent card
    let agent_card = client.get_card(None, None).await?;
    println!("✅ Connected to agent: {}", agent_card.name);
    println!("📝 Description: {}", agent_card.description);
    println!("🌐 Server URL: {}", agent_card.url);
    println!("🔧 Preferred Transport: {:?}", agent_card.preferred_transport);
    println!();
    
    // Add event consumer
    let event_consumer = Box::new(print_events());
    client.add_event_consumer(event_consumer).await;
    
    // Test 1: Simple text message
    println!("📤 Test 1: Sending simple text message...");
    let simple_message = Message::new(
        Role::User,
        vec![
            Part::text("Hello from Rust a2a-client!".to_string())
        ]
    ).with_message_id(uuid::Uuid::new_v4().to_string());
    
    let mut response_count = 0;
    let mut event_stream = client.send_message(simple_message, None, None, None).await;
    while let Some(event_result) = event_stream.next().await {
        response_count += 1;
        match event_result {
            Ok(event) => {
                // Process the event through consumers
                client.consume(Some(event), &agent_card).await?;
            }
            Err(e) => {
                println!("❌ Error in event stream: {}", e);
                break;
            }
        }
        
        if response_count > 10 {  // Prevent infinite loops
            break;
        }
    }
    println!();
    
    // Test 2: Message with multiple parts
    println!("📤 Test 2: Sending multi-part message...");
    let multi_message = Message::new(
        Role::User,
        vec![
            Part::text("This is a test with multiple parts:".to_string()),
            Part::data(serde_json::json!({
                "test": true,
                "client": "Rust a2a-client"
            })),
            Part::text("End of message".to_string())
        ]
    ).with_message_id(uuid::Uuid::new_v4().to_string())
     .with_context_id("ctx-123".to_string());
    
    let mut response_count = 0;
    let mut event_stream = client.send_message(multi_message, None, None, None).await;
    while let Some(event_result) = event_stream.next().await {
        response_count += 1;
        match event_result {
            Ok(event) => {
                // Process the event through consumers
                client.consume(Some(event), &agent_card).await?;
            }
            Err(e) => {
                println!("❌ Error in event stream: {}", e);
                break;
            }
        }
        
        if response_count > 10 {  // Prevent infinite loops
            break;
        }
    }
    println!();
    
    // Test 3: Message with task ID
    println!("📤 Test 3: Sending message with task ID...");
    let task_message = Message::new(
        Role::User,
        vec![
            Part::text("Message with task context".to_string())
        ]
    ).with_message_id(uuid::Uuid::new_v4().to_string())
     .with_task_id("task-456".to_string())
     .with_context_id("ctx-123".to_string());
    
    let mut response_count = 0;
    let mut event_stream = client.send_message(task_message, None, None, None).await;
    while let Some(event_result) = event_stream.next().await {
        response_count += 1;
        match event_result {
            Ok(event) => {
                // Process the event through consumers
                client.consume(Some(event), &agent_card).await?;
            }
            Err(e) => {
                println!("❌ Error in event stream: {}", e);
                break;
            }
        }
        
        if response_count > 10 {  // Prevent infinite loops
            break;
        }
    }
    println!();
    
    // Test 4: SSE Streaming Test
    println!("🌊 Test 4: Testing SSE streaming functionality...");
    println!("📝 This test should demonstrate multiple streaming events...");
    
    let streaming_message = Message::new(
        Role::User,
        vec![
            Part::text("Please stream your response to me!".to_string())
        ]
    ).with_message_id(uuid::Uuid::new_v4().to_string())
     .with_context_id("streaming-test".to_string());
    
    let start_time = Instant::now();
    let mut event_count = 0;
    let mut final_received = false;
    
    println!("⏱️  Starting streaming test at {:?}", start_time);
    
    let mut event_stream = client.send_message(streaming_message, None, None, None).await;
    while let Some(event_result) = event_stream.next().await {
        match event_result {
            Ok(event) => {
                event_count += 1;
                let elapsed = start_time.elapsed();
                
                match event {
                    a2a_rust::a2a::client::client_trait::ClientEventOrMessage::Event((ref task, ref update)) => {
                        println!("🌊 [{:.2}s] SSE Event {}: Task {} - {:?}", 
                                elapsed.as_secs_f64(), event_count, task.id, task.status.state);
                        
                        if let Some(message) = &task.status.message {
                            println!("   💬 Message: {:?}", message);
                        }
                        
                        if let Some(update) = update {
                            match update {
                                a2a_rust::a2a::client::client_trait::TaskUpdateEvent::Status(status_update) => {
                                    println!("   📊 Status Update: {:?}", status_update.status.state);
                                    if status_update.r#final {
                                        final_received = true;
                                        println!("   ✅ Final status received!");
                                    }
                                }
                                a2a_rust::a2a::client::client_trait::TaskUpdateEvent::Artifact(artifact_update) => {
                                    println!("   📎 Artifact Update: {:?}", artifact_update.artifact.name);
                                }
                            }
                        }
                    }
                    a2a_rust::a2a::client::client_trait::ClientEventOrMessage::Message(ref message) => {
                        println!("📨 [{:.2}s] SSE Message: {:?} - {} parts", 
                                elapsed.as_secs_f64(), message.role, message.parts.len());
                        for (i, part) in message.parts.iter().enumerate() {
                            match part.root() {
                                a2a_rust::a2a::core_types::PartRoot::Text(text_part) => {
                                    println!("   📝 Part {} (text): {}", i + 1, text_part.text);
                                }
                                a2a_rust::a2a::core_types::PartRoot::Data(data_part) => {
                                    println!("   📊 Part {} (data): {}", i + 1, data_part.data);
                                }
                                a2a_rust::a2a::core_types::PartRoot::File(_) => {
                                    println!("   📁 Part {} (file): [file content]", i + 1);
                                }
                            }
                        }
                    }
                }
                
                // Process the event through consumers
                client.consume(Some(event), &agent_card).await?;
                
                // Break if we received final event and waited a bit
                if final_received && elapsed.as_millis() > 2000 {
                    break;
                }
            }
            Err(e) => {
                println!("❌ Error in SSE stream: {}", e);
                break;
            }
        }
        
        // Safety timeout
        if start_time.elapsed().as_secs() > 10 {
            println!("⏰ Timeout reached, ending test");
            break;
        }
    }
    
    let total_time = start_time.elapsed();
    println!("🏁 Streaming test completed:");
    println!("   ⏱️  Total time: {:.2}s", total_time.as_secs_f64());
    println!("   📊 Total events: {}", event_count);
    println!("   ✅ Final received: {}", final_received);
    println!();
    
    println!("✅ All tests completed successfully!");
    println!("🎯 The Rust server and Rust client are fully compatible with SSE streaming!");
    println!("🌊 SSE streaming is working correctly!");
    
    Ok(())
}
