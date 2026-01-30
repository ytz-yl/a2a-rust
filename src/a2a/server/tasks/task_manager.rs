//! Task Manager implementation
//! 
//! This module provides the TaskManager which helps manage a task's lifecycle
//! during execution of a request, similar to the Python implementation.
//! 
//! The implementation closely follows the Python version's API and behavior
//! while adapting to Rust's type system and async patterns.

use crate::{Message, Task, TaskStatus, TaskState, A2AError};
use crate::a2a::server::events::{Event};
use crate::a2a::models::{TaskStatusUpdateEvent, TaskArtifactUpdateEvent};
use crate::a2a::server::tasks::TaskStore;
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

/// Task Manager - helps manage a task's lifecycle during execution of a request
/// 
/// Responsible for retrieving, saving, and updating the Task object based on
/// events received from the agent.
/// 
/// This implementation mirrors the Python TaskManager's behavior:
/// - Manages task state transitions
/// - Handles task persistence via TaskStore
/// - Processes task-related events
/// - Maintains conversation history
pub struct TaskManager {
    /// The ID of the task, if known from the request
    task_id: Option<String>,
    /// The ID of the context, if known from the request
    context_id: Option<String>,
    /// The TaskStore instance for persistence
    task_store: Arc<dyn TaskStore>,
    /// The Message that initiated the task, if any
    initial_message: Option<Message>,
    /// Current task object in memory
    current_task: Arc<tokio::sync::Mutex<Option<Task>>>,
}

impl TaskManager {
    /// Creates a new TaskManager
    /// 
    /// # Arguments
    /// * `task_id` - The ID of the task, if known from the request
    /// * `context_id` - The ID of the context, if known from the request
    /// * `task_store` - The TaskStore instance for persistence
    /// * `initial_message` - The Message that initiated the task, if any
    /// * `_context` - The ServerCallContext that this task is produced under (placeholder for future use)
    pub fn new(
        task_id: Option<String>,
        context_id: Option<String>,
        task_store: Arc<dyn TaskStore>,
        initial_message: Option<Message>,
        _context: Option<()>, // Placeholder for ServerCallContext
    ) -> Result<Self, A2AError> {
        // Validate task_id if provided
        if let Some(ref task_id) = task_id {
            if task_id.is_empty() {
                return Err(A2AError::invalid_params("Task ID must be a non-empty string"));
            }
        }

        debug!(
            "TaskManager initialized with task_id: {:?}, context_id: {:?}",
            task_id, context_id
        );

        Ok(Self {
            task_id,
            context_id,
            task_store,
            initial_message,
            current_task: Arc::new(tokio::sync::Mutex::new(None)),
        })
    }

