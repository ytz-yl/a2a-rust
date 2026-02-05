use a2a_rust::a2a::models::*;
use a2a_rust::a2a::core_types::{Message, Role, Part, TaskState};
use a2a_rust::a2a::server::request_handlers::{DefaultRequestHandler, RequestHandler, MessageSendResult};
use a2a_rust::a2a::server::tasks::{
    InMemoryTaskStore, InMemoryPushNotificationConfigStore, HttpPushNotificationSender, 
    TaskStore, PushNotificationConfigStore, PushNotificationSender
};
use std::sync::Arc;
use mockito::Server;

/// Test 1: Basic push notification with in-message config
/// 对应 Python 的 test_notification_triggering_with_in_message_config_e2e
#[tokio::test]
async fn test_push_notification_with_in_message_config() {
    // 1. Setup Mock Push Server
    let mut server = Server::new_async().await;
    let url_str = server.url();
    let url: url::Url = url_str.parse().unwrap();
    
    let mock = server.mock("POST", "/")
        .with_status(200)
        .expect(1) // Expect exactly 1 notification
        .match_header("X-A2A-Notification-Token", "test-token")
        .create_async()
        .await;

    // 2. Setup Handler with Stores and Sender
    let task_store = Arc::new(InMemoryTaskStore::new());
    let push_config_store = Arc::new(InMemoryPushNotificationConfigStore::new());
    let push_sender = Arc::new(HttpPushNotificationSender::new(push_config_store.clone()));
    
    let handler = DefaultRequestHandler::new(
        task_store.clone(),
        Some(push_config_store.clone()),
        Some(push_sender),
    );

    // 3. Send Message with Push Config
    let message = Message::new(Role::User, vec![Part::text("Hello Agent!".to_string())]);
    let config = PushNotificationConfig {
        id: Some("in-message-config".to_string()),
        url,
        token: Some("test-token".to_string()),
        authentication: None,
    };
    
    let params = MessageSendParams::new(message)
        .with_configuration(MessageSendConfiguration::new().with_push_notification_config(config));

    // 4. Execute on_message_send
    let result = handler.on_message_send(params, None).await.unwrap();
    
    // 5. Verify result
    if let MessageSendResult::Task(task) = result {
        assert_eq!(task.status.state, TaskState::Working);
        
        // Verify push config was saved
        let configs = push_config_store.get_info(&task.id).await.unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].id, Some("in-message-config".to_string()));
        assert_eq!(configs[0].token, Some("test-token".to_string()));
    } else {
        panic!("Expected Task result");
    }

    // 6. Verify notification was sent
    mock.assert_async().await;
}

/// Test 2: Push notification after config change
/// 对应 Python 的 test_notification_triggering_after_config_change_e2e
#[tokio::test]
async fn test_push_notification_after_config_change() {
    // 1. Setup Mock Push Server
    let mut server = Server::new_async().await;
    let url_str = server.url();
    let url: url::Url = url_str.parse().unwrap();
    
    // First message should NOT trigger notification
    let mock1 = server.mock("POST", "/")
        .with_status(200)
        .expect(0) // Expect NO notification for first message
        .create_async()
        .await;

    // 2. Setup Handler WITHOUT push sender initially
    let task_store = Arc::new(InMemoryTaskStore::new());
    let push_config_store = Arc::new(InMemoryPushNotificationConfigStore::new());
    
    let handler_without_push = DefaultRequestHandler::new(
        task_store.clone(),
        Some(push_config_store.clone()),
        None, // No push sender
    );

    // 3. Send first message WITHOUT push config
    let message1 = Message::new(Role::User, vec![Part::text("How are you?".to_string())]);
    let params1 = MessageSendParams::new(message1);

    let result1 = handler_without_push.on_message_send(params1, None).await.unwrap();
    
    let task_id = if let MessageSendResult::Task(task) = result1 {
        assert_eq!(task.status.state, TaskState::Working);
        task.id.clone()
    } else {
        panic!("Expected Task result");
    };

    // Verify no notification was sent
    mock1.assert_async().await;

    // 4. Now set push notification config
    let config = PushNotificationConfig {
        id: Some("after-config-change".to_string()),
        url: url.clone(),
        token: Some("new-token".to_string()),
        authentication: None,
    };
    
    let set_config_params = TaskPushNotificationConfig::new(task_id.clone(), config);
    
    // Setup handler WITH push sender
    let push_sender = Arc::new(HttpPushNotificationSender::new(push_config_store.clone()));
    let handler_with_push = DefaultRequestHandler::new(
        task_store.clone(),
        Some(push_config_store.clone()),
        Some(push_sender),
    );
    
    handler_with_push.on_set_task_push_notification_config(set_config_params, None).await.unwrap();

    // 5. Setup mock for second notification
    let mock2 = server.mock("POST", "/")
        .with_status(200)
        .expect(1) // Expect 1 notification for second message
        .match_header("X-A2A-Notification-Token", "new-token")
        .create_async()
        .await;

    // 6. Send second message (should trigger notification now)
    let mut message2 = Message::new(Role::User, vec![Part::text("Good".to_string())]);
    message2.task_id = Some(task_id.clone());
    let params2 = MessageSendParams::new(message2);

    let result2 = handler_with_push.on_message_send(params2, None).await.unwrap();
    
    if let MessageSendResult::Task(task) = result2 {
        assert_eq!(task.id, task_id);
        assert_eq!(task.status.state, TaskState::Working);
    } else {
        panic!("Expected Task result");
    }

    // 7. Verify notification was sent
    mock2.assert_async().await;
}

