//! gRPC transport implementation for A2A Rust client
//!
//! Mirrors the shape of jsonrpc.rs transport, but uses tonic gRPC.

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use tonic::{
    metadata::{MetadataMap, MetadataValue},
    transport::Channel,
    Request,
};

use crate::a2a::client::client_trait::{
    ClientCallContext, ClientCallInterceptor, ClientEvent, ClientTransport,
};
use crate::a2a::client::config::ClientConfig;
use crate::a2a::error::A2AError;
use crate::a2a::extensions::common::HTTP_EXTENSION_HEADER;
use crate::a2a::models::*;
use crate::a2a::utils::proto_utils;

// Use the stub proto types directly from a2a_pb2
use crate::a2a::grpc::a2a_pb2::a2a::{
    a2a_service_client::A2aServiceClient, CancelTaskRequest,
    CreateTaskPushNotificationConfigRequest, GetAgentCardRequest,
    GetTaskPushNotificationConfigRequest, GetTaskRequest, SendMessageRequest,
    SendMessageResponse, StreamResponse, TaskSubscriptionRequest,
};

#[async_trait]
pub trait GrpcClient: Clone + Send + Sync + 'static {
    async fn send_message(
        &mut self,
        request: Request<SendMessageRequest>,
    ) -> Result<SendMessageResponse, tonic::Status>;

    async fn send_streaming_message(
        &mut self,
        request: Request<SendMessageRequest>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamResponse, tonic::Status>> + Send>>, tonic::Status>;

    async fn get_task(
        &mut self,
        request: Request<GetTaskRequest>,
    ) -> Result<crate::a2a::grpc::a2a_pb2::a2a::Task, tonic::Status>;

    async fn cancel_task(
        &mut self,
        request: Request<CancelTaskRequest>,
    ) -> Result<crate::a2a::grpc::a2a_pb2::a2a::Task, tonic::Status>;

    async fn create_task_push_notification_config(
        &mut self,
        request: Request<CreateTaskPushNotificationConfigRequest>,
    ) -> Result<crate::a2a::grpc::a2a_pb2::a2a::TaskPushNotificationConfig, tonic::Status>;

    async fn get_task_push_notification_config(
        &mut self,
        request: Request<GetTaskPushNotificationConfigRequest>,
    ) -> Result<crate::a2a::grpc::a2a_pb2::a2a::TaskPushNotificationConfig, tonic::Status>;

    async fn task_subscription(
        &mut self,
        request: Request<TaskSubscriptionRequest>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamResponse, tonic::Status>> + Send>>, tonic::Status>;

    async fn get_agent_card(
        &mut self,
        request: Request<GetAgentCardRequest>,
    ) -> Result<crate::a2a::grpc::a2a_pb2::a2a::AgentCard, tonic::Status>;
}

#[async_trait]
impl GrpcClient for A2aServiceClient<Channel> {
    async fn send_message(
        &mut self,
        request: Request<SendMessageRequest>,
    ) -> Result<SendMessageResponse, tonic::Status> {
        let response = A2aServiceClient::send_message(self, request).await?;
        Ok(response.into_inner())
    }

    async fn send_streaming_message(
        &mut self,
        request: Request<SendMessageRequest>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamResponse, tonic::Status>> + Send>>, tonic::Status> {
        let response = A2aServiceClient::send_streaming_message(self, request).await?;
        Ok(Box::pin(response.into_inner()))
    }

    async fn get_task(
        &mut self,
        request: Request<GetTaskRequest>,
    ) -> Result<crate::a2a::grpc::a2a_pb2::a2a::Task, tonic::Status> {
        let response = A2aServiceClient::get_task(self, request).await?;
        Ok(response.into_inner())
    }

    async fn cancel_task(
        &mut self,
        request: Request<CancelTaskRequest>,
    ) -> Result<crate::a2a::grpc::a2a_pb2::a2a::Task, tonic::Status> {
        let response = A2aServiceClient::cancel_task(self, request).await?;
        Ok(response.into_inner())
    }

    async fn create_task_push_notification_config(
        &mut self,
        request: Request<CreateTaskPushNotificationConfigRequest>,
    ) -> Result<crate::a2a::grpc::a2a_pb2::a2a::TaskPushNotificationConfig, tonic::Status> {
        let response =
            A2aServiceClient::create_task_push_notification_config(self, request).await?;
        Ok(response.into_inner())
    }

    async fn get_task_push_notification_config(
        &mut self,
        request: Request<GetTaskPushNotificationConfigRequest>,
    ) -> Result<crate::a2a::grpc::a2a_pb2::a2a::TaskPushNotificationConfig, tonic::Status> {
        let response =
            A2aServiceClient::get_task_push_notification_config(self, request).await?;
        Ok(response.into_inner())
    }

    async fn task_subscription(
        &mut self,
        request: Request<TaskSubscriptionRequest>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamResponse, tonic::Status>> + Send>>, tonic::Status> {
        let response = A2aServiceClient::task_subscription(self, request).await?;
        Ok(Box::pin(response.into_inner()))
    }

    async fn get_agent_card(
        &mut self,
        request: Request<GetAgentCardRequest>,
    ) -> Result<crate::a2a::grpc::a2a_pb2::a2a::AgentCard, tonic::Status> {
        let response = A2aServiceClient::get_agent_card(self, request).await?;
        Ok(response.into_inner())
    }
}

/// gRPC transport for A2A client
pub struct GrpcTransport<C = A2aServiceClient<Channel>>
where
    C: GrpcClient,
{
    #[allow(dead_code)]
    url: String,
    agent_card: Option<AgentCard>,
    interceptors: Vec<Box<dyn ClientCallInterceptor>>,
    extensions: Vec<String>,
    needs_extended_card: bool,
    stub: C,
}

impl GrpcTransport<A2aServiceClient<Channel>> {
    pub fn new(url: String, channel: Channel, agent_card: Option<AgentCard>) -> Result<Self, A2AError> {
        let needs_extended_card = agent_card
            .as_ref()
            .and_then(|c| c.supports_authenticated_extended_card)
            .unwrap_or(true);

        Ok(Self {
            url,
            agent_card,
            interceptors: Vec::new(),
            extensions: Vec::new(),
            needs_extended_card,
            stub: A2aServiceClient::new(channel),
        })
    }

    pub fn new_with_config(
        url: String,
        channel: Channel,
        agent_card: Option<AgentCard>,
        config: ClientConfig,
        interceptors: Vec<Box<dyn ClientCallInterceptor>>,
    ) -> Result<Self, A2AError> {
        let needs_extended_card = agent_card
            .as_ref()
            .and_then(|c| c.supports_authenticated_extended_card)
            .unwrap_or(true);

        Ok(Self {
            url,
            agent_card,
            interceptors,
            extensions: config.extensions,
            needs_extended_card,
            stub: A2aServiceClient::new(channel),
        })
    }
}

impl<C> GrpcTransport<C>
where
    C: GrpcClient,
{
    pub fn new_with_stub(
        url: String,
        agent_card: Option<AgentCard>,
        config: ClientConfig,
        interceptors: Vec<Box<dyn ClientCallInterceptor>>,
        stub: C,
    ) -> Self {
        let needs_extended_card = agent_card
            .as_ref()
            .and_then(|c| c.supports_authenticated_extended_card)
            .unwrap_or(true);

        Self {
            url,
            agent_card,
            interceptors,
            extensions: config.extensions,
            needs_extended_card,
            stub,
        }
    }

    fn build_metadata(&self, extensions: Option<Vec<String>>) -> Result<MetadataMap, A2AError> {
        let mut meta = MetadataMap::new();
        let exts = extensions.unwrap_or_else(|| self.extensions.clone());

        if !exts.is_empty() {
            let joined = exts.join(",");
            let v = MetadataValue::try_from(joined).map_err(|e| {
                A2AError::transport_error(format!("Invalid extensions metadata: {e}"))
            })?;
            meta.insert(HTTP_EXTENSION_HEADER, v);
        }

        Ok(meta)
    }

    fn with_metadata<T>(&self, mut req: Request<T>, meta: MetadataMap) -> Request<T> {
        *req.metadata_mut() = meta;
        req
    }
    /// Add interceptors to the transport
    pub fn with_interceptors(mut self, interceptors: Vec<Box<dyn ClientCallInterceptor>>) -> Self {
        self.interceptors = interceptors;
        self
    }
}

#[async_trait]
impl<C> ClientTransport for GrpcTransport<C>
where
    C: GrpcClient,
{
    async fn send_message(
        &self,
        params: MessageSendParams,
        _context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<TaskOrMessage, A2AError> {
        let meta = self.build_metadata(extensions)?;

        let req_pb = SendMessageRequest {
            request: Some(proto_utils::to_proto_message(&params.message)?),
            configuration: params.configuration.as_ref().map(|c| proto_utils::to_proto_message_send_configuration(c)).transpose()?,
            metadata: params.metadata.as_ref().map(|m| proto_utils::to_proto_metadata(m)).transpose()?,
        };

        let req = self.with_metadata(Request::new(req_pb), meta);

        let mut client = self.stub.clone();
        let resp: SendMessageResponse = client
            .send_message(req)
            .await
            .map_err(|e| A2AError::transport_error(format!("gRPC SendMessage failed: {e}")))?;

        // proto response expected: { task?: Task, msg?: Message }
        if let Some(task_pb) = resp.task {
            Ok(TaskOrMessage::Task(proto_utils::from_proto_task(task_pb)?))
        } else if let Some(msg_pb) = resp.msg {
            Ok(TaskOrMessage::Message(proto_utils::from_proto_message(msg_pb)?))
        } else {
            Err(A2AError::invalid_response(
                "SendMessageResponse missing both task and msg",
            ))
        }
    }

    async fn send_message_streaming<'a>(
        &'a self,
        params: MessageSendParams,
        _context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<TaskOrMessage, A2AError>> + Send + 'a>>, A2AError> {
        let meta = self.build_metadata(extensions)?;

        let req_pb = SendMessageRequest {
            request: Some(proto_utils::to_proto_message(&params.message)?),
            configuration: params.configuration.as_ref().map(|c| proto_utils::to_proto_message_send_configuration(c)).transpose()?,
            metadata: params.metadata.as_ref().map(|m| proto_utils::to_proto_metadata(m)).transpose()?,
        };

        let req = self.with_metadata(Request::new(req_pb), meta);

        let mut client = self.stub.clone();
        let resp = client
            .send_streaming_message(req)
            .await
            .map_err(|e| A2AError::transport_error(format!("gRPC SendStreamingMessage failed: {e}")))?;

        let stream = resp.map(|item: Result<StreamResponse, tonic::Status>| {
            let pb = item.map_err(|e| A2AError::transport_error(format!("gRPC stream item error: {e}")))?;
            proto_utils::from_proto_stream_response_as_task_or_message(pb)
        });

        Ok(Box::pin(stream))
    }

    async fn get_task(
        &self,
        request: TaskQueryParams,
        _context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Task, A2AError> {
        let meta = self.build_metadata(extensions)?;

        let req_pb = GetTaskRequest {
            name: format!("tasks/{}", request.id),
            history_length: request.history_length,
        };

        let req = self.with_metadata(Request::new(req_pb), meta);

        let mut client = self.stub.clone();
        let task_pb = client
            .get_task(req)
            .await
            .map_err(|e| A2AError::transport_error(format!("gRPC GetTask failed: {e}")))?;

        proto_utils::from_proto_task(task_pb)
    }

    async fn cancel_task(
        &self,
        request: TaskIdParams,
        _context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Task, A2AError> {
        let meta = self.build_metadata(extensions)?;

        let req_pb = CancelTaskRequest {
            name: format!("tasks/{}", request.id),
        };

        let req = self.with_metadata(Request::new(req_pb), meta);

        let mut client = self.stub.clone();
        let task_pb = client
            .cancel_task(req)
            .await
            .map_err(|e| A2AError::transport_error(format!("gRPC CancelTask failed: {e}")))?;

        proto_utils::from_proto_task(task_pb)
    }

    async fn set_task_callback(
        &self,
        request: TaskPushNotificationConfig,
        _context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        let meta = self.build_metadata(extensions)?;

        let config_id = request
            .push_notification_config
            .id
            .clone()
            .unwrap_or_default();

        let req_pb = CreateTaskPushNotificationConfigRequest {
            parent: format!("tasks/{}", request.task_id),
            config_id,
            config: Some(proto_utils::to_proto_task_push_notification_config(&request)?),
        };

        let req = self.with_metadata(Request::new(req_pb), meta);

        let mut client = self.stub.clone();
        let cfg_pb = client
            .create_task_push_notification_config(req)
            .await
            .map_err(|e| A2AError::transport_error(format!("gRPC CreateTaskPushNotificationConfig failed: {e}")))?;

        proto_utils::from_proto_task_push_notification_config(cfg_pb)
    }

    async fn get_task_callback(
        &self,
        request: GetTaskPushNotificationConfigParams,
        _context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        let meta = self.build_metadata(extensions)?;

        let config_id = request.push_notification_config_id
            .unwrap_or_else(|| "default".to_string());

        let req_pb = GetTaskPushNotificationConfigRequest {
            name: format!(
                "tasks/{}/pushNotificationConfigs/{}",
                request.id, config_id
            ),
        };

        let req = self.with_metadata(Request::new(req_pb), meta);

        let mut client = self.stub.clone();
        let cfg_pb = client
            .get_task_push_notification_config(req)
            .await
            .map_err(|e| A2AError::transport_error(format!("gRPC GetTaskPushNotificationConfig failed: {e}")))?;

        proto_utils::from_proto_task_push_notification_config(cfg_pb)
    }

    async fn resubscribe<'a>(
        &'a self,
        request: TaskIdParams,
        _context: Option<&ClientCallContext>,
        extensions: Option<Vec<String>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ClientEvent, A2AError>> + Send + 'a>>, A2AError> {
        let meta = self.build_metadata(extensions)?;

        let req_pb = TaskSubscriptionRequest {
            name: format!("tasks/{}", request.id),
        };

        let req = self.with_metadata(Request::new(req_pb), meta);

        let mut client = self.stub.clone();
        let resp = client
            .task_subscription(req)
            .await
            .map_err(|e| A2AError::transport_error(format!("gRPC TaskSubscription failed: {e}")))?;

        let stream = resp.map(|item: Result<StreamResponse, tonic::Status>| {
            let pb = item.map_err(|e| A2AError::transport_error(format!("gRPC subscription item error: {e}")))?;
            proto_utils::from_proto_stream_response_as_client_event(pb)
        });

        Ok(Box::pin(stream))
    }

    async fn get_card(
        &self,
        _context: Option<&ClientCallContext>,
        _extensions: Option<Vec<String>>,
    ) -> Result<AgentCard, A2AError> {
        // keep same behavior as jsonrpc.rs: if cached and no need extended, return
        if let Some(ref card) = self.agent_card {
            if !self.needs_extended_card {
                return Ok(card.clone());
            }
        }

        let meta = self.build_metadata(None)?;
        let req = self.with_metadata(Request::new(GetAgentCardRequest {}), meta);

        let mut client = self.stub.clone();
        let card_pb = client
            .get_agent_card(req)
            .await
            .map_err(|e| A2AError::transport_error(format!("gRPC GetAgentCard failed: {e}")))?;

        proto_utils::from_proto_agent_card(card_pb)
    }

    async fn close(&self) -> Result<(), A2AError> {
        // tonic Channel doesn't need explicit close; drop handles it.
        Ok(())
    }
}
