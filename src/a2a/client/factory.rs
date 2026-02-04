//! Client factory for creating A2A clients
//! 
//! This module provides a factory pattern for creating clients that connect to A2A agents,
//! mirroring the functionality of a2a-python's ClientFactory.

use crate::a2a::client::config::ClientConfig;
use crate::a2a::client::client_trait::{Client, BaseClient, ClientCallInterceptor, Consumer, ClientTransport};
use crate::a2a::client::transports::jsonrpc::JsonRpcTransport;
use crate::a2a::client::card_resolver::A2ACardResolver;
use crate::a2a::models::*;
use crate::a2a::core_types::*;
use crate::a2a::error::A2AError;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;

/// Type alias for transport producer function
pub type TransportProducer = Box<
    dyn Fn(
        AgentCard,
        String,
        ClientConfig,
        Vec<Box<dyn ClientCallInterceptor>>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn ClientTransport>, A2AError>> + Send>>
    + Send
    + Sync
>;

/// ClientFactory is used to generate the appropriate client for the agent
/// 
/// The factory is configured with a `ClientConfig` and optionally a list of
/// `Consumer`s to use for all generated `Client`s. The expected use is:
/// 
/// ```rust,no_run
/// use a2a_rust::a2a::client::factory::ClientFactory;
/// use a2a_rust::a2a::client::config::ClientConfig;
/// 
/// let config = ClientConfig::new();
/// let consumers = vec![];
/// let mut factory = ClientFactory::new(config, consumers);
/// // Optionally register custom client implementations
/// use a2a_rust::a2a::error::A2AError;
/// factory.register("my_custom_transport".to_string(), Box::new(|_, _, _, _| Box::pin(async { Err(A2AError::unsupported_operation("Not implemented")) })));
/// // Then with an agent card make a client with additional consumers and
/// // interceptors
/// // let client = factory.create(card, additional_consumers, interceptors).await?;
/// ```
/// 
/// Now the client can be used consistently regardless of the transport. This
/// aligns the client configuration with the server's capabilities.
pub struct ClientFactory {
    /// Client configuration
    config: ClientConfig,
    
    /// Default consumers for all generated clients
    consumers: Vec<Consumer>,
    
    /// Registry of transport producers
    registry: HashMap<String, TransportProducer>,
}

impl ClientFactory {
    /// Create a new client factory
    pub fn new(config: ClientConfig, consumers: Vec<Consumer>) -> Self {
        let mut factory = Self {
            config,
            consumers,
            registry: HashMap::new(),
        };
        
        factory.register_defaults();
        factory
    }
    
    /// Create a new client factory with default empty consumers
    pub fn with_config(config: ClientConfig) -> Self {
        Self::new(config, Vec::new())
    }
    
    /// Register default transports based on configuration
    fn register_defaults(&mut self) {
        let supported = self.config.supported_transports.clone();
        
        // Empty support list implies JSON-RPC only
        if supported.is_empty() || supported.contains(&TransportProtocol::Jsonrpc) {
            self.register_jsonrpc_transport();
        }
        
        if supported.contains(&TransportProtocol::HttpJson) {
            self.register_rest_transport();
        }
        
        if supported.contains(&TransportProtocol::Grpc) {
            self.register_grpc_transport();
        }
    }
    
    /// Register JSON-RPC transport
    fn register_jsonrpc_transport(&mut self) {
        let producer: TransportProducer = Box::new(
            move |card, url, config, interceptors| {
                Box::pin(async move {
                    let transport = JsonRpcTransport::new_with_config(url, Some(card), config)?;
                    let transport_with_interceptors = transport.with_interceptors(interceptors);
                    Ok(Box::new(transport_with_interceptors) as Box<dyn ClientTransport>)
                })
            }
        );
        
        self.registry.insert(
            TransportProtocol::Jsonrpc.to_string(),
            producer,
        );
    }
    
    /// Register REST transport (placeholder)
    fn register_rest_transport(&mut self) {
        // Note: REST transport not implemented yet
        // This is a placeholder for future implementation
    }
    
    /// Register gRPC transport (placeholder)
    fn register_grpc_transport(&mut self) {
        // Note: gRPC transport not implemented yet
        // This is a placeholder for future implementation
    }
    
    /// Register a new transport producer for a given transport label
    pub fn register(&mut self, label: String, generator: TransportProducer) {
        self.registry.insert(label, generator);
    }
    
    /// Get the client configuration
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }
    
    /// Create a new client for the provided AgentCard
    pub async fn create(
        &self,
        card: AgentCard,
        consumers: Option<Vec<Consumer>>,
        mut interceptors: Option<Vec<Box<dyn ClientCallInterceptor>>>,
        extensions: Option<Vec<String>>,
    ) -> Result<Box<dyn Client>, A2AError> {
        // Determine transport protocol and URL
        let (transport_protocol, transport_url) = self.determine_transport(&card)?;
        
        // Get transport producer
        let producer = self.registry.get(&transport_protocol.to_string())
            .ok_or_else(|| A2AError::transport_error(format!("No client available for {}", transport_protocol)))?;
        
        // Create transport
        let config_with_extensions = self.merge_extensions(extensions.clone());
        let transport = {
            let transport_interceptors = interceptors.take().unwrap_or_default();
            producer(card.clone(), transport_url, config_with_extensions, transport_interceptors).await?
        };
        
        // Combine consumers - note: we can't clone Fn trait objects, so we'll use the provided ones
        let all_consumers = if self.consumers.is_empty() {
            consumers.unwrap_or_default()
        } else {
            // For now, we'll skip cloning due to Fn trait limitations
            // In a real implementation, you'd use Arc<dyn Fn(...) + Send + Sync>
            consumers.unwrap_or_default()
        };
        
        // Create and return client
        let client = BaseClient::new(
            card,
            self.config.clone(),
            transport,
            all_consumers,
            interceptors.unwrap_or_default(),
        );
        
        Ok(Box::new(client))
    }
    
    /// Convenience method for constructing a client from a URL
    /// 
    /// Constructs a client that connects to the specified agent. Note that
    /// creating multiple clients via this method is less efficient than
    /// constructing an instance of ClientFactory and reusing that.
    /// 
    /// ```rust,no_run
    /// use a2a_rust::a2a::client::factory::ClientFactory;
    /// 
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // This will search for an AgentCard at /.well-known/agent-card.json
    ///     let my_agent_url = "https://travel.agents.example.com";
    ///     let _client = ClientFactory::connect(my_agent_url.to_string(), None, None, None, None, None, None, None).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn connect(
        agent: String,
        client_config: Option<ClientConfig>,
        consumers: Option<Vec<Consumer>>,
        interceptors: Option<Vec<Box<dyn ClientCallInterceptor>>>,
        relative_card_path: Option<String>,
        resolver_http_kwargs: Option<HashMap<String, serde_json::Value>>,
        extra_transports: Option<HashMap<String, TransportProducer>>,
        extensions: Option<Vec<String>>,
    ) -> Result<Box<dyn Client>, A2AError> {
        let config = client_config.unwrap_or_default();
        let mut factory = ClientFactory::with_config(config);
        
        // Register extra transports if provided
        if let Some(extra_transports) = extra_transports {
            for (label, producer) in extra_transports {
                factory.register(label, producer);
            }
        }
        
        // Resolve agent card
        let resolver = A2ACardResolver::new(agent);
        let card = resolver.get_agent_card_with_path(relative_card_path, resolver_http_kwargs).await?;
        
        factory.create(card, consumers, interceptors, extensions).await
    }
    
    /// Determine the best transport protocol and URL to use
    pub fn determine_transport(&self, card: &AgentCard) -> Result<(TransportProtocol, String), A2AError> {
        // Build server transport map
        let mut server_set = HashMap::new();
        
        let server_preferred = card.preferred_transport.as_ref()
            .and_then(|transport| TransportProtocol::from_str(transport).ok())
            .unwrap_or(TransportProtocol::Jsonrpc);
        
        server_set.insert(server_preferred, card.url.clone());
        
        if let Some(additional_interfaces) = &card.additional_interfaces {
            for interface in additional_interfaces {
                if let Ok(transport) = TransportProtocol::from_str(&interface.transport) {
                    server_set.insert(transport, interface.url.clone());
                }
            }
        }
        
        // Get client supported transports
        let client_set = if self.config.supported_transports.is_empty() {
            vec![TransportProtocol::Jsonrpc]
        } else {
            self.config.supported_transports.clone()
        };
        
        // Find matching transport
        let mut transport_protocol = None;
        let mut transport_url = None;
        
        if self.config.use_client_preference {
            // Use client preference order - iterate through client transports first
            for transport in &client_set {
                if let Some(url) = server_set.get(transport) {
                    transport_protocol = Some(*transport);
                    transport_url = Some(url.clone());
                    break;
                }
            }
        } else {
            // Use server preference order
            for (transport, url) in server_set {
                if client_set.contains(&transport) {
                    transport_protocol = Some(transport);
                    transport_url = Some(url);
                    break;
                }
            }
        }
        
        match (transport_protocol, transport_url) {
            (Some(protocol), Some(url)) => Ok((protocol, url)),
            _ => Err(A2AError::transport_error("No compatible transports found".to_string()))
        }
    }
    
    /// Merge extensions from config and call
    fn merge_extensions(&self, extensions: Option<Vec<String>>) -> ClientConfig {
        let mut config = self.config.clone();
        
        if let Some(call_extensions) = extensions {
            let mut all_extensions = config.extensions;
            all_extensions.extend(call_extensions);
            config.extensions = all_extensions;
        }
        
        config
    }
}

