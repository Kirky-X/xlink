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
/// 支持主备服务器切换，利用 https://ntfy.sh 进行公网推送
pub struct RemoteChannel {
    local_device_id: DeviceId,
    primary_server_url: String,
    backup_server_urls: Vec<String>,
    current_server_index: Arc<Mutex<usize>>,
    // 追踪订阅的主题: DeviceId -> Topic
    peer_topics: Arc<Mutex<HashMap<DeviceId, String>>>,
}

impl RemoteChannel {
    pub fn new(local_device_id: DeviceId, server_url: Option<String>) -> Self {
        let primary_url = server_url.unwrap_or_else(|| "https://ntfy.sh".to_string());
        let backup_urls = vec![
            "https://ntfy.sh".to_string(),
            "https://ntfy.net".to_string(),
            "https://ntfy.dev".to_string(),
        ];
        
        Self {
            local_device_id,
            primary_server_url: primary_url,
            backup_server_urls: backup_urls,
            current_server_index: Arc::new(Mutex::new(0)),
            peer_topics: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// 创建支持主备服务器切换的远程通道
    pub fn with_failover(local_device_id: DeviceId, primary_url: String, backup_urls: Vec<String>) -> Self {
        Self {
            local_device_id,
            primary_server_url: primary_url,
            backup_server_urls: backup_urls,
            current_server_index: Arc::new(Mutex::new(0)),
            peer_topics: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 获取当前使用的服务器URL
    pub async fn current_server_url(&self) -> String {
        let index = *self.current_server_index.lock().await;
        if index == 0 {
            self.primary_server_url.clone()
        } else {
            self.backup_server_urls.get(index - 1)
                .unwrap_or(&self.primary_server_url)
                .clone()
        }
    }
    
    /// 切换到下一个备用服务器
    pub async fn switch_to_next_server(&self) -> bool {
        let mut index = self.current_server_index.lock().await;
        if *index < self.backup_server_urls.len() {
            *index += 1;
            log::warn!("[Remote] Switched to backup server {}: {}", 
                *index, 
                self.backup_server_urls.get(*index - 1).unwrap_or(&self.primary_server_url));
            true
        } else {
            log::error!("[Remote] No more backup servers available, staying with current server");
            false
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
            let mut attempts = 0;
            let max_attempts = self.backup_server_urls.len() + 1; // 主服务器 + 所有备用服务器
            
            loop {
                let current_server = self.current_server_url().await;
                let topic_str = topic.clone().unwrap_or_else(|| message.recipient.to_string());
                let url = format!("{}/{}", current_server, topic_str);
                let payload = serde_json::to_vec(&message).map_err(XPushError::SerializationError)?;
                
                log::info!("[Remote] Attempting to publish message {} to ntfy topic {} on server {}", 
                    message.id, topic_str, current_server);
                
                // 使用 reqwest 发送 POST 请求到 ntfy
                let client = reqwest::Client::new();
                match client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .body(payload)
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.status().is_success() {
                            log::info!("[Remote] Successfully published message {} to ntfy on server {}", message.id, current_server);
                            if topic.is_none() {
                                self.register_peer_topic(message.recipient, topic_str).await;
                            }
                            return Ok(());
                        } else {
                            let status = response.status();
                            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                            log::warn!("[Remote] Server {} returned error for message {}: {} - {}", 
                                current_server, message.id, status, error_text);
                            
                            // 尝试切换到下一个服务器
                            if attempts < max_attempts - 1 {
                                if self.switch_to_next_server().await {
                                    attempts += 1;
                                    continue;
                                }
                            }
                            
                            return Err(XPushError::ChannelError(format!("ntfy request failed: {} - {}", status, error_text)));
                        }
                    }
                    Err(e) => {
                        log::warn!("[Remote] Network error sending message {} to server {}: {}", 
                            message.id, current_server, e);
                        
                        // 网络错误，尝试切换到下一个服务器
                        if attempts < max_attempts - 1 {
                            if self.switch_to_next_server().await {
                                attempts += 1;
                                continue;
                            }
                        }
                        
                        return Err(XPushError::ChannelError(format!("Failed to send to ntfy after {} attempts: {}", attempts + 1, e)));
                    }
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
