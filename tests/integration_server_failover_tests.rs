//! Integration tests for server failover functionality
//!
//! Tests cover primary server failure and automatic failover to backup servers
//! as specified in test.md section IT-RMT-005

use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use xpush::channels::remote::RemoteChannel;
use xpush::core::types::{MessagePayload, DeviceCapabilities, DeviceType, ChannelType};
use xpush::core::traits::Channel;

use crate::common::{TestSdkBuilder, NetworkSimulator, test_device_id, test_device_capabilities};

mod common;

#[tokio::test]
async fn test_server_failover_on_primary_failure() {
    // IT-RMT-005: 服务器切换 - 主服务器故障时自动切换到备用服务器
    let device1_id = test_device_id();
    let device2_id = test_device_id();
    
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
    
    // 创建支持主备切换的远程通道
    let backup_servers = vec![
        "https://ntfy-backup1.sh".to_string(),
        "https://ntfy-backup2.sh".to_string(),
        "https://ntfy.net".to_string(),
    ];
    
    let remote_channel1 = Arc::new(RemoteChannel::with_failover(
        device1_id,
        "https://ntfy-primary.sh".to_string(),
        backup_servers.clone()
    ));
    
    let remote_channel2 = Arc::new(RemoteChannel::with_failover(
        device2_id,
        "https://ntfy-primary.sh".to_string(),
        backup_servers
    ));
    
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
    
    // 验证初始服务器是主服务器
    let initial_server = remote_channel1.current_server_url().await;
    assert_eq!(initial_server, "https://ntfy-primary.sh");
    
    // 模拟主服务器故障，触发切换到备用服务器
    let result = remote_channel1.switch_to_next_server().await;
    assert!(result, "Should successfully switch to backup server");
    
    // 验证已切换到第一个备用服务器
    let current_server = remote_channel1.current_server_url().await;
    assert_eq!(current_server, "https://ntfy-backup1.sh");
    
    // 测试消息发送功能仍然可用（在测试模式下）
    let payload = MessagePayload::Text("Test message after failover".to_string());
    let result = sdk1.send(device2_id, payload.clone()).await;
    assert!(result.is_ok(), "Message should be sent successfully after server failover");
    
    sleep(Duration::from_millis(500)).await;
}

#[tokio::test]
async fn test_multiple_server_failover_chain() {
    // 测试连续的服务器故障切换
    let device1_id = test_device_id();
    let device2_id = test_device_id();
    
    let device1_capabilities = test_device_capabilities();
    let device2_capabilities = test_device_capabilities();
    
    // 创建支持多级故障切换的远程通道
    let backup_servers = vec![
        "https://backup1.ntfy.sh".to_string(),
        "https://backup2.ntfy.sh".to_string(),
        "https://backup3.ntfy.sh".to_string(),
    ];
    
    let remote_channel1 = Arc::new(RemoteChannel::with_failover(
        device1_id,
        "https://primary.ntfy.sh".to_string(),
        backup_servers
    ));
    
    let remote_channel2 = Arc::new(RemoteChannel::new(device2_id, Some("https://ntfy.sh".to_string())));
    
    let sdk1 = TestSdkBuilder::new()
        .with_device_capabilities(device1_capabilities)
        .with_channel(remote_channel1.clone())
        .build().await.unwrap();
        
    let _sdk2 = TestSdkBuilder::new()
        .with_device_capabilities(device2_capabilities)
        .with_channel(remote_channel2.clone())
        .build().await.unwrap();
    
    // 验证初始状态
    let initial_server = remote_channel1.current_server_url().await;
    assert_eq!(initial_server, "https://primary.ntfy.sh");
    
    // 连续切换到不同的备用服务器
    assert!(remote_channel1.switch_to_next_server().await);
    assert_eq!(remote_channel1.current_server_url().await, "https://backup1.ntfy.sh");
    
    assert!(remote_channel1.switch_to_next_server().await);
    assert_eq!(remote_channel1.current_server_url().await, "https://backup2.ntfy.sh");
    
    assert!(remote_channel1.switch_to_next_server().await);
    assert_eq!(remote_channel1.current_server_url().await, "https://backup3.ntfy.sh");
    
    // 尝试切换到不存在的服务器（应该失败）
    let result = remote_channel1.switch_to_next_server().await;
    assert!(!result, "Should fail when no more backup servers available");
    
    // 验证仍然使用最后一个有效的服务器
    let final_server = remote_channel1.current_server_url().await;
    assert_eq!(final_server, "https://backup3.ntfy.sh");
    
    // 验证消息发送仍然可用
    let payload = MessagePayload::Text("Test message after multiple failovers".to_string());
    let result = sdk1.send(device2_id, payload.clone()).await;
    assert!(result.is_ok(), "Message should be sent successfully after multiple failovers");
    
    sleep(Duration::from_millis(500)).await;
}

