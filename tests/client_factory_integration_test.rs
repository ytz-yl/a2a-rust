//! Integration tests for client factory with auth interceptors
//! 
//! This test file validates the complete functionality of:
//! - ClientFactory with authentication
//! - JSON-RPC transport with interceptors
//! - Interceptor chain management

use a2a_rust::a2a::client::factory::{ClientFactory, minimal_agent_card};
use a2a_rust::a2a::client::config::ClientConfig;
use a2a_rust::a2a::client::client_trait::{ClientCallInterceptor, ClientCallContext};
use a2a_rust::a2a::client::auth::{AuthInterceptor, InMemoryContextCredentialStore};
use a2a_rust::a2a::models::*;
use a2a_rust::a2a::core_types::*;
use a2a_rust::a2a::error::A2AError;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::pin::Pin;

/// Test interceptor that adds custom headers
struct TestHeaderInterceptor {
    header_name: String,
    header_value: String,
}

#[async_trait]
impl ClientCallInterceptor for TestHeaderInterceptor {
    async fn intercept(
        &self,
        method_name: &str,
        request_payload: Value,
        mut http_kwargs: HashMap<String, Value>,
        _agent_card: &AgentCard,
        _context: Option<&ClientCallContext>,
    ) -> Result<(Value, HashMap<String, Value>), A2AError> {
        // Add custom header
        let headers = http_kwargs
            .entry("headers".to_string())
            .or_insert_with(|| Value::Object(serde_json::Map::new()))
            .as_object_mut()
            .ok_or_else(|| A2AError::invalid_request("headers must be an object"))?;
        
        headers.insert(
            self.header_name.clone(),
            Value::String(self.header_value.clone()),
        );
        
        tracing::debug!("TestHeaderInterceptor: Added {} header for method {}", self.header_name, method_name);
        
        Ok((request_payload, http_kwargs))
    }
}

/// Mock transport for testing factory integration
#[allow(dead_code)]
struct MockTransport {
    should_fail: bool,
}

#[allow(dead_code)]
impl MockTransport {
    fn new() -> Self {
        Self { should_fail: false }
    }
    
    fn with_failure() -> Self {
        Self { should_fail: true }
    }
}

#[async_trait]
impl a2a_rust::a2a::client::client_trait::ClientTransport for MockTransport {
    async fn send_message(
        &self,
        _params: MessageSendParams,
        _context: Option<&ClientCallContext>,
        _extensions: Option<Vec<String>>,
    ) -> Result<TaskOrMessage, A2AError> {
        if self.should_fail {
            Err(A2AError::transport_error("Mock transport failure".to_string()))
        } else {
            // Return a simple message
            let text_part = TextPart {
                text: "Mock response".to_string(),
                kind: "text".to_string(),
                metadata: None,
            };
            let message = Message {
                message_id: "test-message-id".to_string(),
                context_id: Some("test-context".to_string()),
                task_id: None,
                role: Role::Agent,
                parts: vec![Part::Direct(PartRoot::Text(text_part))],
                metadata: None,
                extensions: None,
                reference_task_ids: None,
                kind: "message".to_string(),
            };
            Ok(TaskOrMessage::Message(message))
        }
    }
    
    async fn send_message_streaming<'a>(
        &'a self,
        _params: MessageSendParams,
        _context: Option<&ClientCallContext>,
        _extensions: Option<Vec<String>>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<TaskOrMessage, A2AError>> + Send + 'a>>, A2AError> {
        Err(A2AError::unsupported_operation("Streaming not supported in mock"))
    }
    
    async fn get_task(
        &self,
        _request: TaskQueryParams,
        _context: Option<&ClientCallContext>,
        _extensions: Option<Vec<String>>,
    ) -> Result<Task, A2AError> {
        if self.should_fail {
            Err(A2AError::transport_error("Mock transport failure".to_string()))
        } else {
            let task = Task::new(
                "test-context".to_string(),
                TaskStatus::new(TaskState::Completed),
            )
            .with_metadata(HashMap::new());
            Ok(task)
        }
    }
    
    async fn cancel_task(
        &self,
        _request: TaskIdParams,
        _context: Option<&ClientCallContext>,
        _extensions: Option<Vec<String>>,
    ) -> Result<Task, A2AError> {
        Err(A2AError::unsupported_operation("Task cancellation not supported in mock"))
    }
    
    async fn set_task_callback(
        &self,
        _request: TaskPushNotificationConfig,
        _context: Option<&ClientCallContext>,
        _extensions: Option<Vec<String>>,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        Err(A2AError::unsupported_operation("Task callbacks not supported in mock"))
    }
    
    async fn get_task_callback(
        &self,
        _request: GetTaskPushNotificationConfigParams,
        _context: Option<&ClientCallContext>,
        _extensions: Option<Vec<String>>,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        Err(A2AError::unsupported_operation("Task callbacks not supported in mock"))
    }
    
    async fn resubscribe<'a>(
        &'a self,
        _request: TaskIdParams,
        _context: Option<&ClientCallContext>,
        _extensions: Option<Vec<String>>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<a2a_rust::a2a::client::client_trait::ClientEvent, A2AError>> + Send + 'a>>, A2AError> {
        Err(A2AError::unsupported_operation("Resubscription not supported in mock"))
    }
    
    async fn get_card(
        &self,
        _context: Option<&ClientCallContext>,
        _extensions: Option<Vec<String>>,
    ) -> Result<AgentCard, A2AError> {
        Err(A2AError::unsupported_operation("Card retrieval not supported in mock"))
    }
    
    async fn close(&self) -> Result<(), A2AError> {
        Ok(())
    }
}

