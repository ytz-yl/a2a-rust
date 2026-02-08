use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;

use a2a_rust::a2a::{
    client::transports::grpc::GrpcClient,
    core_types::{Message, Part, Role, TaskState, TaskStatus},
    grpc::a2a::{
        CancelTaskRequest, CreateTaskPushNotificationConfigRequest, GetAgentCardRequest,
        GetTaskPushNotificationConfigRequest, GetTaskRequest, SendMessageRequest,
        SendMessageResponse, StreamPayload, StreamResponse, TaskSubscriptionRequest,
    },
    models::{
        AgentCapabilities, AgentCard, Artifact, MessageSendParams, PushNotificationConfig,
        SendStreamingMessageResponse, SendStreamingMessageResult, Task, TaskPushNotificationConfig,
        TaskStatusUpdateEvent, TaskArtifactUpdateEvent,
    },
};
use axum::{
    body::{Body, Bytes},
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event, Sse},
        Response,
    },
    routing::{get, post},
    Json, Router,
};
use futures::{stream, Stream};
use tokio::{net::TcpListener, task::JoinHandle};
use tonic::{Request, Status};

pub mod rest {
    use super::*;

    #[derive(Clone)]
    pub struct RestAppState {
        pub task: Task,
        pub message: Message,
        pub push_config: TaskPushNotificationConfig,
        pub streaming_results: Vec<SendStreamingMessageResult>,
        pub resubscribe_results: Vec<SendStreamingMessageResult>,
        pub cancel_task: Task,
    }

    impl RestAppState {
        fn new() -> Self {
            let task = sample_task(TaskState::Working);
            let message = sample_user_message();
            let push_config = sample_push_config(&task);
            let streaming_results = sample_streaming_results(&task);
            let resubscribe_results = sample_resubscribe_results(&task);
            let cancel_task = {
                let mut t = task.clone();
                t.status = TaskStatus::new(TaskState::Canceled);
                t
            };

            Self {
                task,
                message,
                push_config,
                streaming_results,
                resubscribe_results,
                cancel_task,
            }
        }
    }

    #[allow(dead_code)]
    pub struct RestTestServer {
        base_url: String,
        handle: JoinHandle<()>,
        state: RestAppState,
    }

    #[allow(dead_code)]
    impl RestTestServer {
        pub async fn start() -> Self {
            let state = RestAppState::new();
            let router = Router::new()
                .route("/message/send", post(handle_message_send))
                .route("/message/stream", post(handle_message_stream))
                .route("/tasks/:task_id", get(handle_get_task))
                .route("/tasks/:task_id/cancel", post(handle_cancel_task))
                .route(
                    "/tasks/:task_id/push_notifications",
                    post(handle_set_push_notification).get(handle_get_push_notification),
                )
                .route("/tasks/:task_id/resubscribe", post(handle_resubscribe))
                .with_state(state.clone());

            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let handle = tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
            });

