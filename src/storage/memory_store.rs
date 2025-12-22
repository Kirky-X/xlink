use crate::core::error::Result;
use crate::core::traits::Storage;
use crate::core::types::{DeviceId, Message};
use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct MemoryStorage {
    // DeviceId -> List of Messages
    messages: Arc<DashMap<DeviceId, Vec<Message>>>,
    // 待发送消息队列（用于崩溃恢复）
    pending_messages: Arc<DashMap<DeviceId, Vec<Message>>>,
    // 审计日志
    audit_logs: Arc<DashMap<String, Vec<String>>>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(DashMap::new()),
            pending_messages: Arc::new(DashMap::new()),
            audit_logs: Arc::new(DashMap::new()),
        }
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Storage for MemoryStorage {
    async fn save_message(&self, message: &Message) -> Result<()> {
        let mut entry = self.messages.entry(message.recipient).or_default();
        entry.push(message.clone());
        Ok(())
    }

    async fn get_pending_messages(&self, device_id: &DeviceId) -> Result<Vec<Message>> {
        match self.messages.get(device_id) {
            Some(msgs) => Ok(msgs.clone()),
            None => Ok(Vec::new()),
        }
    }

    async fn remove_message(&self, message_id: &Uuid) -> Result<()> {
        // Inefficient for large queues, but fine for MVP Memory Store
        for mut entry in self.messages.iter_mut() {
            entry.retain(|m| m.id != *message_id);
        }
        Ok(())
    }

    async fn save_audit_log(&self, log: String) -> Result<()> {
        let mut logs = self.audit_logs.entry("default".to_string()).or_default();
        logs.push(log);
        Ok(())
    }

    async fn get_audit_logs(&self, limit: usize) -> Result<Vec<String>> {
        match self.audit_logs.get("default") {
            Some(logs) => Ok(logs.iter().rev().take(limit).cloned().collect()),
            None => Ok(Vec::new()),
        }
    }

    async fn cleanup_old_data(&self, _days: u32) -> Result<u64> {
        // 内存存储不清理旧数据
        Ok(0)
    }
    
    async fn save_pending_message(&self, message: &Message) -> Result<()> {
        let mut entry = self.pending_messages.entry(message.recipient).or_default();
        entry.push(message.clone());
        Ok(())
    }

    async fn get_pending_messages_for_recovery(&self, device_id: &DeviceId) -> Result<Vec<Message>> {
        match self.pending_messages.get(device_id) {
            Some(msgs) => Ok(msgs.clone()),
            None => Ok(Vec::new()),
        }
    }

    async fn remove_pending_message(&self, message_id: &Uuid) -> Result<()> {
        for mut entry in self.pending_messages.iter_mut() {
            entry.retain(|m| m.id != *message_id);
        }
        Ok(())
    }

    async fn get_storage_usage(&self) -> Result<u64> {
        // 内存存储返回估算值
        let mut total_size = 0u64;
        
        for entry in self.messages.iter() {
            for msg in entry.value() {
                total_size += std::mem::size_of_val(msg) as u64;
            }
        }
        
        for entry in self.pending_messages.iter() {
            for msg in entry.value() {
                total_size += std::mem::size_of_val(msg) as u64;
            }
        }
        
        for entry in self.audit_logs.iter() {
            for log in entry.value() {
                total_size += log.len() as u64;
            }
        }
        
        Ok(total_size)
    }

    async fn cleanup_storage(&self, target_size_bytes: u64) -> Result<u64> {
        let current_size = self.get_storage_usage().await?;
        if current_size <= target_size_bytes {
            return Ok(0);
        }

        // 内存存储清理：完全清除DashMap条目以避免碎片化
        let mut removed_size = 0u64;

        // 清理消息 - 完全移除条目而不是仅清空
        let message_keys: Vec<_> = self.messages.iter().map(|entry| *entry.key()).collect();
        for device_id in message_keys {
            if let Some((_, messages)) = self.messages.remove(&device_id) {
                removed_size += messages.len() as u64 * std::mem::size_of::<Message>() as u64;
            }
        }

        // 清理待发送消息 - 完全移除条目
        let pending_keys: Vec<_> = self.pending_messages.iter().map(|entry| *entry.key()).collect();
        for device_id in pending_keys {
            if let Some((_, messages)) = self.pending_messages.remove(&device_id) {
                removed_size += messages.len() as u64 * std::mem::size_of::<Message>() as u64;
            }
        }

        // 清理审计日志 - 完全移除条目
        let audit_keys: Vec<_> = self.audit_logs.iter().map(|entry| entry.key().clone()).collect();
        for key in audit_keys {
            if let Some((_, logs)) = self.audit_logs.remove(&key) {
                removed_size += logs.len() as u64 * 100; // 估算每条日志100字节
            }
        }
        
        Ok(removed_size)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clear_indexes(&self) {
        // 清理所有内存存储中的数据，使用 entry removal 避免 DashMap 碎片化
        let message_keys: Vec<_> = self.messages.iter().map(|entry| entry.key().clone()).collect();
        for key in message_keys {
            self.messages.remove(&key);
        }

        let pending_keys: Vec<_> = self.pending_messages.iter().map(|entry| entry.key().clone()).collect();
        for key in pending_keys {
            self.pending_messages.remove(&key);
        }

        let audit_keys: Vec<_> = self.audit_logs.iter().map(|entry| entry.key().clone()).collect();
        for key in audit_keys {
            self.audit_logs.remove(&key);
        }
    }
}

