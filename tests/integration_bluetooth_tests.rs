mod common;

use std::sync::Arc;
use xpush::channels::bluetooth::BluetoothChannel;
use xpush::core::types::MessagePayload;
use crate::common::{test_device_id, TestSdkBuilder};

#[tokio::test]
async fn test_bluetooth_discovery_and_send() {
    // IT-BLE-001: 蓝牙发现与发送集成测试
    let device1_id = test_device_id();
    let device2_id = test_device_id();
    
    // 为 device1 创建 SDK，并添加蓝牙通道
    let ble_channel = Arc::new(BluetoothChannel::new(device1_id));
    let sdk = TestSdkBuilder::new()
        .with_device_capabilities(xpush::core::types::DeviceCapabilities {
            device_id: device1_id,
            device_type: xpush::core::types::DeviceType::Smartphone,
            device_name: "Device 1".to_string(),
            supported_channels: [xpush::core::types::ChannelType::BluetoothLE].into_iter().collect(),
            battery_level: Some(100),
            is_charging: true,
            data_cost_sensitive: false,
        })
        .with_channel(ble_channel.clone())
        .build().await.unwrap();
    
    // 模拟发现 device2
    ble_channel.discover_peer(device2_id, -60).await;
    
    // 发送消息
    let payload = MessagePayload::Text("Hello BLE".to_string());
    let result = sdk.send(device2_id, payload).await;
    
    assert!(result.is_ok(), "Failed to send message via Bluetooth: {:?}", result.err());
}