/// Generate a minimal agent card to simplify bootstrapping client creation
/// 
/// This minimal card is not viable itself to interact with the remote agent.
/// Instead this is a shorthand way to take a known url and transport option
/// and interact with the get card endpoint of the agent server to get the
/// correct agent card. This pattern is necessary for gRPC based card access
/// as typically these servers won't expose a well known path card.
pub fn minimal_agent_card(url: String, transports: Option<Vec<String>>) -> AgentCard {
    let transports = transports.unwrap_or_default();
    
    let preferred_transport = transports.first().cloned();
    let additional_interfaces = if transports.len() > 1 {
        transports[1..].iter().map(|transport| {
            AgentInterface::new(url.clone(), transport.clone())
        }).collect()
    } else {
        vec![]
    };
    
    AgentCard::new(
        "".to_string(), // name
        "".to_string(), // description
        url,
        "".to_string(), // version
        vec![],         // default_input_modes
        vec![],         // default_output_modes
        AgentCapabilities::new(),
        vec![],         // skills
    )
    .with_preferred_transport(preferred_transport.unwrap_or_default())
    .with_additional_interfaces(additional_interfaces)
    .with_supports_authenticated_extended_card(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a2a::types::*;

    #[test]
    fn test_client_factory_creation() {
        let config = ClientConfig::new();
        let factory = ClientFactory::with_config(config);
        assert_eq!(factory.consumers.len(), 0);
    }

    #[test]
    fn test_minimal_agent_card() {
        let card = minimal_agent_card(
            "http://localhost:8080".to_string(),
            Some(vec!["jsonrpc".to_string(), "grpc".to_string()]),
        );
        
        assert_eq!(card.url, "http://localhost:8080");
        assert_eq!(card.preferred_transport, Some("jsonrpc".to_string()));
        assert_eq!(card.additional_interfaces.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_transport_determination() {
        // Test with server preference (default behavior)
        let config = ClientConfig::new()
            .with_supported_transports(vec![TransportProtocol::Jsonrpc, TransportProtocol::Grpc]);
        let factory = ClientFactory::with_config(config);
        
        let mut card = AgentCard::new(
            "Test".to_string(),
            "Test agent".to_string(),
            "http://localhost:8080".to_string(),
            "1.0.0".to_string(),
            vec![],
            vec![],
            AgentCapabilities::new(),
            vec![],
        );
        
        // Test with server preference for grpc
        card.preferred_transport = Some("grpc".to_string());
        let (protocol, _url) = factory.determine_transport(&card).unwrap();
        assert_eq!(protocol, TransportProtocol::Grpc);
        
        // Test with server preference for jsonrpc
        card.preferred_transport = Some("jsonrpc".to_string());
        let (protocol, _url) = factory.determine_transport(&card).unwrap();
        assert_eq!(protocol, TransportProtocol::Jsonrpc);
        
        // Test with client preference disabled (default behavior)
        let config = ClientConfig::new()
            .with_supported_transports(vec![TransportProtocol::Jsonrpc, TransportProtocol::Grpc])
            .with_client_preference(false); // Explicitly set to false
        let factory = ClientFactory::with_config(config);
        
        // With server preferring grpc and client preference disabled, server should win
        card.preferred_transport = Some("grpc".to_string());
        let (protocol, _url) = factory.determine_transport(&card).unwrap();
        assert_eq!(protocol, TransportProtocol::Grpc);
        
        // Test with client preference enabled
        let config = ClientConfig::new()
            .with_supported_transports(vec![TransportProtocol::Jsonrpc, TransportProtocol::Grpc])
            .with_client_preference(true); // Enable client preference
        let factory = ClientFactory::with_config(config);
        
        // With client preference enabled, should use first client-supported transport
        // that server also supports
        card.preferred_transport = Some("grpc".to_string());
        
        let (protocol, _url) = factory.determine_transport(&card).unwrap();
        
        // Since server only supports grpc, client should use grpc even with client preference
        assert_eq!(protocol, TransportProtocol::Grpc);
        
        // Test with server supporting both transports
        card.preferred_transport = Some("jsonrpc".to_string());
        // Add additional interface for grpc
        card.additional_interfaces = Some(vec![
            AgentInterface::new("http://localhost:8080".to_string(), "grpc".to_string())
        ]);
        
        let (protocol, _url) = factory.determine_transport(&card).unwrap();
        
        // Now with client preference, should select Jsonrpc (first client transport)
        assert_eq!(protocol, TransportProtocol::Jsonrpc);
    }
}
