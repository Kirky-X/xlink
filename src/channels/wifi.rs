use crate::channels::base::BaseChannel;
use crate::core::error::Result;
use crate::core::traits::Channel;
use crate::core::types::{ChannelState, ChannelType, DeviceId, Message, NetworkType};
use async_trait::async_trait;
use std::net::SocketAddr;

/// WiFi Direct 通道实现
pub struct WiFiDirectChannel {
    base: BaseChannel<SocketAddr>,
}

impl WiFiDirectChannel {
    pub fn new(local_device_id: DeviceId) -> Self {
        Self {
            base: BaseChannel::new(local_device_id, ChannelType::WiFiDirect, NetworkType::WiFi),
        }
    }

    pub async fn add_peer(&self, device_id: DeviceId, addr: SocketAddr) {
        self.base.add_peer(device_id, addr).await;
    }
}

#[async_trait]
impl Channel for WiFiDirectChannel {
    fn channel_type(&self) -> ChannelType {
        self.base.channel_type()
    }

    async fn send(&self, message: Message) -> Result<()> {
        self.base
            .send_generic(
                message,
                |_addr| true, // WiFi Direct 只要有地址就认为可用
                |addr| {
                    log::info!("[WiFiDirect] Sending message to {} via P2P", addr);
                    // 此处应执行 TCP/UDP 传输
                    Ok(())
                },
                |recipient| format!("Device {} not connected via WiFi Direct", recipient),
            )
            .await
    }

    async fn check_state(&self, target: &DeviceId) -> Result<ChannelState> {
        self.base
            .check_state_generic(target, |_addr| {
                ChannelState {
                    available: true,
                    rtt_ms: 20,
                    jitter_ms: 5,
                    packet_loss_rate: 0.02,
                    bandwidth_bps: 50_000_000, // 50 Mbps
                    signal_strength: Some(-60),
                    distance_meters: Some(15.0),
                    network_type: NetworkType::WiFi,
                    failure_count: 0,
                    last_heartbeat: 0,
                }
            })
            .await
    }

    async fn start(&self) -> Result<()> {
        log::info!(
            "WiFi Direct channel started for device {}",
            self.base.local_device_id()
        );
        Ok(())
    }
}
