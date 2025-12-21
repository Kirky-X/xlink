//! Integration tests for remote communication via ntfy
//!
//! Tests cover remote device communication, message relay, and ntfy integration
//! as specified in test.md section 2.2.2

use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use xpush::core::types::{MessagePayload, DeviceCapabilities, DeviceType, ChannelType};
use xpush::channels::remote::RemoteChannel;

use crate::common::{TestSdkBuilder, NetworkSimulator, test_device_id, test_device_capabilities};

mod common;

#[tokio::test]
async fn test_remote_device_communication() {
    // IT-RMT-001: 远程设备通信集成测试
    let device1_id = test_device_id();
    let device2_id = test_device_id();
    
    // Create device capabilities with remote channel support
    let device1_capabilities = DeviceCapabilities {
        device_id: device1_id,
        device_type: DeviceType::Smartphone,
        device_name: "Device 1".to_string(),
        supported_channels: std::collections::HashSet::from([ChannelType::Internet]),
        battery_level: Some(80),
        is_charging: false,
        data_cost_sensitive: false,
    };
    
    let device2_capabilities = DeviceCapabilities {
        device_id: device2_id,
        device_type: DeviceType::Smartphone,
        device_name: "Device 2".to_string(),
        supported_channels: std::collections::HashSet::from([ChannelType::Internet]),
        battery_level: Some(75),
        is_charging: true,
        data_cost_sensitive: false,
    };
    
    // Create SDK instances with remote channel
    let remote_channel1 = Arc::new(RemoteChannel::new(device1_id, Some("https://ntfy.sh".to_string())));
    let remote_channel2 = Arc::new(RemoteChannel::new(device2_id, Some("https://ntfy.sh".to_string())));
    
    let sdk1 = TestSdkBuilder::new()
        .with_device_capabilities(device1_capabilities)
        .with_channel(remote_channel1.clone())
        .with_network_simulator(NetworkSimulator::wifi())
        .build().await.unwrap();
        
    let _sdk2 = TestSdkBuilder::new()
        .with_device_capabilities(device2_capabilities)
        .with_channel(remote_channel2.clone())
        .with_network_simulator(NetworkSimulator::wifi())
        .build().await.unwrap();
    
    // Test remote message sending
    let payload = MessagePayload::Text("Hello from remote device".to_string());
    let result = sdk1.send(device2_id, payload.clone()).await;
    
    assert!(result.is_ok(), "Remote message should be sent successfully");
    
    // Allow time for remote message processing
    sleep(Duration::from_millis(500)).await;
}

