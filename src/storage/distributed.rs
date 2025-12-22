use crate::core::error::{Result, XPushError};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// F10: 分布式存储接口
/// 允许集成 IPFS, Arweave 或其他去中心化存储
#[async_trait]
pub trait DistributedStore: Send + Sync {
    /// 上传内容，返回内容哈希 (CID)
    async fn upload(&self, data: &[u8]) -> Result<String>;
    
    /// 根据哈希下载内容
    async fn download(&self, hash: &str) -> Result<Vec<u8>>;
    
    /// 获取存储类型名称 (e.g., "IPFS", "Arweave", "FileSystem")
    fn protocol_name(&self) -> &str;
}

/// 模拟 IPFS 的行为：内容寻址
pub struct FileDistributedStore {
    base_path: std::path::PathBuf,
}

impl FileDistributedStore {
    pub async fn new<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let base_path = path.as_ref().to_path_buf();
        if !base_path.exists() {
            tokio::fs::create_dir_all(&base_path).await.map_err(XPushError::IoError)?;
        }
        Ok(Self { base_path })
    }
    
    fn compute_hash(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        // 模拟 CID 格式 (Qm...)
        format!("Qm{}", hex::encode(result))
    }

    fn get_path(&self, hash: &str) -> std::path::PathBuf {
        self.base_path.join(hash)
    }
}

#[async_trait]
impl DistributedStore for FileDistributedStore {
    async fn upload(&self, data: &[u8]) -> Result<String> {
        let hash = Self::compute_hash(data);
        let path = self.get_path(&hash);
        tokio::fs::write(path, data).await.map_err(XPushError::IoError)?;
        log::info!("[DistStore] Uploaded {} bytes, CID: {}", data.len(), hash);
        Ok(hash)
    }

    async fn download(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.get_path(hash);
        if !path.exists() {
            return Err(XPushError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound, 
                format!("Content not found for hash: {}", hash)
            )));
        }
        let data = tokio::fs::read(path).await.map_err(XPushError::IoError)?;
        log::info!("[DistStore] Downloaded {} bytes from CID: {}", data.len(), hash);
        Ok(data)
    }
    
    fn protocol_name(&self) -> &str {
        "FileSystem(IPFS-like)"
    }
}

/// 分布式存储适配器，实现标准存储接口
pub struct DistributedStorageAdapter {
    distributed_store: Arc<dyn DistributedStore>,
    local_cache: Arc<crate::storage::memory_store::MemoryStorage>,
}

impl DistributedStorageAdapter {
    pub fn new(distributed_store: Arc<dyn DistributedStore>) -> Self {
        Self {
            distributed_store,
            local_cache: Arc::new(crate::storage::memory_store::MemoryStorage::new()),
        }
    }
}

#[async_trait]
impl crate::core::traits::Storage for DistributedStorageAdapter {
    async fn save_message(&self, message: &crate::core::types::Message) -> crate::core::error::Result<()> {
        // 将消息序列化并上传到分布式存储
        let data = serde_json::to_vec(message)
            .map_err(crate::core::error::XPushError::SerializationError)?;
        let hash = self.distributed_store.upload(&data).await?;
        
        // 在本地缓存中保存哈希引用
        let hash_message = crate::core::types::Message {
            id: message.id,
            sender: message.sender,
            recipient: message.recipient,
            group_id: message.group_id,
            payload: crate::core::types::MessagePayload::Text(hash),
            priority: message.priority,
            timestamp: message.timestamp,
            require_ack: message.require_ack,
        };
        self.local_cache.save_message(&hash_message).await
    }

    async fn get_pending_messages(&self, device_id: &crate::core::types::DeviceId) -> crate::core::error::Result<Vec<crate::core::types::Message>> {
        // 从本地缓存获取哈希引用
        let hash_messages = self.local_cache.get_pending_messages(device_id).await?;
        let mut messages = Vec::new();
        
        // 从分布式存储下载实际消息内容
        for hash_msg in hash_messages {
            if let crate::core::types::MessagePayload::Text(hash) = &hash_msg.payload {
                let data = self.distributed_store.download(hash).await?;
                let message: crate::core::types::Message = serde_json::from_slice(&data)
                    .map_err(crate::core::error::XPushError::SerializationError)?;
                messages.push(message);
            }
        }
        
        Ok(messages)
    }

    async fn remove_message(&self, message_id: &uuid::Uuid) -> crate::core::error::Result<()> {
        self.local_cache.remove_message(message_id).await
    }

    async fn save_audit_log(&self, log: String) -> crate::core::error::Result<()> {
        self.local_cache.save_audit_log(log).await
    }

    async fn get_audit_logs(&self, limit: usize) -> crate::core::error::Result<Vec<String>> {
        self.local_cache.get_audit_logs(limit).await
    }

    async fn cleanup_old_data(&self, days: u32) -> crate::core::error::Result<u64> {
        self.local_cache.cleanup_old_data(days).await
    }

    async fn save_pending_message(&self, message: &crate::core::types::Message) -> crate::core::error::Result<()> {
        self.local_cache.save_pending_message(message).await
    }

    async fn get_pending_messages_for_recovery(&self, device_id: &crate::core::types::DeviceId) -> crate::core::error::Result<Vec<crate::core::types::Message>> {
        self.local_cache.get_pending_messages_for_recovery(device_id).await
    }

    async fn remove_pending_message(&self, message_id: &uuid::Uuid) -> crate::core::error::Result<()> {
        self.local_cache.remove_pending_message(message_id).await
    }

    async fn get_storage_usage(&self) -> crate::core::error::Result<u64> {
        self.local_cache.get_storage_usage().await
    }

    async fn cleanup_storage(&self, target_size_bytes: u64) -> crate::core::error::Result<u64> {
        self.local_cache.cleanup_storage(target_size_bytes).await
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clear_indexes(&self) {
        // 清理本地缓存索引
        self.local_cache.clear_indexes();
    }
}