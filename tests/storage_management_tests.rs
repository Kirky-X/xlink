use std::collections::HashSet;
use uuid::Uuid;
use xpush::core::types::{DeviceCapabilities, DeviceId, Message, MessagePayload, MessagePriority, DeviceType};
use xpush::storage::file_store::FileStorage;
use xpush::core::traits::Storage;

mod common;
use common::TestSdkBuilder;

#[tokio::test]
async fn test_storage_full_cleanup() {
    // 创建测试用的临时存储目录
    let storage_path = "./test_storage_full";
    
    // 清理并创建存储目录
    let _ = tokio::fs::remove_dir_all(storage_path).await;
    let storage = FileStorage::new(storage_path).await.unwrap();
    
    // 创建设备ID
    let device_id = DeviceId(uuid::uuid!("550e8400-e29b-41d4-a716-446655440000"));
    let recipient_id = DeviceId(uuid::uuid!("550e8400-e29b-41d4-a716-446655440001"));
    
    // 创建大量消息来模拟存储空间满的情况
    let mut total_size = 0u64;
    let _target_size = 1024 * 1024; // 1MB 目标大小
    let message_count = 50; // 创建50条消息
    
    for i in 0..message_count {
        let payload = MessagePayload::Text(format!("Test message {} with some padding to make it larger and consume more storage space", i));
        let message = Message {
            id: Uuid::new_v4(),
            sender: device_id,
            recipient: recipient_id,
            group_id: None,
            payload,
            priority: MessagePriority::Normal,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            require_ack: false,
        };
        
        storage.save_message(&message).await.unwrap();
        
        // 获取消息大小
        let message_size = serde_json::to_vec(&message).unwrap().len() as u64;
        total_size += message_size;
    }
    
    println!("Created {} messages, total size: {} bytes", message_count, total_size);
    
    // 验证存储使用情况
    let initial_usage = storage.get_storage_usage().await.unwrap();
    println!("Initial storage usage: {} bytes", initial_usage);
    assert!(initial_usage > 0, "Storage should contain messages");
    
    // 模拟存储空间清理 - 将目标大小设置为当前大小的一半
    let target_size = initial_usage / 2;
    println!("Cleaning up storage to target size: {} bytes", target_size);
    
    let removed_size = storage.cleanup_storage(target_size).await.unwrap();
    println!("Removed {} bytes during cleanup", removed_size);
    
    // 验证清理后的存储使用情况
    let final_usage = storage.get_storage_usage().await.unwrap();
    println!("Final storage usage: {} bytes", final_usage);
    
    // 验证清理效果
    assert!(final_usage <= target_size + 1024, "Storage usage should be close to target size (within 1KB tolerance)");
    assert!(removed_size > 0, "Should have removed some data during cleanup");
    
    // 验证剩余的消息数量减少
    let remaining_messages = storage.get_pending_messages(&recipient_id).await.unwrap();
    println!("Remaining messages after cleanup: {}", remaining_messages.len());
    assert!(remaining_messages.len() < message_count, "Should have fewer messages after cleanup");
    
    // 清理测试数据
    let _ = tokio::fs::remove_dir_all(storage_path).await;
}

