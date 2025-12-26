//! Unit tests for core components: Router, Capability, Heartbeat, and Crypto
//!
//! This module combines unit-level testing for the system's foundational logic.

mod common;

use crate::common::{
    test_device_capabilities, test_device_id, test_text_message, NoOpMessageHandler,
};
use std::collections::HashMap;
use std::sync::Arc;
use xpush::capability::manager::CapabilityManager;
use xpush::core::types::{ChannelType, DeviceCapabilities, DeviceType, MessagePayload};
use xpush::heartbeat::manager::HeartbeatManager;
use xpush::router::scoring::Scorer;
use xpush::router::selector::Router;

// ==================== Router & Scoring Tests ====================

#[tokio::test]
async fn test_router_scoring_logic() {
    // UT-ROU-001/002: 通道评分与选择
    let caps = test_device_capabilities();
    let message = test_text_message("test");

    // Simulate channel state
    let state = xpush::core::types::ChannelState {
        available: true,
        rtt_ms: 50,
        jitter_ms: 5,
        packet_loss_rate: 0.01,
        bandwidth_bps: 1000000,
        signal_strength: Some(-50),
        distance_meters: Some(5.0),
        network_type: xpush::core::types::NetworkType::WiFi,
        failure_count: 0,
        last_heartbeat: 0,
    };

    let score = Scorer::score(ChannelType::BluetoothLE, &state, &caps, message.priority);
    assert!(score > 0.0);
}

#[tokio::test]
async fn test_router_cost_sensitive() {
    // UT-ROU-005: 成本感知路由
    let mut caps = test_device_capabilities();
    caps.data_cost_sensitive = true;

    let cap_manager = Arc::new(CapabilityManager::new(caps));

    // Setup channel state for the target device
    let target_device = test_device_id();
    let state = xpush::core::types::ChannelState {
        available: true,
        rtt_ms: 50,
        jitter_ms: 5,
        packet_loss_rate: 0.01,
        bandwidth_bps: 1000000,
        signal_strength: Some(-50),
        distance_meters: Some(5.0),
        network_type: xpush::core::types::NetworkType::WiFi,
        failure_count: 0,
        last_heartbeat: 0,
    };
    cap_manager.update_channel_state(target_device, ChannelType::BluetoothLE, state);

    let mut channels: HashMap<ChannelType, Arc<dyn xpush::core::traits::Channel>> = HashMap::new();

    let ble_channel = Arc::new(
        xpush::channels::memory::MemoryChannel::new(Arc::new(NoOpMessageHandler), 10)
            .with_type(ChannelType::BluetoothLE),
    );
    channels.insert(ChannelType::BluetoothLE, ble_channel.clone());

    let router = Router::new(channels, cap_manager);
    let mut msg = test_text_message("test cost");
    msg.recipient = target_device;

    let selected = router.select_channel(&msg).await.unwrap();
    assert_eq!(selected.channel_type(), ChannelType::BluetoothLE);
}

// ==================== Capability Manager Tests ====================

#[tokio::test]
async fn test_capability_detection() {
    // UT-CAP-001/004: 能力与电池检测
    let caps = DeviceCapabilities {
        device_id: test_device_id(),
        device_type: DeviceType::Smartphone,
        device_name: "Test".to_string(),
        supported_channels: [ChannelType::BluetoothLE].into_iter().collect(),
        battery_level: Some(75),
        is_charging: true,
        data_cost_sensitive: false,
    };

    let manager = CapabilityManager::new(caps);
    let detected = manager.get_local_caps();

    assert!(detected
        .supported_channels
        .contains(&ChannelType::BluetoothLE));
    assert_eq!(detected.battery_level, Some(75));
}

// ==================== Heartbeat Manager Tests ====================

#[tokio::test]
async fn test_heartbeat_ping_pong() {
    // UT-HBT-002: 心跳 Ping-Pong 机制
    let d1 = test_device_id();
    let d2 = test_device_id();

    let cap_manager = Arc::new(CapabilityManager::new(test_device_capabilities()));
    let router = Arc::new(Router::new(HashMap::new(), cap_manager.clone()));
    let heartbeat_manager = HeartbeatManager::new(d1, router, cap_manager);

    let ping = xpush::core::types::Message {
        id: uuid::Uuid::new_v4(),
        sender: d2,
        recipient: d1,
        group_id: None,
        payload: MessagePayload::Ping(12345),
        timestamp: 12345,
        priority: xpush::core::types::MessagePriority::Normal,
        require_ack: false,
    };

    heartbeat_manager.handle_heartbeat(&ping).await;
    // Success means no panic during handling
}
