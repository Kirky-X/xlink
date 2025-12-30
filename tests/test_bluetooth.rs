mod common;

use crate::common::test_device_id;
use xlink::channels::bluetooth::BluetoothChannel;
use xlink::core::traits::Channel;
use xlink::core::types::{ChannelType, Message, MessagePayload, NetworkType};

#[tokio::test]
async fn test_bluetooth_channel_creation() {
    let device_id = test_device_id();
    let channel = BluetoothChannel::new(device_id);

    assert_eq!(channel.channel_type(), ChannelType::BluetoothLE);
}

#[tokio::test]
async fn test_bluetooth_discover_peer() {
    let device_id = test_device_id();
    let channel = BluetoothChannel::new(device_id);

    let peer_id = test_device_id();
    channel.discover_peer(peer_id, -50).await;

    let state = channel.check_state(&peer_id).await.unwrap();
    assert!(state.available);
    assert_eq!(state.signal_strength, Some(-50));
}

#[tokio::test]
async fn test_bluetooth_send_to_connected_peer() {
    let device_id = test_device_id();
    let channel = BluetoothChannel::new(device_id);

    let peer_id = test_device_id();
    channel.discover_peer(peer_id, -60).await;

    let message = Message::new(
        device_id,
        peer_id,
        MessagePayload::Text("Hello Bluetooth".to_string()),
    );

    let result = channel.send(message).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_bluetooth_send_to_unconnected_peer() {
    let device_id = test_device_id();
    let channel = BluetoothChannel::new(device_id);

    let peer_id = test_device_id();
    let message = Message::new(device_id, peer_id, MessagePayload::Text("Test".to_string()));

    let result = channel.send(message).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_bluetooth_check_state_unknown_peer() {
    let device_id = test_device_id();
    let channel = BluetoothChannel::new(device_id);

    let peer_id = test_device_id();
    let state = channel.check_state(&peer_id).await.unwrap();

    assert!(!state.available);
    assert_eq!(state.network_type, NetworkType::Unknown);
}

#[tokio::test]
async fn test_bluetooth_distance_estimation() {
    let device_id = test_device_id();
    let channel = BluetoothChannel::new(device_id);

    let peer_id = test_device_id();

    channel.discover_peer(peer_id, -30).await;
    let state = channel.check_state(&peer_id).await.unwrap();
    assert!(state.distance_meters.unwrap() < 1.0);

    channel.discover_peer(peer_id, -70).await;
    let state = channel.check_state(&peer_id).await.unwrap();
    assert!(state.distance_meters.unwrap() > 0.5);
}

#[tokio::test]
async fn test_bluetooth_state_with_rssi() {
    let device_id = test_device_id();
    let channel = BluetoothChannel::new(device_id);

    let peer_id = test_device_id();

    for rssi in [-30i8, -50, -70, -90] {
        channel.discover_peer(peer_id, rssi).await;
        let state = channel.check_state(&peer_id).await.unwrap();

        assert_eq!(state.signal_strength, Some(rssi));
        assert!(state.rtt_ms > 0);
        assert!(state.packet_loss_rate > 0.0);
    }
}

#[tokio::test]
async fn test_bluetooth_start() {
    let device_id = test_device_id();
    let channel = BluetoothChannel::new(device_id);

    let result = channel.start().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_bluetooth_multiple_peers() {
    let device_id = test_device_id();
    let channel = BluetoothChannel::new(device_id);

    let peer1 = test_device_id();
    let peer2 = test_device_id();
    let peer3 = test_device_id();

    channel.discover_peer(peer1, -40).await;
    channel.discover_peer(peer2, -60).await;
    channel.discover_peer(peer3, -80).await;

    let state1 = channel.check_state(&peer1).await.unwrap();
    let state2 = channel.check_state(&peer2).await.unwrap();
    let state3 = channel.check_state(&peer3).await.unwrap();

    assert!(state1.available);
    assert!(state2.available);
    assert!(state3.available);

    assert_eq!(state1.signal_strength, Some(-40));
    assert_eq!(state2.signal_strength, Some(-60));
    assert_eq!(state3.signal_strength, Some(-80));
}

#[tokio::test]
async fn test_bluetooth_send_multiple_messages() {
    let device_id = test_device_id();
    let channel = BluetoothChannel::new(device_id);

    let peer_id = test_device_id();
    channel.discover_peer(peer_id, -50).await;

    for i in 0..10 {
        let message = Message::new(
            device_id,
            peer_id,
            MessagePayload::Text(format!("Message {}", i)),
        );

        let result = channel.send(message).await;
        assert!(result.is_ok(), "Message {} should succeed", i);
    }
}
