//! JSON-RPC server implementation for A2A protocol
//! 
//! This module provides a JSON-RPC server implementation that handles
//! A2A protocol requests over HTTP/HTTPS.

use crate::a2a::models::*;
use crate::a2a::server::context::ServerCallContextBuilder;
use crate::a2a::server::request_handlers::{RequestHandler, JSONRPCHandler};
use crate::a2a::utils::constants::*;
use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use futures::StreamExt;
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{error, info};

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// The address to bind the server to
    pub bind_addr: SocketAddr,
    /// The URL path for the agent card endpoint
    pub agent_card_path: String,
    /// The URL path for the JSON-RPC endpoint
    pub rpc_path: String,
    /// The URL path for the authenticated extended agent card endpoint
    pub extended_agent_card_path: String,
    /// Maximum content length for requests (in bytes)
    pub max_content_length: Option<usize>,
    /// CORS configuration
    pub enable_cors: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:8080".parse().unwrap(),
            agent_card_path: AGENT_CARD_WELL_KNOWN_PATH.to_string(),
            rpc_path: DEFAULT_RPC_URL.to_string(),
            extended_agent_card_path: EXTENDED_AGENT_CARD_PATH.to_string(),
            max_content_length: Some(10 * 1024 * 1024), // 10MB
            enable_cors: true,
        }
    }
}

/// Internal server state
#[derive(Clone)]
struct ServerState {
    agent_card: AgentCard,
    extended_agent_card: Option<AgentCard>,
    handler: Arc<JSONRPCHandler>,
    context_builder: Arc<dyn ServerCallContextBuilder>,
    config: ServerConfig,
}

/// A2A JSON-RPC Server
/// 
/// This server implements the A2A protocol over JSON-RPC 2.0, providing
/// HTTP endpoints for agent communication.
#[derive(Clone)]
pub struct A2AServer {
    state: Arc<RwLock<ServerState>>,
}

impl A2AServer {
    /// Create a new A2A server
    /// 
    /// # Arguments
    /// * `agent_card` - The AgentCard describing the agent's capabilities
    /// * `request_handler` - The handler for processing A2A requests
    /// * `context_builder` - Builder for creating server call contexts
    pub fn new(
        agent_card: AgentCard,
        request_handler: Arc<dyn RequestHandler>,
        context_builder: Arc<dyn ServerCallContextBuilder>,
    ) -> Self {
        let handler = Arc::new(JSONRPCHandler::new(
            agent_card.clone(),
            request_handler,
        ));

        let state = ServerState {
            agent_card,
            extended_agent_card: None,
            handler,
            context_builder,
            config: ServerConfig::default(),
        };

        Self {
            state: Arc::new(RwLock::new(state)),
        }
    }

    /// Set the extended agent card
    pub async fn with_extended_agent_card(self, card: AgentCard) -> Self {
        {
            let mut state = self.state.write().await;
            state.extended_agent_card = Some(card);
        }
        self
    }

    /// Set the server configuration
    pub async fn with_config(self, config: ServerConfig) -> Self {
        {
            let mut state = self.state.write().await;
            state.config = config;
        }
        self
    }

    /// Build the Axum router
    pub async fn build_router(&self) -> Router {
        let state = self.state.read().await.clone();
        let mut router = Router::new()
            .route(&state.config.agent_card_path, get(get_agent_card))
            .route(&state.config.rpc_path, post(handle_jsonrpc_request));

        // Add extended agent card endpoint if supported
        if state.agent_card.supports_authenticated_extended_card.unwrap_or(false) {
            router = router.route(
                &state.config.extended_agent_card_path,
                get(get_authenticated_extended_agent_card),
            );
        }

        // Add deprecated endpoint for backward compatibility
        if state.config.agent_card_path == AGENT_CARD_WELL_KNOWN_PATH {
            router = router.route(
                PREV_AGENT_CARD_WELL_KNOWN_PATH,
                get(get_agent_card),
            );
        }

        // Add CORS if enabled
        if state.config.enable_cors {
            router = router.layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            );
        }

        // Add tracing
        router = router.layer(TraceLayer::new_for_http());

