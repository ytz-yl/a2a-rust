//! Task Store interface and implementations
//! 
//! This module defines the interface for persisting and retrieving Task objects,
//! along with in-memory and database implementations.
//! 
//! This implementation aligns with the Python version which uses string IDs
//! for better compatibility.

use crate::{Task, A2AError};
use async_trait::async_trait;

/// Task Store interface for persisting and retrieving Task objects
/// 
/// This trait mirrors the Python TaskStore interface exactly, using string
/// identifiers for compatibility with the A2A specification.
#[async_trait]
pub trait TaskStore: Send + Sync {
    /// Saves or updates a task in the store
    async fn save(&self, task: Task) -> Result<(), A2AError>;
    
    /// Retrieves a task from the store by ID
    async fn get(&self, task_id: &str) -> Result<Option<Task>, A2AError>;
    
    /// Deletes a task from the store by ID
    async fn delete(&self, task_id: &str) -> Result<(), A2AError>;
    
    /// Lists all tasks in the store (optional implementation)
    async fn list(&self) -> Result<Vec<Task>, A2AError> {
        Err(A2AError::unsupported_operation("Task listing not supported"))
    }
    
    /// Lists tasks by context ID (optional implementation)
    async fn list_by_context(&self, _context_id: &str) -> Result<Vec<Task>, A2AError> {
        Err(A2AError::unsupported_operation("Task listing by context not supported"))
    }
}

/// In-memory implementation of TaskStore
/// 
/// Uses a HashMap with string keys to store tasks, compatible with the
/// Python implementation's string-based identifiers.
pub struct InMemoryTaskStore {
    tasks: std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, Task>>>,
}

impl InMemoryTaskStore {
    /// Creates a new in-memory task store
    pub fn new() -> Self {
        Self {
            tasks: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }
    
    /// Creates a new in-memory task store with initial capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            tasks: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::with_capacity(capacity))),
        }
    }
}

impl Default for InMemoryTaskStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskStore for InMemoryTaskStore {
    async fn save(&self, task: Task) -> Result<(), A2AError> {
        let mut tasks = self.tasks.write().await;
        // Convert UUID to string for storage key
        let task_id_str = task.id.to_string();
        tasks.insert(task_id_str, task);
        Ok(())
    }
    
    async fn get(&self, task_id: &str) -> Result<Option<Task>, A2AError> {
        let tasks = self.tasks.read().await;
        Ok(tasks.get(task_id).cloned())
    }
    
    async fn delete(&self, task_id: &str) -> Result<(), A2AError> {
        let mut tasks = self.tasks.write().await;
        tasks.remove(task_id);
        Ok(())
    }
    
    async fn list(&self) -> Result<Vec<Task>, A2AError> {
        let tasks = self.tasks.read().await;
        Ok(tasks.values().cloned().collect())
    }
    
    async fn list_by_context(&self, context_id: &str) -> Result<Vec<Task>, A2AError> {
        let tasks = self.tasks.read().await;
        let filtered_tasks: Vec<Task> = tasks
            .values()
            .filter(|task| task.context_id.to_string() == context_id)
            .cloned()
            .collect();
        Ok(filtered_tasks)
    }
}

/// Database implementation of TaskStore (placeholder for future implementation)
/// 
/// This would integrate with a database backend for persistent storage.
pub struct DatabaseTaskStore {
    // Database connection and configuration would go here
    _connection_string: String,
}

impl DatabaseTaskStore {
    /// Creates a new database task store
    pub fn new(connection_string: String) -> Self {
        Self {
            _connection_string: connection_string,
        }
    }
}

#[async_trait]
impl TaskStore for DatabaseTaskStore {
    async fn save(&self, _task: Task) -> Result<(), A2AError> {
        // TODO: Implement database save logic
        Err(A2AError::unsupported_operation("DatabaseTaskStore::save not yet implemented"))
    }
    
