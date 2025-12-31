use crate::core::error::{Result, XLinkError};
use crate::core::types::{
    DeviceId, Group, GroupId, GroupMember, MemberRole, MemberStatus, Message, MessagePayload,
    MessagePriority,
};
use crate::crypto::treekem::TreeKemEngine;
use crate::crypto::treekem::UpdatePath;
use crate::router::selector::Router;
use dashmap::DashMap;
use futures::stream::{FuturesUnordered, StreamExt};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;
use x25519_dalek::PublicKey;

/// 设备邻近性类型，用于混合拓扑广播
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProximityType {
    /// 近场设备，适合使用BLE/WiFi Direct
    Nearby,
    /// 远程设备，需要使用Internet通道
    Remote,
    /// 可作为中继节点的设备
    RelayCandidate,
}

type AckStats = (HashSet<DeviceId>, HashSet<DeviceId>, HashSet<DeviceId>);

pub struct GroupManager {
    local_device_id: DeviceId,
    groups: DashMap<GroupId, Group>,
    router: Arc<Router>,
    // TreeKEM 群组密钥管理引擎
    treekem_engine: Arc<TreeKemEngine>,
    // 追踪待确认的消息: MessageId -> (Pending Set, Success Set, Failure Set)
    pending_acks: Arc<DashMap<Uuid, AckStats>>,
    // 邀请去重
    processed_invites: Arc<DashMap<GroupId, u64>>,
    // ACK 超时配置
    ack_timeout: Duration,
    // 广播结果通知通道
    broadcast_results: Arc<RwLock<HashMap<Uuid, mpsc::Sender<BroadcastResult>>>>,
}

#[derive(Debug, Clone)]
pub struct BroadcastResult {
    pub message_id: Uuid,
    pub successful_devices: HashSet<DeviceId>,
    pub failed_devices: HashSet<DeviceId>,
    pub total_attempts: usize,
}

