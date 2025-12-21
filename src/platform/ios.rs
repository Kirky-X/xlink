use crate::core::error::Result;
use crate::core::types::{DeviceId, DeviceCapabilities, DeviceType};
use std::collections::HashSet;

/// iOS 平台适配层
/// 处理 iOS 特有的推送通知 (APNs) 和后台执行逻辑
pub struct IosPlatform;

impl IosPlatform {
    pub fn new() -> Self {
        Self
    }

    /// 获取 iOS 设备的初始能力
    pub fn get_initial_capabilities(&self, device_id: DeviceId) -> DeviceCapabilities {
        let mut supported_channels = HashSet::new();
        // iOS 通常支持的通道
        // 注意：iOS 对 BLE 和 WiFi Direct 有严格的后台限制
        supported_channels.insert(crate::core::types::ChannelType::Internet);
        supported_channels.insert(crate::core::types::ChannelType::BluetoothLE);
        
        DeviceCapabilities {
            device_id,
            device_type: DeviceType::Smartphone,
            device_name: "iPhone".to_string(),
            supported_channels,
            battery_level: 100, // 初始值，由 detector 更新
            is_charging: false,
            data_cost_sensitive: true, // 移动端通常对流量敏感
        }
    }

    /// 处理后台唤醒 (APNs Silent Push)
    pub async fn handle_background_wakeup(&self) -> Result<()> {
        log::info!("iOS: Handling background wakeup via APNs");
        // 触发同步逻辑
        Ok(())
    }
}
