//! Integration and performance tests for group communication
//!
//! This module combines group management, secure communication (TreeKEM),
//! synchronization, and large-scale performance tests.

mod common;

use crate::common::{test_device_capabilities, test_device_id, NetworkSimulator, TestSdkBuilder};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use xlink::capability::manager::CapabilityManager;
use xlink::core::types::MessagePayload;
use xlink::group::manager::GroupManager;
use xlink::router::selector::Router;

// ==================== Group Management (Unit-like Integration) ====================

#[tokio::test]
async fn test_create_and_manage_group() {
    // UT-GRP-001/002: 创建和成员管理
    let creator_id = test_device_id();
    let channels = HashMap::new();
    let cap_manager = Arc::new(CapabilityManager::new(test_device_capabilities()));
    let router = Arc::new(Router::new(channels, cap_manager));
    let group_manager = GroupManager::new(creator_id, router);

    // Register keys for TreeKEM
    let creator_pk = x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::random_from_rng(
        rand::rngs::OsRng,
    ));
    group_manager
        .register_device_key(creator_id, creator_pk)
        .unwrap();

    let group = group_manager
        .create_group("Test Group".to_string(), vec![creator_id])
        .await
        .unwrap();

    assert_eq!(group.name, "Test Group");
    assert_eq!(group.members.len(), 1);

    // Member leaving
    group_manager.leave_group(group.id).await.unwrap();
    assert!(group_manager.get_group(group.id).await.is_none());
}

// ==================== Secure Group Communication (TreeKEM) ====================

#[tokio::test]
async fn test_treekem_group_security() {
    // IT-GRP-001: TreeKEM 组密钥协商与加密
    let sdk = TestSdkBuilder::new().build().await.unwrap();
    let device1_id = sdk.device_id();
    sdk.register_device_key(device1_id, sdk.public_key())
        .unwrap();

    let mut member_ids = Vec::new();
    for _ in 0..5 {
        let member_id = test_device_id();
        let member_pk = x25519_dalek::PublicKey::from(
            &x25519_dalek::StaticSecret::random_from_rng(rand::rngs::OsRng),
        );
        sdk.register_device_key(member_id, member_pk).unwrap();
        member_ids.push(member_id);
    }

    let mut all_members = member_ids.clone();
    all_members.push(device1_id);

    let group_id = sdk
        .create_group("Secure Group".to_string(), all_members)
        .await
        .unwrap();

    let payload = MessagePayload::Text("Secure message".to_string());
    let encrypted = sdk.encrypt_group_message(group_id, &payload).unwrap();

    if let MessagePayload::Binary(data) = encrypted {
        let decrypted = sdk
            .decrypt_group_message(group_id, &MessagePayload::Binary(data))
            .unwrap();
        assert_eq!(decrypted, payload);
    } else {
        panic!("Encryption failed to return binary payload");
    }

    // Key rotation
    assert!(sdk.rotate_group_key(group_id).await.is_ok());
}

// ==================== Multi-device Integration ====================

#[tokio::test]
async fn test_multi_device_group_broadcast() {
    // IT-GRP-001/002: 多设备群组广播
    let sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build()
        .await
        .unwrap();

    let mut device_ids = Vec::new();
    for _ in 0..10 {
        device_ids.push(test_device_id());
    }

    let group_id = sdk
        .create_group("Broadcast Group".to_string(), device_ids)
        .await
        .unwrap();
    let result = sdk
        .send_to_group(
            group_id,
            MessagePayload::Text("Hello everyone!".to_string()),
        )
        .await;
    assert!(result.is_ok());
}

// ==================== Large Scale Performance ====================

#[tokio::test]
async fn test_large_group_performance() {
    // PERF-GRP-004: 500人群组广播测试
    let sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build()
        .await
        .unwrap();

    let mut device_ids = Vec::new();
    for _ in 0..500 {
        device_ids.push(test_device_id());
    }

    let start_time = Instant::now();
    let group_id = sdk
        .create_group("Large Group".to_string(), device_ids)
        .await
        .unwrap();
    let creation_time = start_time.elapsed();

    let broadcast_start = Instant::now();
    let result = sdk
        .send_to_group(
            group_id,
            MessagePayload::Text("Hello 500 members!".to_string()),
        )
        .await;
    let broadcast_time = broadcast_start.elapsed();

    assert!(result.is_ok());
    println!(
        "500-person group: creation={:?}, broadcast={:?}",
        creation_time, broadcast_time
    );
}
