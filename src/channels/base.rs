//! 通道基类 - 提供通用实现模式
//!
//! 此模块提供通用的通道实现，帮助减少蓝牙、Mesh、WiFi等物理通道的重复代码

use crate::core::error::{Result, XPushError};
use crate::core::types::{ChannelState, DeviceId, Message, NetworkType};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 通用通道基类
///
/// 提供通用的通道实现模式，包括：
/// - 对端设备管理
/// - 通用的发送和状态检查逻辑
///
/// # 泛型参数
///
/// * `T` - 对端设备信息的类型（例如蓝牙的 RSSI，Mesh 的跳数等）
pub struct BaseChannel<T> {
    local_device_id: DeviceId,
    channel_type: crate::core::types::ChannelType,
    network_type: NetworkType,
    peers: Arc<Mutex<HashMap<DeviceId, T>>>,
}

impl<T> BaseChannel<T> {
    /// 创建新的基类通道
    pub fn new(
        local_device_id: DeviceId,
        channel_type: crate::core::types::ChannelType,
        network_type: NetworkType,
    ) -> Self {
        Self {
            local_device_id,
            channel_type,
            network_type,
            peers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 获取本地设备ID
    pub fn local_device_id(&self) -> &DeviceId {
        &self.local_device_id
    }

    /// 获取通道类型
    pub fn channel_type(&self) -> crate::core::types::ChannelType {
        self.channel_type
    }

    /// 获取网络类型
    pub fn network_type(&self) -> NetworkType {
        self.network_type
    }

    /// 获取对端设备映射的克隆引用
    pub fn peers_clone(&self) -> Arc<Mutex<HashMap<DeviceId, T>>> {
        Arc::clone(&self.peers)
    }

    /// 添加对端设备
    pub async fn add_peer(&self, device_id: DeviceId, info: T) {
        let mut peers = self.peers.lock().await;
        peers.insert(device_id, info);
    }

    /// 移除对端设备
    pub async fn remove_peer(&self, device_id: &DeviceId) {
        let mut peers = self.peers.lock().await;
        peers.remove(device_id);
    }

    /// 检查对端设备是否存在
    pub async fn has_peer(&self, device_id: &DeviceId) -> bool {
        let peers = self.peers.lock().await;
        peers.contains_key(device_id)
    }

    /// 获取对端设备信息
    pub async fn get_peer(&self, device_id: &DeviceId) -> Option<T>
    where
        T: Clone,
    {
        let peers = self.peers.lock().await;
        peers.get(device_id).cloned()
    }

    /// 通用的消息发送逻辑
    ///
    /// # 参数
    ///
    /// * `message` - 要发送的消息
    /// * `check_connected` - 检查对端是否连接的闭包
    /// * `send_impl` - 实际发送操作的闭包
    /// * `get_error_msg` - 获取错误消息的闭包
    ///
    /// # 示例
    ///
    /// ```ignore
    /// base_channel.send_generic(
    ///     message,
    ///     |peer_info| peer_info.1, // 检查 connected 字段
    ///     |peer_info| {
    ///         log::info!("Sending to {:?}", peer_info);
    ///         Ok(())
    ///     },
    ///     |recipient| format!("Device {} not connected", recipient)
    /// ).await
    /// ```
    pub async fn send_generic<F1, F2, F3>(
        &self,
        message: Message,
        mut check_connected: F1,
        mut send_impl: F2,
        get_error_msg: F3,
    ) -> Result<()>
    where
        T: Clone,
        F1: FnMut(&T) -> bool,
        F2: FnMut(&T) -> Result<()>,
        F3: FnOnce(&DeviceId) -> String,
    {
        let peers = self.peers.lock().await;
        if let Some(peer_info) = peers.get(&message.recipient) {
            let peer_info = peer_info.clone();
            drop(peers);

            if check_connected(&peer_info) {
                send_impl(&peer_info)?;
                return Ok(());
            }
        }

        Err(XPushError::channel_init_failed(
            get_error_msg(&message.recipient),
            file!(),
        ))
    }

    /// 通用的状态检查逻辑
    ///
    /// # 参数
    ///
    /// * `target` - 目标设备ID
    /// * `state_builder` - 构建 ChannelState 的闭包
    ///
    /// # 示例
    ///
    /// ```ignore
    /// base_channel.check_state_generic(
    ///     target,
    ///     |peer_info| ChannelState {
    ///         available: peer_info.1,
    ///         rtt_ms: 50,
    ///         // ...
    ///     }
    /// ).await
    /// ```
    pub async fn check_state_generic<F>(
        &self,
        target: &DeviceId,
        state_builder: F,
    ) -> Result<ChannelState>
    where
        T: Clone,
        F: FnOnce(&T) -> ChannelState,
    {
        let peers = self.peers.lock().await;
        if let Some(peer_info) = peers.get(target) {
            let peer_info = peer_info.clone();
            drop(peers);
            Ok(state_builder(&peer_info))
        } else {
            Ok(ChannelState::default())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::MessagePayload;

    #[tokio::test]
    async fn test_base_channel_add_remove_peer() {
        let channel = BaseChannel::new(
            DeviceId::new(),
            crate::core::types::ChannelType::BluetoothLE,
            NetworkType::Bluetooth,
        );

        let device_id = DeviceId::new();
        channel.add_peer(device_id, (true, -50)).await;

        assert!(channel.has_peer(&device_id).await);
        channel.remove_peer(&device_id).await;
        assert!(!channel.has_peer(&device_id).await);
    }

    #[tokio::test]
    async fn test_base_channel_send_generic() {
        let channel = BaseChannel::new(
            DeviceId::new(),
            crate::core::types::ChannelType::BluetoothLE,
            NetworkType::Bluetooth,
        );

        let device_id = DeviceId::new();
        channel.add_peer(device_id, (true, -50)).await;

        let message = Message::new(
            DeviceId::new(),
            device_id,
            MessagePayload::Text("test".to_string()),
        );

        let result = channel
            .send_generic(
                message,
                |peer_info| peer_info.0,
                |_peer_info| {
                    log::info!("Sending message");
                    Ok(())
                },
                |recipient| format!("Device {} not connected", recipient),
            )
            .await;

        assert!(result.is_ok());
    }
}