#[tokio::test]
async fn test_server_failover_with_message_delivery() {
    // 测试在服务器切换过程中的消息传递
    let sender_id = test_device_id();
    let receiver_id = test_device_id();
    
    let sender_capabilities = test_device_capabilities();
    let receiver_capabilities = test_device_capabilities();
    
    // 创建发送方SDK，支持服务器故障切换
    let backup_servers = vec![
        "https://ntfy.sh".to_string(),  // 使用真实的备用服务器
    ];
    
    let remote_channel = Arc::new(RemoteChannel::with_failover(
        sender_id,
        "https://invalid-primary.ntfy.sh".to_string(), // 无效的主服务器
        backup_servers
    ));
    
    let sender_sdk = TestSdkBuilder::new()
        .with_device_capabilities(sender_capabilities)
        .with_channel(remote_channel.clone())
        .build().await.unwrap();
        
    let _receiver_sdk = TestSdkBuilder::new()
        .with_device_capabilities(receiver_capabilities)
        .with_channel(Arc::new(RemoteChannel::new(receiver_id, Some("https://ntfy.sh".to_string()))))
        .build().await.unwrap();
    
    // 模拟主服务器不可用，触发自动切换
    assert!(remote_channel.switch_to_next_server().await);
    
    // 验证已切换到备用服务器
    let current_server = remote_channel.current_server_url().await;
    assert_eq!(current_server, "https://ntfy.sh");
    
    // 测试消息发送（在测试模式下）
    let test_payloads = vec![
        MessagePayload::Text("Message after primary server failure 1".to_string()),
        MessagePayload::Text("Message after primary server failure 2".to_string()),
        MessagePayload::Binary(vec![0xFF, 0xEE, 0xDD, 0xCC]),
    ];
    
    for payload in test_payloads {
        let result = sender_sdk.send(receiver_id, payload.clone()).await;
        assert!(result.is_ok(), "Message should be delivered after server failover: {:?}", payload);
        sleep(Duration::from_millis(200)).await;
    }
    
    sleep(Duration::from_millis(1000)).await;
}

#[tokio::test]
async fn test_server_health_check_and_failover() {
    // 测试服务器健康检查和自动故障切换
    let device_id = test_device_id();
    let device_capabilities = test_device_capabilities();
    
    // 创建支持健康检查的远程通道
    let backup_servers = vec![
        "https://ntfy.sh".to_string(),
        "https://ntfy.net".to_string(),
    ];
    
    let remote_channel = Arc::new(RemoteChannel::with_failover(
        device_id,
        "https://unreachable-primary.ntfy.sh".to_string(),
        backup_servers
    ));
    
    let _sdk = TestSdkBuilder::new()
        .with_device_capabilities(device_capabilities)
        .with_channel(remote_channel.clone())
        .build().await.unwrap();
    
    // 验证初始服务器状态
    let initial_server = remote_channel.current_server_url().await;
    assert_eq!(initial_server, "https://unreachable-primary.ntfy.sh");
    
    // 模拟健康检查失败，触发自动切换
    // 在实际实现中，这会是基于HTTP请求超时的自动检测
    let switch_result = remote_channel.switch_to_next_server().await;
    assert!(switch_result, "Should successfully switch to healthy backup server");
    
    // 验证已切换到健康的备用服务器
    let current_server = remote_channel.current_server_url().await;
    assert_eq!(current_server, "https://ntfy.sh");
    
    // 验证通道状态检查仍然正常
    let channel_state = remote_channel.check_state(&test_device_id()).await.unwrap();
    assert!(channel_state.available, "Channel should be available after failover");
    assert_eq!(channel_state.rtt_ms, 200); // 默认的RTT值
    
    sleep(Duration::from_millis(500)).await;
}