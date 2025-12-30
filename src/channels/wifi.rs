use crate::core::error::{Result, XPushError};
use crate::core::traits::Channel;
use crate::core::types::{ChannelState, ChannelType, DeviceId, Message, NetworkType};
use async_trait::async_trait;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

/// WiFi Direct 通道实现
pub struct WiFiDirectChannel {
    local_device_id: DeviceId,
    // 模拟对端地址映射: DeviceId -> SocketAddr
    peers: Arc<Mutex<HashMap<DeviceId, SocketAddr>>>,
}

impl WiFiDirectChannel {
    pub fn new(local_device_id: DeviceId) -> Self {
        Self {
            local_device_id,
            peers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn add_peer(&self, device_id: DeviceId, addr: SocketAddr) {
        let mut peers = self.peers.lock().await;
        peers.insert(device_id, addr);
    }
}

#[async_trait]
impl Channel for WiFiDirectChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::WiFiDirect
    }

    async fn send(&self, message: Message) -> Result<()> {
        let peers = self.peers.lock().await;
        if let Some(addr) = peers.get(&message.recipient) {
            log::info!(
                "[WiFiDirect] Sending message {} to {} via P2P",
                message.id,
                addr
            );
            // 此处应执行 TCP/UDP 传输
            return Ok(());
        }

        Err(XPushError::channel_init_failed(
            format!("Device {} not connected via WiFi Direct", message.recipient),
            file!(),
        ))
    }

    async fn check_state(&self, target: &DeviceId) -> Result<ChannelState> {
        let peers = self.peers.lock().await;
        let available = peers.contains_key(target);

        Ok(ChannelState {
            available,
            rtt_ms: if available { 20 } else { 0 },
            jitter_ms: 5,
            packet_loss_rate: 0.02,
            bandwidth_bps: 50_000_000, // 50 Mbps
            signal_strength: Some(-60),
            distance_meters: Some(15.0),
            network_type: NetworkType::WiFi,
            failure_count: 0,
            last_heartbeat: 0,
        })
    }

    async fn start(&self) -> Result<()> {
        log::info!(
            "WiFi Direct channel started for device {}",
            self.local_device_id
        );
        Ok(())
    }
}
