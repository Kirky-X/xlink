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
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(DashMap::new()),
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

    async fn save_audit_log(&self, _log: String) -> Result<()> {
        Ok(())
    }

    async fn get_audit_logs(&self, _limit: usize) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    async fn cleanup_old_data(&self, _days: u32) -> Result<u64> {
        Ok(0)
    }
}