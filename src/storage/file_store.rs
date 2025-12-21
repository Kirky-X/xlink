use crate::core::error::{Result, XPushError};
use crate::core::traits::Storage;
use crate::core::types::{DeviceId, Message};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

pub struct FileStorage {
    base_path: PathBuf,
}

impl FileStorage {
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let base_path = path.as_ref().to_path_buf();
        if !base_path.exists() {
            fs::create_dir_all(&base_path).await.map_err(XPushError::IoError)?;
        }
        Ok(Self { base_path })
    }

    fn get_device_dir(&self, device_id: &DeviceId) -> PathBuf {
        self.base_path.join(device_id.to_string())
    }

    fn get_message_path(&self, device_id: &DeviceId, message_id: &Uuid) -> PathBuf {
        self.get_device_dir(device_id).join(format!("{}.json", message_id))
    }
}

#[async_trait]
impl Storage for FileStorage {
    async fn save_message(&self, message: &Message) -> Result<()> {
        let device_dir = self.get_device_dir(&message.recipient);
        if !device_dir.exists() {
            fs::create_dir_all(&device_dir).await.map_err(XPushError::IoError)?;
        }

        let path = self.get_message_path(&message.recipient, &message.id);
        let content = serde_json::to_vec(message).map_err(XPushError::SerializationError)?;
        fs::write(path, content).await.map_err(XPushError::IoError)?;
        Ok(())
    }

    async fn get_pending_messages(&self, device_id: &DeviceId) -> Result<Vec<Message>> {
        let device_dir = self.get_device_dir(device_id);
        if !device_dir.exists() {
            return Ok(Vec::new());
        }

        let mut messages = Vec::new();
        let mut entries = fs::read_dir(device_dir).await.map_err(XPushError::IoError)?;

        while let Some(entry) = entries.next_entry().await.map_err(XPushError::IoError)? {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = fs::read(path).await.map_err(XPushError::IoError)?;
                let message: Message = serde_json::from_slice(&content)
                    .map_err(XPushError::SerializationError)?;
                messages.push(message);
            }
        }

        Ok(messages)
    }

    async fn remove_message(&self, message_id: &Uuid) -> Result<()> {
        // Since we don't know the device_id here easily without scanning (unless we change the trait or storage structure)
        // We'll scan the base directory. For a real implementation, we might want a better indexing.
        let mut entries = fs::read_dir(&self.base_path).await.map_err(XPushError::IoError)?;
        
        while let Some(device_entry) = entries.next_entry().await.map_err(XPushError::IoError)? {
            if device_entry.path().is_dir() {
                let mut msg_entries = fs::read_dir(device_entry.path()).await.map_err(XPushError::IoError)?;
                while let Some(msg_entry) = msg_entries.next_entry().await.map_err(XPushError::IoError)? {
                    let path = msg_entry.path();
                    if path.file_name().and_then(|s| s.to_str()) == Some(&format!("{}.json", message_id)) {
                        fs::remove_file(path).await.map_err(XPushError::IoError)?;
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }

    async fn save_audit_log(&self, log: String) -> Result<()> {
        let audit_dir = self.base_path.join("audit");
        if !audit_dir.exists() {
            fs::create_dir_all(&audit_dir).await.map_err(XPushError::IoError)?;
        }
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = audit_dir.join(format!("{}.log", timestamp));
        fs::write(path, log).await.map_err(XPushError::IoError)?;
        Ok(())
    }

    async fn get_audit_logs(&self, limit: usize) -> Result<Vec<String>> {
        let audit_dir = self.base_path.join("audit");
        if !audit_dir.exists() {
            return Ok(Vec::new());
        }
        let mut logs = Vec::new();
        let mut entries = fs::read_dir(audit_dir).await.map_err(XPushError::IoError)?;
        while let Some(entry) = entries.next_entry().await.map_err(XPushError::IoError)? {
            if entry.path().is_file() {
                let content = fs::read_to_string(entry.path()).await.map_err(XPushError::IoError)?;
                logs.push(content);
                if logs.len() >= limit {
                    break;
                }
            }
        }
        Ok(logs)
    }

    async fn cleanup_old_data(&self, days: u32) -> Result<u64> {
        let mut count = 0;
        let now = std::time::SystemTime::now();
        let threshold = std::time::Duration::from_secs((days * 24 * 3600) as u64);

        // 递归清理旧文件 (简单逻辑)
        let mut stack = vec![self.base_path.clone()];
        while let Some(dir) = stack.pop() {
            let mut entries = fs::read_dir(dir).await.map_err(XPushError::IoError)?;
            while let Some(entry) = entries.next_entry().await.map_err(XPushError::IoError)? {
                let metadata = entry.metadata().await.map_err(XPushError::IoError)?;
                let modified = metadata.modified().map_err(XPushError::IoError)?;
                if now.duration_since(modified).unwrap_or(std::time::Duration::ZERO) > threshold {
                    if metadata.is_file() {
                        fs::remove_file(entry.path()).await.map_err(XPushError::IoError)?;
                        count += 1;
                    }
                } else if metadata.is_dir() {
                    stack.push(entry.path());
                }
            }
        }
        Ok(count)
    }
}
