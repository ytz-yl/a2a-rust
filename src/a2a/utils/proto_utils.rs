//! Proto <-> domain conversions for gRPC transport.
//!
//! These helpers keep the gRPC transport implementation decoupled from the
//! concrete protobuf representations on the wire.

use crate::a2a::client::client_trait::{ClientEvent, TaskUpdateEvent};
use crate::a2a::core_types::{Message, Metadata, TaskState, TaskStatus};
use crate::a2a::error::A2AError;
use crate::a2a::models::*;

// Use the same stub proto types as grpc.rs
use crate::a2a::grpc::a2a_pb2::a2a::{self as pb, StreamPayload};

/// ---------- Unary conversions (domain -> proto) ----------

pub fn to_proto_message(m: &Message) -> Result<pb::Message, A2AError> {
    Ok(m.clone())
}

pub fn to_proto_message_send_configuration(
    c: &MessageSendConfiguration,
) -> Result<pb::MessageSendConfiguration, A2AError> {
    Ok(c.clone())
}

pub fn to_proto_metadata(m: &Metadata) -> Result<pb::Metadata, A2AError> {
    Ok(m.clone())
}

pub fn to_proto_task_push_notification_config(
    cfg: &TaskPushNotificationConfig,
) -> Result<pb::TaskPushNotificationConfig, A2AError> {
    Ok(cfg.clone())
}

/// ---------- Unary conversions (proto -> domain) ----------

pub fn from_proto_message(m: pb::Message) -> Result<Message, A2AError> {
    Ok(m)
}

pub fn from_proto_task(t: pb::Task) -> Result<Task, A2AError> {
    Ok(t)
}

pub fn from_proto_agent_card(c: pb::AgentCard) -> Result<AgentCard, A2AError> {
    Ok(c)
}

pub fn from_proto_task_push_notification_config(
    cfg: pb::TaskPushNotificationConfig,
) -> Result<TaskPushNotificationConfig, A2AError> {
    Ok(cfg)
}

pub fn from_proto_task_status_update_event(
    ev: pb::TaskStatusUpdateEvent,
) -> Result<TaskStatusUpdateEvent, A2AError> {
    Ok(ev)
}

pub fn from_proto_task_artifact_update_event(
    ev: pb::TaskArtifactUpdateEvent,
) -> Result<TaskArtifactUpdateEvent, A2AError> {
    Ok(ev)
}

/// ---------- Streaming conversions ----------

pub fn from_proto_stream_response_as_task_or_message(
    r: pb::StreamResponse,
) -> Result<TaskOrMessage, A2AError> {
    match r.result {
        Some(StreamPayload::Task(task)) => Ok(TaskOrMessage::Task(task)),
        Some(StreamPayload::Message(message)) => Ok(TaskOrMessage::Message(message)),
        Some(StreamPayload::TaskStatusUpdateEvent(update)) => Ok(TaskOrMessage::TaskUpdate(update)),
        Some(StreamPayload::TaskArtifactUpdateEvent(update)) => {
            Ok(TaskOrMessage::TaskArtifactUpdateEvent(update))
        }
        None => Err(A2AError::invalid_response(
            "Empty stream payload in gRPC response",
        )),
    }
}

pub fn from_proto_stream_response_as_client_event(
    r: pb::StreamResponse,
) -> Result<ClientEvent, A2AError> {
    match r.result {
        Some(StreamPayload::Task(task)) => Ok((task, None)),
        Some(StreamPayload::TaskStatusUpdateEvent(update)) => {
            let event = TaskUpdateEvent::Status(update.clone());
            Ok((task_from_status_update(&update), Some(event)))
        }
        Some(StreamPayload::TaskArtifactUpdateEvent(update)) => {
            let event = TaskUpdateEvent::Artifact(update.clone());
            Ok((task_from_artifact_update(&update), Some(event)))
        }
        Some(StreamPayload::Message(_)) => Err(A2AError::invalid_response(
            "Unexpected message payload in task subscription stream",
        )),
        None => Err(A2AError::invalid_response(
            "Empty stream payload in gRPC response",
        )),
    }
}

fn task_from_status_update(update: &TaskStatusUpdateEvent) -> Task {
    Task::new(update.context_id.clone(), update.status.clone()).with_task_id(update.task_id.clone())
}

fn task_from_artifact_update(update: &TaskArtifactUpdateEvent) -> Task {
    let status = TaskStatus::new(TaskState::Working);
    Task::new(update.context_id.clone(), status).with_task_id(update.task_id.clone())
}