    /// Retrieves the current task object, either from memory or the store
    /// 
    /// If task_id is set, it first checks the in-memory current_task,
    /// then attempts to load it from the task_store.
    pub async fn get_task(&self) -> Result<Option<Task>, A2AError> {
        if self.task_id.is_none() {
            debug!("task_id is not set, cannot get task.");
            return Ok(None);
        }

        // Check in-memory cache first
        {
            let current = self.current_task.lock().await;
            if current.is_some() {
                return Ok(current.clone());
            }
        }

        // Try to load from store
        let task_id = self.task_id.as_ref().unwrap();
        debug!("Attempting to get task from store with id: {}", task_id);
        
        match self.task_store.get(task_id).await {
            Ok(Some(task)) => {
                debug!("Task {} retrieved successfully.", task_id);
                let mut current = self.current_task.lock().await;
                *current = Some(task.clone());
                Ok(Some(task))
            }
            Ok(None) => {
                debug!("Task {} not found.", task_id);
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    /// Processes a task-related event and saves the updated task state
    /// 
    /// Ensures task and context IDs match or are set from the event.
    /// 
    /// # Arguments
    /// * `event` - The task-related event (Task, TaskStatusUpdateEvent, or TaskArtifactUpdateEvent)
    pub async fn save_task_event(&mut self, event: TaskEvent) -> Result<Task, A2AError> {
        let task_id_from_event = event.task_id();
        let context_id_from_event = event.context_id();
        
        // Validate task ID match
        if let Some(ref task_id) = self.task_id {
            if task_id != &task_id_from_event {
                return Err(A2AError::invalid_params(&format!(
                    "Task in event doesn't match TaskManager {} : {}",
                    task_id, task_id_from_event
                )));
            }
        } else {
            self.task_id = Some(task_id_from_event.clone());
        }
        
        // Validate context ID match
        if let Some(ref context_id) = self.context_id {
            if context_id != &context_id_from_event {
                return Err(A2AError::invalid_params(&format!(
                    "Context in event doesn't match TaskManager {} : {}",
                    context_id, context_id_from_event
                )));
            }
        } else {
            self.context_id = Some(context_id_from_event.clone());
        }

        debug!(
            "Processing save of task event of type {} for task_id: {}",
            event.event_type(),
            task_id_from_event
        );

        match event {
            TaskEvent::Task(task) => {
                self.save_task(task.clone()).await?;
                Ok(task)
            }
            TaskEvent::StatusUpdate(status_event) => {
                let mut task = self.ensure_task(&status_event).await?;
                
                debug!("Updating task {} status to: {:?}", task.id.to_string(), status_event.status.state);
                
                // Move current status message to history if present
                if let Some(ref message) = task.status.message {
                    if task.history.is_none() {
                        task.history = Some(vec![*message.clone()]);
                    } else if let Some(ref mut history) = task.history {
                        history.push(*message.clone());
                    }
                    task.status.message = None;
                }
                
                // Update metadata if provided
                if let Some(ref metadata) = status_event.metadata {
                    if task.metadata.is_none() {
                        task.metadata = Some(std::collections::HashMap::new());
                    }
                    if let Some(ref mut task_metadata) = task.metadata {
                        for (key, value) in metadata {
                            task_metadata.insert(key.clone(), value.clone());
                        }
                    }
                }
                
                task.status = status_event.status.clone();
                self.save_task(task.clone()).await?;
                Ok(task)
            }
            TaskEvent::ArtifactUpdate(artifact_event) => {
                let mut task = self.ensure_task(&artifact_event).await?;
                
                debug!("Appending artifact to task {}", task.id.to_string());
                
                // Append artifact to task
                if task.artifacts.is_none() {
                    task.artifacts = Some(vec![artifact_event.artifact.clone()]);
                } else if let Some(ref mut artifacts) = task.artifacts {
                    artifacts.push(artifact_event.artifact.clone());
                }
                
                self.save_task(task.clone()).await?;
                Ok(task)
            }
        }
    }

    /// Ensures a Task object exists in memory, loading from store or creating new if needed
    async fn ensure_task(&self, event: &dyn TaskEventWrapper) -> Result<Task, A2AError> {
        // Try to get current task from memory
        {
            let current = self.current_task.lock().await;
            if let Some(ref task) = *current {
                return Ok(task.clone());
            }
        }

        // Try to load from store if we have a task_id
        if let Some(ref task_id) = self.task_id {
            debug!("Attempting to retrieve existing task with id: {}", task_id);
            match self.task_store.get(task_id).await {
                Ok(Some(task)) => return Ok(task),
                Ok(None) => debug!("Task not found in store, will create new"),
                Err(e) => return Err(e),
            }
        }

        // Create new task
        info!(
            "Task not found or task_id not set. Creating new task for event (task_id: {}, context_id: {}).",
            event.task_id(),
            event.context_id()
        );
        
        let task = self.init_task_obj(event.task_id(), event.context_id());
        self.save_task(task.clone()).await?;
        Ok(task)
    }

    /// Processes an event, updates the task state if applicable, stores it, and returns the event
    pub async fn process_event(&mut self, event: &Event) -> Result<Event, A2AError> {
        match event {
            Event::Task(task) => {
                self.save_task_event(TaskEvent::Task(task.clone())).await?;
            }
            Event::TaskStatusUpdate(status_event) => {
                self.save_task_event(TaskEvent::StatusUpdate(status_event.clone())).await?;
            }
            Event::TaskArtifactUpdate(artifact_event) => {
                self.save_task_event(TaskEvent::ArtifactUpdate(artifact_event.clone())).await?;
            }
            Event::Message(_) => {
                // Non-task events are just passed through
            }
        }
        
        Ok(event.clone())
    }

    /// Initializes a new task object in memory
    fn init_task_obj(&self, task_id: &str, context_id: &str) -> Task {
        debug!(
            "Initializing new Task object with task_id: {}, context_id: {}",
            task_id, context_id
        );
        
        let task_id_uuid = Uuid::parse_str(task_id).unwrap_or_else(|_| Uuid::new_v4());
        let context_id_uuid = Uuid::parse_str(context_id).unwrap_or_else(|_| Uuid::new_v4());
        
        let history = if self.initial_message.is_some() {
            Some(vec![self.initial_message.clone().unwrap()])
        } else {
            None
        };

        Task {
            id: task_id_uuid.to_string(),
            context_id: context_id_uuid.to_string(),
            status: TaskStatus {
                state: TaskState::Submitted,
                timestamp: Some(chrono::Utc::now().to_string()),
                message: None,
            },
            artifacts: None,
            history,
            metadata: None,
            kind: "task".to_string(),
        }
    }

    /// Saves the given task to the task store and updates the in-memory current_task
    async fn save_task(&self, task: Task) -> Result<(), A2AError> {
        debug!("Saving task with id: {}", task.id.to_string());
        
        self.task_store.save(task.clone()).await?;
        
        {
            let mut current = self.current_task.lock().await;
            *current = Some(task.clone());
        }
        
        // Update task_id and context_id if they weren't set
        if self.task_id.is_none() {
            info!("New task created with id: {}", task.id.to_string());
            // Note: In a real implementation, we'd need to update self.task_id
            // but since self is immutable here, we'd need to restructure the code
        }
        
        Ok(())
    }

    /// Updates a task object in memory by adding a new message to its history
    /// 
    /// If the task has a message in its current status, that message is moved
    /// to the history first.
    pub async fn update_with_message(&self, message: Message, mut task: Task) -> Task {
        if let Some(ref status_message) = task.status.message {
            if task.history.is_none() {
                task.history = Some(vec![*status_message.clone()]);
            } else if let Some(ref mut history) = task.history {
                history.push(*status_message.clone());
            }
            task.status.message = None;
        }
        
        if task.history.is_none() {
            task.history = Some(vec![message]);
        } else if let Some(ref mut history) = task.history {
            history.push(message);
        }
        
        // Update in-memory cache
        {
            let mut current = self.current_task.lock().await;
            *current = Some(task.clone());
        }
        
        task
    }

    /// Gets the current task ID
    pub fn task_id(&self) -> Option<&str> {
        self.task_id.as_deref()
    }

    /// Gets the current context ID
    pub fn context_id(&self) -> Option<&str> {
        self.context_id.as_deref()
    }
}

/// Enum representing different types of task-related events
#[derive(Debug, Clone)]
pub enum TaskEvent {
    Task(Task),
    StatusUpdate(TaskStatusUpdateEvent),
    ArtifactUpdate(TaskArtifactUpdateEvent),
}

impl TaskEvent {
    /// Gets the task ID from the event
    pub fn task_id(&self) -> String {
        match self {
            TaskEvent::Task(task) => task.id.to_string(),
            TaskEvent::StatusUpdate(event) => event.task_id.to_string(),
            TaskEvent::ArtifactUpdate(event) => event.task_id.to_string(),
        }
    }

    /// Gets the context ID from the event
    pub fn context_id(&self) -> String {
        match self {
            TaskEvent::Task(task) => task.context_id.to_string(),
            TaskEvent::StatusUpdate(event) => event.context_id.to_string(),
            TaskEvent::ArtifactUpdate(event) => event.context_id.to_string(),
        }
    }

    /// Gets the event type name
    pub fn event_type(&self) -> &'static str {
        match self {
            TaskEvent::Task(_) => "Task",
            TaskEvent::StatusUpdate(_) => "TaskStatusUpdateEvent",
            TaskEvent::ArtifactUpdate(_) => "TaskArtifactUpdateEvent",
        }
    }
}

/// Wrapper trait for events that have task_id and context_id
pub trait TaskEventWrapper {
    fn task_id(&self) -> &str;
    fn context_id(&self) -> &str;
}

impl TaskEventWrapper for TaskStatusUpdateEvent {
    fn task_id(&self) -> &str {
        &self.task_id
    }

    fn context_id(&self) -> &str {
        &self.context_id
    }
}

impl TaskEventWrapper for TaskArtifactUpdateEvent {
    fn task_id(&self) -> &str {
        &self.task_id
    }

    fn context_id(&self) -> &str {
        &self.context_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Part, Role};
    use crate::a2a::server::tasks::InMemoryTaskStore;

    fn create_test_task_manager() -> (TaskManager, Arc<InMemoryTaskStore>) {
        let store = Arc::new(InMemoryTaskStore::new());
        let manager = TaskManager::new(
            Some("550e8400-e29b-41d4-a716-446655440000".to_string()),
            Some("550e8400-e29b-41d4-a716-446655440001".to_string()),
            store.clone(),
            None,
            None,
        ).unwrap();
        (manager, store)
    }

    #[tokio::test]
    async fn test_task_manager_initialization() {
        let store = Arc::new(InMemoryTaskStore::new());
        let manager = TaskManager::new(
            Some("550e8400-e29b-41d4-a716-446655440000".to_string()),
            Some("550e8400-e29b-41d4-a716-446655440001".to_string()),
            store,
            Some(Message::new(Role::User, vec![Part::text("Hello".to_string())])),
            None,
        );
        
        assert!(manager.is_ok());
        let manager = manager.unwrap();
        assert_eq!(manager.task_id(), Some("550e8400-e29b-41d4-a716-446655440000"));
        assert_eq!(manager.context_id(), Some("550e8400-e29b-41d4-a716-446655440001"));
    }

    #[tokio::test]
    async fn test_task_manager_invalid_task_id() {
        let store = Arc::new(InMemoryTaskStore::new());
        let manager = TaskManager::new(
            Some("".to_string()),
            Some("550e8400-e29b-41d4-a716-446655440001".to_string()),
            store,
            None,
            None,
        );
        
        assert!(manager.is_err());
    }

    #[tokio::test]
    async fn test_save_task_event() {
        let (mut manager, store) = create_test_task_manager();
        
        let task_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let context_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").unwrap();
        
        let task = Task {
            id: task_id.to_string(),
            context_id: context_id.to_string(),
            status: TaskStatus {
                state: TaskState::Working,
                timestamp: Some(chrono::Utc::now().to_string()),
                message: None,
            },
            artifacts: None,
            history: None,
            metadata: None,
            kind: "task".to_string(),
        };

        let saved_task = manager.save_task_event(TaskEvent::Task(task.clone())).await.unwrap();
        assert_eq!(saved_task.id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
        
        // Verify it was saved to store
        let retrieved = store.get("550e8400-e29b-41d4-a716-446655440000").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().status.state, TaskState::Working);
    }

    #[tokio::test]
    async fn test_ensure_task_creates_new() {
        let (manager, store) = create_test_task_manager();
        
        let status_event = TaskStatusUpdateEvent {
            task_id: "550e8400-e29b-41d4-a716-446655440002".to_string(),
            context_id: "550e8400-e29b-41d4-a716-446655440003".to_string(),
            status: TaskStatus {
                state: TaskState::Working,
                timestamp: Some(chrono::Utc::now().to_string()),
                message: None,
            },
            r#final: false,
            kind: "status-update".to_string(),
            metadata: None,
        };

        let task = manager.ensure_task(&status_event).await.unwrap();
        assert_eq!(task.id.to_string(), "550e8400-e29b-41d4-a716-446655440002");
        assert_eq!(task.context_id.to_string(), "550e8400-e29b-41d4-a716-446655440003");
        assert_eq!(task.status.state, TaskState::Submitted); // Initial state
        
        // Verify it was saved to store
        let retrieved = store.get("550e8400-e29b-41d4-a716-446655440002").await.unwrap();
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_update_with_message() {
        let (manager, _) = create_test_task_manager();
        
        let task_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let context_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").unwrap();
        
        let task = Task {
            id: task_id.to_string(),
            context_id: context_id.to_string(),
            status: TaskStatus {
                state: TaskState::Working,
                timestamp: Some(chrono::Utc::now().to_string()),
                message: Some(Box::new(Message::new(Role::Agent, vec![Part::text("Current status".to_string())]))),
            },
            artifacts: None,
            history: None,
            metadata: None,
            kind: "task".to_string(),
        };

        let new_message = Message::new(Role::User, vec![Part::text("New input".to_string())]);
        let updated_task = manager.update_with_message(new_message.clone(), task.clone()).await;

        assert!(updated_task.history.is_some());
        assert_eq!(updated_task.history.as_ref().unwrap().len(), 2);
        assert_eq!(updated_task.history.as_ref().unwrap()[0].role, Role::Agent);
        assert_eq!(updated_task.history.as_ref().unwrap()[1].role, Role::User);
        assert!(updated_task.status.message.is_none());
    }
}