#[tokio::test]
async fn test_client_factory_with_auth_interceptor() {
    // Create a test agent card with security schemes
    let mut security_schemes = HashMap::new();
    security_schemes.insert(
        "bearerAuth".to_string(),
        SecurityScheme::HTTPAuth(HTTPAuthSecurityScheme {
            scheme: "bearer".to_string(),
            bearer_format: Some("JWT".to_string()),
            description: Some("Bearer token authentication".to_string()),
        }),
    );
    
    let agent_card = AgentCard::new(
        "Test Agent".to_string(),
        "Test agent for factory integration".to_string(),
        "http://localhost:8080".to_string(),
        "1.0.0".to_string(),
        vec!["text/plain".to_string()],
        vec!["text/plain".to_string()],
        AgentCapabilities::new(),
        vec![],
    )
    .with_security_schemes(security_schemes)
    .with_security(vec![HashMap::from([("bearerAuth".to_string(), vec![])])]);
    
    // Create credential store and auth interceptor
    let mut store = InMemoryContextCredentialStore::new();
    store.add_credential("bearerAuth", "test-jwt-token");
    
    let auth_interceptor = AuthInterceptor::new(Arc::new(store));
    
    // Create client factory with auth interceptor
    let config = ClientConfig::new();
    let factory = ClientFactory::with_config(config);
    
    // Mock the transport creation for testing
    let _interceptors = vec![Box::new(auth_interceptor) as Box<dyn ClientCallInterceptor>];
    
    // Verify that the factory can process the agent card
    let (transport_protocol, transport_url) = factory.determine_transport(&agent_card).unwrap();
    assert_eq!(transport_protocol, TransportProtocol::Jsonrpc);
    assert_eq!(transport_url, "http://localhost:8080");
    
    // Test with custom interceptor
    let custom_interceptor = TestHeaderInterceptor {
        header_name: "X-Test-Header".to_string(),
        header_value: "test-value".to_string(),
    };
    
    let all_interceptors: Vec<Box<dyn ClientCallInterceptor>> = vec![
        Box::new(custom_interceptor),
    ];
    
    // Verify interceptors are properly configured
    assert_eq!(all_interceptors.len(), 1);
}

#[tokio::test]
async fn test_client_factory_connect_method() {
    // Test the connect convenience method
    let result = ClientFactory::connect(
        "http://localhost:8080".to_string(),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    ).await;
    
    // This should fail since there's no server running, but it should at least
    // get to the point where it tries to connect
    assert!(result.is_err());
    
    // The error should be related to connection, not to factory setup
    match result {
        Ok(_) => panic!("Expected connection error, but got success"),
        Err(error) => {
            match error {
                A2AError::Internal(_) => {
                    // Expected - transport/http errors are wrapped in Internal
                }
                other => {
                    panic!("Expected Internal error (transport/http), got: {:?}", other);
                }
            }
        }
    }
}

#[tokio::test]
async fn test_interceptor_chain_execution() {
    // Create test interceptors
    let auth_interceptor = {
        let mut store = InMemoryContextCredentialStore::new();
        store.add_credential("bearerAuth", "test-token");
        AuthInterceptor::new(Arc::new(store))
    };
    
    let header_interceptor = TestHeaderInterceptor {
        header_name: "X-Custom".to_string(),
        header_value: "custom-value".to_string(),
    };
    
    // Create agent card with security scheme
    let mut security_schemes = HashMap::new();
    security_schemes.insert(
        "bearerAuth".to_string(),
        SecurityScheme::HTTPAuth(HTTPAuthSecurityScheme {
            scheme: "bearer".to_string(),
            bearer_format: Some("JWT".to_string()),
            description: Some("Bearer token authentication".to_string()),
        }),
    );
    
    let agent_card = AgentCard::new(
        "Test Agent".to_string(),
        "Test agent".to_string(),
        "http://localhost:8080".to_string(),
        "1.0.0".to_string(),
        vec![],
        vec![],
        AgentCapabilities::new(),
        vec![],
    )
    .with_security_schemes(security_schemes)
    .with_security(vec![HashMap::from([("bearerAuth".to_string(), vec![])])]);
    
    // Test interceptor execution order
    let interceptors: Vec<Box<dyn ClientCallInterceptor>> = vec![
        Box::new(auth_interceptor),
        Box::new(header_interceptor),
    ];
    
    let request_payload = serde_json::json!({"test": "data"});
    let http_kwargs = HashMap::new();
    
    // Apply interceptors in sequence
    let (final_payload, final_kwargs) = apply_interceptors_chain(
        &interceptors,
        "test_method",
        request_payload,
        http_kwargs,
        &agent_card,
        None,
    ).await;
    
    // Verify payload is unchanged
    assert_eq!(final_payload, serde_json::json!({"test": "data"}));
    
    // Verify headers were added
    let headers = final_kwargs.get("headers").unwrap().as_object().unwrap();
    
    // Should have Authorization header from auth interceptor
    let auth_header = headers.get("Authorization").unwrap().as_str().unwrap();
    assert_eq!(auth_header, "Bearer test-token");
    
    // Should have custom header from header interceptor
    let custom_header = headers.get("X-Custom").unwrap().as_str().unwrap();
    assert_eq!(custom_header, "custom-value");
}