        router.with_state(state)
    }

    /// Start the server
    pub async fn serve(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let state = self.state.read().await.clone();
        let router = self.build_router().await;

        info!(
            "Starting A2A server on {}",
            state.config.bind_addr
        );
        info!(
            "Agent card available at: {}",
            state.config.agent_card_path
        );
        info!("JSON-RPC endpoint at: {}", state.config.rpc_path);

        let listener = tokio::net::TcpListener::bind(state.config.bind_addr).await?;
        axum::serve(listener, router).await?;

        Ok(())
    }
}

/// Builder for creating an A2A server
pub struct A2AServerBuilder {
    agent_card: Option<AgentCard>,
    request_handler: Option<Arc<dyn RequestHandler>>,
    context_builder: Option<Arc<dyn ServerCallContextBuilder>>,
    extended_agent_card: Option<AgentCard>,
    config: ServerConfig,
}

impl A2AServerBuilder {
    /// Create a new server builder
    pub fn new() -> Self {
        Self {
            agent_card: None,
            request_handler: None,
            context_builder: None,
            extended_agent_card: None,
            config: ServerConfig::default(),
        }
    }

    /// Set the agent card
    pub fn with_agent_card(mut self, card: AgentCard) -> Self {
        self.agent_card = Some(card);
        self
    }

    /// Set the request handler
    pub fn with_request_handler(mut self, handler: Arc<dyn RequestHandler>) -> Self {
        self.request_handler = Some(handler);
        self
    }

    /// Set the context builder
    pub fn with_context_builder(mut self, builder: Arc<dyn ServerCallContextBuilder>) -> Self {
        self.context_builder = Some(builder);
        self
    }

    /// Set the extended agent card
    pub fn with_extended_agent_card(mut self, card: AgentCard) -> Self {
        self.extended_agent_card = Some(card);
        self
    }

    /// Set the server configuration
    pub fn with_config(mut self, config: ServerConfig) -> Self {
        self.config = config;
        self
    }

    /// Build the server
    pub fn build(self) -> Result<A2AServer, String> {
        let agent_card = self.agent_card.ok_or("Agent card is required")?;
        let request_handler = self.request_handler.ok_or("Request handler is required")?;
        let context_builder = self.context_builder
            .ok_or("Context builder is required")?;

        let state = ServerState {
            agent_card: agent_card.clone(),
            extended_agent_card: self.extended_agent_card,
            handler: Arc::new(JSONRPCHandler::new(
                agent_card.clone(),
                request_handler,
            )),
            context_builder,
            config: self.config,
        };

        Ok(A2AServer {
            state: Arc::new(RwLock::new(state)),
        })
    }
}

impl Default for A2AServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// HTTP handler for getting the agent card
async fn get_agent_card(
    State(state): State<ServerState>,
) -> impl IntoResponse {
    Json(serde_json::to_value(&state.agent_card).unwrap())
}

/// HTTP handler for getting the authenticated extended agent card
async fn get_authenticated_extended_agent_card(
    State(state): State<ServerState>,
) -> impl IntoResponse {
    if !state.agent_card.supports_authenticated_extended_card.unwrap_or(false) {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Extended agent card not supported or not enabled."
            })),
        );
    }

    if let Some(card) = &state.extended_agent_card {
        (StatusCode::OK, Json(serde_json::to_value(card).unwrap()))
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Authenticated extended agent card is supported but not configured on the server."
            })),
        )
    }
}

/// HTTP handler for JSON-RPC requests
async fn handle_jsonrpc_request(
    State(state): State<ServerState>,
    headers: HeaderMap,
    request: Request,
) -> impl IntoResponse {
    // Check content length
    if let Some(max_length) = state.config.max_content_length {
        if let Some(content_length) = headers.get("content-length") {
            if let Ok(length) = content_length.to_str().unwrap_or("0").parse::<usize>() {
                if length > max_length {
                    return error_response(
                        None,
                        &crate::a2a::jsonrpc::JSONRPCError::new(
                            crate::a2a::jsonrpc::standard_error_codes::INVALID_REQUEST,
                            "Payload too large".to_string(),
                        ),
                    );
                }
            }
        }
    }

    // Parse request body
    let body = match axum::body::to_bytes(request.into_body(), usize::MAX).await {
        Ok(body) => body,
        Err(e) => {
            error!("Failed to read request body: {}", e);
            return error_response(
                None,
                &crate::a2a::jsonrpc::JSONRPCError::new(
                    crate::a2a::jsonrpc::standard_error_codes::INVALID_REQUEST,
                    "Failed to read request body".to_string(),
                ),
            );
        }
    };

    // Parse JSON
    let json_value: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(e) => {
            error!("Failed to parse JSON: {}", e);
            return error_response(
                None,
                &crate::a2a::jsonrpc::JSONRPCError::new(
                    crate::a2a::jsonrpc::standard_error_codes::PARSE_ERROR,
                    format!("Invalid JSON: {}", e),
                ),
            );
        }
    };

    // Check if this is a streaming request
    let method = json_value.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let is_streaming = method == "message/stream";

    if is_streaming {
        // Handle streaming request
        handle_streaming_request(state, headers, json_value).await
    } else {
        // Handle non-streaming request
        handle_non_streaming_request(state, headers, json_value).await
    }
}