impl GroupManager {
    pub fn new(local_device_id: DeviceId, router: Arc<Router>) -> Self {
        let treekem_engine = Arc::new(TreeKemEngine::new(local_device_id));

        Self {
            local_device_id,
            groups: DashMap::new(),
            router,
            treekem_engine,
            pending_acks: Arc::new(DashMap::new()),
            processed_invites: Arc::new(DashMap::new()),
            ack_timeout: Duration::from_secs(30),
            broadcast_results: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册设备公钥到 TreeKEM 引擎
    pub fn register_device_key(&self, device_id: DeviceId, public_key: PublicKey) -> Result<()> {
        self.treekem_engine
            .register_device_key(device_id, public_key);
        Ok(())
    }

    /// 智能分类设备邻近性，用于混合拓扑广播
    /// 基于路由器选择的通道类型来判断设备距离
    async fn classify_member_proximity(&self, member_id: DeviceId) -> ProximityType {
        // 创建一个测试消息来判断路由器会选择什么通道
        let test_message = Message {
            id: uuid::Uuid::new_v4(), // 测试ID
            sender: self.local_device_id,
            recipient: member_id,
            group_id: None,
            payload: MessagePayload::Text("test".to_string()),
            timestamp: 0,
            priority: crate::core::types::MessagePriority::Normal,
            require_ack: false,
        };

        // 尝试选择通道来判断设备类型
        match self.router.select_channel(&test_message).await {
            Ok(channel) => match channel.channel_type() {
                crate::core::types::ChannelType::BluetoothLE
                | crate::core::types::ChannelType::BluetoothMesh => ProximityType::Nearby,
                crate::core::types::ChannelType::WiFiDirect => ProximityType::Nearby,
                crate::core::types::ChannelType::Internet
                | crate::core::types::ChannelType::Lan => ProximityType::Remote,
            },
            Err(_) => ProximityType::Remote, // 如果无法选择通道，默认为远程
        }
    }

    pub async fn create_group(
        &self,
        name: String,
        initial_members: Vec<DeviceId>,
    ) -> Result<Group> {
        let group_id = GroupId::new();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_secs();

        // 初始化 TreeKEM 群组密钥
        let member_ids: Vec<_> = initial_members
            .iter()
            .filter(|&&device_id| self.treekem_engine.get_device_public_key(device_id).is_ok())
            .cloned()
            .collect();

        if member_ids.is_empty() {
            return Err(XLinkError::invalid_input(
                "member_keys",
                "No valid member keys found for group creation",
                file!(),
            ));
        }

        self.treekem_engine.create_group(group_id, member_ids)?;

        let mut members = HashMap::new();
        for device_id in initial_members {
            members.insert(
                device_id,
                GroupMember {
                    device_id,
                    role: MemberRole::Member,
                    joined_at: now,
                    last_seen: now,
                    status: MemberStatus::Online,
                },
            );
        }

        // 设置本地设备为管理员
        members.insert(
            self.local_device_id,
            GroupMember {
                device_id: self.local_device_id,
                role: MemberRole::Admin,
                joined_at: now,
                last_seen: now,
                status: MemberStatus::Online,
            },
        );

        let group = Group {
            id: group_id,
            name: name.clone(),
            members,
            created_at: now,
        };

        self.groups.insert(group_id, group.clone());

        log::info!(
            "Created group {} with {} members",
            group_id,
            group.members.len()
        );
        Ok(group)
    }

    pub async fn add_member(&self, group_id: GroupId, device_id: DeviceId) -> Result<()> {
        let mut group = self
            .groups
            .get_mut(&group_id)
            .ok_or_else(|| XLinkError::group_not_found(group_id.to_string(), file!()))?;

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_secs();

        // 注册新成员公钥到 TreeKEM (如果已有)
        if self.treekem_engine.get_device_public_key(device_id).is_ok() {
            self.treekem_engine.add_member(group_id, device_id)?;
        } else {
            use rand::rngs::OsRng;
            use x25519_dalek::StaticSecret;
            let secret = StaticSecret::random_from_rng(OsRng);
            let public = PublicKey::from(&secret);
            self.treekem_engine.register_device_key(device_id, public);
            self.treekem_engine.add_member(group_id, device_id)?;
        }

        group.members.insert(
            device_id,
            GroupMember {
                device_id,
                role: MemberRole::Member,
                joined_at: now,
                last_seen: now,
                status: MemberStatus::Online,
            },
        );

        log::info!("Added member {} to group {}", device_id, group_id);
        Ok(())
    }

    pub async fn join_group(&self, group: Group) -> Result<()> {
        let group_id = group.id;

        // 检查是否已存在
        if self.groups.contains_key(&group_id) {
            return Err(XLinkError::group_already_exists(
                group_id.to_string(),
                file!(),
            ));
        }

        // 初始化 TreeKEM 群组成员密钥
        let member_keys: Vec<_> = group
            .members
            .keys()
            .filter_map(
                |&device_id| match self.treekem_engine.get_device_public_key(device_id) {
                    Ok(key) => Some((device_id, key)),
                    Err(_) => None,
                },
            )
            .collect();

        if member_keys.is_empty() {
            return Err(XLinkError::invalid_input(
                "member_keys",
                "No valid member keys found for joining group",
                file!(),
            ));
        }

        // Add each member to the TreeKEM group
        for (device_id, _public_key) in member_keys {
            self.treekem_engine.add_member(group_id, device_id)?;
        }

        self.groups.insert(group_id, group.clone());

        log::info!(
            "Joined group {} with {} members",
            group_id,
            group.members.len()
        );
        Ok(())
    }

    pub async fn leave_group(&self, group_id: GroupId) -> Result<()> {
        // 从 TreeKEM 群组中移除
        self.treekem_engine
            .remove_member(group_id, self.local_device_id)?;

        // 从本地群组列表中移除
        self.groups.remove(&group_id);

        log::info!("Left group {}", group_id);
        Ok(())
    }

    pub async fn get_group(&self, group_id: GroupId) -> Option<Group> {
        self.groups.get(&group_id).map(|g| g.clone())
    }

    /// 清理所有群组信息，防止内存泄漏 - use proper entry removal to avoid DashMap fragmentation
    pub fn clear_groups(&self) {
        let group_keys: Vec<_> = self.groups.iter().map(|entry| *entry.key()).collect();
        for group_id in group_keys {
            self.groups.remove(&group_id);
        }

        let pending_keys: Vec<_> = self.pending_acks.iter().map(|entry| *entry.key()).collect();
        for key in pending_keys {
            self.pending_acks.remove(&key);
        }

        let invite_keys: Vec<_> = self
            .processed_invites
            .iter()
            .map(|entry| *entry.key())
            .collect();
        for key in invite_keys {
            self.processed_invites.remove(&key);
        }

        // TreeKemEngine 可能也需要清理
        self.treekem_engine.clear_keys();

        // 清理广播结果通知通道
        if let Ok(mut results) = self.broadcast_results.try_write() {
            results.clear();
        }
        log::info!(
            "GroupManager: Cleared all groups and related data structures using entry removal"
        );
    }

    pub async fn broadcast(&self, group_id: GroupId, payload: MessagePayload) -> Result<Uuid> {
        let group = self
            .groups
            .get(&group_id)
            .ok_or_else(|| XLinkError::group_not_found(group_id.to_string(), file!()))?;

        let message_id = Uuid::new_v4();
        let mut successful_devices = HashSet::new();
        let mut failed_devices = HashSet::new();

        // 使用 TreeKEM 加密消息
        let encrypted_payload = match self
            .treekem_engine
            .encrypt_group_message(group_id, &payload)
        {
            Ok(encrypted) => encrypted,
            Err(e) => {
                log::error!("Failed to encrypt group message: {}", e);
                return Err(XLinkError::encryption_failed(
                    "TreeKEM",
                    &e.to_string(),
                    file!(),
                ));
            }
        };

        // 获取当前在线成员并分类（近场 vs 远程 vs 需要中继）
        let mut nearby_members = Vec::new(); // 近场设备（BLE/WiFi Direct）
        let mut remote_members = Vec::new(); // 远程设备（Internet）
        let mut relay_candidates = Vec::new(); // 可作为中继的设备

        for &member_id in group.members.keys() {
            if member_id == self.local_device_id {
                continue;
            }

            // 基于距离和网络类型智能分类
            // 在实际实现中，这里会使用更复杂的启发式算法
            match self.classify_member_proximity(member_id).await {
                ProximityType::Nearby => nearby_members.push(member_id),
                ProximityType::Remote => remote_members.push(member_id),
                ProximityType::RelayCandidate => {
                    relay_candidates.push(member_id);
                    // 中继候选者也可能需要接收消息
                    nearby_members.push(member_id);
                }
            }
        }

        // IT-GRP-003: 混合拓扑广播 - 根据设备距离选择不同的通信通道
        // 近场设备通过 BLE/WiFi 直连，远程设备通过 ntfy 服务器

        // 并行发送消息给所有群组成员
        let mut futures = FuturesUnordered::new();
        let router_clone = self.router.clone();
        let local_device_id = self.local_device_id;

        // 合并所有成员到一个列表中处理
        let all_members: Vec<(DeviceId, bool)> = nearby_members
            .into_iter()
            .map(|id| (id, true)) // true 表示近场设备
            .chain(remote_members.into_iter().map(|id| (id, false))) // false 表示远程设备
            .collect();

        for (member_id, is_nearby) in all_members {
            let router = router_clone.clone();
            let encrypted_payload = encrypted_payload.clone();

            futures.push(async move {
                let priority = if is_nearby {
                    MessagePriority::High
                } else {
                    MessagePriority::Normal
                };
                let require_ack = !is_nearby; // 远程设备需要ACK确认

                let message = Message {
                    id: message_id,
                    sender: local_device_id,
                    recipient: member_id,
                    group_id: Some(group_id),
                    payload: encrypted_payload.clone(),
                    timestamp: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_else(|_| Duration::from_secs(0))
                        .as_secs(),
                    priority,
                    require_ack,
                };

                // 选择通道并发送消息
                match router.select_channel(&message).await {
                    Ok(channel) => {
                        let channel_type = channel.channel_type();
                        let is_nearby_channel = matches!(
                            channel_type,
                            crate::core::types::ChannelType::BluetoothLE
                                | crate::core::types::ChannelType::BluetoothMesh
                                | crate::core::types::ChannelType::WiFiDirect
                        );

                        let can_relay = is_nearby_channel; // 只有近场通道可以作为中继

                        match channel.send(message).await {
                            Ok(_) => {
                                if is_nearby {
                                    log::debug!(
                                        "[Nearby] Message {} sent to device {} via {:?}",
                                        message_id,
                                        member_id,
                                        channel_type
                                    );
                                } else {
                                    log::debug!(
                                        "[Remote] Message {} sent to device {} via {:?}",
                                        message_id,
                                        member_id,
                                        channel_type
                                    );
                                }
                                Ok((member_id, can_relay))
                            }
                            Err(e) => {
                                if is_nearby {
                                    log::warn!(
                                        "[Nearby] Failed to send message {} to device {}: {}",
                                        message_id,
                                        member_id,
                                        e
                                    );
                                } else {
                                    log::warn!(
                                        "[Remote] Failed to send message {} to device {}: {}",
                                        message_id,
                                        member_id,
                                        e
                                    );
                                }
                                Err((member_id, e))
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to select channel for message {} to device {}: {}",
                            message_id,
                            member_id,
                            e
                        );
                        Err((member_id, e))
                    }
                }
            });
        }

        // 等待发送完成，并收集可作为中继节点的成员
        let mut available_relays = Vec::new(); // 可用的中继节点
        while let Some(result) = futures.next().await {
            match result {
                Ok((device_id, can_relay)) => {
                    successful_devices.insert(device_id);
                    if can_relay {
                        available_relays.push(device_id);
                    }
                }
                Err((device_id, _)) => {
                    failed_devices.insert(device_id);
                }
            };
        }

        // --- F4: Mesh 中继模式实现 ---
        // 如果有成员发送失败，且我们有可用的中继候选者，尝试请求中继
        if !failed_devices.is_empty() && !available_relays.is_empty() {
            log::info!(
                "Attempting Mesh relay for {} failed devices via {} candidates",
                failed_devices.len(),
                available_relays.len()
            );

            // 模拟 Mesh 中继逻辑：请求已成功的节点转发消息
            // 在真实实现中，这需要定义一种新的 RelayRequest 消息类型
            for &failed_id in &failed_devices {
                // 简化模拟：假设第一个中继候选者能帮我们触达
                if let Some(&relay_id) = available_relays.first() {
                    log::info!(
                        "[Mesh Relay] Requesting device {} to relay message {} to {}",
                        relay_id,
                        message_id,
                        failed_id
                    );
                    // 实际中这里会调用 router.send(RelayMessage { target: failed_id, content: ... })
                }
            }
        }

        // 记录ACK追踪信息
        if !successful_devices.is_empty() {
            self.pending_acks.insert(
                message_id,
                (successful_devices.clone(), HashSet::new(), HashSet::new()),
            );

            // 启动ACK超时任务
            let pending_acks = self.pending_acks.clone();
            let ack_timeout = self.ack_timeout;

            tokio::spawn(async move {
                tokio::time::sleep(ack_timeout).await;
                pending_acks.remove(&message_id);
                log::debug!("ACK timeout for message {}", message_id);
            });
        }

        let total_attempts = successful_devices.len() + failed_devices.len();

        log::info!(
            "Broadcast message {} to group {}: {} successful, {} failed out of {} attempts",
            message_id,
            group_id,
            successful_devices.len(),
            failed_devices.len(),
            total_attempts
        );

        Ok(message_id)
    }

    /// 标记设备为成功接收（收到ACK）
    pub async fn mark_device_success(&self, msg_id: Uuid, device_id: DeviceId) {
        if let Some(mut entry) = self.pending_acks.get_mut(&msg_id) {
            let (ref mut pending, ref mut success, ref mut _failure) = *entry;

            if pending.remove(&device_id) {
                success.insert(device_id);
                log::debug!("Device {} acknowledged message {}", device_id, msg_id);
            }
        }
    }

    /// 标记设备为失败（无法送达或超时）
    pub async fn mark_device_failed(&self, msg_id: Uuid, device_id: DeviceId) {
        if let Some(mut entry) = self.pending_acks.get_mut(&msg_id) {
            let (ref mut pending, ref mut _success, ref mut failure) = *entry;

            if pending.remove(&device_id) {
                failure.insert(device_id);
                log::warn!(
                    "Device {} marked as failed for message {}",
                    device_id,
                    msg_id
                );
            }
        }
    }

    /// 获取消息ACK状态
    pub async fn get_ack_status(&self, msg_id: Uuid) -> Option<(usize, usize, usize)> {
        self.pending_acks.get(&msg_id).map(|entry| {
            let (pending, success, failure) = &*entry;
            (pending.len(), success.len(), failure.len())
        })
    }

    /// 执行群组密钥更新（前向保密性）
    pub async fn rotate_group_key(&self, group_id: GroupId) -> Result<()> {
        match self
            .treekem_engine
            .update_group_key(group_id, self.local_device_id)
        {
            Ok(update_path) => {
                log::info!(
                    "Group key rotated for group {} at epoch {}",
                    group_id,
                    update_path.epoch
                );

                // 广播密钥更新消息给所有群组成员
                let update_path_bytes: Vec<u8> = update_path
                    .path_public_keys
                    .iter()
                    .flat_map(|pk| pk.clone())
                    .collect();

                let update_payload = MessagePayload::GroupKeyUpdate {
                    group_id,
                    epoch: update_path.epoch,
                    update_path: update_path_bytes,
                };

                self.broadcast(group_id, update_payload).await?;
                Ok(())
            }
            Err(e) => {
                log::error!("Failed to rotate group key for group {}: {}", group_id, e);
                Err(XLinkError::key_derivation_failed(
                    "TreeKEM key rotation",
                    &e.to_string(),
                    file!(),
                ))
            }
        }
    }

    pub fn encrypt_group_message(
        &self,
        group_id: GroupId,
        payload: &MessagePayload,
    ) -> Result<MessagePayload> {
        self.treekem_engine
            .encrypt_group_message(group_id, payload)
            .map_err(|e| XLinkError::encryption_failed("TreeKEM", &e.to_string(), file!()))
    }

    pub fn decrypt_group_message(
        &self,
        group_id: GroupId,
        encrypted_payload: &MessagePayload,
    ) -> Result<MessagePayload> {
        self.treekem_engine
            .decrypt_group_message(group_id, encrypted_payload)
            .map_err(|e| XLinkError::encryption_failed("TreeKEM", &e.to_string(), file!()))
    }

    /// 处理群组密钥更新
    pub async fn handle_key_update(
        &self,
        group_id: GroupId,
        epoch: u64,
        _update_path: Vec<u8>,
    ) -> Result<()> {
        // TreeKEM引擎没有apply_group_key_update方法，使用apply_update_path
        let update_path_struct = UpdatePath {
            updater_id: self.local_device_id,
            path_secrets: vec![],     // 这里需要根据update_path重构
            path_public_keys: vec![], // 这里需要根据update_path重构
            epoch,
        };
        match self
            .treekem_engine
            .apply_update_path(group_id, &update_path_struct)
        {
            Ok(_) => {
                log::info!(
                    "Applied group key update for group {} at epoch {}",
                    group_id,
                    epoch
                );
                Ok(())
            }
            Err(e) => {
                log::error!(
                    "Failed to apply group key update for group {}: {}",
                    group_id,
                    e
                );
                Err(XLinkError::key_derivation_failed(
                    "TreeKEM key update",
                    &e.to_string(),
                    file!(),
                ))
            }
        }
    }

    /// 处理ACK消息
    pub async fn handle_ack(&self, original_msg_id: Uuid, responder: DeviceId) {
        let mut completed = false;
        let mut stats = None;

        if let Some(mut entry) = self.pending_acks.get_mut(&original_msg_id) {
            let (ref mut pending, ref mut success, ref mut _failure) = *entry;

            if pending.remove(&responder) {
                success.insert(responder);
                log::debug!(
                    "Device {} acknowledged message {}",
                    responder,
                    original_msg_id
                );
            }

            if pending.is_empty() {
                completed = true;
                stats = Some((success.clone(), _failure.clone()));
            }
        }

        if completed {
            if let Some((success_set, failure_set)) = stats {
                let success_count = success_set.len();
                let failure_count = failure_set.len();

                log::info!(
                    "Message {} ACK complete: {} successful, {} failed",
                    original_msg_id,
                    success_count,
                    failure_count
                );

                // 清理ACK追踪
                self.pending_acks.remove(&original_msg_id);

                // 发送广播结果通知并清理结果通道
                if let Some(tx) = self
                    .broadcast_results
                    .write()
                    .await
                    .remove(&original_msg_id)
                {
                    let _ = tx
                        .send(BroadcastResult {
                            message_id: original_msg_id,
                            successful_devices: success_set,
                            failed_devices: failure_set,
                            total_attempts: success_count + failure_count,
                        })
                        .await;
                }
            }
        }
    }

    /// 清理过期的广播结果通道，防止内存泄漏
    pub async fn cleanup_expired_broadcast_results(&self) {
        let results = self.broadcast_results.read().await;
        let count = results.len();
        drop(results);

        if count > 0 {
            let mut results = self.broadcast_results.write().await;
            results.clear();
            log::debug!("Cleaned up {} broadcast result channels", count);
        }
    }

    /// 清理过期的邀请记录，防止内存泄漏
    pub fn cleanup_expired_invites(&self, max_age_hours: u64) {
        let current_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|_| {
                log::warn!("SystemTime before UNIX_EPOCH, using 0");
            })
            .unwrap_or(std::time::Duration::ZERO)
            .as_secs();
        let max_age_seconds = max_age_hours * 3600;

        let mut removed_count = 0;
        self.processed_invites.retain(|_, &mut timestamp| {
            let should_retain = current_time.saturating_sub(timestamp) < max_age_seconds;
            if !should_retain {
                removed_count += 1;
            }
            should_retain
        });

        if removed_count > 0 {
            log::debug!("Cleaned up {} expired invite records", removed_count);
        }
    }

    /// 更新群组成员状态
    pub async fn update_member_state(
        &self,
        group_id: GroupId,
        device_id: DeviceId,
        status: MemberStatus,
    ) -> Result<()> {
        if let Some(mut group) = self.groups.get_mut(&group_id) {
            if let Some(member) = group.members.get_mut(&device_id) {
                // 更新成员状态
                match status {
                    MemberStatus::Online => {
                        member.status = MemberStatus::Online;
                        member.last_seen = SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                    }
                    MemberStatus::Offline => {
                        member.status = MemberStatus::Offline;
                    }
                    MemberStatus::Away => {
                        member.status = MemberStatus::Away;
                    }
                    MemberStatus::Busy => {
                        member.status = MemberStatus::Busy;
                    }
                }

                // 更新成员的最后活跃时间、状态等信息
                log::info!(
                    "Updated member {} state in group {}: {:?}",
                    device_id,
                    group_id,
                    status
                );
            }
        }

        // 触发状态同步事件
        self.notify_group_state_change(group_id).await;
        Ok(())
    }

    /// 通知群组状态变更
    pub async fn notify_group_state_change(&self, group_id: GroupId) {
        log::info!("Group {} state changed", group_id);
        // TODO: 实现状态变更通知逻辑
        // 这里可以发送通知给其他成员，或者触发事件监听器
    }

    /// 大规模群组性能优化 - 分层广播
    pub async fn broadcast_large_group(
        &self,
        group_id: GroupId,
        payload: MessagePayload,
    ) -> Result<Vec<Uuid>> {
        const MAX_SUBGROUP_SIZE: usize = 50; // 每个子群组最大成员数

        let group = self
            .groups
            .get(&group_id)
            .ok_or_else(|| XLinkError::group_not_found(group_id.to_string(), file!()))?;

        if group.members.len() <= MAX_SUBGROUP_SIZE {
            // 小群组直接广播
            let msg_id = self.broadcast(group_id, payload).await?;
            return Ok(vec![msg_id]);
        }

        // 大群组分层广播
        let member_ids: Vec<DeviceId> = group
            .members
            .keys()
            .copied()
            .filter(|&id| id != self.local_device_id)
            .collect();

        let mut message_ids = Vec::new();

        // 将成员分成子群组
        for chunk in member_ids.chunks(MAX_SUBGROUP_SIZE) {
            let sub_group_id = GroupId::new(); // 创建临时子群组ID
            let mut sub_group_members = HashMap::new();

            for &member_id in chunk {
                sub_group_members.insert(
                    member_id,
                    GroupMember {
                        device_id: member_id,
                        role: MemberRole::Member,
                        joined_at: SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                        last_seen: SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                        status: MemberStatus::Online,
                    },
                );
            }

            // 创建临时子群组
            let sub_group = Group {
                id: sub_group_id,
                name: format!("{}_sub_{}", group.name, sub_group_id),
                members: sub_group_members,
                created_at: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            };

            self.groups.insert(sub_group_id, sub_group);

            // 在子群组中广播消息
            let sub_msg_id = self.broadcast(sub_group_id, payload.clone()).await?;
            message_ids.push(sub_msg_id);

            // 延迟清理临时子群组（给消息发送留时间）
            let groups = self.groups.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(60)).await;
                groups.remove(&sub_group_id);
            });
        }

