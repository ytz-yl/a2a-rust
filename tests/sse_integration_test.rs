#![allow(clippy::unwrap_used)]

#[path = "common/mod.rs"]
mod common;

use a2a_rust::a2a::{
    client::{client_trait::ClientTransport, transports::rest::RestTransport},
    models::{MessageSendParams, Task, TaskOrMessage},
};
use common::rest::{ChunkedSseServer, RestTestServer};
use futures::StreamExt;
use axum::{routing::post, Json, Router};
use tokio::{net::TcpListener, task::JoinHandle};

fn message_params(server: &RestTestServer) -> MessageSendParams {
    MessageSendParams {
        message: server.sample_message(),
        configuration: None,
        metadata: None,
    }
}

#[tokio::test]
async fn sse_parser_handles_chunked_messages() {
    let sse_server = ChunkedSseServer::start().await;
    let rest_server = RestTestServer::start().await;
    let transport =
        RestTransport::new(sse_server.base_url(), Some(rest_server.agent_card())).unwrap();

    let stream = transport
        .send_message_streaming(message_params(&rest_server), None, None)
        .await
        .unwrap();
    let events: Vec<_> = stream.collect::<Vec<_>>().await;
    assert!(!events.is_empty());
    let first = events.into_iter().find_map(Result::ok).expect("at least one ok event");
    assert!(matches!(first, TaskOrMessage::TaskUpdate(_)));
}

#[tokio::test]
async fn streaming_falls_back_to_json_response() {
    let server = RestTestServer::start().await;
    let (base_url, handle) = start_json_stream_stub(server.sample_task()).await;
    let transport = RestTransport::new(base_url, Some(server.agent_card())).unwrap();

    let response = transport
        .send_message_streaming(message_params(&server), None, None)
        .await
        .unwrap();
    let events: Vec<_> = response.collect::<Vec<_>>().await;
    assert_eq!(events.len(), 1);
    handle.abort();
}

async fn start_json_stream_stub(task: Task) -> (String, JoinHandle<()>) {
    let router = Router::new().route(
        "/message/stream",
        post({
            let task = task.clone();
            move || {
                let task = task.clone();
                async move { Json(task) }
            }
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (format!("http://{}", addr), handle)
}
