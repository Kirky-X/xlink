use crate::core::error::{Result, XPushError};
use crate::core::types::{DeviceId, GroupId, MessagePayload};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use dashmap::DashMap;
use hkdf::Hkdf;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::Arc;
use x25519_dalek::{EphemeralSecret, PublicKey, StaticSecret};

// 32字节密钥
type Key = [u8; 32];

/// TreeKEM 节点结构
#[derive(Clone, Serialize, Deserialize)]
pub struct TreeKemNode {
    pub node_id: u32,
    pub public_key: Option<PublicKey>,
    #[serde(skip)]
    pub private_key: Option<StaticSecret>, // 仅叶子节点有私钥
    pub parent_id: Option<u32>,
    pub children: Vec<u32>,
}

impl std::fmt::Debug for TreeKemNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TreeKemNode")
            .field("node_id", &self.node_id)
            .field("public_key", &self.public_key)
            .field("private_key", &"<redacted>")
            .field("parent_id", &self.parent_id)
            .field("children", &self.children)
            .finish()
    }
}

/// TreeKEM 群组状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeKemGroup {
    pub group_id: GroupId,
    pub tree: HashMap<u32, TreeKemNode>,
    pub epoch: u64,
    pub group_secret: Key,
    pub member_devices: HashMap<DeviceId, u32>, // DeviceId -> NodeId mapping
}

/// TreeKEM 更新路径
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePath {
    pub updater_id: DeviceId,
    pub path_secrets: Vec<Key>,
    pub path_public_keys: Vec<PublicKey>,
    pub epoch: u64,
}

/// TreeKEM 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TreeKemMessage {
    Welcome {
        group_id: GroupId,
        tree_data: Vec<u8>,
        group_secret: Key,
        epoch: u64,
    },
    Update {
        group_id: GroupId,
        update_path: UpdatePath,
        epoch: u64,
    },
    Commit {
        group_id: GroupId,
        update_path: UpdatePath,
        proposals: Vec<Proposal>,
        epoch: u64,
    },
}

/// 提案类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Proposal {
    Add {
        device_id: DeviceId,
        public_key: PublicKey,
    },
    Remove {
        device_id: DeviceId,
    },
    Update {
        device_id: DeviceId,
        public_key: PublicKey,
    },
}

pub struct TreeKemEngine {
    // GroupId -> TreeKemGroup
    groups: Arc<DashMap<GroupId, TreeKemGroup>>,
    // DeviceId -> PublicKey (设备公钥存储)
    device_public_keys: Arc<DashMap<DeviceId, PublicKey>>,
    // 当前设备的ID
    _local_device_id: DeviceId,
}

impl TreeKemEngine {
    pub fn new(local_device_id: DeviceId) -> Self {
        Self {
            groups: Arc::new(DashMap::new()),
            device_public_keys: Arc::new(DashMap::new()),
            _local_device_id: local_device_id,
        }
    }

    /// 创建新的TreeKEM群组
    pub fn create_group(
        &self,
        group_id: GroupId,
        initial_members: Vec<(DeviceId, PublicKey)>,
    ) -> Result<TreeKemGroup> {
        let mut tree = HashMap::new();
        let mut member_devices = HashMap::new();

        // 创建平衡的树结构
        let (root_id, tree_nodes) = self.build_balanced_tree(&initial_members);

        for node in tree_nodes {
            tree.insert(node.node_id, node.clone());
            if let Some(device_id) = initial_members
                .iter()
                .find(|(_, pk)| node.public_key == Some(*pk))
                .map(|(id, _)| *id)
            {
                member_devices.insert(device_id, node.node_id);
            }
        }

        // 生成群组密钥
        let group_secret = self.generate_group_secret(&tree, root_id)?;

        let group = TreeKemGroup {
            group_id,
            tree,
            epoch: 0,
            group_secret,
            member_devices,
        };

        self.groups.insert(group_id, group.clone());
        Ok(group)
    }

