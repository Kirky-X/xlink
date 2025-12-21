use crate::core::error::{Result, XPushError};
use crate::core::traits::Channel;
use crate::core::types::{ChannelState, ChannelType, DeviceId, Message, NetworkType};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;

/// 蓝牙 Mesh 通道实现
/// 注意：Mesh 通道通常基于 Flooding 或 Routing 机制在 BLE 之上工作
pub struct BluetoothMeshChannel {
    local_device_id: DeviceId,
    // 模拟邻居节点: DeviceId -> (Hops, Connected)
    neighbors: Arc<Mutex<HashMap<DeviceId, (u8, bool)>>>,
}

impl BluetoothMeshChannel {
    pub fn new(local_device_id: DeviceId) -> Self {
        Self {
            local_device_id,
            neighbors: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 模拟发现邻居节点
    pub async fn add_neighbor(&self, device_id: DeviceId, hops: u8) {
        let mut neighbors = self.neighbors.lock().await;
        neighbors.insert(device_id, (hops, true));
    }
}

#[async_trait]
impl Channel for BluetoothMeshChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::BluetoothMesh
    }

    async fn send(&self, message: Message) -> Result<()> {
        let neighbors = self.neighbors.lock().await;
        if let Some((hops, connected)) = neighbors.get(&message.recipient) {
            if *connected {
                log::info!("[Bluetooth Mesh] Routing message {} to device {} via {} hops", 
                    message.id, message.recipient, hops);
                // 模拟多跳传输成功
                return Ok(());
            }
        }
        
        // 如果不是直接邻居，Mesh 协议会自动尝试泛洪或路由，此处模拟失败
        Err(XPushError::ChannelError(format!("Device {} not reachable in Bluetooth Mesh network", message.recipient)))
    }

    async fn check_state(&self, target: &DeviceId) -> Result<ChannelState> {
        let neighbors = self.neighbors.lock().await;
        if let Some((hops, connected)) = neighbors.get(target) {
            Ok(ChannelState {
                available: *connected,
                rtt_ms: (50 * (*hops as u32)).max(50), // 延迟随跳数增加
                jitter_ms: 20,
                packet_loss_rate: 0.1, // Mesh 丢包率通常较高
                bandwidth_bps: 100_000, // Mesh 带宽非常受限 (e.g. 100 kbps)
                signal_strength: None,
                distance_meters: None, // Mesh 中距离不直观
                network_type: NetworkType::Bluetooth,
                failure_count: 0,
                last_heartbeat: 0,
            })
        } else {
            Ok(ChannelState::default())
        }
    }

    async fn start(&self) -> Result<()> {
        log::info!("Bluetooth Mesh channel started for device {}", self.local_device_id);
        Ok(())
    }
}