    async fn get(&self, _task_id: &str) -> Result<Option<Task>, A2AError> {
        // TODO: Implement database get logic
        Err(A2AError::unsupported_operation("DatabaseTaskStore::get not yet implemented"))
    }
    
    async fn delete(&self, _task_id: &str) -> Result<(), A2AError> {
        // TODO: Implement database delete logic
        Err(A2AError::unsupported_operation("DatabaseTaskStore::delete not yet implemented"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TaskStatus, TaskState};
    
    fn create_test_task(id: &str, context_id: &str) -> Task {
        Task {
            id: id.to_string(),
            context_id: context_id.to_string(),
            status: TaskStatus {
                state: TaskState::Submitted,
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                message: None,
            },
            artifacts: None,
            history: None,
            metadata: None,
            kind: "task".to_string(),
        }
    }
    
    #[tokio::test]
    async fn test_in_memory_task_store_basic_operations() {
        let store = InMemoryTaskStore::new();
        let task = create_test_task("550e8400-e29b-41d4-a716-446655440000", "550e8400-e29b-41d4-a716-446655440001");
        
        // Test save
        store.save(task.clone()).await.unwrap();
        
        // Test get
        let retrieved = store.get("550e8400-e29b-41d4-a716-446655440000").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
        
        // Test get non-existent
        let not_found = store.get("nonexistent").await.unwrap();
        assert!(not_found.is_none());
    }
    
    #[tokio::test]
    async fn test_in_memory_task_store_update() {
        let store = InMemoryTaskStore::new();
        let mut task = create_test_task("550e8400-e29b-41d4-a716-446655440000", "550e8400-e29b-41d4-a716-446655440001");
        
        // Save initial task
        store.save(task.clone()).await.unwrap();
        
        // Update task
        task.status.state = TaskState::Completed;
        store.save(task.clone()).await.unwrap();
        
        // Verify update
        let retrieved = store.get("550e8400-e29b-41d4-a716-446655440000").await.unwrap().unwrap();
        assert_eq!(retrieved.status.state, TaskState::Completed);
    }
    
    #[tokio::test]
    async fn test_in_memory_task_store_delete() {
        let store = InMemoryTaskStore::new();
        let task = create_test_task("550e8400-e29b-41d4-a716-446655440000", "550e8400-e29b-41d4-a716-446655440001");
        
        // Save task
        store.save(task.clone()).await.unwrap();
        assert!(store.get("550e8400-e29b-41d4-a716-446655440000").await.unwrap().is_some());
        
        // Delete task
        store.delete("550e8400-e29b-41d4-a716-446655440000").await.unwrap();
        assert!(store.get("550e8400-e29b-41d4-a716-446655440000").await.unwrap().is_none());
    }
    
    #[tokio::test]
    async fn test_in_memory_task_store_list() {
        let store = InMemoryTaskStore::new();
        let task1 = create_test_task("550e8400-e29b-41d4-a716-446655440000", "550e8400-e29b-41d4-a716-446655440001");
        let task2 = create_test_task("550e8400-e29b-41d4-a716-446655440002", "550e8400-e29b-41d4-a716-446655440001");
        let task3 = create_test_task("550e8400-e29b-41d4-a716-446655440003", "550e8400-e29b-41d4-a716-446655440002");
        
        // Save tasks
        store.save(task1).await.unwrap();
        store.save(task2).await.unwrap();
        store.save(task3).await.unwrap();
        
        // List all tasks
        let all_tasks = store.list().await.unwrap();
        assert_eq!(all_tasks.len(), 3);
        
        // List by context
        let context1_tasks = store.list_by_context("550e8400-e29b-41d4-a716-446655440001").await.unwrap();
        assert_eq!(context1_tasks.len(), 2);
        
        let context2_tasks = store.list_by_context("550e8400-e29b-41d4-a716-446655440002").await.unwrap();
        assert_eq!(context2_tasks.len(), 1);
    }
}
