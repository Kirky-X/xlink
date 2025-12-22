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

    /// Start listening for incoming messages with a handler
    async fn start_with_handler(&self, _handler: std::sync::Arc<dyn MessageHandler>) -> Result<Option<tokio::task::JoinHandle<()>>> {
        self.start().await?;
        Ok(None)
    }

    /// Clear the message handler (for cleanup to prevent memory leaks)
    async fn clear_handler(&self) -> Result<()> {
        // Default implementation does nothing
        Ok(())
    }
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
    
    // 消息队列持久化支持（用于设备崩溃恢复）
    async fn save_pending_message(&self, message: &Message) -> Result<()>;
    async fn get_pending_messages_for_recovery(&self, device_id: &DeviceId) -> Result<Vec<Message>>;
    async fn remove_pending_message(&self, message_id: &uuid::Uuid) -> Result<()>;
    
    // 存储空间管理
    async fn get_storage_usage(&self) -> Result<u64>;
    async fn cleanup_storage(&self, target_size_bytes: u64) -> Result<u64>;

    // 索引清理（用于内存泄漏防护）
    fn clear_indexes(&self);

    // 类型转换支持
    fn as_any(&self) -> &dyn std::any::Any;
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