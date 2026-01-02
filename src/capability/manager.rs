use crate::core::types::{ChannelState, ChannelType, DeviceCapabilities, DeviceId};
use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

/// 能力变化事件类型
#[derive(Debug, Clone)]
pub enum CapabilityChange {
    /// 通道支持状态变化
    ChannelSupportChanged {
        device_id: DeviceId,
        channel: ChannelType,
        supported: bool,
    },
    /// 电池状态变化
    BatteryStateChanged {
        device_id: DeviceId,
        battery_level: Option<u8>,
        is_charging: bool,
    },
    /// 网络类型变化
    NetworkTypeChanged {
        device_id: DeviceId,
        network_type: crate::core::types::NetworkType,
    },
    /// 设备能力完全更新
    CapabilitiesUpdated {
        device_id: DeviceId,
        new_capabilities: DeviceCapabilities,
    },
}

/// 能力变化监听器
pub type CapabilityChangeHandler = Box<dyn Fn(CapabilityChange) + Send + Sync>;

#[derive(Clone)]
pub struct CapabilityManager {
    local_capabilities: Arc<RwLock<DeviceCapabilities>>,
    // Map: Remote DeviceId -> (ChannelType -> ChannelState)
    remote_states: Arc<DashMap<DeviceId, DashMap<ChannelType, ChannelState>>>,
    // Map: Remote DeviceId -> Remote Capabilities
    remote_caps: Arc<DashMap<DeviceId, DeviceCapabilities>>,
    // 能力变化监听器列表
    change_handlers: Arc<dashmap::DashMap<String, CapabilityChangeHandler>>,
}

impl CapabilityManager {
    pub fn new(local_caps: DeviceCapabilities) -> Self {
        Self {
            local_capabilities: Arc::new(RwLock::new(local_caps)),
            remote_states: Arc::new(DashMap::new()),
            remote_caps: Arc::new(DashMap::new()),
            change_handlers: Arc::new(dashmap::DashMap::new()),
        }
    }

    pub fn get_local_caps(&self) -> DeviceCapabilities {
        self.local_capabilities.read().expect("Failed to acquire read lock for local_capabilities").clone()
    }

    pub fn update_channel_state(
        &self,
        device: DeviceId,
        channel: ChannelType,
        state: ChannelState,
    ) {
        let device_entry = self.remote_states.entry(device).or_default();
        device_entry.insert(channel, state);
    }

    pub fn get_channel_state(
        &self,
        device: &DeviceId,
        channel: &ChannelType,
    ) -> Option<ChannelState> {
        self.remote_states
            .get(device)
            .and_then(|map| map.get(channel).map(|v| v.clone()))
    }

    /// 清理所有远程设备信息，防止内存泄漏
    pub fn clear_remote_devices(&self) {
        // Remove remote_states entries one by one to avoid fragmentation
        crate::utils::remove_keys(
            &self.remote_states,
            crate::utils::get_all_keys(&self.remote_states),
        );

        // Remove remote_caps entries one by one to avoid fragmentation
        crate::utils::remove_keys(
            &self.remote_caps,
            crate::utils::get_all_keys(&self.remote_caps),
        );

        // Remove change_handlers entries one by one to avoid fragmentation
        crate::utils::remove_keys(
            &self.change_handlers,
            crate::utils::get_all_keys(&self.change_handlers),
        );
    }

    pub fn register_remote_device(&self, caps: DeviceCapabilities) {
        self.remote_caps.insert(caps.device_id, caps);
    }

    /// 获取指定远程设备的能力
    pub fn get_remote_device(&self, device_id: DeviceId) -> Option<DeviceCapabilities> {
        self.remote_caps
            .get(&device_id)
            .map(|entry| entry.value().clone())
    }

    /// 获取所有远程设备 ID
    pub fn get_all_remote_devices(&self) -> Vec<DeviceId> {
        self.remote_caps.iter().map(|r| *r.key()).collect()
    }

