use crate::core::types::{ChannelType, DeviceCapabilities, DeviceType};
use crate::capability::manager::CapabilityManager;
use sysinfo::{System, SystemExt};
use std::collections::HashSet;
use std::sync::Arc;
use log::{info, debug};

/// 本地能力检测器
pub struct LocalCapabilityDetector {
    manager: Arc<CapabilityManager>,
    system: System,
}

impl LocalCapabilityDetector {
    pub fn new(manager: Arc<CapabilityManager>) -> Self {
        Self {
            manager,
            system: System::new_all(),
        }
    }

    /// 执行一次完整的本地能力检测并更新 Manager
    pub fn detect_and_update(&mut self) {
        debug!("Starting local capability detection...");
        self.system.refresh_all();
        
        let old_caps = self.manager.get_local_caps();
        let mut supported_channels = HashSet::new();

        // 1. 检测网络接口，确定是否支持 LAN
        if self.detect_lan_support() {
            supported_channels.insert(ChannelType::Lan);
            supported_channels.insert(ChannelType::Internet); // 假设有 LAN 就可能有 Internet
        }

        // 2. 检测蓝牙支持 (这里简化逻辑，假设支持 BLE)
        // 在真实场景中，我们会通过 platform-specific 代码或 sysinfo/btleplug 确认
        supported_channels.insert(ChannelType::BluetoothLE);

        // 3. 检测 WiFi Direct 支持 (假设逻辑)
        // supported_channels.insert(ChannelType::WiFiDirect);

        // 4. 获取电池状态
        let (battery_level, is_charging) = self.get_battery_info();

        // 5. 确定设备类型 (这里简化处理，可以从环境变量或系统信息推断)
        let device_type = self.infer_device_type();

        let new_caps = DeviceCapabilities {
            device_id: old_caps.device_id,
            device_type,
            device_name: self.system.host_name().unwrap_or_else(|| "Unknown Device".to_string()),
            supported_channels,
            battery_level,
            is_charging,
            data_cost_sensitive: false, // 默认不敏感，可以通过配置调整
        };

        info!("Local capabilities detected: {} (Type: {:?}, Channels: {:?})", 
            new_caps.device_name, new_caps.device_type, new_caps.supported_channels);

        self.manager.update_local_capabilities(new_caps);
    }

    fn detect_lan_support(&self) -> bool {
        // 简单通过是否有非 loopback 的网络接口来判断
        pnet_datalink::interfaces().iter().any(|iface| {
            !iface.is_loopback() && iface.is_up() && !iface.ips.is_empty()
        })
    }

    fn get_battery_info(&self) -> (Option<u8>, bool) {
        // 针对 Linux 的增强检测
        #[cfg(target_os = "linux")]
        {
            if let Ok(entries) = std::fs::read_dir("/sys/class/power_supply/") {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    if name.starts_with("BAT") {
                        let capacity = std::fs::read_to_string(path.join("capacity"))
                            .ok()
                            .and_then(|s| s.trim().parse::<u8>().ok());
                        let status = std::fs::read_to_string(path.join("status"))
                            .unwrap_or_else(|_| "Unknown".to_string());
                        let is_charging = status.trim() == "Charging" || status.trim() == "Full";
                        return (capacity, is_charging);
                    }
                }
            }
        }

        // 针对 Android 的检测 (通常通过 JNI 暴露，此处留出 hook)
        #[cfg(target_os = "android")]
        {
            // 在实际 Android 环境中，这里会调用 FFI 接口
            // return crate::platform::android::get_battery_stats();
        }

        (None, true) // 默认值
    }

    fn infer_device_type(&self) -> DeviceType {
        #[cfg(target_os = "android")]
        return DeviceType::Smartphone;

        #[cfg(target_os = "ios")]
        return DeviceType::Smartphone;

        let os_name = self.system.name().unwrap_or_default().to_lowercase();
        if os_name.contains("windows") || os_name.contains("darwin") || os_name.contains("linux") {
            // 简单推断：如果有电池且是移动操作系统则是笔记本/平板
            // 这里为了通用性，默认返回 Laptop
            return DeviceType::Laptop;
        }

        DeviceType::Server
    }
}
