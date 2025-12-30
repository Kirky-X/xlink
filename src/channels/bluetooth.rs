use crate::channels::base::BaseChannel;
use crate::core::error::Result;
use crate::core::traits::Channel;
use crate::core::types::{ChannelState, ChannelType, DeviceId, Message, NetworkType};
use async_trait::async_trait;

/// 蓝牙 BLE 通道实现
/// 注意：在实际生产环境中应集成 btleplug 或平台原生蓝牙栈
pub struct BluetoothChannel {
    base: BaseChannel<(i8, bool)>,
}

impl BluetoothChannel {
    pub fn new(local_device_id: DeviceId) -> Self {
        Self {
            base: BaseChannel::new(
                local_device_id,
                ChannelType::BluetoothLE,
                NetworkType::Bluetooth,
            ),
        }
    }

    /// 模拟发现对端蓝牙设备
    pub async fn discover_peer(&self, device_id: DeviceId, rssi: i8) {
        self.base.add_peer(device_id, (rssi, true)).await;
    }
}

#[async_trait]
impl Channel for BluetoothChannel {
    fn channel_type(&self) -> ChannelType {
        self.base.channel_type()
    }

    async fn send(&self, message: Message) -> Result<()> {
        self.base
            .send_generic(
                message,
                |peer_info| peer_info.1, // 检查 connected 字段
                |peer_info| {
                    // 在此处执行真实的 GATT 写入操作
                    log::info!(
                        "[Bluetooth] Sending message to device with RSSI {} via BLE",
                        peer_info.0
                    );
                    Ok(())
                },
                |recipient| format!("Device {} not connected via Bluetooth", recipient),
            )
            .await
    }

    async fn check_state(&self, target: &DeviceId) -> Result<ChannelState> {
        self.base
            .check_state_generic(target, |peer_info| {
                let (rssi, connected) = *peer_info;
                // 蓝牙距离估算公式 (简单模拟)
                let distance = 10.0_f32.powf((-69.0 - rssi as f32) / (10.0 * 2.0));

                ChannelState {
                    available: connected,
                    rtt_ms: 50, // 蓝牙典型延迟较高
                    jitter_ms: 10,
                    packet_loss_rate: 0.05,
                    bandwidth_bps: 1_000_000, // 1 Mbps
                    signal_strength: Some(rssi),
                    distance_meters: Some(distance),
                    network_type: NetworkType::Bluetooth,
                    failure_count: 0,
                    last_heartbeat: 0,
                }
            })
            .await
    }

    async fn start(&self) -> Result<()> {
        log::info!(
            "Bluetooth channel started for device {}",
            self.base.local_device_id()
        );
        // 此处应启动扫描任务
        Ok(())
    }
}