    /// 注册能力变化监听器
    ///
    /// # 参数
    /// * `handler_id` - 监听器唯一标识
    /// * `handler` - 能力变化处理函数
    ///
    /// # 示例
    /// ```rust,no_run
    /// use std::sync::Arc;
    /// use xlink::capability::manager::{CapabilityManager, CapabilityChange};
    /// use xlink::core::types::{DeviceCapabilities, DeviceType, ChannelType, DeviceId};
    /// use std::collections::HashSet;
    ///
    /// # fn main() {
    /// # let caps = DeviceCapabilities {
    /// #     device_id: DeviceId::new(),
    /// #     device_type: DeviceType::Smartphone,
    /// #     device_name: "Test Device".to_string(),
    /// #     supported_channels: HashSet::new(),
    /// #     battery_level: Some(80),
    /// #     is_charging: false,
    /// #     data_cost_sensitive: true,
    /// # };
    /// # let manager = Arc::new(CapabilityManager::new(caps));
    /// manager.watch_capability_changes("my_handler", Box::new(|change| {
    ///     match change {
    ///         CapabilityChange::ChannelSupportChanged { device_id, channel, supported } => {
    ///             println!("Device {} channel {:?} support changed to {}", device_id, channel, supported);
    ///         }
    ///         CapabilityChange::BatteryStateChanged { device_id, battery_level, is_charging } => {
    ///             println!("Device {} battery: {:?}% charging: {}", device_id, battery_level, is_charging);
    ///         }
    ///         _ => {}
    ///     }
    /// }));
    /// # }
    pub fn watch_capability_changes(&self, handler_id: &str, handler: CapabilityChangeHandler) {
        self.change_handlers.insert(handler_id.to_string(), handler);
        log::info!("Registered capability change handler: {}", handler_id);
    }

    /// 移除能力变化监听器
    pub fn unwatch_capability_changes(&self, handler_id: &str) {
        self.change_handlers.remove(handler_id);
        log::info!("Removed capability change handler: {}", handler_id);
    }

    /// 触发能力变化事件
    fn notify_capability_change(&self, change: CapabilityChange) {
        for entry in self.change_handlers.iter() {
            let handler = entry.value();
            handler(change.clone());
        }
    }

    /// 更新本地设备能力并通知变化
    pub fn update_local_capabilities(&self, new_capabilities: DeviceCapabilities) {
        // 获取当前能力
        let current_capabilities = self.local_capabilities.read().expect("Failed to acquire read lock for local_capabilities").clone();

        // 检查能力变化
        let changes = self.detect_capability_changes(&current_capabilities, &new_capabilities);

        // 更新本地能力
        *self.local_capabilities.write().expect("Failed to acquire write lock for local_capabilities") = new_capabilities.clone();

        // 通知所有变化
        for change in changes {
            self.notify_capability_change(change);
        }
    }

    /// 检测能力变化
    fn detect_capability_changes(
        &self,
        old: &DeviceCapabilities,
        new: &DeviceCapabilities,
    ) -> Vec<CapabilityChange> {
        let mut changes = Vec::new();
        let device_id = new.device_id;

        // 检查通道支持变化
        let old_channels: HashSet<_> = old.supported_channels.iter().collect();
        let new_channels: HashSet<_> = new.supported_channels.iter().collect();

        // 检查新增的通道
        for &channel in new_channels.difference(&old_channels) {
            changes.push(CapabilityChange::ChannelSupportChanged {
                device_id,
                channel: *channel,
                supported: true,
            });
        }

        // 检查移除的通道
        for &channel in old_channels.difference(&new_channels) {
            changes.push(CapabilityChange::ChannelSupportChanged {
                device_id,
                channel: *channel,
                supported: false,
            });
        }

        // 检查电池状态变化
        if old.battery_level != new.battery_level || old.is_charging != new.is_charging {
            changes.push(CapabilityChange::BatteryStateChanged {
                device_id,
                battery_level: new.battery_level,
                is_charging: new.is_charging,
            });
        }

        // 如果有任何变化，发送完整更新事件
        if !changes.is_empty() {
            changes.push(CapabilityChange::CapabilitiesUpdated {
                device_id,
                new_capabilities: new.clone(),
            });
        }

        changes
    }
}
