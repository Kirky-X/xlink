use crate::channels::base::BaseChannel;
use crate::core::error::Result;
use crate::core::traits::Channel;
use crate::core::types::{ChannelState, ChannelType, DeviceId, Message, NetworkType};
use async_trait::async_trait;

/// 蓝牙 Mesh 通道实现
/// 注意：Mesh 通道通常基于 Flooding 或 Routing 机制在 BLE 之上工作
pub struct BluetoothMeshChannel {
    base: BaseChannel<(u8, bool)>,
}

impl BluetoothMeshChannel {
    pub fn new(local_device_id: DeviceId) -> Self {
        Self {
            base: BaseChannel::new(
                local_device_id,
                ChannelType::BluetoothMesh,
                NetworkType::Bluetooth,
            ),
        }
    }

    /// 模拟发现邻居节点
    pub async fn add_neighbor(&self, device_id: DeviceId, hops: u8) {
        self.base.add_peer(device_id, (hops, true)).await;
    }
}

#[async_trait]
impl Channel for BluetoothMeshChannel {
    fn channel_type(&self) -> ChannelType {
        self.base.channel_type()
    }

    async fn send(&self, message: Message) -> Result<()> {
        self.base
            .send_generic(
                message,
                |peer_info| peer_info.1, // 检查 connected 字段
                |peer_info| {
                    log::info!("[Bluetooth Mesh] Routing message via {} hops", peer_info.0);
                    Ok(())
                },
                |recipient| {
                    format!(
                        "Device {} not reachable in Bluetooth Mesh network",
                        recipient
                    )
                },
            )
            .await
    }

    async fn check_state(&self, target: &DeviceId) -> Result<ChannelState> {
        self.base
            .check_state_generic(target, |peer_info| {
                let (hops, connected) = *peer_info;
                ChannelState {
                    available: connected,
                    rtt_ms: (50 * hops as u32).max(50), // 延迟随跳数增加
                    jitter_ms: 20,
                    packet_loss_rate: 0.1,  // Mesh 丢包率通常较高
                    bandwidth_bps: 100_000, // Mesh 带宽非常受限 (e.g. 100 kbps)
                    signal_strength: None,
                    distance_meters: None, // Mesh 中距离不直观
                    network_type: NetworkType::Bluetooth,
                    failure_count: 0,
                    last_heartbeat: 0,
                }
            })
            .await
    }

    async fn start(&self) -> Result<()> {
        log::info!(
            "Bluetooth Mesh channel started for device {}",
            self.base.local_device_id()
        );
        Ok(())
    }
}