            RestTestServer {
                base_url: format!("http://{}", addr),
                handle,
                state,
            }
        }

        pub fn base_url(&self) -> String {
            self.base_url.clone()
        }

        pub fn agent_card(&self) -> AgentCard {
            sample_agent_card(&self.base_url)
        }

        pub fn sample_task(&self) -> Task {
            self.state.task.clone()
        }

        pub fn cancel_task(&self) -> Task {
            self.state.cancel_task.clone()
        }

        pub fn sample_message(&self) -> Message {
            self.state.message.clone()
        }

        pub fn push_config(&self) -> TaskPushNotificationConfig {
            self.state.push_config.clone()
        }
    }

    impl Drop for RestTestServer {
        fn drop(&mut self) {
            self.handle.abort();
        }
    }

    async fn handle_message_send(
        State(state): State<RestAppState>,
        Json(_payload): Json<MessageSendParams>,
    ) -> Json<Task> {
        Json(state.task)
    }

    async fn handle_get_task(
        State(state): State<RestAppState>,
        Path(task_id): Path<String>,
    ) -> Response<Body> {
        // 对于不存在的任务ID返回404错误
        if task_id.contains("non-existent") || task_id == "invalid-task-id-regression-test" {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&serde_json::json!({
                    "error": {
                        "code": -32001,
                        "message": "Task not found",
                        "data": None::<()>
                    }
                })).unwrap()))
                .unwrap();
        }
        
        // 对于其他任务ID返回正常响应
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&state.task).unwrap()))
            .unwrap()
    }

    async fn handle_cancel_task(
        State(state): State<RestAppState>,
        Path(_task_id): Path<String>,
    ) -> Json<Task> {
        Json(state.cancel_task)
    }

    async fn handle_set_push_notification(
        State(state): State<RestAppState>,
        Json(mut payload): Json<TaskPushNotificationConfig>,
    ) -> Json<TaskPushNotificationConfig> {
        payload.push_notification_config.token =
            state.push_config.push_notification_config.token.clone();
        Json(payload)
    }

    async fn handle_get_push_notification(
        State(state): State<RestAppState>,
        Path(_task_id): Path<String>,
    ) -> Json<TaskPushNotificationConfig> {
        Json(state.push_config)
    }

    async fn handle_message_stream(
        State(state): State<RestAppState>,
    ) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
        sse_from_results(state.streaming_results)
    }

    async fn handle_resubscribe(
        State(state): State<RestAppState>,
    ) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
        sse_from_results(state.resubscribe_results)
    }

    fn sse_from_results(
        results: Vec<SendStreamingMessageResult>,
    ) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
        let stream = stream::iter(results.into_iter().map(|result| {
            let payload = SendStreamingMessageResponse::success(None, result);
            let json = serde_json::to_string(&payload).unwrap();
            Ok(Event::default().data(json))
        }));

        Sse::new(stream)
    }

    pub fn sample_task(state: TaskState) -> Task {
        Task::new(
            "rest-context-001".to_string(),
            TaskStatus::new(state),
        )
        .with_task_id("rest-task-123".to_string())
    }

    pub fn sample_user_message() -> Message {
        Message::new(
            Role::User,
            vec![Part::text("Hello via REST transport".to_string())],
        )
        .with_message_id("rest-message-1".to_string())
        .with_task_id("rest-task-123".to_string())
        .with_context_id("rest-context-001".to_string())
    }

    pub fn sample_push_config(task: &Task) -> TaskPushNotificationConfig {
        let push_config = PushNotificationConfig {
            id: Some("rest-push-config".to_string()),
            url: url::Url::parse("https://example.com/hook").unwrap(),
            token: Some("push-token".to_string()),
            authentication: None,
        };

        TaskPushNotificationConfig {
            task_id: task.id.clone(),
            push_notification_config: push_config,
        }
    }

    pub fn sample_streaming_results(task: &Task) -> Vec<SendStreamingMessageResult> {
        let working = TaskStatusUpdateEvent::new(
            task.id.clone(),
            task.context_id.clone(),
            TaskStatus::new(TaskState::Working),
            false,
        );
        let agent_message = Message::new(
            Role::Agent,
            vec![Part::text("REST stream event".to_string())],
        )
        .with_task_id(task.id.clone())
        .with_context_id(task.context_id.clone());
        let complete = TaskStatusUpdateEvent::new(
            task.id.clone(),
            task.context_id.clone(),
            TaskStatus::new(TaskState::Completed),
            true,
        );

        vec![
            SendStreamingMessageResult::TaskStatusUpdateEvent(working),
            SendStreamingMessageResult::Message(agent_message),
            SendStreamingMessageResult::TaskStatusUpdateEvent(complete),
        ]
    }

    pub fn sample_resubscribe_results(task: &Task) -> Vec<SendStreamingMessageResult> {
        let artifact_content = Artifact::new(vec![Part::text("artifact".to_string())]);
        let artifact = TaskArtifactUpdateEvent::new(
            task.id.clone(),
            task.context_id.clone(),
            artifact_content,
        );
        vec![
            SendStreamingMessageResult::Task(Task::new(
                task.context_id.clone(),
                TaskStatus::new(TaskState::Working),
            )
            .with_task_id(task.id.clone())),
            SendStreamingMessageResult::TaskArtifactUpdateEvent(artifact),
        ]
    }

    pub fn sample_agent_card(base_url: &str) -> AgentCard {
        AgentCard::new(
            "REST Test Agent".to_string(),
            "Integration test agent".to_string(),
            base_url.to_string(),
            "1.0.0".to_string(),
            vec!["text/plain".to_string()],
            vec!["text/plain".to_string()],
            AgentCapabilities::new().with_streaming(true).with_push_notifications(true),
            vec![],
        )
    }

    // Chunked SSE server to stress the SSE parser
    #[allow(dead_code)]
    pub struct ChunkedSseServer {
        base_url: String,
        handle: JoinHandle<()>,
    }

    #[allow(dead_code)]
    impl ChunkedSseServer {
        pub async fn start() -> Self {
            let router = Router::new().route("/message/stream", post(chunked_stream_handler));
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let handle = tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
            });

            ChunkedSseServer {
                base_url: format!("http://{}", addr),
                handle,
            }
        }

        pub fn base_url(&self) -> String {
            self.base_url.clone()
        }
    }

    impl Drop for ChunkedSseServer {
        fn drop(&mut self) {
            self.handle.abort();
        }
    }

    async fn chunked_stream_handler() -> Response {
        let working = TaskStatusUpdateEvent::new(
            "rest-task-123".to_string(),
            "rest-context-001".to_string(),
            TaskStatus::new(TaskState::Working),
            false,
        );
        let payload = SendStreamingMessageResponse::success(
            None,
            SendStreamingMessageResult::TaskStatusUpdateEvent(working),
        );
        let json = serde_json::to_string(&payload).unwrap();
        let body_chunks = vec![
            b"data: {\"partial\"".to_vec(),
            b":true}\n\n".to_vec(),
            format!("data: {}\n\n", json).into_bytes(),
        ];
        let stream = stream::iter(body_chunks.into_iter().map(|chunk| {
            Ok::<Bytes, Infallible>(Bytes::from(chunk))
        }));

        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/event-stream")
            .body(Body::from_stream(stream))
            .unwrap()
    }

}