        Ok(message_ids)
    }

    pub async fn handle_incoming_group_message(&self, message: &Message) -> Result<()> {
        log::info!("Handling group message: {:?}", message.id);
        if let Some(group_id) = message.group_id {
            // 首先尝试解密消息
            let decrypted_payload = match self
                .treekem_engine
                .decrypt_group_message(group_id, &message.payload)
            {
                Ok(decrypted) => decrypted,
                Err(e) => {
                    log::warn!(
                        "Failed to decrypt group message from {} in group {}: {}",
                        message.sender,
                        group_id,
                        e
                    );
                    // 尝试反序列化原始payload
                    match &message.payload {
                        MessagePayload::Binary(data) => {
                            match serde_json::from_slice::<MessagePayload>(data) {
                                Ok(payload) => payload,
                                Err(_) => message.payload.clone(), // 回退到原始payload
                            }
                        }
                        _ => message.payload.clone(), // 回退到原始payload
                    }
                }
            };

            match &decrypted_payload {
                MessagePayload::GroupInvite { name, .. } => {
                    if !self.groups.contains_key(&group_id) {
                        // 简单去重
                        if self.processed_invites.contains_key(&group_id) {
                            return Ok(());
                        }
                        self.processed_invites.insert(group_id, message.timestamp);

                        let now = SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        let mut members = HashMap::new();
                        members.insert(
                            message.sender,
                            GroupMember {
                                device_id: message.sender,
                                role: MemberRole::Admin,
                                joined_at: now,
                                last_seen: now,
                                status: MemberStatus::Online,
                            },
                        );
                        members.insert(
                            self.local_device_id,
                            GroupMember {
                                device_id: self.local_device_id,
                                role: MemberRole::Member,
                                joined_at: now,
                                last_seen: now,
                                status: MemberStatus::Online,
                            },
                        );

                        let group = Group {
                            id: group_id,
                            name: name.clone(),
                            members,
                            created_at: now,
                        };
                        self.groups.insert(group_id, group);

                        // 初始化 TreeKEM 群组密钥
                        if self
                            .treekem_engine
                            .get_device_public_key(message.sender)
                            .is_ok()
                        {
                            let member_ids = vec![message.sender];
                            if let Err(e) = self.treekem_engine.create_group(group_id, member_ids) {
                                log::warn!(
                                    "Failed to initialize TreeKEM for invited group {}: {}",
                                    group_id,
                                    e
                                );
                            }
                        } else {
                            log::warn!("Failed to get public key for device {} when initializing TreeKEM for group {}", message.sender, group_id);
                        }
                    }
                }
                MessagePayload::GroupAck {
                    original_msg_id,
                    responder,
                } => {
                    // 处理群组ACK消息
                    self.handle_ack(*original_msg_id, *responder).await;
                }
                MessagePayload::GroupKeyUpdate {
                    group_id,
                    epoch,
                    update_path,
                } => {
                    // 处理群组密钥更新消息
                    if let Err(e) = self
                        .handle_key_update(*group_id, *epoch, update_path.clone())
                        .await
                    {
                        log::error!("Failed to handle key update for group {}: {}", group_id, e);
                    }
                }
                _ => {
                    // 更新发送者的最后活跃时间
                    if let Some(mut group) = self.groups.get_mut(&group_id) {
                        if let Some(member) = group.members.get_mut(&message.sender) {
                            member.last_seen = SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap()
                                .as_secs();
                            member.status = MemberStatus::Online;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
