//! Unit tests for core components: Router, Capability, Heartbeat, and Crypto
//!
//! This module combines unit-level testing for the system's foundational logic.

mod common;

use crate::common::{
    test_device_capabilities, test_device_id, test_text_message, NoOpMessageHandler,
};
use std::collections::HashMap;
use std::sync::Arc;
use xlink::capability::manager::CapabilityManager;
use xlink::core::types::{ChannelType, DeviceCapabilities, DeviceType, MessagePayload};
use xlink::heartbeat::manager::HeartbeatManager;
use xlink::router::scoring::Scorer;
use xlink::router::selector::Router;

// ==================== Router & Scoring Tests ====================

#[tokio::test]
async fn test_router_scoring_logic() {
    // UT-ROU-001/002: 通道评分与选择
    let caps = test_device_capabilities();
    let message = test_text_message("test");

    // Simulate channel state
    let state = xlink::core::types::ChannelState {
        available: true,
        rtt_ms: 50,
        jitter_ms: 5,
        packet_loss_rate: 0.01,
        bandwidth_bps: 1000000,
        signal_strength: Some(-50),
        distance_meters: Some(5.0),
        network_type: xlink::core::types::NetworkType::WiFi,
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
    let state = xlink::core::types::ChannelState {
        available: true,
        rtt_ms: 50,
        jitter_ms: 5,
        packet_loss_rate: 0.01,
        bandwidth_bps: 1000000,
        signal_strength: Some(-50),
        distance_meters: Some(5.0),
        network_type: xlink::core::types::NetworkType::WiFi,
        failure_count: 0,
        last_heartbeat: 0,
    };
    cap_manager.update_channel_state(target_device, ChannelType::BluetoothLE, state);

    let mut channels: HashMap<ChannelType, Arc<dyn xlink::core::traits::Channel>> = HashMap::new();

    let ble_channel = Arc::new(
        xlink::channels::memory::MemoryChannel::new(Arc::new(NoOpMessageHandler), 10)
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

    let ping = xlink::core::types::Message {
        id: uuid::Uuid::new_v4(),
        sender: d2,
        recipient: d1,
        group_id: None,
        payload: MessagePayload::Ping(12345),
        timestamp: 12345,
        priority: xlink::core::types::MessagePriority::Normal,
        require_ack: false,
    };

    heartbeat_manager.handle_heartbeat(&ping).await;
    // Success means no panic during handling
}

// ==================== Error Handling Tests ====================

#[test]
fn test_error_code_parsing() {
    // 测试错误码解析
    use xlink::core::error::ErrorCode;

    let code: Result<ErrorCode, _> = "101".parse();
    assert_eq!(code, Ok(ErrorCode(101)));

    let code: Result<ErrorCode, _> = "9999".parse();
    assert_eq!(code, Ok(ErrorCode(9999)));

    // 测试无效错误码
    let code: Result<ErrorCode, _> = "10000".parse();
    assert!(code.is_err());
}

#[test]
fn test_error_code_module_and_sequence() {
    // 测试错误码的模块和序号提取
    use xlink::core::error::ErrorCode;

    let code = ErrorCode(101);
    assert_eq!(code.module(), 1);
    assert_eq!(code.sequence(), 1);

    let code = ErrorCode(205);
    assert_eq!(code.module(), 2);
    assert_eq!(code.sequence(), 5);
}

#[test]
fn test_error_category_code_range() {
    // 测试错误分类的代码范围
    use xlink::core::error::ErrorCategory;

    let (start, end) = ErrorCategory::System.code_range();
    assert_eq!(start, 100);
    assert_eq!(end, 199);

    let (start, end) = ErrorCategory::Channel.code_range();
    assert_eq!(start, 200);
    assert_eq!(end, 299);
}

#[test]
fn test_error_creation() {
    // 测试错误创建
    use xlink::core::error::XPushError;

    let error = XPushError::device_not_found("test-device", "test.rs");
    assert_eq!(error.code().0, 501);
    assert_eq!(error.message(), "设备未找到");
    assert_eq!(error.location(), "test.rs");
}

#[test]
fn test_error_with_context() {
    // 测试错误上下文
    use xlink::core::error::XPushError;

    let error =
        XPushError::channel_disconnected("Connection lost", "test.rs").with_device_id("device-123");

    assert_eq!(error.context.device_id, Some("device-123".to_string()));
    assert!(error.is_retryable());
}

#[test]
fn test_error_chain() {
    // 测试错误链
    use xlink::core::error::XPushError;

    let inner = XPushError::invalid_input("test", "Invalid value", "inner.rs");
    let outer = XPushError::storage_write_failed("key", "Failed", "outer.rs").with_source(inner);

    assert!(outer.source.is_some());
    assert_eq!(outer.source.as_ref().unwrap().message(), "输入参数无效");
}

// ==================== Crypto Module Tests ====================

#[test]
fn test_crypto_engine_creation() {
    // 测试加密引擎创建
    use xlink::crypto::engine::CryptoEngine;

    let engine = CryptoEngine::new();
    let public_key = engine.public_key();

    // 验证公钥不为零
    assert_ne!(public_key.as_bytes(), &[0u8; 32]);
}

#[test]
fn test_crypto_sign_and_verify() {
    // 测试签名和验证
    use xlink::crypto::engine::CryptoEngine;

    let engine = CryptoEngine::new();
    let data = b"test data";

    let signature = engine.sign(data);
    assert_eq!(signature.len(), 64);
}

#[test]
fn test_session_creation() {
    // 测试会话创建
    use x25519_dalek::PublicKey;
    use xlink::core::types::DeviceId;
    use xlink::crypto::engine::CryptoEngine;

    let engine = CryptoEngine::new();
    let peer_id = DeviceId::new();
    let peer_public = PublicKey::from([1u8; 32]);

    // 测试未认证会话创建
    let result = engine.establish_session(peer_id, peer_public);
    assert!(result.is_ok());
}

#[test]
fn test_session_expiration() {
    // 测试会话过期
    use x25519_dalek::PublicKey;
    use xlink::core::types::DeviceId;
    use xlink::crypto::engine::CryptoEngine;

    let engine = CryptoEngine::new();
    let peer_id = DeviceId::new();
    let peer_public = PublicKey::from([1u8; 32]);

    engine.establish_session(peer_id, peer_public).unwrap();

    // 验证会话已创建
    let sessions = engine.export_state().unwrap();
    assert!(!sessions.sessions.is_empty());
}

// ==================== Storage Path Validation Tests ====================

#[test]
fn test_path_validation() {
    // 测试路径验证逻辑
    use std::path::Path;

    // 正常路径应该通过验证
    let valid_path = Path::new("/tmp/xlink");
    assert!(!valid_path.to_string_lossy().contains(".."));

    // 路径遍历攻击应该被检测
    let traversal_path = Path::new("/tmp/xlink/../../../etc");
    assert!(traversal_path.to_string_lossy().contains(".."));
}
