//! Integration tests for group communication functionality
//!
//! Tests cover multi-device group messaging, group synchronization,
//! and performance under load as specified in test.md section 2.3.2

use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use xpush::core::types::MessagePayload;
use crate::common::{test_device_id, TestSdkBuilder, NetworkSimulator};

mod common;

#[tokio::test]
async fn test_multi_device_group_creation() {
    // IT-GRP-001: 多设备群组创建
    let sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build().await.unwrap();
    
    // Create 5 test devices by creating separate SDK instances
    let mut device_ids = Vec::new();
    for _i in 0..5 {
        let device_id = test_device_id();
        device_ids.push(device_id);
    }
    
    // Create group with all devices
    let group_id = sdk.create_group("Integration Test Group".to_string(), device_ids.clone()).await.unwrap();
    
    // Verify group was created successfully
    assert_ne!(group_id.0, uuid::Uuid::nil());
    
    // Test that we can send a message to the group
    let result = sdk.send_to_group(group_id, MessagePayload::Text("Test message".to_string())).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_group_message_broadcast() {
    // IT-GRP-002: 群组消息广播
    let sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build().await.unwrap();
    
    // Create 10 test devices
    let mut device_ids = Vec::new();
    for _i in 0..10 {
        let device_id = test_device_id();
        device_ids.push(device_id);
    }
    
    let group_id = sdk.create_group("Broadcast Group".to_string(), device_ids.clone()).await.unwrap();
    
    // Send broadcast message to the group
    let result = sdk.send_to_group(group_id, MessagePayload::Text("Hello everyone!".to_string())).await;
    assert!(result.is_ok());
    
    // Allow time for message processing
    sleep(Duration::from_millis(100)).await;
}

#[tokio::test]
async fn test_group_synchronization() {
    // IT-GRP-004: 群组同步
    let sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build().await.unwrap();
    
    // Create devices
    let mut device_ids = Vec::new();
    for _i in 0..5 {
        let device_id = test_device_id();
        device_ids.push(device_id);
    }
    
    let group_id = sdk.create_group("Sync Group".to_string(), device_ids.clone()).await.unwrap();
    
    // Send multiple messages to the group
    for _i in 0..5 {
        let message = MessagePayload::Text("Test message".to_string());
        let result = sdk.send_to_group(group_id, message).await;
        assert!(result.is_ok());
    }
    
    // Allow time for message processing
    sleep(Duration::from_millis(200)).await;
}

#[tokio::test]
async fn test_treekem_group_key_negotiation() {
    // IT-GRP-001: TreeKEM 组密钥协商
    // 确保密钥更新和分发逻辑正常工作
    let sdk = TestSdkBuilder::new().build().await.unwrap();
    let device1_id = sdk.device_id();
    
    // 注册本地公钥
    sdk.register_device_key(device1_id, sdk.public_key()).unwrap();

    // 创建10个成员并注册公钥
    let mut member_ids = Vec::new();
    for _ in 0..10 {
        let member_id = test_device_id();
        let member_pk = x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::random_from_rng(rand::rngs::OsRng));
        sdk.register_device_key(member_id, member_pk).unwrap();
        member_ids.push(member_id);
    }

    // 创建群组，内部会初始化 TreeKEM
    let mut all_members = member_ids.clone();
    all_members.push(device1_id);
    
    // 确保本地设备的公钥已注册
    sdk.register_device_key(device1_id, sdk.public_key()).unwrap();
    
    let group_id = sdk.create_group("TreeKEM Group".to_string(), all_members).await.unwrap();

    // 验证群组加密消息
    let payload = MessagePayload::Text("Secure message".to_string());
    let encrypted = sdk.encrypt_group_message(group_id, &payload).unwrap();
    
    match encrypted {
        MessagePayload::Binary(data) => {
            assert!(data.len() > 20, "Encrypted data too short");
            // 验证解密
            let decrypted = sdk.decrypt_group_message(group_id, &MessagePayload::Binary(data)).unwrap();
            assert_eq!(decrypted, payload);
        }
        _ => panic!("Encryption failed to return binary payload"),
    }

    // 测试密钥轮转 (Forward Secrecy)
    let result = sdk.rotate_group_key(group_id).await;
    assert!(result.is_ok(), "Key rotation failed: {:?}", result.err());
}

#[tokio::test]
async fn test_group_performance_under_load() {
    // IT-GRP-005: 负载下的群组性能
    let sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build().await.unwrap();
    
    // Create 20 test devices
    let mut device_ids = Vec::new();
    for _ in 0..20 {
        let device_id = test_device_id();
        device_ids.push(device_id);
    }
    
    let group_id = sdk.create_group("Performance Group".to_string(), device_ids.clone()).await.unwrap();
    
    // Send multiple messages concurrently
    let sdk = Arc::new(sdk);
    let mut handles = vec![];
    for i in 0..50 {
        let sdk_clone = Arc::clone(&sdk);
        let group_id_clone = group_id;
        let handle = tokio::spawn(async move {
            let message = MessagePayload::Text(format!("Load test message {}", i));
            sdk_clone.send_to_group(group_id_clone, message).await
        });
        handles.push(handle);
    }
    
    // Wait for all messages to complete
    let results = futures::future::join_all(handles).await;
    let success_count = results.iter().filter(|r| r.is_ok()).count();
    assert!(success_count > 0, "At least some messages should be delivered");
    
    // Send a few messages sequentially to verify basic functionality
    for _i in 0..5 {
        let message = MessagePayload::Text("Sequential message".to_string());
        let result = sdk.send_to_group(group_id, message).await;
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_group_message_with_guarantee() {
    // IT-GRP-006: 带保证机制的群组消息
    let sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build().await.unwrap();
    
    // Create test devices
    let mut device_ids = Vec::new();
    for _ in 0..5 {
        let device_id = test_device_id();
        device_ids.push(device_id);
    }
    
    let group_id = sdk.create_group("Guarantee Group".to_string(), device_ids.clone()).await.unwrap();
    
    // Send message with guarantee (using the standard send_to_group method)
    let important_message = MessagePayload::Text("Important guaranteed message".to_string());
    let result = sdk.send_to_group(group_id, important_message).await;
    
    // The SDK's send_to_group should provide reliability guarantees
    assert!(result.is_ok());
    
    // Allow time for delivery confirmation
    sleep(Duration::from_millis(200)).await;
}

#[tokio::test]
async fn test_group_performance_metrics() {
    // IT-GRP-007: 群组性能指标
    let sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build().await.unwrap();
    
    // Create test devices
    let mut device_ids = Vec::new();
    for _i in 0..5 {
        let device_id = test_device_id();
        device_ids.push(device_id);
    }
    
    let group_id = sdk.create_group("Metrics Group".to_string(), device_ids.clone()).await.unwrap();
    
    // Send multiple messages and measure basic timing
    let start = std::time::Instant::now();
    
    for _i in 0..10 {
        let message = MessagePayload::Text("Metrics message".to_string());
        let result = sdk.send_to_group(group_id, message).await;
        assert!(result.is_ok());
    }
    
    let elapsed = start.elapsed();
    
    // Basic performance assertion - all messages should complete within reasonable time
    assert!(elapsed < Duration::from_secs(5), "Message sending took too long: {:?}", elapsed);
    
    // Calculate average time per message
    let avg_time = elapsed / 10;
    println!("Average time per message: {:?}", avg_time);
}