/// Test 3: Multiple push notification configs for same task
#[tokio::test]
async fn test_multiple_push_configs() {
    // 1. Setup two mock servers
    let mut server1 = Server::new_async().await;
    let url1 = server1.url().parse().unwrap();
    // Expect 2 requests: one from on_message_send, one from manual send_notification
    let mock1 = server1.mock("POST", "/")
        .with_status(200)
        .expect(2)
        .create_async()
        .await;

    let mut server2 = Server::new_async().await;
    let url2 = server2.url().parse().unwrap();
    // Expect 1 request: only from manual send_notification (not in initial config)
    let mock2 = server2.mock("POST", "/")
        .with_status(200)
        .expect(1)
        .create_async()
        .await;

    // 2. Setup Handler
    let task_store = Arc::new(InMemoryTaskStore::new());
    let push_config_store = Arc::new(InMemoryPushNotificationConfigStore::new());
    let push_sender = Arc::new(HttpPushNotificationSender::new(push_config_store.clone()));
    
    let handler = DefaultRequestHandler::new(
        task_store.clone(),
        Some(push_config_store.clone()),
        Some(push_sender),
    );

    // 3. Send message with first config (this will trigger first notification to url1)
    let message = Message::new(Role::User, vec![Part::text("Test".to_string())]);
    let config1 = PushNotificationConfig {
        id: Some("config1".to_string()),
        url: url1,
        token: Some("token1".to_string()),
        authentication: None,
    };
    
    let params = MessageSendParams::new(message)
        .with_configuration(MessageSendConfiguration::new().with_push_notification_config(config1));

    let result = handler.on_message_send(params, None).await.unwrap();
    
    let task_id = if let MessageSendResult::Task(task) = result {
        task.id.clone()
    } else {
        panic!("Expected Task result");
    };

    // 4. Add second config to same task
    let config2 = PushNotificationConfig {
        id: Some("config2".to_string()),
        url: url2,
        token: Some("token2".to_string()),
        authentication: None,
    };
    
    push_config_store.set_info(&task_id, config2).await.unwrap();

    // 5. Verify both configs exist
    let configs = push_config_store.get_info(&task_id).await.unwrap();
    assert_eq!(configs.len(), 2);

    // 6. Update task to trigger notifications to both endpoints
    // This will send to both url1 (second time) and url2 (first time)
    let task = task_store.get(&task_id).await.unwrap().unwrap();
    let push_sender = Arc::new(HttpPushNotificationSender::new(push_config_store.clone()));
    push_sender.send_notification(&task).await.unwrap();

    // 7. Verify both notifications were sent
    mock1.assert_async().await;
    mock2.assert_async().await;
}

