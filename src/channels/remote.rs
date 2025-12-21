use crate::core::error::{Result, XPushError};
use crate::core::traits::Channel;
use crate::core::types::{ChannelState, ChannelType, DeviceId, Message, NetworkType};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;

/// ntfy 远程通道实现
/// 利用 https://ntfy.sh 进行公网推送
pub struct RemoteChannel {
    local_device_id: DeviceId,
    server_url: String,
    // 追踪订阅的主题: DeviceId -> Topic
    peer_topics: Arc<Mutex<HashMap<DeviceId, String>>>,
}

impl RemoteChannel {
    pub fn new(local_device_id: DeviceId, server_url: Option<String>) -> Self {
        Self {
            local_device_id,
            server_url: server_url.unwrap_or_else(|| "https://ntfy.sh".to_string()),
            peer_topics: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn register_peer_topic(&self, device_id: DeviceId, topic: String) {
        let mut topics = self.peer_topics.lock().await;
        topics.insert(device_id, topic);
    }
}

#[async_trait]
impl Channel for RemoteChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Internet
    }

    async fn send(&self, message: Message) -> Result<()> {
        let topic = {
            let topics = self.peer_topics.lock().await;
            topics.get(&message.recipient).cloned()
        };

        if let Some(topic) = topic {
            let _url = format!("{}/{}", self.server_url, topic);
            let _payload = serde_json::to_vec(&message).map_err(XPushError::SerializationError)?;
            
            log::info!("[Remote] Publishing message {} to ntfy topic {}", message.id, topic);
            
            // 在实际实现中，这里会使用 reqwest 发送 POST 请求
            // match reqwest::Client::new().post(&url).body(payload).send().await { ... }
            
            Ok(())
        } else {
            Err(XPushError::ChannelError(format!("No ntfy topic registered for device {}", message.recipient)))
        }
    }

    async fn check_state(&self, _target: &DeviceId) -> Result<ChannelState> {
        // 远程通道通常认为始终可用，但延迟较高且成本敏感
        Ok(ChannelState {
            available: true,
            rtt_ms: 200,
            jitter_ms: 50,
            packet_loss_rate: 0.01,
            bandwidth_bps: 10_000_000, // 10 Mbps
            signal_strength: None,
            distance_meters: None, // 远程无法估算物理距离
            network_type: NetworkType::Cellular5G, // 默认假设为广域网
            failure_count: 0,
            last_heartbeat: 0,
        })
    }

    async fn start(&self) -> Result<()> {
        log::info!("Remote ntfy channel started for device {}", self.local_device_id);
        Ok(())
    }
}
