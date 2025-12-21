mod common;

use std::sync::Arc;
use std::net::SocketAddr;
use xpush::channels::wifi::WiFiDirectChannel;
use xpush::core::types::MessagePayload;
use crate::common::{test_device_id, TestSdkBuilder};

#[tokio::test]
async fn test_wifi_direct_discovery_and_send() {
    // IT-WFD-001: WiFi Direct 发现与发送集成测试
    let device1_id = test_device_id();
    let device2_id = test_device_id();
    
    // 为 device1 创建 SDK，并添加 WiFi Direct 通道
    let wfd_channel = Arc::new(WiFiDirectChannel::new(device1_id));
    let sdk = TestSdkBuilder::new()
        .with_device_capabilities(xpush::core::types::DeviceCapabilities {
            device_id: device1_id,
            device_type: xpush::core::types::DeviceType::Smartphone,
            device_name: "Device 1".to_string(),
            supported_channels: [xpush::core::types::ChannelType::WiFiDirect].into_iter().collect(),
            battery_level: Some(100),
            is_charging: true,
            data_cost_sensitive: false,
        })
        .with_channel(wfd_channel.clone())
        .build().await.unwrap();
    
    // 模拟发现 device2
    let addr: SocketAddr = "192.168.49.1:8080".parse().unwrap();
    wfd_channel.add_peer(device2_id, addr).await;
    
    // 发送消息
    let payload = MessagePayload::Text("Hello WiFi Direct".to_string());
    let result = sdk.send(device2_id, payload).await;
    
    assert!(result.is_ok(), "Failed to send message via WiFi Direct: {:?}", result.err());
}