    /// 构建平衡的树结构
    fn build_balanced_tree(&self, members: &[(DeviceId, PublicKey)]) -> (u32, Vec<TreeKemNode>) {
        let n = members.len();
        if n == 0 {
            return (0, vec![]);
        }

        let mut nodes = Vec::new();
        let mut node_id = 1u32;

        // 创建叶子节点（成员节点）
        for (_device_id, public_key) in members {
            let node = TreeKemNode {
                node_id,
                public_key: Some(*public_key),
                private_key: None, // 只有设备自己知道私钥
                parent_id: None,
                children: vec![],
            };
            nodes.push(node);
            node_id += 1;
        }

        // 构建父节点直到根节点
        let mut current_level = (0..n).collect::<Vec<_>>();
        while current_level.len() > 1 {
            let mut next_level = Vec::new();

            for i in (0..current_level.len()).step_by(2) {
                let left_idx = current_level[i];
                let right_idx = if i + 1 < current_level.len() {
                    current_level[i + 1]
                } else {
                    current_level[i] // 奇数个节点时复制最后一个
                };

                // 创建父节点
                let parent_node = TreeKemNode {
                    node_id,
                    public_key: None, // 父节点没有长期密钥
                    private_key: None,
                    parent_id: None,
                    children: vec![nodes[left_idx].node_id, nodes[right_idx].node_id],
                };

                // 更新子节点的父节点引用
                if let Some(left_node) = nodes.get_mut(left_idx) {
                    left_node.parent_id = Some(node_id);
                }
                if let Some(right_node) = nodes.get_mut(right_idx) {
                    right_node.parent_id = Some(node_id);
                }

                next_level.push(nodes.len());
                nodes.push(parent_node);
                node_id += 1;
            }

            current_level = next_level;
        }

        let root_id = if let Some(&root_idx) = current_level.first() {
            nodes[root_idx].node_id
        } else {
            0
        };

        (root_id, nodes)
    }

    /// 生成群组密钥
    fn generate_group_secret(&self, tree: &HashMap<u32, TreeKemNode>, root_id: u32) -> Result<Key> {
        // 使用根节点的路径密钥生成群组密钥
        let root_node = tree
            .get(&root_id)
            .ok_or_else(|| XPushError::CryptoError("Root node not found".into()))?;

        // 简化的群组密钥生成：使用根节点的公钥哈希
        if let Some(public_key) = root_node.public_key {
            let mut hasher = sha2::Sha256::new();
            use sha2::Digest;
            hasher.update(public_key.as_bytes());
            let result = hasher.finalize();

            let mut secret = [0u8; 32];
            secret.copy_from_slice(&result[..32]);
            Ok(secret)
        } else {
            // 生成随机群组密钥
            let mut secret = [0u8; 32];
            OsRng.fill_bytes(&mut secret);
            Ok(secret)
        }
    }

    /// 更新群组密钥（前向保密）
    pub fn update_group_key(&self, group_id: GroupId, updater_id: DeviceId) -> Result<UpdatePath> {
        let mut group = self
            .groups
            .get_mut(&group_id)
            .ok_or_else(|| XPushError::CryptoError("Group not found".into()))?;

        // 生成新的更新路径
        let update_path = self.generate_update_path(&group, updater_id)?;

        // 更新群组密钥
        group.epoch += 1;
        group.group_secret = self.derive_new_group_secret(&update_path.path_secrets)?;

        Ok(update_path)
    }

    /// 生成更新路径
    fn generate_update_path(
        &self,
        group: &TreeKemGroup,
        updater_id: DeviceId,
    ) -> Result<UpdatePath> {
        let updater_node_id = group
            .member_devices
            .get(&updater_id)
            .ok_or_else(|| XPushError::CryptoError("Updater not in group".into()))?;

        let mut path_secrets = Vec::new();
        let mut path_public_keys = Vec::new();
        let mut current_node_id = *updater_node_id;

        // 从更新者节点到根节点的路径
        while let Some(node) = group.tree.get(&current_node_id) {
            // 生成新的路径密钥
            let mut path_secret = [0u8; 32];
            OsRng.fill_bytes(&mut path_secret);

            // 生成对应的公钥
            let ephemeral_secret = EphemeralSecret::random_from_rng(OsRng);
            let public_key = PublicKey::from(&ephemeral_secret);

            path_secrets.push(path_secret);
            path_public_keys.push(public_key);

            // 移动到父节点
            if let Some(parent_id) = node.parent_id {
                current_node_id = parent_id;
            } else {
                break; // 到达根节点
            }
        }

        Ok(UpdatePath {
            updater_id,
            path_secrets,
            path_public_keys,
            epoch: group.epoch,
        })
    }

