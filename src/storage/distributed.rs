use crate::core::error::{Result, XPushError};
use async_trait::async_trait;
use sha2::{Digest, Sha256};

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