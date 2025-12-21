use crate::core::error::Result;
use crate::core::types::{ChannelState, ChannelType, DeviceId, Message};
use async_trait::async_trait;

#[async_trait]
pub trait Channel: Send + Sync {
    /// Get the type of this channel
    fn channel_type(&self) -> ChannelType;

    /// Send a message to a specific device
    async fn send(&self, message: Message) -> Result<()>;

    /// Check the state of the channel for a specific target
    async fn check_state(&self, target: &DeviceId) -> Result<ChannelState>;

    /// Start listening for incoming messages
    async fn start(&self) -> Result<()>;
}

#[async_trait]
pub trait Storage: Send + Sync {
    async fn save_message(&self, message: &Message) -> Result<()>;
    async fn get_pending_messages(&self, device_id: &DeviceId) -> Result<Vec<Message>>;
    async fn remove_message(&self, message_id: &uuid::Uuid) -> Result<()>;
    
    // 审计日志支持
    async fn save_audit_log(&self, log: String) -> Result<()>;
    async fn get_audit_logs(&self, limit: usize) -> Result<Vec<String>>;
    
    // 数据清理支持
    async fn cleanup_old_data(&self, days: u32) -> Result<u64>;
}

#[async_trait]
pub trait MessageHandler: Send + Sync {
    async fn handle_message(&self, message: Message) -> Result<()>;
}

/// 插件系统 Trait
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn on_load(&self) -> Result<()>;
    fn on_unload(&self) -> Result<()>;
}

/// 自定义通道插件
pub trait ChannelPlugin: Plugin {
    fn get_channel(&self) -> std::sync::Arc<dyn Channel>;
}