    /// 派生新的群组密钥
    fn derive_new_group_secret(&self, path_secrets: &[Key]) -> Result<Key> {
        if path_secrets.is_empty() {
            return Err(XPushError::CryptoError("No path secrets".into()));
        }

        // 使用最后一个路径密钥派生新的群组密钥
        let last_secret = path_secrets.last().unwrap();
        let hk = Hkdf::<Sha256>::new(None, last_secret);
        let mut new_group_secret = [0u8; 32];
        hk.expand(b"group_secret", &mut new_group_secret)
            .map_err(|e| XPushError::CryptoError(e.to_string()))?;

        Ok(new_group_secret)
    }

    /// 加密群组消息
    pub fn encrypt_group_message(
        &self,
        group_id: GroupId,
        payload: &MessagePayload,
    ) -> Result<MessagePayload> {
        let group = self
            .groups
            .get(&group_id)
            .ok_or_else(|| XPushError::CryptoError("Group not found".into()))?;

        // 序列化消息负载
        let plaintext = serde_json::to_vec(payload)
            .map_err(|e| XPushError::CryptoError(format!("Failed to serialize payload: {}", e)))?;

        // 使用 ChaCha20-Poly1305 加密
        let cipher = ChaCha20Poly1305::new(&group.group_secret.into());
        let nonce_bytes = rand::random::<[u8; 12]>();
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_ref())
            .map_err(|e| XPushError::CryptoError(e.to_string()))?;

