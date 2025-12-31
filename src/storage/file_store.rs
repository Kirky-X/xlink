use crate::core::error::{Result, XLinkError};
use crate::core::traits::Storage;
use crate::core::types::{DeviceId, Message};
use async_trait::async_trait;
use dashmap::DashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use uuid::Uuid;

pub struct FileStorage {
    base_path: PathBuf,
    // 消息 ID 到接收者 DeviceId 的索引，用于优化 remove_message 的 O(N) 扫描问题
    message_index: Arc<DashMap<Uuid, DeviceId>>,
    // 待发送消息 ID 到发送者 DeviceId 的索引，用于优化 remove_pending_message 的 O(N) 扫描问题
    pending_index: Arc<DashMap<Uuid, DeviceId>>,
}

impl FileStorage {
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let base_path = path.as_ref().to_path_buf();

        // 验证路径安全性
        Self::validate_path(&base_path)?;

        if !base_path.exists() {
            fs::create_dir_all(&base_path)
                .await
                .map_err(Into::<XLinkError>::into)?;
        }

        let storage = Self {
            base_path,
            message_index: Arc::new(DashMap::new()),
            pending_index: Arc::new(DashMap::new()),
        };

        storage.rebuild_index().await?;
        Ok(storage)
    }

    /// 验证路径安全性，防止路径遍历攻击
    fn validate_path(path: &Path) -> Result<()> {
        // 检查路径是否包含 ".." 或其他危险模式
        let path_str = path.to_string_lossy();
        if path_str.contains("..") {
            return Err(XLinkError::invalid_input(
                "storage_path",
                "Path traversal not allowed",
                file!(),
            ));
        }

        // 检查路径长度
        if path_str.len() > 4096 {
            return Err(XLinkError::invalid_input(
                "storage_path",
                "Path too long",
                file!(),
            ));
        }

        // 如果路径存在，获取规范化路径进行进一步验证
        if path.exists() {
            let canonical = std::fs::canonicalize(path).map_err(|e| {
                XLinkError::storage_init_failed(
                    "FileStorage",
                    &format!("Invalid path: {}", e),
                    file!(),
                )
            })?;

            // 检查规范化路径是否包含 ".."
            let canonical_str = canonical.to_string_lossy();
            if canonical_str.contains("..") {
                return Err(XLinkError::invalid_input(
                    "storage_path",
                    "Path traversal not allowed",
                    file!(),
                ));
            }

            // 检查路径是否为绝对路径
            if !canonical.is_absolute() {
                return Err(XLinkError::invalid_input(
                    "storage_path",
                    "Path must be absolute",
                    file!(),
                ));
            }
        }

        Ok(())
    }

    /// 清理内存索引，使用entry removal避免内存碎片
    fn clear_indexes_internal(&self) {
        crate::utils::remove_keys(
            &self.message_index,
            crate::utils::get_all_keys(&self.message_index),
        );
        crate::utils::remove_keys(
            &self.pending_index,
            crate::utils::get_all_keys(&self.pending_index),
        );
    }

    /// 清理内存索引，防止内存泄漏
    pub fn clear_indexes(&self) {
        self.clear_indexes_internal();
    }

    /// 彻底清理内存索引，使用entry removal避免内存碎片
    pub fn cleanup_indexes(&self) {
        self.clear_indexes_internal();
    }

    /// 启动时重建内存索引
    async fn rebuild_index(&self) -> Result<()> {
        // 重建消息索引
        let mut entries = fs::read_dir(&self.base_path)
            .await
            .map_err(Into::<XLinkError>::into)?;
        while let Some(device_entry) = entries
            .next_entry()
            .await
            .map_err(Into::<XLinkError>::into)?
        {
            let path = device_entry.path();
            if path.is_dir() {
                let file_name = device_entry.file_name();
                let dir_name = file_name.to_string_lossy();

                if dir_name == "pending" {
                    // 处理待发送消息目录
                    let mut p_entries = fs::read_dir(&path)
                        .await
                        .map_err(Into::<XLinkError>::into)?;
                    while let Some(p_device_entry) = p_entries
                        .next_entry()
                        .await
                        .map_err(Into::<XLinkError>::into)?
                    {
                        if p_device_entry.path().is_dir() {
                            let p_file_name = p_device_entry.file_name();
                            let p_device_id_str = p_file_name.to_string_lossy();
                            if let Ok(device_id) = p_device_id_str.parse::<DeviceId>() {
                                let mut msg_entries = fs::read_dir(p_device_entry.path())
                                    .await
                                    .map_err(Into::<XLinkError>::into)?;
                                while let Some(msg_entry) = msg_entries
                                    .next_entry()
                                    .await
                                    .map_err(Into::<XLinkError>::into)?
                                {
                                    let msg_path = msg_entry.path();
                                    if msg_path.is_file()
                                        && msg_path.extension().and_then(|s| s.to_str())
                                            == Some("json")
                                    {
                                        if let Some(file_stem) =
                                            msg_path.file_stem().and_then(|s| s.to_str())
                                        {
                                            if let Ok(message_id) = Uuid::parse_str(file_stem) {
                                                self.pending_index.insert(message_id, device_id);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if dir_name != "audit" {
                    // 处理普通消息目录
                    if let Ok(device_id) = dir_name.parse::<DeviceId>() {
                        let mut msg_entries = fs::read_dir(&path)
                            .await
                            .map_err(Into::<XLinkError>::into)?;
                        while let Some(msg_entry) = msg_entries
                            .next_entry()
                            .await
                            .map_err(Into::<XLinkError>::into)?
                        {
                            let msg_path = msg_entry.path();
                            if msg_path.is_file()
                                && msg_path.extension().and_then(|s| s.to_str()) == Some("json")
                            {
                                if let Some(file_stem) =
                                    msg_path.file_stem().and_then(|s| s.to_str())
                                {
                                    if let Ok(message_id) = Uuid::parse_str(file_stem) {
                                        self.message_index.insert(message_id, device_id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn get_device_dir(&self, device_id: &DeviceId) -> PathBuf {
        self.base_path.join(device_id.to_string())
    }

    /// 安全地获取设备目录路径，包含验证
    fn get_device_dir_safe(&self, device_id: &DeviceId) -> Result<PathBuf> {
        let device_id_str = device_id.to_string();

        // 验证设备 ID 不包含危险字符
        if device_id_str.contains("..")
            || device_id_str.contains('/')
            || device_id_str.contains('\\')
        {
            return Err(XLinkError::invalid_input(
                "device_id",
                "Invalid device ID",
                file!(),
            ));
        }

        Ok(self.base_path.join(device_id_str))
    }

    fn get_message_path(&self, device_id: &DeviceId, message_id: &Uuid) -> PathBuf {
        self.get_device_dir(device_id)
            .join(format!("{}.json", message_id))
    }

    fn get_pending_device_dir(&self, device_id: &DeviceId) -> PathBuf {
        self.base_path.join("pending").join(device_id.to_string())
    }

    /// 安全地获取待发送设备目录路径，包含验证
    fn get_pending_device_dir_safe(&self, device_id: &DeviceId) -> Result<PathBuf> {
        let device_id_str = device_id.to_string();

        // 验证设备 ID 不包含危险字符
        if device_id_str.contains("..")
            || device_id_str.contains('/')
            || device_id_str.contains('\\')
        {
            return Err(XLinkError::invalid_input(
                "device_id",
                "Invalid device ID",
                file!(),
            ));
        }

        Ok(self.base_path.join("pending").join(device_id_str))
    }

    fn get_pending_message_path(&self, device_id: &DeviceId, message_id: &Uuid) -> PathBuf {
        self.get_pending_device_dir(device_id)
            .join(format!("{}.json", message_id))
    }
}

#[async_trait]
impl Storage for FileStorage {
    async fn save_message(&self, message: &Message) -> Result<()> {
        let device_dir = self.get_device_dir_safe(&message.recipient)?;
        if !device_dir.exists() {
            fs::create_dir_all(&device_dir)
                .await
                .map_err(Into::<XLinkError>::into)?;
        }

        let path = self.get_message_path(&message.recipient, &message.id);
        let content = serde_json::to_vec(message).map_err(Into::<XLinkError>::into)?;
        fs::write(path, content)
            .await
            .map_err(Into::<XLinkError>::into)?;

        // 更新索引
        self.message_index.insert(message.id, message.recipient);
        Ok(())
    }

    async fn get_pending_messages(&self, device_id: &DeviceId) -> Result<Vec<Message>> {
        let device_dir = self.get_device_dir_safe(device_id)?;
        if !device_dir.exists() {
            return Ok(Vec::new());
        }

        let mut messages = Vec::new();
        let mut entries = fs::read_dir(device_dir)
            .await
            .map_err(Into::<XLinkError>::into)?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(Into::<XLinkError>::into)?
        {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = fs::read(path).await.map_err(Into::<XLinkError>::into)?;
                let message: Message =
                    serde_json::from_slice(&content).map_err(Into::<XLinkError>::into)?;
                messages.push(message);
            }
        }

        Ok(messages)
    }

    async fn remove_message(&self, message_id: &Uuid) -> Result<()> {
        // 优化：从 O(N) 扫描变为基于索引的 O(1) 定位
        if let Some((_, device_id)) = self.message_index.remove(message_id) {
            let path = self.get_message_path(&device_id, message_id);
            if path.exists() {
                fs::remove_file(path)
                    .await
                    .map_err(Into::<XLinkError>::into)?;
            }
        }
        Ok(())
    }

    async fn save_audit_log(&self, log: String) -> Result<()> {
        let audit_dir = self.base_path.join("audit");
        if !audit_dir.exists() {
            fs::create_dir_all(&audit_dir)
                .await
                .map_err(Into::<XLinkError>::into)?;
        }
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| {
                XLinkError::invalid_input(
                    "timestamp",
                    &format!("SystemTime before UNIX_EPOCH: {}", e),
                    file!(),
                )
            })?
            .as_nanos();
        let path = audit_dir.join(format!("{}.log", timestamp));
        fs::write(path, log)
            .await
            .map_err(Into::<XLinkError>::into)?;
        Ok(())
    }

    async fn get_audit_logs(&self, limit: usize) -> Result<Vec<String>> {
        let audit_dir = self.base_path.join("audit");
        if !audit_dir.exists() {
            return Ok(Vec::new());
        }
        let mut logs = Vec::new();
        let mut entries = fs::read_dir(audit_dir)
            .await
            .map_err(Into::<XLinkError>::into)?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(Into::<XLinkError>::into)?
        {
            if entry.path().is_file() {
                let content = fs::read_to_string(entry.path())
                    .await
                    .map_err(Into::<XLinkError>::into)?;
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

        let mut stack = vec![self.base_path.clone()];
        while let Some(dir) = stack.pop() {
            let mut entries = fs::read_dir(dir).await.map_err(Into::<XLinkError>::into)?;
            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(Into::<XLinkError>::into)?
            {
                let metadata = entry.metadata().await.map_err(Into::<XLinkError>::into)?;
                let modified = match metadata.modified() {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let elapsed = match now.duration_since(modified) {
                    Ok(d) => d,
                    Err(_) => continue,
                };
                if elapsed > threshold {
                    if metadata.is_file() {
                        fs::remove_file(entry.path())
                            .await
                            .map_err(Into::<XLinkError>::into)?;
                        count += 1;
                    }
                } else if metadata.is_dir() {
                    stack.push(entry.path());
                }
            }
        }
        Ok(count)
    }

    async fn save_pending_message(&self, message: &Message) -> Result<()> {
        let device_dir = self.get_pending_device_dir_safe(&message.sender)?;
        if !device_dir.exists() {
            fs::create_dir_all(&device_dir)
                .await
                .map_err(Into::<XLinkError>::into)?;
        }

        let path = self.get_pending_message_path(&message.sender, &message.id);
        let content = serde_json::to_vec(message).map_err(Into::<XLinkError>::into)?;
        fs::write(path, content)
            .await
            .map_err(Into::<XLinkError>::into)?;

        // 更新索引
        self.pending_index.insert(message.id, message.sender);
        Ok(())
    }

    async fn get_pending_messages_for_recovery(
        &self,
        device_id: &DeviceId,
    ) -> Result<Vec<Message>> {
        let pending_dir = self.get_pending_device_dir_safe(device_id)?;
        if !pending_dir.exists() {
            return Ok(Vec::new());
        }

        let mut messages = Vec::new();
        let mut entries = fs::read_dir(pending_dir)
            .await
            .map_err(Into::<XLinkError>::into)?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(Into::<XLinkError>::into)?
        {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = fs::read(path).await.map_err(Into::<XLinkError>::into)?;
                let message: Message =
                    serde_json::from_slice(&content).map_err(Into::<XLinkError>::into)?;
                messages.push(message);
            }
        }

        Ok(messages)
    }

    async fn remove_pending_message(&self, message_id: &uuid::Uuid) -> Result<()> {
        // 优化：从 O(N) 扫描变为基于索引的 O(1) 定位
        if let Some((_, device_id)) = self.pending_index.remove(message_id) {
            let path = self.get_pending_message_path(&device_id, message_id);
            if path.exists() {
                fs::remove_file(path)
                    .await
                    .map_err(Into::<XLinkError>::into)?;
            }
        }
        Ok(())
    }

    async fn get_storage_usage(&self) -> Result<u64> {
        let mut total_size = 0u64;
        let mut stack = vec![self.base_path.clone()];

        while let Some(dir) = stack.pop() {
            let mut entries = fs::read_dir(dir).await.map_err(Into::<XLinkError>::into)?;
            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(Into::<XLinkError>::into)?
            {
                let metadata = entry.metadata().await.map_err(Into::<XLinkError>::into)?;
                if metadata.is_file() {
                    total_size += metadata.len();
                } else if metadata.is_dir() {
                    stack.push(entry.path());
                }
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
        let mut files_to_remove = Vec::new();

        // 收集所有文件及其修改时间
        let mut stack = vec![self.base_path.clone()];
        while let Some(dir) = stack.pop() {
            let mut entries = fs::read_dir(dir).await.map_err(Into::<XLinkError>::into)?;
            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(Into::<XLinkError>::into)?
            {
                let metadata = entry.metadata().await.map_err(Into::<XLinkError>::into)?;
                if metadata.is_file() {
                    let modified = metadata.modified().map_err(Into::<XLinkError>::into)?;
                    files_to_remove.push((entry.path(), modified, metadata.len()));
                } else if metadata.is_dir() {
                    stack.push(entry.path());
                }
            }
        }

        // 按修改时间排序（最旧的优先删除）
        files_to_remove.sort_by_key(|(_, modified, _)| *modified);

        // 删除文件直到达到目标大小
        for (path, _, size) in files_to_remove {
            if current_size - removed_size <= target_size_bytes {
                break;
            }

            fs::remove_file(&path)
                .await
                .map_err(Into::<XLinkError>::into)?;
            removed_size += size;
        }

        Ok(removed_size)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clear_indexes(&self) {
        self.clear_indexes_internal();
    }
}
