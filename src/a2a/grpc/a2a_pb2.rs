//! Protobuf message definitions for A2A protocol (stubbed for testing)
//!
//! This module intentionally keeps a lightweight, test-friendly representation of
//! the proto types so the rest of the transport layer can be exercised without
//! fully generated gRPC bindings.

pub mod a2a {
    use crate::a2a::core_types::{Message as DomainMessage, Metadata as DomainMetadata};
    use crate::a2a::models::{
        AgentCard as DomainAgentCard, MessageSendConfiguration as DomainMessageSendConfiguration,
        SendStreamingMessageResult, Task as DomainTask,
        TaskArtifactUpdateEvent as DomainTaskArtifactUpdateEvent,
        TaskPushNotificationConfig as DomainTaskPushNotificationConfig,
        TaskStatusUpdateEvent as DomainTaskStatusUpdateEvent,
    };

    pub type Message = DomainMessage;
    pub type Metadata = DomainMetadata;
    pub type Task = DomainTask;
    pub type TaskPushNotificationConfig = DomainTaskPushNotificationConfig;
    pub type AgentCard = DomainAgentCard;
    pub type TaskStatusUpdateEvent = DomainTaskStatusUpdateEvent;
    pub type TaskArtifactUpdateEvent = DomainTaskArtifactUpdateEvent;
    pub type MessageSendConfiguration = DomainMessageSendConfiguration;

    pub mod a2a_service_client {
        use tonic::{client::Grpc, transport::Channel};

        use super::*;

        #[derive(Clone)]
        pub struct A2aServiceClient<T> {
            pub(crate) inner: tonic::client::Grpc<T>,
        }

        impl A2aServiceClient<Channel> {
            pub fn new(channel: Channel) -> Self {
                Self {
                    inner: Grpc::new(channel),
                }
            }

            pub async fn send_message(
                &mut self,
                _request: tonic::Request<SendMessageRequest>,
            ) -> Result<tonic::Response<SendMessageResponse>, tonic::Status> {
                Err(tonic::Status::unimplemented(
                    "gRPC client not implemented - placeholder",
                ))
            }

            pub async fn send_streaming_message(
                &mut self,
                _request: tonic::Request<SendMessageRequest>,
            ) -> Result<tonic::Response<tonic::codec::Streaming<StreamResponse>>, tonic::Status> {
                Err(tonic::Status::unimplemented(
                    "gRPC client not implemented - placeholder",
                ))
            }

            pub async fn get_task(
                &mut self,
                _request: tonic::Request<GetTaskRequest>,
            ) -> Result<tonic::Response<Task>, tonic::Status> {
                Err(tonic::Status::unimplemented(
                    "gRPC client not implemented - placeholder",
                ))
            }

            pub async fn cancel_task(
                &mut self,
                _request: tonic::Request<CancelTaskRequest>,
            ) -> Result<tonic::Response<Task>, tonic::Status> {
                Err(tonic::Status::unimplemented(
                    "gRPC client not implemented - placeholder",
                ))
            }

            pub async fn create_task_push_notification_config(
                &mut self,
                _request: tonic::Request<CreateTaskPushNotificationConfigRequest>,
            ) -> Result<tonic::Response<TaskPushNotificationConfig>, tonic::Status> {
                Err(tonic::Status::unimplemented(
                    "gRPC client not implemented - placeholder",
                ))
            }

            pub async fn get_task_push_notification_config(
                &mut self,
                _request: tonic::Request<GetTaskPushNotificationConfigRequest>,
            ) -> Result<tonic::Response<TaskPushNotificationConfig>, tonic::Status> {
                Err(tonic::Status::unimplemented(
                    "gRPC client not implemented - placeholder",
                ))
            }

            pub async fn task_subscription(
                &mut self,
                _request: tonic::Request<TaskSubscriptionRequest>,
            ) -> Result<tonic::Response<tonic::codec::Streaming<StreamResponse>>, tonic::Status> {
                Err(tonic::Status::unimplemented(
                    "gRPC client not implemented - placeholder",
                ))
            }

            pub async fn get_agent_card(
                &mut self,
                _request: tonic::Request<GetAgentCardRequest>,
            ) -> Result<tonic::Response<AgentCard>, tonic::Status> {
                Err(tonic::Status::unimplemented(
                    "gRPC client not implemented - placeholder",
                ))
            }
        }
    }

    #[derive(Clone, Debug, Default)]
    pub struct SendMessageRequest {
        pub request: Option<Message>,
        pub configuration: Option<MessageSendConfiguration>,
        pub metadata: Option<Metadata>,
    }

    #[derive(Clone, Debug, Default)]
    pub struct SendMessageResponse {
        pub task: Option<Task>,
        pub msg: Option<Message>,
    }

    #[derive(Clone, Debug, Default)]
    pub struct GetTaskRequest {
        pub name: String,
        pub history_length: Option<i32>,
    }

    #[derive(Clone, Debug, Default)]
    pub struct CancelTaskRequest {
        pub name: String,
    }

    #[derive(Clone, Debug, Default)]
    pub struct CreateTaskPushNotificationConfigRequest {
        pub parent: String,
        pub config_id: String,
        pub config: Option<TaskPushNotificationConfig>,
    }

    #[derive(Clone, Debug, Default)]
    pub struct GetTaskPushNotificationConfigRequest {
        pub name: String,
    }

    #[derive(Clone, Debug, Default)]
    pub struct TaskSubscriptionRequest {
        pub name: String,
    }

    #[derive(Clone, Debug, Default)]
    pub struct GetAgentCardRequest;

    #[derive(Clone, Debug, Default)]
    pub struct StreamResponse {
        pub result: Option<StreamPayload>,
    }

    #[derive(Clone, Debug)]
    pub enum StreamPayload {
        Task(Task),
        Message(Message),
        TaskStatusUpdateEvent(TaskStatusUpdateEvent),
        TaskArtifactUpdateEvent(TaskArtifactUpdateEvent),
    }

    impl From<SendStreamingMessageResult> for StreamPayload {
        fn from(value: SendStreamingMessageResult) -> Self {
            match value {
                SendStreamingMessageResult::Task(task) => StreamPayload::Task(task),
                SendStreamingMessageResult::Message(message) => StreamPayload::Message(message),
                SendStreamingMessageResult::TaskStatusUpdateEvent(event) => {
                    StreamPayload::TaskStatusUpdateEvent(event)
                }
                SendStreamingMessageResult::TaskArtifactUpdateEvent(event) => {
                    StreamPayload::TaskArtifactUpdateEvent(event)
                }
            }
        }
    }
}

pub use a2a::*;
