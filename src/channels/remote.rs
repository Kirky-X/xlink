use crate::core::error::Result;
#[cfg(not(feature = "test_no_external_deps"))]
use crate::core::error::XPushError;
use crate::core::traits::Channel;
use crate::core::types::{ChannelState, ChannelType, DeviceId, Message, NetworkType};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
#[cfg(not(feature = "test_no_external_deps"))]
use reqwest;

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

    /// 获取服务器URL
    pub fn server_url(&self) -> &str {
        &self.server_url
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

        #[cfg(feature = "test_no_external_deps")]
        {
            // 测试模式：模拟成功发送，不实际进行HTTP请求
            let topic = topic.unwrap_or_else(|| message.recipient.to_string());
            log::info!("[Remote] Mock sending message {} to ntfy topic {} (test mode)", message.id, topic);
            self.register_peer_topic(message.recipient, topic.clone()).await;
            Ok(())
        }

        #[cfg(not(feature = "test_no_external_deps"))]
        {
            if let Some(topic) = topic {
                let url = format!("{}/{}", self.server_url, topic);
                let payload = serde_json::to_vec(&message).map_err(XPushError::SerializationError)?;
                
                log::info!("[Remote] Publishing message {} to ntfy topic {}", message.id, topic);
                
                // 使用 reqwest 发送 POST 请求到 ntfy
                let client = reqwest::Client::new();
                let response = client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .body(payload)
                    .send()
                    .await
                    .map_err(|e| XPushError::NetworkError(format!("Failed to send to ntfy: {}", e)))?;
                
                if response.status().is_success() {
                    log::info!("[Remote] Successfully published message {} to ntfy", message.id);
                    Ok(())
                } else {
                    let status = response.status();
                    let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                    log::error!("[Remote] Failed to publish message {} to ntfy: {} - {}", message.id, status, error_text);
                    Err(XPushError::ChannelError(format!("ntfy request failed: {} - {}", status, error_text)))
                }
            } else {
                // 如果没有注册主题，使用设备ID作为主题
                let topic = message.recipient.to_string();
                let url = format!("{}/{}", self.server_url, topic);
                let payload = serde_json::to_vec(&message).map_err(XPushError::SerializationError)?;
                
                log::info!("[Remote] Publishing message {} to ntfy topic {} (auto-generated)", message.id, topic);
                
                // 使用 reqwest 发送 POST 请求到 ntfy
                let client = reqwest::Client::new();
                let response = client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .body(payload)
                    .send()
                    .await
                    .map_err(|e| XPushError::NetworkError(format!("Failed to send to ntfy: {}", e)))?;
                
                if response.status().is_success() {
                    log::info!("[Remote] Successfully published message {} to ntfy", message.id);
                    // 注册这个主题以便后续使用
                    self.register_peer_topic(message.recipient, topic).await;
                    Ok(())
                } else {
                    let status = response.status();
                    let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                    log::error!("[Remote] Failed to publish message {} to ntfy: {} - {}", message.id, status, error_text);
                    Err(XPushError::ChannelError(format!("ntfy request failed: {} - {}", status, error_text)))
                }
            }
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