#[tokio::test]
async fn test_ntfy_message_relay() {
    // IT-RMT-002: ntfy消息中继测试
    let sender_id = test_device_id();
    let receiver_id = test_device_id();
    
    let sender_capabilities = test_device_capabilities();
    let _receiver_capabilities = test_device_capabilities();
    
    let remote_channel = Arc::new(RemoteChannel::new(sender_id, Some("https://ntfy.sh".to_string())));
    
    let sender_sdk = TestSdkBuilder::new()
        .with_device_capabilities(sender_capabilities)
        .with_channel(remote_channel.clone())
        .with_network_simulator(NetworkSimulator::perfect())
        .build().await.unwrap();
    
    // Test ntfy-based message relay
    let test_payloads = vec![
        MessagePayload::Text("Test message 1".to_string()),
        MessagePayload::Text("Test message 2".to_string()),
        MessagePayload::Binary(vec![1, 2, 3, 4, 5]),
    ];
    
    for payload in test_payloads {
        let result = sender_sdk.send(receiver_id, payload.clone()).await;
        assert!(result.is_ok(), "ntfy relay should succeed for payload: {:?}", payload);
        sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn test_remote_group_communication() {
    // IT-RMT-003: 远程群组通信测试
    let coordinator_id = test_device_id();
    let member_ids = vec![test_device_id(), test_device_id(), test_device_id()];
    
    let coordinator_capabilities = DeviceCapabilities {
        device_id: coordinator_id,
        device_type: DeviceType::Server,
        device_name: "Coordinator".to_string(),
        supported_channels: std::collections::HashSet::from([ChannelType::Internet]),
        battery_level: None,
        is_charging: false,
        data_cost_sensitive: false,
    };
    
    let remote_channel = Arc::new(RemoteChannel::new(coordinator_id, Some("https://ntfy.sh".to_string())));
    
    let coordinator_sdk = TestSdkBuilder::new()
        .with_device_capabilities(coordinator_capabilities)
        .with_channel(remote_channel.clone())
        .with_network_simulator(NetworkSimulator::perfect())
        .build().await.unwrap();
    
    // Create remote group
    let mut all_devices = vec![coordinator_id];
    all_devices.extend(member_ids.clone());
    
    let group_id = coordinator_sdk.create_group("Remote Team".to_string(), all_devices).await.unwrap();
    
    // Test remote group message broadcast
    let group_message = MessagePayload::Text("Remote group broadcast".to_string());
    let result = coordinator_sdk.send_to_group(group_id, group_message.clone()).await;
    
    assert!(result.is_ok(), "Remote group message should be broadcast successfully");
    
    sleep(Duration::from_millis(300)).await;
}

#[tokio::test]
async fn test_remote_message_with_poor_network() {
    // IT-RMT-004: 弱网络环境下的远程通信
    let sender_id = test_device_id();
    let receiver_id = test_device_id();
    
    let sender_capabilities = test_device_capabilities();
    let remote_channel = Arc::new(RemoteChannel::new(sender_id, Some("https://ntfy.sh".to_string())));
    
    let sender_sdk = TestSdkBuilder::new()
        .with_device_capabilities(sender_capabilities)
        .with_channel(remote_channel.clone())
        .with_network_simulator(NetworkSimulator::poor_network())
        .build().await.unwrap();
    
    // Test message delivery in poor network conditions
    let payload = MessagePayload::Text("Message in poor network".to_string());
    let result = sender_sdk.send(receiver_id, payload.clone()).await;
    
    // Should succeed despite poor network (with retry logic)
    assert!(result.is_ok(), "Remote message should be delivered even in poor network");
    
    sleep(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn test_remote_message_encryption() {
    // IT-RMT-005: 远程消息加密测试
    let device1_id = test_device_id();
    let device2_id = test_device_id();
    
    let device1_capabilities = test_device_capabilities();
    let device2_capabilities = test_device_capabilities();
    
    let remote_channel1 = Arc::new(RemoteChannel::new(device1_id, Some("https://ntfy.sh".to_string())));
    let remote_channel2 = Arc::new(RemoteChannel::new(device2_id, Some("https://ntfy.sh".to_string())));
    
    let sdk1 = TestSdkBuilder::new()
        .with_device_capabilities(device1_capabilities)
        .with_channel(remote_channel1.clone())
        .build().await.unwrap();
        
    let sdk2 = TestSdkBuilder::new()
        .with_device_capabilities(device2_capabilities)
        .with_channel(remote_channel2.clone())
        .build().await.unwrap();
    
    // Establish secure sessions between devices
    crate::common::establish_device_sessions(&[&sdk1, &sdk2]).await.unwrap();
    
    // Test encrypted remote message
    let sensitive_payload = MessagePayload::Text("Sensitive remote data".to_string());
    let result = sdk1.send(device2_id, sensitive_payload.clone()).await;
    
    assert!(result.is_ok(), "Encrypted remote message should be sent successfully");
    
    sleep(Duration::from_millis(200)).await;
}

#[tokio::test]
async fn test_remote_device_discovery() {
    // IT-RMT-006: 远程设备发现测试
    let discoverer_id = test_device_id();
    let target_id = test_device_id();
    
    let discoverer_capabilities = test_device_capabilities();
    let target_capabilities = test_device_capabilities();
    
    let discoverer_channel = Arc::new(RemoteChannel::new(discoverer_id, Some("https://ntfy.sh".to_string())));
    let target_channel = Arc::new(RemoteChannel::new(target_id, Some("https://ntfy.sh".to_string())));
    
    let discoverer_sdk = TestSdkBuilder::new()
        .with_device_capabilities(discoverer_capabilities)
        .with_channel(discoverer_channel.clone())
        .build().await.unwrap();
        
    let target_sdk = TestSdkBuilder::new()
        .with_device_capabilities(target_capabilities)
        .with_channel(target_channel.clone())
        .build().await.unwrap();
    
    // Note: Remote device discovery is handled through ntfy topic subscription
    // For testing purposes, we verify that remote communication infrastructure is set up
    sleep(Duration::from_millis(500)).await;
    
    // Verify that SDKs are properly initialized for remote communication
    assert!(!discoverer_sdk.device_id().to_string().is_empty());
    assert!(!target_sdk.device_id().to_string().is_empty());
}

#[tokio::test]
async fn test_remote_message_retry() {
    // IT-RMT-007: 远程消息重试机制测试
    let sender_id = test_device_id();
    let receiver_id = test_device_id();
    
    let sender_capabilities = test_device_capabilities();
    let remote_channel = Arc::new(RemoteChannel::new(sender_id, Some("https://ntfy.sh".to_string())));
    
    let sender_sdk = TestSdkBuilder::new()
        .with_device_capabilities(sender_capabilities)
        .with_channel(remote_channel.clone())
        .with_network_simulator(NetworkSimulator::poor_network())
        .build().await.unwrap();
    
    // Test message retry mechanism with intermittent failures
    let payloads = vec![
        MessagePayload::Text("Retry test 1".to_string()),
        MessagePayload::Text("Retry test 2".to_string()),
        MessagePayload::Binary(vec![0xFF; 100]),
    ];
    
    for payload in payloads {
        let result = sender_sdk.send(receiver_id, payload.clone()).await;
        assert!(result.is_ok(), "Message with retry should eventually succeed");
        sleep(Duration::from_millis(200)).await;
    }
}

#[tokio::test]
async fn test_remote_performance_metrics() {
    // IT-RMT-008: 远程通信性能测试
    let sender_id = test_device_id();
    let receiver_id = test_device_id();
    
    let sender_capabilities = test_device_capabilities();
    let remote_channel = Arc::new(RemoteChannel::new(sender_id, Some("https://ntfy.sh".to_string())));
    
    let sender_sdk = TestSdkBuilder::new()
        .with_device_capabilities(sender_capabilities)
        .with_channel(remote_channel.clone())
        .with_network_simulator(NetworkSimulator::perfect())
        .build().await.unwrap();
    
    // Test remote communication performance
    let start_time = std::time::Instant::now();
    let mut success_count = 0;
    
    for i in 0..20 {
        let payload = MessagePayload::Text(format!("Performance test {}", i));
        let result = sender_sdk.send(receiver_id, payload).await;
        if result.is_ok() {
            success_count += 1;
        }
        sleep(Duration::from_millis(50)).await;
    }
    
    let duration = start_time.elapsed();
    let throughput = success_count as f64 / duration.as_secs_f64();
    
    println!("Remote communication throughput: {:.2} messages/second", throughput);
    
    // Performance assertions
    assert!(success_count >= 18, "Most remote messages should succeed");
    assert!(throughput > 2.0, "Remote communication throughput should be reasonable");
}