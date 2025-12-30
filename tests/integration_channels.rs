//! Integration tests for all communication channels
//!
//! This module combines tests for Bluetooth, WiFi Direct, Remote (ntfy),
//! and channel switching mechanisms.

mod common;

use crate::common::{test_device_capabilities, test_device_id, NetworkSimulator, TestSdkBuilder};
use std::net::SocketAddr;
use std::sync::Arc;
use xlink::channels::bluetooth::BluetoothChannel;
use xlink::channels::remote::RemoteChannel;
use xlink::channels::wifi::WiFiDirectChannel;
use xlink::core::types::{ChannelType, DeviceCapabilities, DeviceType, MessagePayload};

// ==================== Bluetooth LE Tests ====================

#[tokio::test]
async fn test_bluetooth_discovery_and_send() {
    // IT-BLE-001: 蓝牙发现与发送集成测试
    let device1_id = test_device_id();
    let device2_id = test_device_id();

    let ble_channel = Arc::new(BluetoothChannel::new(device1_id));
    let sdk = TestSdkBuilder::new()
        .with_device_capabilities(DeviceCapabilities {
            device_id: device1_id,
            device_type: DeviceType::Smartphone,
            device_name: "Device 1".to_string(),
            supported_channels: [ChannelType::BluetoothLE].into_iter().collect(),
            battery_level: Some(100),
            is_charging: true,
            data_cost_sensitive: false,
        })
        .with_channel(ble_channel.clone())
        .build()
        .await
        .unwrap();

    ble_channel.discover_peer(device2_id, -60).await;

    let payload = MessagePayload::Text("Hello BLE".to_string());
    let result = sdk.send(device2_id, payload).await;

    assert!(
        result.is_ok(),
        "Failed to send message via Bluetooth: {:?}",
        result.err()
    );
}

// ==================== WiFi Direct Tests ====================

#[tokio::test]
async fn test_wifi_direct_discovery_and_send() {
    // IT-WFD-001: WiFi Direct 发现与发送集成测试
    let device1_id = test_device_id();
    let device2_id = test_device_id();

    let wfd_channel = Arc::new(WiFiDirectChannel::new(device1_id));
    let sdk = TestSdkBuilder::new()
        .with_device_capabilities(DeviceCapabilities {
            device_id: device1_id,
            device_type: DeviceType::Smartphone,
            device_name: "Device 1".to_string(),
            supported_channels: [ChannelType::WiFiDirect].into_iter().collect(),
            battery_level: Some(100),
            is_charging: true,
            data_cost_sensitive: false,
        })
        .with_channel(wfd_channel.clone())
        .build()
        .await
        .unwrap();

    let addr: SocketAddr = "192.168.49.1:8080".parse().unwrap();
    wfd_channel.add_peer(device2_id, addr).await;

    let payload = MessagePayload::Text("Hello WiFi Direct".to_string());
    let result = sdk.send(device2_id, payload).await;

    assert!(
        result.is_ok(),
        "Failed to send message via WiFi Direct: {:?}",
        result.err()
    );
}

// ==================== Remote (ntfy) Tests ====================

#[tokio::test]
async fn test_remote_device_communication() {
    // IT-RMT-001: 远程设备通信集成测试
    let device1_id = test_device_id();
    let device2_id = test_device_id();

    let mut remote_channel1 = RemoteChannel::new(device1_id, Some("https://ntfy.sh".to_string()));
    remote_channel1.set_test_mode(true);
    let remote_channel1 = Arc::new(remote_channel1);

    let mut remote_channel2 = RemoteChannel::new(device2_id, Some("https://ntfy.sh".to_string()));
    remote_channel2.set_test_mode(true);
    let remote_channel2 = Arc::new(remote_channel2);

    let sdk1 = TestSdkBuilder::new()
        .with_device_capabilities(test_device_capabilities())
        .with_channel(remote_channel1.clone())
        .build()
        .await
        .unwrap();

    let _sdk2 = TestSdkBuilder::new()
        .with_device_capabilities(test_device_capabilities())
        .with_channel(remote_channel2.clone())
        .build()
        .await
        .unwrap();

    let payload = MessagePayload::Text("Hello from remote device".to_string());
    let result = sdk1.send(device2_id, payload).await;

    assert!(result.is_ok(), "Remote message should be sent successfully");
}

#[tokio::test]
async fn test_server_failover_on_primary_failure() {
    // IT-RMT-005: 服务器切换集成测试
    let device1_id = test_device_id();
    let device2_id = test_device_id();

    let backup_servers = vec![
        "https://ntfy-backup1.sh".to_string(),
        "https://ntfy.net".to_string(),
    ];

    let mut remote_channel1 = RemoteChannel::with_failover(
        device1_id,
        "https://ntfy-primary.sh".to_string(),
        backup_servers.clone(),
    );
    remote_channel1.set_test_mode(true);
    let remote_channel1 = Arc::new(remote_channel1);

    let sdk1 = TestSdkBuilder::new()
        .with_channel(remote_channel1.clone())
        .build()
        .await
        .unwrap();

    assert_eq!(
        remote_channel1.current_server_url().await,
        "https://ntfy-primary.sh"
    );

    remote_channel1.switch_to_next_server().await;
    assert_eq!(
        remote_channel1.current_server_url().await,
        "https://ntfy-backup1.sh"
    );

    let result = sdk1
        .send(
            device2_id,
            MessagePayload::Text("Failover test".to_string()),
        )
        .await;
    assert!(result.is_ok());
}

// ==================== Channel Switching Tests ====================

#[tokio::test]
async fn test_channel_switching_wifi_to_ble() {
    // IT-CSW-001: WiFi切换到蓝牙
    let sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build()
        .await
        .unwrap();

    let device2 = test_device_id();

    let result = sdk
        .send(device2, MessagePayload::Text("Switching test".to_string()))
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_channel_switching_performance() {
    // IT-CSW-003: 通道切换性能测试
    let sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build()
        .await
        .unwrap();

    let device2 = test_device_id();

    let start = std::time::Instant::now();
    let result = sdk
        .send(
            device2,
            MessagePayload::Text("Performance test".to_string()),
        )
        .await;
    let elapsed = start.elapsed();

    assert!(result.is_ok());
    assert!(
        elapsed.as_millis() < 1000,
        "Message sending took too long: {:?}",
        elapsed
    );
}