        // 打包: Epoch + Nonce + Ciphertext
        let mut result = Vec::with_capacity(8 + 12 + ciphertext.len());
        result.extend_from_slice(&group.epoch.to_be_bytes());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(MessagePayload::Binary(result))
    }

    /// 解密群组消息
    pub fn decrypt_group_message(
        &self,
        group_id: GroupId,
        encrypted_payload: &MessagePayload,
    ) -> Result<MessagePayload> {
        let ciphertext_data = match encrypted_payload {
            MessagePayload::Binary(data) => data,
            _ => return Ok(encrypted_payload.clone()), // 非加密消息直接返回
        };

        if ciphertext_data.len() < 20 {
            // 8 (epoch) + 12 (nonce)
            return Err(XPushError::CryptoError("Data too short".into()));
        }

        let group = self
            .groups
            .get(&group_id)
            .ok_or_else(|| XPushError::CryptoError("Group not found".into()))?;

        // 解析数据
        let epoch = u64::from_be_bytes([
            ciphertext_data[0],
            ciphertext_data[1],
            ciphertext_data[2],
            ciphertext_data[3],
            ciphertext_data[4],
            ciphertext_data[5],
            ciphertext_data[6],
            ciphertext_data[7],
        ]);

        // 检查epoch是否匹配
        if epoch != group.epoch {
            return Err(XPushError::CryptoError(format!(
                "Epoch mismatch: expected {}, got {}",
                group.epoch, epoch
            )));
        }

        let nonce = Nonce::from_slice(&ciphertext_data[8..20]);
        let ciphertext = &ciphertext_data[20..];

        // 使用群组密钥解密
        let cipher = ChaCha20Poly1305::new(&group.group_secret.into());
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| XPushError::CryptoError(e.to_string()))?;

        // 反序列化消息负载
        serde_json::from_slice(&plaintext)
            .map_err(|e| XPushError::CryptoError(format!("Failed to deserialize payload: {}", e)))
    }

    /// 添加成员到群组
    pub fn add_member(
        &self,
        group_id: GroupId,
        device_id: DeviceId,
        public_key: PublicKey,
    ) -> Result<()> {
        let mut group = self
            .groups
            .get_mut(&group_id)
            .ok_or_else(|| XPushError::CryptoError("Group not found".into()))?;

        if group.member_devices.contains_key(&device_id) {
            return Err(XPushError::CryptoError("Device already in group".into()));
        }

        // 创建新的叶子节点
        let new_node_id = group.tree.len() as u32 + 1;
        let new_node = TreeKemNode {
            node_id: new_node_id,
            public_key: Some(public_key),
            private_key: None,
            parent_id: None,
            children: vec![],
        };

        group.tree.insert(new_node_id, new_node);
        group.member_devices.insert(device_id, new_node_id);

        // 重新平衡树结构（简化实现）
        self.rebalance_tree(&mut group)?;

        Ok(())
    }

    /// 从群组中移除成员
    pub fn remove_member(&self, group_id: GroupId, device_id: DeviceId) -> Result<()> {
        let mut group = match self.groups.get_mut(&group_id) {
            Some(g) => g,
            None => return Ok(()), // 如果群组不存在，直接返回成功（幂等性）
        };

        let node_id = match group.member_devices.remove(&device_id) {
            Some(id) => id,
            None => return Ok(()), // 如果设备不在群组中，直接返回成功
        };

        // 从树中移除节点
        group.tree.remove(&node_id);

        // 重新平衡树结构
        self.rebalance_tree(&mut group)?;

        Ok(())
    }

    /// 重新平衡树结构（简化实现）
    fn rebalance_tree(&self, group: &mut TreeKemGroup) -> Result<()> {
        // 获取所有叶子节点（成员节点）
        let leaf_nodes: Vec<_> = group
            .tree
            .values()
            .filter(|node| node.public_key.is_some())
            .cloned()
            .collect();

        if leaf_nodes.is_empty() {
            return Ok(());
        }

        // 重新构建平衡的树
        let (_new_root_id, new_nodes) = self.build_balanced_tree(
            &leaf_nodes
                .iter()
                .filter_map(|node| {
                    group
                        .member_devices
                        .iter()
                        .find(|(_, &node_id)| node_id == node.node_id)
                        .map(|(device_id, _)| (*device_id, node.public_key.unwrap()))
                })
                .collect::<Vec<_>>(),
        );

        // 更新树结构
        group.tree.clear();
        for node in new_nodes {
            group.tree.insert(node.node_id, node);
        }

        // 更新成员映射
        group.member_devices.clear();
        for node in leaf_nodes {
            if let Some(device_id) = group
                .member_devices
                .iter()
                .find(|(_, &node_id)| node_id == node.node_id)
                .map(|(id, _)| *id)
            {
                group.member_devices.insert(device_id, node.node_id);
            }
        }

        Ok(())
    }

    /// 获取群组信息
    pub fn get_group(&self, group_id: GroupId) -> Option<TreeKemGroup> {
        self.groups.get(&group_id).map(|g| g.clone())
    }

    /// 获取所有群组
    pub fn get_all_groups(&self) -> Vec<GroupId> {
        self.groups.iter().map(|entry| *entry.key()).collect()
    }

    /// 注册设备公钥
    pub fn register_device_key(&self, device_id: DeviceId, public_key: PublicKey) -> Result<()> {
        self.device_public_keys.insert(device_id, public_key);
        Ok(())
    }

    /// 获取设备公钥
    pub fn get_device_public_key(&self, device_id: DeviceId) -> Result<PublicKey> {
        self.device_public_keys
            .get(&device_id)
            .map(|entry| *entry.value())
            .ok_or_else(|| {
                XPushError::CryptoError(format!("Public key for device {} not found", device_id))
            })
    }

    /// 应用密钥更新路径
    pub fn apply_update_path(&self, group_id: GroupId, update_path: &UpdatePath) -> Result<()> {
        let mut group = self
            .groups
            .get_mut(&group_id)
            .ok_or_else(|| XPushError::CryptoError("Group not found".into()))?;

        // 验证更新路径的有效性
        if update_path.epoch != group.epoch + 1 {
            return Err(XPushError::CryptoError(format!(
                "Invalid epoch: expected {}, got {}",
                group.epoch + 1,
                update_path.epoch
            )));
        }

        // 更新群组密钥
        group.epoch = update_path.epoch;
        group.group_secret = self.derive_new_group_secret(&update_path.path_secrets)?;

        log::info!(
            "Applied update path for group {} at epoch {}",
            group_id,
            group.epoch
        );
        Ok(())
    }

    /// 清理所有数据，防止内存泄漏 - use proper entry removal to avoid DashMap fragmentation
    pub fn clear_keys(&self) {
        let group_keys: Vec<_> = self.groups.iter().map(|entry| *entry.key()).collect();
        for group_id in group_keys {
            self.groups.remove(&group_id);
        }

        let device_keys: Vec<_> = self
            .device_public_keys
            .iter()
            .map(|entry| *entry.key())
            .collect();
        for device_id in device_keys {
            self.device_public_keys.remove(&device_id);
        }
    }
}
