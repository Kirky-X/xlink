mod common;

use crate::common::{test_device_id, NoOpMessageHandler};
use std::net::SocketAddr;
use std::sync::Arc;
use xlink::channels::lan::LanChannel;
use xlink::core::error::Result;
use xlink::core::traits::Channel;
use xlink::core::types::ChannelType;

#[tokio::test]
async fn test_lan_channel_creation() -> Result<()> {
    let addr: SocketAddr = "127.0.0.1:18080".parse().unwrap();
    let handler = Arc::new(NoOpMessageHandler);
    let channel = LanChannel::new(addr, handler).await?;

    assert_eq!(channel.channel_type(), ChannelType::Lan);
    Ok(())
}

#[tokio::test]
async fn test_lan_register_peer() -> Result<()> {
    let addr: SocketAddr = "127.0.0.1:18081".parse().unwrap();
    let handler = Arc::new(NoOpMessageHandler);
    let channel = LanChannel::new(addr, handler).await?;

    let peer_id = test_device_id();
    let peer_addr: SocketAddr = "192.168.1.100:8080".parse().unwrap();

    channel.register_peer(peer_id, peer_addr).await;

    let state = channel.check_state(&peer_id).await?;
    assert!(state.available);
    assert_eq!(state.rtt_ms, 5);
    assert_eq!(state.bandwidth_bps, 100_000_000);
    Ok(())
}

#[tokio::test]
async fn test_lan_check_state_unknown_peer() -> Result<()> {
    let addr: SocketAddr = "127.0.0.1:18082".parse().unwrap();
    let handler = Arc::new(NoOpMessageHandler);
    let channel = LanChannel::new(addr, handler).await?;

    let peer_id = test_device_id();
    let state = channel.check_state(&peer_id).await?;

    assert!(!state.available);
    assert_eq!(state.rtt_ms, 0);
    Ok(())
}

#[tokio::test]
async fn test_lan_start() -> Result<()> {
    let addr: SocketAddr = "127.0.0.1:18085".parse().unwrap();
    let handler = Arc::new(NoOpMessageHandler);
    let channel = LanChannel::new(addr, handler).await?;

    let result = channel.start().await;
    assert!(result.is_ok());
    Ok(())
}

#[tokio::test]
async fn test_lan_multiple_peers() -> Result<()> {
    let addr: SocketAddr = "127.0.0.1:18086".parse().unwrap();
    let handler = Arc::new(NoOpMessageHandler);
    let channel = LanChannel::new(addr, handler).await?;

    let peer1 = test_device_id();
    let peer2 = test_device_id();
    let peer3 = test_device_id();

    channel
        .register_peer(peer1, "192.168.1.100:8080".parse().unwrap())
        .await;
    channel
        .register_peer(peer2, "192.168.1.101:8080".parse().unwrap())
        .await;
    channel
        .register_peer(peer3, "192.168.1.102:8080".parse().unwrap())
        .await;

    let state1 = channel.check_state(&peer1).await?;
    let state2 = channel.check_state(&peer2).await?;
    let state3 = channel.check_state(&peer3).await?;

    assert!(state1.available);
    assert!(state2.available);
    assert!(state3.available);

    assert_eq!(state1.distance_meters, Some(50.0));
    assert_eq!(state2.distance_meters, Some(50.0));
    assert_eq!(state3.distance_meters, Some(50.0));
    Ok(())
}

#[tokio::test]
async fn test_lan_state_bandwidth() -> Result<()> {
    let addr: SocketAddr = "127.0.0.1:18088".parse().unwrap();
    let handler = Arc::new(NoOpMessageHandler);
    let channel = LanChannel::new(addr, handler).await?;

    let peer_id = test_device_id();
    channel
        .register_peer(peer_id, "192.168.1.100:8080".parse().unwrap())
        .await;

    let state = channel.check_state(&peer_id).await?;

    assert_eq!(state.bandwidth_bps, 100_000_000);
    assert_eq!(state.packet_loss_rate, 0.0);
    assert_eq!(state.jitter_ms, 0);
    assert_eq!(state.signal_strength, Some(100));
    Ok(())
}

#[tokio::test]
async fn test_lan_peer_update() -> Result<()> {
    let addr: SocketAddr = "127.0.0.1:18089".parse().unwrap();
    let handler = Arc::new(NoOpMessageHandler);
    let channel = LanChannel::new(addr, handler).await?;

    let peer_id = test_device_id();

    let state_before = channel.check_state(&peer_id).await?;
    assert!(!state_before.available);

    channel
        .register_peer(peer_id, "192.168.1.100:8080".parse().unwrap())
        .await;

    let state_after = channel.check_state(&peer_id).await?;
    assert!(state_after.available);
    Ok(())
}
