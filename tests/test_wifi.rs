mod common;

use crate::common::test_device_id;
use std::net::SocketAddr;
use xpush::channels::wifi::WiFiDirectChannel;
use xpush::core::traits::Channel;
use xpush::core::types::{ChannelType, Message, MessagePayload, NetworkType};

#[tokio::test]
async fn test_wifi_direct_channel_creation() {
    let device_id = test_device_id();
    let channel = WiFiDirectChannel::new(device_id);

    assert_eq!(channel.channel_type(), ChannelType::WiFiDirect);
}

#[tokio::test]
async fn test_wifi_direct_add_peer() {
    let device_id = test_device_id();
    let channel = WiFiDirectChannel::new(device_id);

    let peer_id = test_device_id();
    let peer_addr: SocketAddr = "192.168.49.1:8888".parse().unwrap();

    channel.add_peer(peer_id, peer_addr).await;

    let state = channel.check_state(&peer_id).await.unwrap();
    assert!(state.available);
    assert_eq!(state.rtt_ms, 20);
    assert_eq!(state.bandwidth_bps, 50_000_000);
}

#[tokio::test]
async fn test_wifi_direct_send() {
    let device_id = test_device_id();
    let channel = WiFiDirectChannel::new(device_id);

    let peer_id = test_device_id();
    let peer_addr: SocketAddr = "192.168.49.1:8888".parse().unwrap();

    channel.add_peer(peer_id, peer_addr).await;

    let message = Message::new(
        device_id,
        peer_id,
        MessagePayload::Text("Hello WiFi Direct".to_string()),
    );

    let result = channel.send(message).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_wifi_direct_send_to_unknown_peer() {
    let device_id = test_device_id();
    let channel = WiFiDirectChannel::new(device_id);

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
async fn test_wifi_direct_check_state_unknown_peer() {
    let device_id = test_device_id();
    let channel = WiFiDirectChannel::new(device_id);

    let unknown_id = test_device_id();
    let state = channel.check_state(&unknown_id).await.unwrap();

    assert!(!state.available);
    assert_eq!(state.rtt_ms, 9999); // 默认值
    assert_eq!(state.network_type, NetworkType::Unknown); // 默认值
}

#[tokio::test]
async fn test_wifi_direct_state_properties() {
    let device_id = test_device_id();
    let channel = WiFiDirectChannel::new(device_id);

    let peer_id = test_device_id();
    let peer_addr: SocketAddr = "192.168.49.1:8888".parse().unwrap();

    channel.add_peer(peer_id, peer_addr).await;

    let state = channel.check_state(&peer_id).await.unwrap();

    assert_eq!(state.jitter_ms, 5);
    assert_eq!(state.packet_loss_rate, 0.02);
    assert_eq!(state.signal_strength, Some(-60));
    assert_eq!(state.distance_meters, Some(15.0));
    assert_eq!(state.network_type, NetworkType::WiFi);
}

#[tokio::test]
async fn test_wifi_direct_start() {
    let device_id = test_device_id();
    let channel = WiFiDirectChannel::new(device_id);

    let result = channel.start().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_wifi_direct_multiple_peers() {
    let device_id = test_device_id();
    let channel = WiFiDirectChannel::new(device_id);

    let peer1 = test_device_id();
    let peer2 = test_device_id();
    let peer3 = test_device_id();

    channel
        .add_peer(peer1, "192.168.49.1:8888".parse().unwrap())
        .await;
    channel
        .add_peer(peer2, "192.168.49.2:8888".parse().unwrap())
        .await;
    channel
        .add_peer(peer3, "192.168.49.3:8888".parse().unwrap())
        .await;

    let state1 = channel.check_state(&peer1).await.unwrap();
    let state2 = channel.check_state(&peer2).await.unwrap();
    let state3 = channel.check_state(&peer3).await.unwrap();

    assert!(state1.available);
    assert!(state2.available);
    assert!(state3.available);

    assert_eq!(state1.rtt_ms, 20);
    assert_eq!(state2.rtt_ms, 20);
    assert_eq!(state3.rtt_ms, 20);

    assert_eq!(state1.bandwidth_bps, 50_000_000);
    assert_eq!(state2.bandwidth_bps, 50_000_000);
    assert_eq!(state3.bandwidth_bps, 50_000_000);
}

#[tokio::test]
async fn test_wifi_direct_send_multiple_messages() {
    let device_id = test_device_id();
    let channel = WiFiDirectChannel::new(device_id);

    let peer_id = test_device_id();
    let peer_addr: SocketAddr = "192.168.49.1:8888".parse().unwrap();

    channel.add_peer(peer_id, peer_addr).await;

    for i in 0..10 {
        let message = Message::new(
            device_id,
            peer_id,
            MessagePayload::Text(format!("WiFi Direct message {}", i)),
        );

        let result = channel.send(message).await;
        assert!(result.is_ok(), "Message {} should succeed", i);
    }
}

#[tokio::test]
async fn test_wifi_direct_binary_message() {
    let device_id = test_device_id();
    let channel = WiFiDirectChannel::new(device_id);

    let peer_id = test_device_id();
    let peer_addr: SocketAddr = "192.168.49.1:8888".parse().unwrap();

    channel.add_peer(peer_id, peer_addr).await;

    let binary_data = vec![0u8; 2048];
    let message = Message::new(device_id, peer_id, MessagePayload::Binary(binary_data));

    let result = channel.send(message).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_wifi_direct_bandwidth() {
    let device_id = test_device_id();
    let channel = WiFiDirectChannel::new(device_id);

    let peer_id = test_device_id();
    let peer_addr: SocketAddr = "192.168.49.1:8888".parse().unwrap();

    channel.add_peer(peer_id, peer_addr).await;

    let state = channel.check_state(&peer_id).await.unwrap();

    assert_eq!(state.bandwidth_bps, 50_000_000);
    assert!(state.bandwidth_bps > 10_000_000);
}

#[tokio::test]
async fn test_wifi_direct_failure_count() {
    let device_id = test_device_id();
    let channel = WiFiDirectChannel::new(device_id);

    let unknown_id = test_device_id();
    let state = channel.check_state(&unknown_id).await.unwrap();

    assert_eq!(state.failure_count, 0);
    assert_eq!(state.last_heartbeat, 0);
}

#[tokio::test]
async fn test_wifi_direct_concurrent_sends() {
    let device_id = test_device_id();
    let channel = std::sync::Arc::new(WiFiDirectChannel::new(device_id));

    let peer_id = test_device_id();
    let peer_addr: SocketAddr = "192.168.49.1:8888".parse().unwrap();

    channel.add_peer(peer_id, peer_addr).await;

    let mut handles = Vec::new();
    for i in 0..5 {
        let channel = channel.clone();
        let message = Message::new(
            device_id,
            peer_id,
            MessagePayload::Text(format!("Concurrent WiFi message {}", i)),
        );

        let handle = tokio::spawn(async move { channel.send(message).await });
        handles.push(handle);
    }

    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
}