/// Test 4: Push notification with failed endpoint
#[tokio::test]
async fn test_push_notification_with_failed_endpoint() {
    // 1. Setup Mock Push Server that returns error
    let mut server = Server::new_async().await;
    let url_str = server.url();
    let url = url_str.parse().unwrap();
    
    let mock = server.mock("POST", "/")
        .with_status(500) // Server error
        .expect(1)
        .create_async()
        .await;

    // 2. Setup Handler
    let task_store = Arc::new(InMemoryTaskStore::new());
    let push_config_store = Arc::new(InMemoryPushNotificationConfigStore::new());
    let push_sender = Arc::new(HttpPushNotificationSender::new(push_config_store.clone()));
    
    let handler = DefaultRequestHandler::new(
        task_store,
        Some(push_config_store),
        Some(push_sender),
    );

    // 3. Send Message with Push Config
    let message = Message::new(Role::User, vec![Part::text("Test".to_string())]);
    let config = PushNotificationConfig {
        id: Some("config1".to_string()),
        url,
        token: Some("test-token".to_string()),
        authentication: None,
    };
    
    let params = MessageSendParams::new(message)
        .with_configuration(MessageSendConfiguration::new().with_push_notification_config(config));

    // 4. Execute on_message_send - should not fail even if notification fails
    let result = handler.on_message_send(params, None).await;
    assert!(result.is_ok(), "Handler should not fail even if push notification fails");
    
    if let Ok(MessageSendResult::Task(task)) = result {
        assert_eq!(task.status.state, TaskState::Working);
    } else {
        panic!("Expected Task result");
    }

    // 5. Verify notification was attempted
    mock.assert_async().await;
}

/// Test 5: Push notification config CRUD operations
#[tokio::test]
async fn test_push_notification_config_crud() {
    let task_store = Arc::new(InMemoryTaskStore::new());
    let push_config_store = Arc::new(InMemoryPushNotificationConfigStore::new());
    let push_sender = Arc::new(HttpPushNotificationSender::new(push_config_store.clone()));
    
    let handler = DefaultRequestHandler::new(
        task_store.clone(),
        Some(push_config_store.clone()),
        Some(push_sender),
    );

    // 1. Create a task first
    let message = Message::new(Role::User, vec![Part::text("Test".to_string())]);
    let params = MessageSendParams::new(message);
    let result = handler.on_message_send(params, None).await.unwrap();
    
    let task_id = if let MessageSendResult::Task(task) = result {
        task.id.clone()
    } else {
        panic!("Expected Task result");
    };

    // 2. Set push notification config
    let config = PushNotificationConfig {
        id: Some("test-config".to_string()),
        url: "http://example.com/webhook".parse().unwrap(),
        token: Some("test-token".to_string()),
        authentication: None,
    };
    
    let set_params = TaskPushNotificationConfig::new(task_id.clone(), config.clone());
    let set_result = handler.on_set_task_push_notification_config(set_params, None).await;
    assert!(set_result.is_ok());

    // 3. Get push notification config
    let get_params = a2a_rust::a2a::server::request_handlers::TaskPushNotificationConfigQueryParams {
        task_id: task_id.clone(),
        push_notification_config_id: Some("test-config".to_string()),
        metadata: None,
    };
    let get_result = handler.on_get_task_push_notification_config(get_params, None).await;
    assert!(get_result.is_ok());
    
    if let Ok(retrieved_config) = get_result {
        assert_eq!(retrieved_config.task_id, task_id);
        assert_eq!(retrieved_config.push_notification_config.id, Some("test-config".to_string()));
    }

    // 4. List push notification configs
    let list_params = TaskIdParams::new(task_id.clone());
    let list_result = handler.on_list_task_push_notification_config(list_params, None).await;
    assert!(list_result.is_ok());
    
    if let Ok(configs) = list_result {
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].push_notification_config.id, Some("test-config".to_string()));
    }

    // 5. Delete push notification config
    let delete_params = DeleteTaskPushNotificationConfigParams::new(
        task_id.clone(),
        "test-config".to_string()
    );
    let delete_result = handler.on_delete_task_push_notification_config(delete_params, None).await;
    assert!(delete_result.is_ok());

    // 6. Verify config was deleted
    let configs_after_delete = push_config_store.get_info(&task_id).await.unwrap();
    assert_eq!(configs_after_delete.len(), 0);
}