pub mod grpc {
    use super::*;

    #[derive(Clone)]
    #[allow(dead_code)]
    pub struct MockGrpcClient {
        data: Arc<MockGrpcData>,
    }

    #[allow(dead_code)]
    struct MockGrpcData {
        send_message: SendMessageResponse,
        streaming_events: Vec<StreamResponse>,
        resubscribe_events: Vec<StreamResponse>,
        task: Task,
        cancel_task: Task,
        push_config: TaskPushNotificationConfig,
        agent_card: AgentCard,
    }

    #[allow(dead_code)]
    impl MockGrpcClient {
        pub fn new(task: Task) -> Self {
            let send_message = SendMessageResponse {
                task: Some(task.clone()),
                msg: None,
            };
            let streaming_events =
                super::rest::sample_streaming_results(&task).into_iter().map(to_stream_response).collect();
            let resubscribe_events =
                super::rest::sample_resubscribe_results(&task).into_iter().map(to_stream_response).collect();
            let cancel_task = {
                let mut t = task.clone();
                t.status = TaskStatus::new(TaskState::Canceled);
                t
            };
            let push_config = super::rest::sample_push_config(&task);
            let agent_card = super::rest::sample_agent_card("grpc://localhost");
            let data = MockGrpcData {
                send_message,
                streaming_events,
                resubscribe_events,
                task,
                cancel_task,
                push_config,
                agent_card,
            };

            Self {
                data: Arc::new(data),
            }
        }

        pub fn agent_card(&self) -> AgentCard {
            self.data.agent_card.clone()
        }

        pub fn task(&self) -> Task {
            self.data.task.clone()
        }

        pub fn push_config(&self) -> TaskPushNotificationConfig {
            self.data.push_config.clone()
        }
    }

    fn to_stream_response(result: SendStreamingMessageResult) -> StreamResponse {
        StreamResponse {
            result: Some(StreamPayload::from(result)),
        }
    }

    #[async_trait::async_trait]
    impl GrpcClient for MockGrpcClient {
        async fn send_message(
            &mut self,
            _request: Request<SendMessageRequest>,
        ) -> Result<SendMessageResponse, Status> {
            Ok(self.data.send_message.clone())
        }

        async fn send_streaming_message(
            &mut self,
            _request: Request<SendMessageRequest>,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamResponse, Status>> + Send>>, Status> {
            let stream = stream::iter(self.data.streaming_events.clone().into_iter().map(Ok));
            Ok(Box::pin(stream))
        }

        async fn get_task(
            &mut self,
            _request: Request<GetTaskRequest>,
        ) -> Result<Task, Status> {
            Ok(self.data.task.clone())
        }

        async fn cancel_task(
            &mut self,
            _request: Request<CancelTaskRequest>,
        ) -> Result<Task, Status> {
            Ok(self.data.cancel_task.clone())
        }

        async fn create_task_push_notification_config(
            &mut self,
            _request: Request<CreateTaskPushNotificationConfigRequest>,
        ) -> Result<TaskPushNotificationConfig, Status> {
            Ok(self.data.push_config.clone())
        }

        async fn get_task_push_notification_config(
            &mut self,
            _request: Request<GetTaskPushNotificationConfigRequest>,
        ) -> Result<TaskPushNotificationConfig, Status> {
            Ok(self.data.push_config.clone())
        }

        async fn task_subscription(
            &mut self,
            _request: Request<TaskSubscriptionRequest>,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamResponse, Status>> + Send>>, Status> {
            let stream = stream::iter(self.data.resubscribe_events.clone().into_iter().map(Ok));
            Ok(Box::pin(stream))
        }

        async fn get_agent_card(
            &mut self,
            _request: Request<GetAgentCardRequest>,
        ) -> Result<AgentCard, Status> {
            Ok(self.data.agent_card.clone())
        }
    }

    pub fn default_mock_client() -> MockGrpcClient {
        MockGrpcClient::new(super::rest::sample_task(TaskState::Working))
    }

    pub fn sample_message() -> Message {
        super::rest::sample_user_message()
    }

    pub fn agent_card() -> AgentCard {
        super::rest::sample_agent_card("grpc://localhost")
    }
}
