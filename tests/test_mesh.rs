mod common;

use crate::common::test_device_id;
use xpush::channels::mesh::BluetoothMeshChannel;
use xpush::core::traits::Channel;
use xpush::core::types::{ChannelType, Message, MessagePayload, NetworkType};

#[tokio::test]
async fn test_mesh_channel_creation() {
    let device_id = test_device_id();
    let channel = BluetoothMeshChannel::new(device_id);

    assert_eq!(channel.channel_type(), ChannelType::BluetoothMesh);
}

#[tokio::test]
async fn test_mesh_add_neighbor() {
    let device_id = test_device_id();
    let channel = BluetoothMeshChannel::new(device_id);

    let neighbor_id = test_device_id();
    channel.add_neighbor(neighbor_id, 2).await;

    let state = channel.check_state(&neighbor_id).await.unwrap();
    assert!(state.available);
    assert_eq!(state.rtt_ms, 100);
}

#[tokio::test]
async fn test_mesh_send_to_neighbor() {
    let device_id = test_device_id();
    let channel = BluetoothMeshChannel::new(device_id);

    let neighbor_id = test_device_id();
    channel.add_neighbor(neighbor_id, 1).await;

    let message = Message::new(
        device_id,
        neighbor_id,
        MessagePayload::Text("Hello Mesh".to_string()),
    );

    let result = channel.send(message).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mesh_send_to_non_neighbor() {
    let device_id = test_device_id();
    let channel = BluetoothMeshChannel::new(device_id);

    let unknown_id = test_device_id();
    let message = Message::new(
        device_id,
        unknown_id,
        MessagePayload::Text("Test".to_string()),
    );

    let result = channel.send(message).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_mesh_check_state_unknown_device() {
    let device_id = test_device_id();
    let channel = BluetoothMeshChannel::new(device_id);

    let unknown_id = test_device_id();
    let state = channel.check_state(&unknown_id).await.unwrap();

    assert!(!state.available);
    assert_eq!(state.rtt_ms, 9999);
    assert_eq!(state.network_type, NetworkType::Unknown);
}

#[tokio::test]
async fn test_mesh_latency_with_hops() {
    let device_id = test_device_id();
    let channel = BluetoothMeshChannel::new(device_id);

    let neighbor_id = test_device_id();

    for hops in [1u8, 2, 3, 5] {
        channel.add_neighbor(neighbor_id, hops).await;
        let state = channel.check_state(&neighbor_id).await.unwrap();

        let expected_latency = (50 * hops as u32).max(50);
        assert_eq!(state.rtt_ms, expected_latency);
    }
}

#[tokio::test]
async fn test_mesh_high_packet_loss() {
    let device_id = test_device_id();
    let channel = BluetoothMeshChannel::new(device_id);

    let neighbor_id = test_device_id();
    channel.add_neighbor(neighbor_id, 1).await;

    let state = channel.check_state(&neighbor_id).await.unwrap();
    assert!(state.packet_loss_rate > 0.05);
    assert!(state.bandwidth_bps < 1_000_000);
}

#[tokio::test]
async fn test_mesh_start() {
    let device_id = test_device_id();
    let channel = BluetoothMeshChannel::new(device_id);

    let result = channel.start().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mesh_multiple_neighbors() {
    let device_id = test_device_id();
    let channel = BluetoothMeshChannel::new(device_id);

    let peer1 = test_device_id();
    let peer2 = test_device_id();
    let peer3 = test_device_id();

    channel.add_neighbor(peer1, 1).await;
    channel.add_neighbor(peer2, 2).await;
    channel.add_neighbor(peer3, 3).await;

    let state1 = channel.check_state(&peer1).await.unwrap();
    let state2 = channel.check_state(&peer2).await.unwrap();
    let state3 = channel.check_state(&peer3).await.unwrap();

    assert!(state1.available);
    assert!(state2.available);
    assert!(state3.available);

    assert_eq!(state1.rtt_ms, 50);
    assert_eq!(state2.rtt_ms, 100);
    assert_eq!(state3.rtt_ms, 150);
}

#[tokio::test]
async fn test_mesh_send_multiple_messages() {
    let device_id = test_device_id();
    let channel = BluetoothMeshChannel::new(device_id);

    let neighbor_id = test_device_id();
    channel.add_neighbor(neighbor_id, 1).await;

    for i in 0..10 {
        let message = Message::new(
            device_id,
            neighbor_id,
            MessagePayload::Text(format!("Mesh message {}", i)),
        );

        let result = channel.send(message).await;
        assert!(result.is_ok(), "Message {} should succeed", i);
    }
}

#[tokio::test]
async fn test_mesh_state_properties() {
    let device_id = test_device_id();
    let channel = BluetoothMeshChannel::new(device_id);

    let neighbor_id = test_device_id();
    channel.add_neighbor(neighbor_id, 2).await;

    let state = channel.check_state(&neighbor_id).await.unwrap();

    assert_eq!(state.jitter_ms, 20);
    assert_eq!(state.bandwidth_bps, 100_000);
    assert_eq!(state.signal_strength, None);
    assert_eq!(state.distance_meters, None);
    assert_eq!(state.network_type, NetworkType::Bluetooth);
}

#[tokio::test]
async fn test_mesh_unknown_device_state() {
    let device_id = test_device_id();
    let channel = BluetoothMeshChannel::new(device_id);

    let unknown_id = test_device_id();
    let state = channel.check_state(&unknown_id).await.unwrap();

    assert!(!state.available);
    assert_eq!(state.rtt_ms, 9999);
    assert_eq!(state.jitter_ms, 0);
    assert_eq!(state.packet_loss_rate, 1.0);
    assert_eq!(state.bandwidth_bps, 0);
    assert_eq!(state.failure_count, 0);
}

#[tokio::test]
async fn test_mesh_broadcast_message() {
    let device_id = test_device_id();
    let channel = BluetoothMeshChannel::new(device_id);

    let neighbors = vec![test_device_id(), test_device_id(), test_device_id()];

    for neighbor in &neighbors {
        channel.add_neighbor(*neighbor, 1).await;
    }

    for neighbor in neighbors {
        let message = Message::new(
            device_id,
            neighbor,
            MessagePayload::Text("Broadcast".to_string()),
        );

        let result = channel.send(message).await;
        assert!(result.is_ok());
    }
}