#[tokio::test]
async fn test_storage_full_cleanup_with_sdk() {
    // 测试SDK在存储空间满时的行为
    let storage_path = "./test_sdk_storage_full";
    let _ = tokio::fs::remove_dir_all(storage_path).await;
    
    // 创建SDK实例
    let device_capabilities = DeviceCapabilities {
        device_id: DeviceId(uuid::uuid!("550e8400-e29b-41d4-a716-446655440002")),
        device_type: DeviceType::Smartphone,
        device_name: "Test Device".to_string(),
        supported_channels: HashSet::new(),
        battery_level: Some(80),
        is_charging: false,
        data_cost_sensitive: false,
    };
    
    let _sdk = TestSdkBuilder::new()
        .with_device_capabilities(device_capabilities)
        .build()
        .await
        .unwrap();
    
    // 获取存储实例并填充大量数据
    let storage = FileStorage::new(storage_path).await.unwrap();
    
    // 创建大量消息来填充存储
    let device_id = DeviceId(uuid::uuid!("550e8400-e29b-41d4-a716-446655440002"));
    let recipient_id = DeviceId(uuid::uuid!("550e8400-e29b-41d4-a716-446655440003"));
    
    for i in 0..30 {
        let payload = MessagePayload::Text(format!("Large message content with padding to consume storage space - message number {}", i));
        let message = Message {
            id: Uuid::new_v4(),
            sender: device_id,
            recipient: recipient_id,
            group_id: None,
            payload,
            priority: MessagePriority::Normal,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            require_ack: false,
        };
        
        storage.save_message(&message).await.unwrap();
    }
    
    // 获取初始存储使用情况
    let initial_usage = storage.get_storage_usage().await.unwrap();
    println!("Initial SDK storage usage: {} bytes", initial_usage);
    
    // 模拟存储空间清理 - 设置较小的目标大小
    let target_size = 5 * 1024; // 5KB - smaller than current usage
    let removed_size = storage.cleanup_storage(target_size).await.unwrap();
    
    println!("Removed {} bytes during SDK storage cleanup", removed_size);
    
    // 验证清理后的存储使用情况
    let final_usage = storage.get_storage_usage().await.unwrap();
    println!("Final SDK storage usage: {} bytes", final_usage);
    
    // 验证清理效果
    assert!(final_usage <= target_size + 1024, "Storage usage should be close to target size");
    assert!(removed_size > 0, "Should have removed data during cleanup");
    
    // 清理测试数据
    let _ = tokio::fs::remove_dir_all(storage_path).await;
}

#[tokio::test]
async fn test_storage_cleanup_preserves_recent_messages() {
    // 测试存储清理时保留最近的消息
    let storage_path = "./test_storage_recent";
    let _ = tokio::fs::remove_dir_all(storage_path).await;
    let storage = FileStorage::new(storage_path).await.unwrap();
    
    let device_id = DeviceId(uuid::uuid!("550e8400-e29b-41d4-a716-446655440004"));
    let recipient_id = DeviceId(uuid::uuid!("550e8400-e29b-41d4-a716-446655440005"));
    
    // 创建一些旧消息（模拟1天前的消息）
    let old_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - 24 * 3600; // 1天前
    
    for i in 0..10 {
        let payload = MessagePayload::Text(format!("Old message {}", i));
        let message = Message {
            id: Uuid::new_v4(),
            sender: device_id,
            recipient: recipient_id,
            group_id: None,
            payload,
            priority: MessagePriority::Normal,
            timestamp: old_timestamp,
            require_ack: false,
        };
        
        // 为了模拟旧消息，我们需要先保存，然后修改文件时间
        storage.save_message(&message).await.unwrap();
    }
    
    // 创建一些新消息
    for i in 0..5 {
        let payload = MessagePayload::Text(format!("Recent message {}", i));
        let message = Message {
            id: Uuid::new_v4(),
            sender: device_id,
            recipient: recipient_id,
            group_id: None,
            payload,
            priority: MessagePriority::Normal,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            require_ack: false,
        };
        
        storage.save_message(&message).await.unwrap();
    }
    
    // 获取初始消息数量
    let initial_messages = storage.get_pending_messages(&recipient_id).await.unwrap();
    println!("Initial message count: {}", initial_messages.len());
    
    // 获取当前存储使用情况
    let initial_storage_usage = storage.get_storage_usage().await.unwrap();
    println!("Initial storage usage: {} bytes", initial_storage_usage);
    
    // 执行存储清理 - 设置较小的目标大小以触发清理
    let target_size = 500; // 500 bytes - 足够小以触发清理
    println!("Target size: {} bytes", target_size);
    let removed_size = storage.cleanup_storage(target_size).await.unwrap();
    println!("Removed {} bytes during cleanup", removed_size);
    
    // 验证清理后保留了足够的消息
    let final_messages = storage.get_pending_messages(&recipient_id).await.unwrap();
    println!("Final message count: {}", final_messages.len());
    
    // 验证清理效果
    assert!(final_messages.len() > 0, "Should retain some messages after cleanup");
    assert!(final_messages.len() < initial_messages.len(), "Should have fewer messages after cleanup");
    
    // 清理测试数据
    let _ = tokio::fs::remove_dir_all(storage_path).await;
}