use crate::core::error::Result;
use crate::core::traits::{Channel, MessageHandler};
use crate::core::types::{ChannelState, ChannelType, DeviceId, Message};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

use tokio::task::JoinHandle;

/// A simulated in-memory channel for testing and demonstration.
/// It simulates network latency and delivery.
pub struct MemoryChannel {
    channel_type: ChannelType,
    // Simulate the network: A shared bus where messages are put
    // In a real scenario, this would be a socket or radio interface
    _inbox: Arc<Mutex<Vec<Message>>>,
    handler: Arc<Mutex<Arc<dyn MessageHandler>>>,
    latency_ms: u64,
    should_fail: Arc<Mutex<bool>>,
    sent_messages: Arc<Mutex<Vec<Message>>>,
}

impl MemoryChannel {
    pub fn new(handler: Arc<dyn MessageHandler>, latency_ms: u64) -> Self {
        Self {
            channel_type: ChannelType::Lan,
            _inbox: Arc::new(Mutex::new(Vec::new())),
            handler: Arc::new(Mutex::new(handler)),
            latency_ms,
            should_fail: Arc::new(Mutex::new(false)),
            sent_messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn with_type(mut self, channel_type: ChannelType) -> Self {
        self.channel_type = channel_type;
        self
    }

    pub fn set_failure(&self, fail: bool) {
        if let Ok(mut guard) = self.should_fail.try_lock() {
            *guard = fail;
        }
    }

    pub async fn get_sent_messages(&self) -> Vec<Message> {
        self.sent_messages.lock().await.clone()
    }

    pub async fn clear_sent_messages(&self) {
        self.sent_messages.lock().await.clear();
    }

    /// Helper to simulate receiving a message from "outside"
    pub async fn simulate_incoming(&self, message: Message) {
        let h = self.handler.lock().await.clone();
        if let Err(e) = h.handle_message(message).await {
            log::error!("Error handling incoming message: {}", e);
        }
    }
}

#[async_trait]
impl Channel for MemoryChannel {
    fn channel_type(&self) -> ChannelType {
        self.channel_type
    }

    async fn send(&self, message: Message) -> Result<()> {
        if *self.should_fail.lock().await {
            return Err(crate::core::error::XPushError::ChannelError(
                "Simulated failure".into(),
            ));
        }

        // Simulate network delay
        tokio::time::sleep(tokio::time::Duration::from_millis(self.latency_ms)).await;

        log::info!(
            "[MemoryChannel] Transmitting message {} from {} to {}",
            message.id,
            message.sender,
            message.recipient
        );

        self.sent_messages.lock().await.push(message);

        Ok(())
    }

    async fn check_state(&self, _target: &DeviceId) -> Result<ChannelState> {
        let failed = *self.should_fail.lock().await;
        // Simulate a perfect connection
        Ok(ChannelState {
            available: !failed,
            rtt_ms: if failed { 0 } else { self.latency_ms as u32 },
            jitter_ms: 0,
            packet_loss_rate: if failed { 1.0 } else { 0.0 },
            bandwidth_bps: 1_000_000_000, // 1Gbps
            signal_strength: Some(-50),
            network_type: crate::core::types::NetworkType::Loopback,
            failure_count: if failed { 1 } else { 0 },
            last_heartbeat: std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            distance_meters: Some(0.0), // 内存通道，距离为0米
        })
    }

    async fn start(&self) -> Result<()> {
        log::info!("[MemoryChannel] Started listening...");
        Ok(())
    }

    async fn start_with_handler(
        &self,
        handler: Arc<dyn MessageHandler>,
    ) -> Result<Option<JoinHandle<()>>> {
        log::info!("[MemoryChannel] Started listening with custom handler...");
        let mut h = self.handler.lock().await;
        *h = handler;
        Ok(None)
    }

    async fn clear_handler(&self) -> Result<()> {
        log::info!("[MemoryChannel] Clearing message handler...");
        let mut h = self.handler.lock().await;
        // Replace with a no-op handler to break the reference cycle
        *h = Arc::new(crate::channels::dummy::DummyMessageHandler);
        Ok(())
    }
}
