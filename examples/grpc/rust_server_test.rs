//! gRPC Server Test for A2A Rust Implementation
//!
//! This test demonstrates a basic gRPC server implementation for A2A protocol.
//! It shows how to set up a gRPC server that can handle A2A requests.

use a2a_rust::a2a::{
    models::*,
    core_types::{Message, Part, Role, TaskStatus, TaskState},
    server::{
        context::DefaultServerCallContextBuilder,
        request_handlers::{RequestHandler, MessageSendResult, TaskPushNotificationConfigQueryParams, Event},
    },
    grpc::a2a_pb2::a2a::{a2a_service_server::A2aServiceServer, *},
};
use futures::Stream;
use std::net::SocketAddr;
use std::sync::Arc;
use std::pin::Pin;
use tonic::{transport::Server, Request, Response, Status, Streaming};
use uuid::Uuid;

/// Simple echo handler for gRPC testing
struct GrpcTestHandler;

impl GrpcTestHandler {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl RequestHandler for GrpcTestHandler {
    async fn on_get_task(
        &self,
        params: TaskQueryParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<Option<Task>, a2a_rust::a2a::error::A2AError> {
        println!("gRPC Test Handler: Get task {}", params.id);
        Ok(None)
    }

    async fn on_cancel_task(
        &self,
        params: TaskIdParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<Option<Task>, a2a_rust::a2a::error::A2AError> {
        println!("gRPC Test Handler: Cancel task {}", params.id);
        Ok(None)
    }

    async fn on_message_send(
        &self,
        params: MessageSendParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<MessageSendResult, a2a_rust::a2a::error::A2AError> {
        println!("gRPC Test Handler: Message send with {} parts", params.message.parts.len());
        
        let mut response_parts = Vec::new();
        response_parts.push(Part::text("gRPC Echo: ".to_string()));
        
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
        println!("gRPC Test Handler: Message stream with {} parts", params.message.parts.len());
        
        let context_id = params.message.context_id.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
        let task_id = params.message.task_id.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
        
        let status_message = Message::new(
            Role::Agent,
            vec![Part::text("Processing gRPC streaming message...".to_string())]
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
                vec![Part::text("gRPC Streaming Response".to_string())]
            )
            .with_context_id(context_id.clone())
            .with_task_id(task_id.clone());
            
            yield Ok(Event::Message(response_message));
            
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            let final_status = TaskStatusUpdateEvent::new(
                task_id.clone(),
                context_id,
                TaskStatus::new(TaskState::Completed).with_message(
                    Message::new(Role::Agent, vec![Part::text("gRPC stream completed".to_string())])
                ),
                true,
            );
            yield Ok(Event::TaskStatusUpdate(final_status));
        };
        
        Ok(Box::pin(stream))
    }

    async fn on_set_task_push_notification_config(
        &self,
        _params: TaskPushNotificationConfig,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<TaskPushNotificationConfig, a2a_rust::a2a::error::A2AError> {
        Err(a2a_rust::a2a::error::A2AError::unsupported_operation("Push notifications not supported in test"))
    }

    async fn on_get_task_push_notification_config(
        &self,
        _params: TaskPushNotificationConfigQueryParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<TaskPushNotificationConfig, a2a_rust::a2a::error::A2AError> {
        Err(a2a_rust::a2a::error::A2AError::unsupported_operation("Push notifications not supported in test"))
    }

    async fn on_list_task_push_notification_config(
        &self,
        _params: TaskIdParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<Vec<TaskPushNotificationConfig>, a2a_rust::a2a::error::A2AError> {
        Ok(vec![])
    }

    async fn on_delete_task_push_notification_config(
        &self,
        _params: DeleteTaskPushNotificationConfigParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<(), a2a_rust::a2a::error::A2AError> {
        Ok(())
    }

    async fn on_resubscribe_to_task(
        &self,
        params: TaskIdParams,
        _context: Option<&a2a_rust::a2a::server::context::ServerCallContext>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Event, a2a_rust::a2a::error::A2AError>> + Send>>, a2a_rust::a2a::error::A2AError> {
        println!("gRPC Test Handler: Resubscribe to task {}", params.id);
        
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

/// gRPC service implementation for testing
struct GrpcTestService {
    agent_card: AgentCard,
    handler: Arc<GrpcTestHandler>,
    context_builder: Arc<DefaultServerCallContextBuilder>,
}

impl GrpcTestService {
    fn new() -> Self {
        let agent_card = AgentCard::new(
            "gRPC Test Server".to_string(),
            "A gRPC test server for A2A protocol".to_string(),
            "http://localhost:50051".to_string(),
            "1.0.0".to_string(),
            vec!["text/plain".to_string()],
            vec!["text/plain".to_string()],
            AgentCapabilities::new().with_streaming(true),
            vec![],
        );

        Self {
            agent_card,
            handler: Arc::new(GrpcTestHandler::new()),
            context_builder: Arc::new(DefaultServerCallContextBuilder),
        }
    }
}

#[tonic::async_trait]
impl A2aService for GrpcTestService {
    /// Get agent card
    async fn get_agent_card(
        &self,
        request: Request<GetAgentCardRequest>,
    ) -> Result<Response<AgentCardResponse>, Status> {
        println!("gRPC: GetAgentCard request from {:?}", request.remote_addr());
        
        // In a real implementation, we would convert AgentCard to protobuf
        // For now, return a simple response
        let response = AgentCardResponse {
            card: Some(self.agent_card.clone().into()),
        };
        
        Ok(Response::new(response))
    }

    /// Send message
    async fn send_message(
        &self,
        request: Request<SendMessageRequest>,
    ) -> Result<Response<SendMessageResponse>, Status> {
        println!("gRPC: SendMessage request");
        
        // In a real implementation, handle the message
        // For test purposes, return a success response
        let response = SendMessageResponse {
            task: None,
            msg: None,
        };
        
        Ok(Response::new(response))
    }

    /// Streaming message send
    type SendStreamingMessageStream = tonic::codec::Streaming<StreamResponse>;
    
    async fn send_streaming_message(
        &self,
        request: Request<SendMessageRequest>,
    ) -> Result<Response<Self::SendStreamingMessageStream>, Status> {
        println!("gRPC: SendStreamingMessage request");
        
        // Create a simple streaming response for testing
        let stream = async_stream::stream! {
            yield Ok(StreamResponse {
                response: Some(stream_response::Response::Task(
                    TaskResponse {
                        task: Some(Task::new(
                            "test-context".to_string(),
                            TaskStatus::new(TaskState::Working)
                        ).with_task_id("test-task".to_string()).into())
                    }
                ))
            });
            
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            yield Ok(StreamResponse {
                response: Some(stream_response::Response::Msg(
                    MessageResponse {
                        msg: Some(Message::new(Role::Agent, vec![Part::text("Test message".to_string())]).into())
                    }
                ))
            });
        };
        
        Ok(Response::new(Box::pin(stream)))
    }

    /// Get task
    async fn get_task(
        &self,
        request: Request<GetTaskRequest>,
    ) -> Result<Response<TaskResponse>, Status> {
        let req = request.into_inner();
        println!("gRPC: GetTask request for {}", req.name);
        
        let response = TaskResponse {
            task: None,
        };
        
        Ok(Response::new(response))
    }

    /// Cancel task
    async fn cancel_task(
        &self,
        request: Request<CancelTaskRequest>,
    ) -> Result<Response<TaskResponse>, Status> {
        let req = request.into_inner();
        println!("gRPC: CancelTask request for {}", req.name);
        
        let response = TaskResponse {
            task: None,
        };
        
        Ok(Response::new(response))
    }

    /// Task subscription
    type TaskSubscriptionStream = tonic::codec::Streaming<StreamResponse>;
    
    async fn task_subscription(
        &self,
        request: Request<TaskSubscriptionRequest>,
    ) -> Result<Response<Self::TaskSubscriptionStream>, Status> {
        let req = request.into_inner();
        println!("gRPC: TaskSubscription request for {}", req.name);
        
        let stream = async_stream::stream! {
            yield Ok(StreamResponse {
                response: Some(stream_response::Response::Task(
                    TaskResponse {
                        task: Some(Task::new(
                            "test-context".to_string(),
                            TaskStatus::new(TaskState::Working)
                        ).with_task_id("test-task".to_string()).into())
                    }
                ))
            });
            
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            yield Ok(StreamResponse {
                response: Some(stream_response::Response::StatusUpdate(
                    TaskStatusUpdateResponse {
                        update: Some(TaskStatusUpdateEvent::new(
                            "test-task".to_string(),
                            "test-context".to_string(),
                            TaskStatus::new(TaskState::Completed),
                            true,
                        ).into())
                    }
                ))
            });
        };
        
        Ok(Response::new(Box::pin(stream)))
    }

    /// Create task push notification config
    async fn create_task_push_notification_config(
        &self,
        request: Request<CreateTaskPushNotificationConfigRequest>,
    ) -> Result<Response<TaskPushNotificationConfigResponse>, Status> {
        println!("gRPC: CreateTaskPushNotificationConfig request");
        
        let response = TaskPushNotificationConfigResponse {
            config: None,
        };
        
        Ok(Response::new(response))
    }

    /// Get task push notification config
    async fn get_task_push_notification_config(
        &self,
        request: Request<GetTaskPushNotificationConfigRequest>,
    ) -> Result<Response<TaskPushNotificationConfigResponse>, Status> {
        let req = request.into_inner();
        println!("gRPC: GetTaskPushNotificationConfig request for {}", req.name);
        
        let response = TaskPushNotificationConfigResponse {
            config: None,
        };
        
        Ok(Response::new(response))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Starting gRPC Test Server on 127.0.0.1:50051");
    
    let addr: SocketAddr = "127.0.0.1:50051".parse()?;
    let service = GrpcTestService::new();
    
    println!("✅ gRPC Test Server is ready to accept connections!");
    println!("📡 Endpoints:");
    println!("   - gRPC endpoint: grpc://127.0.0.1:50051");
    println!("   - Agent card: Available via GetAgentCard RPC");
    
    Server::builder()
        .add_service(A2aServiceServer::new(service))
        .serve(addr)
        .await?;
    
    Ok(())
}