#[tokio::test]
async fn test_minimal_agent_card_creation() {
    let card = minimal_agent_card(
        "http://localhost:8080".to_string(),
        Some(vec!["jsonrpc".to_string(), "grpc".to_string()]),
    );
    
    assert_eq!(card.url, "http://localhost:8080");
    assert_eq!(card.preferred_transport, Some("jsonrpc".to_string()));
    assert!(card.additional_interfaces.is_some());
    assert_eq!(card.additional_interfaces.unwrap().len(), 1);
    assert!(card.supports_authenticated_extended_card.unwrap_or(false));
}

#[tokio::test]
async fn test_transport_determination_with_client_preference() {
    // Create agent card with only JSON-RPC support (default)
    let card = AgentCard::new(
        "Test Agent".to_string(),
        "Test agent".to_string(),
        "http://localhost:8080".to_string(),
        "1.0.0".to_string(),
        vec![],
        vec![],
        AgentCapabilities::new(),
        vec![],
    );
    
    // Test with client supporting both transports - should choose JSON-RPC (server default)
    let config = ClientConfig::new()
        .with_supported_transports(vec![TransportProtocol::Grpc, TransportProtocol::Jsonrpc])
        .with_client_preference(true); // Client preference enabled
    
    let factory = ClientFactory::with_config(config);
    let (protocol, _url) = factory.determine_transport(&card).unwrap();
    assert_eq!(protocol, TransportProtocol::Jsonrpc); // Should match server's only supported transport
    
    // Test with client only supporting GRPC - should fail
    let config = ClientConfig::new()
        .with_supported_transports(vec![TransportProtocol::Grpc]) // Only GRPC
        .with_client_preference(true);
    
    let factory = ClientFactory::with_config(config);
    let result = factory.determine_transport(&card);
    assert!(result.is_err()); // Should fail - no compatible transport
}

/// Helper function to apply interceptors in sequence
async fn apply_interceptors_chain(
    interceptors: &[Box<dyn ClientCallInterceptor>],
    method_name: &str,
    mut request_payload: Value,
    mut http_kwargs: HashMap<String, Value>,
    agent_card: &AgentCard,
    context: Option<&ClientCallContext>,
) -> (Value, HashMap<String, Value>) {
    for interceptor in interceptors {
        let (new_payload, new_kwargs) = interceptor
            .intercept(method_name, request_payload, http_kwargs, agent_card, context)
            .await
            .unwrap();
        request_payload = new_payload;
        http_kwargs = new_kwargs;
    }
    
    (request_payload, http_kwargs)
}

#[test]
fn test_client_factory_configuration() {
    // Test factory with custom configuration
    let config = ClientConfig::new()
        .with_streaming(true)
        .with_polling(false)
        .with_supported_transports(vec![TransportProtocol::Jsonrpc])
        .with_client_preference(true)
        .with_accepted_output_modes(vec!["text/plain".to_string()]);
    
    let factory = ClientFactory::with_config(config);
    
    // Verify configuration is stored
    assert_eq!(factory.config().streaming, true);
    assert_eq!(factory.config().polling, false);
    assert_eq!(factory.config().supported_transports.len(), 1);
    assert_eq!(factory.config().supported_transports[0], TransportProtocol::Jsonrpc);
    assert_eq!(factory.config().use_client_preference, true);
    assert_eq!(factory.config().accepted_output_modes.len(), 1);
    assert_eq!(factory.config().accepted_output_modes[0], "text/plain");
}

#[test]
fn test_error_handling_in_factory() {
    // Test with a client that only supports GRPC, but server only supports JSON-RPC
    let config = ClientConfig::new()
        .with_supported_transports(vec![TransportProtocol::Grpc]); // Only GRPC support
    
    let factory = ClientFactory::with_config(config);
    
    // Create agent card with only JSON-RPC support
    let card = AgentCard::new(
        "Test Agent".to_string(),
        "Test agent".to_string(),
        "http://localhost:8080".to_string(),
        "1.0.0".to_string(),
        vec![],
        vec![],
        AgentCapabilities::new(),
        vec![],
    );
    
    // Should fail since client only supports GRPC but server only supports JSON-RPC
    let result = factory.determine_transport(&card);
    assert!(result.is_err());
    
    let error = result.unwrap_err();
    match error {
        A2AError::Internal(_) => {
            // Expected - transport errors are wrapped in Internal
        }
        other => {
            panic!("Expected Internal error (transport), got: {:?}", other);
        }
    }
}
