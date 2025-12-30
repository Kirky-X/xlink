use crate::core::error::Result;
use crate::core::traits::Storage;
use crate::core::types::{DeviceId, Message};
use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct MemoryStorage {
    messages: Arc<DashMap<DeviceId, Vec<Message>>>,
    pending_messages: Arc<DashMap<DeviceId, Vec<Message>>>,
    audit_logs: Arc<DashMap<String, Vec<String>>>,
    message_index: Arc<DashMap<Uuid, DeviceId>>,
    pending_index: Arc<DashMap<Uuid, DeviceId>>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(DashMap::new()),
            pending_messages: Arc::new(DashMap::new()),
            audit_logs: Arc::new(DashMap::new()),
            message_index: Arc::new(DashMap::new()),
            pending_index: Arc::new(DashMap::new()),
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
        self.message_index.insert(message.id, message.recipient);
        Ok(())
    }

    async fn get_pending_messages(&self, device_id: &DeviceId) -> Result<Vec<Message>> {
        match self.messages.get(device_id) {
            Some(msgs) => Ok(msgs.clone()),
            None => Ok(Vec::new()),
        }
    }

    async fn remove_message(&self, message_id: &Uuid) -> Result<()> {
        if let Some((_, device_id)) = self.message_index.remove(message_id) {
            if let Some(mut entry) = self.messages.get_mut(&device_id) {
                entry.retain(|m| m.id != *message_id);
            }
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
        Ok(0)
    }

    async fn save_pending_message(&self, message: &Message) -> Result<()> {
        let mut entry = self.pending_messages.entry(message.recipient).or_default();
        entry.push(message.clone());
        self.pending_index.insert(message.id, message.recipient);
        Ok(())
    }

    async fn get_pending_messages_for_recovery(
        &self,
        device_id: &DeviceId,
    ) -> Result<Vec<Message>> {
        match self.pending_messages.get(device_id) {
            Some(msgs) => Ok(msgs.clone()),
            None => Ok(Vec::new()),
        }
    }

    async fn remove_pending_message(&self, message_id: &Uuid) -> Result<()> {
        if let Some((_, device_id)) = self.pending_index.remove(message_id) {
            if let Some(mut entry) = self.pending_messages.get_mut(&device_id) {
                entry.retain(|m| m.id != *message_id);
            }
        }
        Ok(())
    }

    async fn get_storage_usage(&self) -> Result<u64> {
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

        let mut removed_size = 0u64;

        let message_keys: Vec<_> = self.messages.iter().map(|entry| *entry.key()).collect();
        for device_id in message_keys {
            if let Some((_, messages)) = self.messages.remove(&device_id) {
                removed_size += messages.len() as u64 * std::mem::size_of::<Message>() as u64;
            }
        }

        let pending_keys: Vec<_> = self
            .pending_messages
            .iter()
            .map(|entry| *entry.key())
            .collect();
        for device_id in pending_keys {
            if let Some((_, messages)) = self.pending_messages.remove(&device_id) {
                removed_size += messages.len() as u64 * std::mem::size_of::<Message>() as u64;
            }
        }

        let audit_keys: Vec<_> = self
            .audit_logs
            .iter()
            .map(|entry| entry.key().clone())
            .collect();
        for key in audit_keys {
            if let Some((_, logs)) = self.audit_logs.remove(&key) {
                removed_size += logs.len() as u64 * 100;
            }
        }

        Ok(removed_size)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clear_indexes(&self) {
        let message_keys: Vec<_> = self.messages.iter().map(|entry| *entry.key()).collect();
        for key in message_keys {
            self.messages.remove(&key);
        }

        let pending_keys: Vec<_> = self
            .pending_messages
            .iter()
            .map(|entry| *entry.key())
            .collect();
        for key in pending_keys {
            self.pending_messages.remove(&key);
        }

        let audit_keys: Vec<_> = self
            .audit_logs
            .iter()
            .map(|entry| entry.key().clone())
            .collect();
        for key in audit_keys {
            self.audit_logs.remove(&key);
        }

        let index_message_keys: Vec<_> = self
            .message_index
            .iter()
            .map(|entry| *entry.key())
            .collect();
        for key in index_message_keys {
            self.message_index.remove(&key);
        }

        let index_pending_keys: Vec<_> = self
            .pending_index
            .iter()
            .map(|entry| *entry.key())
            .collect();
        for key in index_pending_keys {
            self.pending_index.remove(&key);
        }
    }
}