/// Handle streaming requests with SSE response
async fn handle_streaming_request(
    state: ServerState,
    headers: HeaderMap,
    json_value: Value,
) -> Response {
    // Build server call context
    let context = state.context_builder.build(&headers).await;

    // Parse the JSON-RPC request to get the ID
    let jsonrpc_request = match state.handler.parse_request(json_value.clone()) {
        Ok(req) => req,
        Err(e) => {
            return error_response(
                None,
                &e,
            );
        }
    };

    // Get the streaming SSE stream
    match state.handler.handle_message_stream_sse(jsonrpc_request, &context).await {
        Ok(sse_stream) => {
            let mut response_headers = HeaderMap::new();
            
            // Set SSE headers
            response_headers.insert("Content-Type", HeaderValue::from_static("text/event-stream"));
            response_headers.insert("Cache-Control", HeaderValue::from_static("no-cache"));
            response_headers.insert("Connection", HeaderValue::from_static("keep-alive"));
            
            // Add extension headers if any
            let extensions = context.get_activated_extensions();
            if !extensions.is_empty() {
                let ext_header = extensions.join(",");
                response_headers.insert(
                    "A2A-Extensions",
                    HeaderValue::from_str(&ext_header).unwrap(),
                );
            }

            // Convert SSE stream to Axum response
            let body_stream = sse_stream.map(|result| {
                match result {
                    Ok(sse_data) => Ok::<axum::body::Bytes, axum::Error>(axum::body::Bytes::from(sse_data)),
                    Err(_) => Ok::<axum::body::Bytes, axum::Error>(axum::body::Bytes::from("data: {\"error\":\"Stream error\"}\n\n")),
                }
            });

            let response = axum::response::Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "text/event-stream")
                .header("Cache-Control", "no-cache")
                .header("Connection", "keep-alive")
                .body(axum::body::Body::from_stream(body_stream))
                .unwrap();

            response
        }
        Err(error) => error_response(
            json_value.get("id").cloned(),
            &error,
        ),
    }
}

/// Handle non-streaming requests with JSON response
async fn handle_non_streaming_request(
    state: ServerState,
    headers: HeaderMap,
    json_value: Value,
) -> Response {
    // Build server call context
    let context = state.context_builder.build(&headers).await;

    // Handle the request
    match state.handler.handle_request(json_value.clone(), &context).await {
        Ok(response) => {
            let mut response_headers = HeaderMap::new();
            
            // Add extension headers if any
            let extensions = context.get_activated_extensions();
            if !extensions.is_empty() {
                let ext_header = extensions.join(",");
                response_headers.insert(
                    "A2A-Extensions",
                    HeaderValue::from_str(&ext_header).unwrap(),
                );
            }

            (StatusCode::OK, response_headers, Json(response)).into_response()
        }
        Err(error) => error_response(json_value.get("id").cloned(), &error),
    }
}

/// Create an error response
fn error_response(
    request_id: Option<Value>,
    error: &crate::a2a::jsonrpc::JSONRPCError,
) -> Response {
    let error_response = crate::a2a::jsonrpc::JSONRPCErrorResponse::new(
        request_id.and_then(|id| {
            match id {
                Value::String(s) => Some(crate::a2a::jsonrpc::JSONRPCId::String(s)),
                Value::Number(n) => n.as_i64().map(crate::a2a::jsonrpc::JSONRPCId::Number),
                Value::Null => Some(crate::a2a::jsonrpc::JSONRPCId::Null),
                _ => None,
            }
        }),
        error.clone(),
    );

    (
        StatusCode::OK,
        Json(serde_json::to_value(error_response).unwrap()),
    )
        .into_response()
}
