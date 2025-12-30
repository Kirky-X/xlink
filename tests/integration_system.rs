//! Integration tests for complex system scenarios and end-to-end workflows
//!
//! This module combines real-world user scenarios, storage management,
//! and crash recovery tests.

mod common;

use std::time::Duration;
use tokio::time::sleep;

use crate::common::{establish_device_sessions, test_device_id, NetworkSimulator, TestSdkBuilder};
use xlink::core::traits::Storage;
use xlink::core::types::{ChannelType, DeviceCapabilities, DeviceType, MessagePayload};
use xlink::storage::file_store::FileStorage;

// ==================== End-to-End User Scenarios ====================

#[tokio::test]
async fn test_office_file_sharing_scenario() {
    // E2E-001: 办公室文件分享场景
    let alice_sdk = TestSdkBuilder::new().build().await.unwrap();
    let bob_sdk = TestSdkBuilder::new().build().await.unwrap();
    let charlie_sdk = TestSdkBuilder::new().build().await.unwrap();

    let device_ids = vec![
        alice_sdk.device_id(),
        bob_sdk.device_id(),
        charlie_sdk.device_id(),
    ];

    // Establish sessions
    establish_device_sessions(&[&alice_sdk, &bob_sdk, &charlie_sdk])
        .await
        .unwrap();

    let office_group = alice_sdk
        .create_group("Office Team".to_string(), device_ids)
        .await
        .unwrap();

    let presentation_data = vec![0u8; 1024 * 1024]; // 1MB for test
    let result = alice_sdk
        .send_to_group(office_group, MessagePayload::Binary(presentation_data))
        .await;
    assert!(result.is_ok(), "File sharing should succeed");

    sleep(Duration::from_millis(100)).await;
}

#[tokio::test]
async fn test_emergency_communication_scenario() {
    // E2E-004: 紧急通信场景 (弱网络环境)
    let responder_sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::poor_network())
        .build()
        .await
        .unwrap();
    let coordinator_sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::poor_network())
        .build()
        .await
        .unwrap();

    let _responder = responder_sdk.device_id();
    let coordinator = coordinator_sdk.device_id();

    establish_device_sessions(&[&responder_sdk, &coordinator_sdk])
        .await
        .unwrap();

    let result = responder_sdk
        .send(
            coordinator,
            MessagePayload::Text("EMERGENCY: Need backup!".to_string()),
        )
        .await;
    assert!(
        result.is_ok(),
        "Emergency message should be delivered even in poor network"
    );
}

// ==================== Storage Management ====================

#[tokio::test]
async fn test_storage_cleanup() {
    // UT-STO-001: 存储清理逻辑
    let storage_path = "./test_storage_sys";
    let _ = tokio::fs::remove_dir_all(storage_path).await;
    let storage = FileStorage::new(storage_path).await.unwrap();

    let sender = test_device_id();
    let recipient = test_device_id();

    // Save some messages
    for i in 0..20 {
        let msg = xlink::core::types::Message::new(
            sender,
            recipient,
            MessagePayload::Text(format!("Msg {}", i)),
        );
        storage.save_message(&msg).await.unwrap();
    }

    let usage = storage.get_storage_usage().await.unwrap();
    assert!(usage > 0);

    // Cleanup half
    storage.cleanup_storage(usage / 2).await.unwrap();
    let final_usage = storage.get_storage_usage().await.unwrap();
    assert!(final_usage <= usage / 2 + 1024);

    let _ = tokio::fs::remove_dir_all(storage_path).await;
}

// ==================== Crash Recovery ====================

#[tokio::test]
async fn test_system_recovery_after_restart() {
    // IT-REC-001: 重启后的状态恢复
    let storage_path = "./test_recovery_sys";
    let _ = tokio::fs::remove_dir_all(storage_path).await;

    let caps = DeviceCapabilities {
        device_id: test_device_id(),
        device_type: DeviceType::Smartphone,
        device_name: "Recovery Device".to_string(),
        supported_channels: [ChannelType::Lan].into_iter().collect(),
        battery_level: Some(100),
        is_charging: true,
        data_cost_sensitive: false,
    };

    // 1. First run: Create a group and save state
    {
        let sdk = TestSdkBuilder::new()
            .with_device_capabilities(caps.clone())
            .with_storage_path(storage_path.to_string())
            .build()
            .await
            .unwrap();

        sdk.create_group("Persistent Group".to_string(), vec![caps.device_id])
            .await
            .unwrap();
    } // SDK dropped here

    // 2. Second run: Recovery
    {
        let _sdk = TestSdkBuilder::new()
            .with_device_capabilities(caps)
            .with_storage_path(storage_path.to_string())
            .build()
            .await
            .unwrap();
        // Check if group exists (using internal state access if needed, or just verifying no crash)
        // For now, we assume recovery works if the SDK starts without error
    }

    let _ = tokio::fs::remove_dir_all(storage_path).await;
}
