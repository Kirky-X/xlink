use crate::core::error::{Result, XPushError};
use crate::core::traits::{Channel, MessageHandler};
use crate::core::types::{ChannelState, ChannelType, DeviceId, Message, NetworkType};
use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

use tokio::task::JoinHandle;

/// A real LAN channel implementation using UDP
pub struct LanChannel {
    local_addr: SocketAddr,
    socket: Arc<UdpSocket>,
    handler: Arc<Mutex<Arc<dyn MessageHandler>>>,
    // Map of DeviceId to their last known SocketAddr
    peers: Arc<Mutex<std::collections::HashMap<DeviceId, SocketAddr>>>,
}

impl LanChannel {
    pub async fn new(
        local_addr: SocketAddr,
        handler: Arc<dyn MessageHandler>,
    ) -> Result<Self> {
        let socket = UdpSocket::bind(local_addr)
            .await
            .map_err(XPushError::IoError)?;
        
        Ok(Self {
            local_addr,
            socket: Arc::new(socket),
            handler: Arc::new(Mutex::new(handler)),
            peers: Arc::new(Mutex::new(std::collections::HashMap::new())),
        })
    }

    pub async fn register_peer(&self, device_id: DeviceId, addr: SocketAddr) {
        let mut peers = self.peers.lock().await;
        peers.insert(device_id, addr);
    }
}

#[async_trait]
impl Channel for LanChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Lan
    }

    async fn send(&self, message: Message) -> Result<()> {
        let target_addr = {
            let peers = self.peers.lock().await;
            peers.get(&message.recipient).cloned()
        };

        match target_addr {
            Some(addr) => {
                let data = serde_json::to_vec(&message)
                    .map_err(XPushError::SerializationError)?;
                
                self.socket.send_to(&data, addr)
                    .await
                    .map_err(XPushError::IoError)?;
                
                log::info!("[LanChannel] Sent message {} to {}", message.id, addr);
                Ok(())
            }
            None => Err(XPushError::ChannelError(format!(
                "No address known for device {}",
                message.recipient
            ))),
        }
    }

    async fn check_state(&self, target: &DeviceId) -> Result<ChannelState> {
        let peers = self.peers.lock().await;
        let available = peers.contains_key(target);
        
        Ok(ChannelState {
            available,
            rtt_ms: if available { 5 } else { 0 },
            jitter_ms: 0,
            packet_loss_rate: 0.0,
            bandwidth_bps: 100_000_000,
            signal_strength: Some(100),
            network_type: NetworkType::WiFi,
            failure_count: 0,
            last_heartbeat: 0,
            distance_meters: Some(50.0), // 局域网，默认50米距离
        })
    }

    async fn start(&self) -> Result<()> {
        // Default start does nothing, as it requires a handler
        Ok(())
    }

    async fn start_with_handler(&self, handler: Arc<dyn MessageHandler>) -> Result<Option<JoinHandle<()>>> {
        {
            let mut h = self.handler.lock().await;
            *h = handler;
        }

        let socket = self.socket.clone();
        let handler_mutex = self.handler.clone();
        let peers = self.peers.clone();

        let task = tokio::spawn(async move {
            let mut buf = [0u8; 65535];
            loop {
                match socket.recv_from(&mut buf).await {
                    Ok((size, addr)) => {
                        let data = &buf[..size];
                        match serde_json::from_slice::<Message>(data) {
                            Ok(msg) => {
                                // Update peer address on receipt
                                {
                                    let mut p = peers.lock().await;
                                    p.insert(msg.sender, addr);
                                }

                                let handler = handler_mutex.lock().await.clone();
                                if let Err(e) = handler.handle_message(msg).await {
                                    log::error!("[LanChannel] Error handling message: {}", e);
                                }
                            }
                            Err(e) => {
                                log::error!("[LanChannel] Failed to deserialize message: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("[LanChannel] Socket receive error: {}", e);
                        break;
                    }
                }
            }
        });

        log::info!("[LanChannel] Started listening on {}", self.local_addr);
        Ok(Some(task))
    }
}
