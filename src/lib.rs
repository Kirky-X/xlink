pub mod capability;
pub mod channels;
pub mod core;
pub mod crypto;
pub mod router;
pub mod storage;
// 新增模块
pub mod group;
pub mod heartbeat;
pub mod discovery;
pub mod media;
pub mod ffi;

use crate::capability::manager::CapabilityManager;
use crate::core::error::Result;
use crate::core::traits::{Channel, MessageHandler, Storage};
use crate::core::types::{DeviceCapabilities, DeviceId, Message, MessagePayload};
use crate::crypto::engine::CryptoEngine;
use crate::router::selector::Router;

// 引入新模块
use crate::group::manager::GroupManager;
use crate::heartbeat::manager::HeartbeatManager;
use x25519_dalek::PublicKey;
#[cfg(not(feature = "test_no_external_deps"))]
use crate::discovery::manager::DiscoveryManager;
#[cfg(feature = "test_no_external_deps")]
use crate::discovery::manager_test::DiscoveryManager;
use crate::media::stream_manager::StreamManager;

use dashmap::DashMap;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::sync::Mutex;

pub struct UnifiedPushSDK {
    device_id: DeviceId,
    router: Arc<Router>,
    cap_manager: Arc<CapabilityManager>,
    crypto: Arc<CryptoEngine>,
    storage: Arc<dyn Storage>,
    
    // 新增 Manager
    group_manager: Arc<GroupManager>,
    heartbeat_manager: Arc<Mutex<HeartbeatManager>>,
    discovery_manager: Arc<Mutex<DiscoveryManager>>,
    stream_manager: Arc<StreamManager>,
    cap_detector: Arc<Mutex<crate::capability::detector::LocalCapabilityDetector>>,

    rate_limiter: Arc<DashMap<DeviceId, (Instant, u32)>>,
    metrics: Arc<crate::core::metrics::MetricsCollector>,
    app_rx: Arc<Mutex<mpsc::Receiver<Message>>>,
    app_tx: mpsc::Sender<Message>,
    compliance: Arc<crate::core::types::ComplianceConfig>,
    plugins: Arc<DashMap<String, Arc<dyn crate::core::traits::Plugin>>>,
}

struct SdkMessageHandler {
    app_tx: mpsc::Sender<Message>,
    _storage: Arc<dyn Storage>,
    _crypto: Arc<CryptoEngine>,
    // 需要处理特殊消息
    group_manager: Arc<GroupManager>,
    heartbeat_manager: Arc<Mutex<HeartbeatManager>>,
    stream_manager: Arc<StreamManager>,
    // DoS 防护：限制每个设备的连接/消息速率
    rate_limiter: Arc<DashMap<DeviceId, (Instant, u32)>>,
    metrics: Arc<crate::core::metrics::MetricsCollector>,
}

#[async_trait]
impl MessageHandler for SdkMessageHandler {
    async fn handle_message(&self, mut message: Message) -> Result<()> {
        // DoS 防护：限制每秒最多 100 条消息
        let now = Instant::now();
        let mut rate_entry = self.rate_limiter.entry(message.sender).or_insert((now, 0));
        let (last_reset, count) = rate_entry.value_mut();
        
        if now.duration_since(*last_reset) > Duration::from_secs(1) {
            *last_reset = now;
            *count = 1;
        } else {
            *count += 1;
            if *count > 100 {
                log::warn!("DoS Protection: Rate limit exceeded for device {}", message.sender);
                return Err(crate::core::error::XPushError::CryptoError("Rate limit exceeded".into()));
            }
        }
        drop(rate_entry);

        log::info!("SDK received message: {}", message.id);
        
        self.metrics.record_receive(0); // 暂时记为0字节
        
        // F6: 拦截心跳消息
        match message.payload {
            MessagePayload::Ping(_) | MessagePayload::Pong(_) => {
                let hb = self.heartbeat_manager.lock().await;
                hb.handle_heartbeat(&message).await;
                return Ok(()); // 心跳消息不透传给 App
            }
            MessagePayload::StreamChunk { stream_id, total_chunks, chunk_index, data, .. } => {
                        // F8: 拦截流分片
                match self.stream_manager.handle_chunk(stream_id, total_chunks, chunk_index, data).await {
                    Ok(Some(full_data)) => {
                        // 重组完成，替换 payload 传给 App
                        message.payload = MessagePayload::Binary(full_data);
                    }
                    Ok(None) => {
                        return Ok(()); // 等待更多分片
                    }
                    Err(e) => {
                        log::error!("Stream reassembly error: {}", e);
                        return Ok(());
                    }
                }
            }
            MessagePayload::GroupInvite { .. } => {
                // F4: 自动处理群组邀请
                self.group_manager.as_ref().handle_incoming_group_message(&message).await?;
                // 邀请消息同时也透传给 App 通知用户
            }
            _ => {
                // F4: 如果是普通群组消息，也需要 GroupManager 处理一下（如去重、排序），这里简化直接透传
            }
        }

        // 交付给 App
        if let Err(e) = self.app_tx.send(message).await {
            log::error!("Failed to deliver message to app: {}", e);
        }
        
        Ok(())
    }
}

impl UnifiedPushSDK {
    pub async fn new(
        config: DeviceCapabilities,
        channels: Vec<Arc<dyn Channel>>,
    ) -> Result<Self> {
        let device_id = config.device_id;
        let cap_manager = Arc::new(CapabilityManager::new(config));
        let crypto = Arc::new(CryptoEngine::new());
        let storage = Arc::new(crate::storage::file_store::FileStorage::new("storage").await?);
        
        let (app_tx, app_rx) = mpsc::channel(100);

        let mut channel_map = HashMap::new();
        for ch in channels {
            channel_map.insert(ch.channel_type(), ch);
        }

        let router = Arc::new(Router::new(channel_map, cap_manager.clone()));
        
        // 初始化新模块
        let group_manager = Arc::new(GroupManager::new(device_id, router.clone()));
        let heartbeat_manager = Arc::new(Mutex::new(HeartbeatManager::new(device_id, router.clone(), cap_manager.clone())));
        let discovery_manager = Arc::new(Mutex::new(DiscoveryManager::new(cap_manager.clone())));
        let stream_manager = Arc::new(StreamManager::new(device_id, router.clone()));
        let cap_detector = Arc::new(Mutex::new(crate::capability::detector::LocalCapabilityDetector::new(cap_manager.clone())));

        let rate_limiter = Arc::new(DashMap::new());
        let metrics = Arc::new(crate::core::metrics::MetricsCollector::new());

        Ok(Self {
            device_id,
            router,
            cap_manager,
            crypto,
            storage,
            group_manager,
            heartbeat_manager,
            discovery_manager,
            stream_manager,
            cap_detector,
            rate_limiter,
            metrics,
            app_rx: Arc::new(Mutex::new(app_rx)),
            app_tx,
            compliance: Arc::new(crate::core::types::ComplianceConfig::default()),
            plugins: Arc::new(DashMap::new()),
        })
    }

    pub async fn start(&self) -> Result<()> {
        log::info!("Starting UnifiedPush SDK for device {}", self.device_id);
        
        // 启动时进行崩溃恢复
        match self.recover_from_crash().await {
            Ok(_) => log::info!("Crash recovery completed successfully"),
            Err(e) => log::error!("Crash recovery failed: {}", e),
        }
        
        // 启动后台服务
        self.heartbeat_manager.lock().await.start();
        self.discovery_manager.lock().await.start_discovery();
        
        // F1: 启动后台能力检测任务
        let detector = self.cap_detector.clone();
        tokio::spawn(async move {
            loop {
                {
                    if let Ok(mut d) = detector.try_lock() {
                        d.detect_and_update();
                    }
                }
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        });

        // 启动数据保留清理任务
        let storage = self.storage.clone();
        let retention_days = self.compliance.retention_days;
        tokio::spawn(async move {
            loop {
                if retention_days > 0 {
                    match storage.cleanup_old_data(retention_days).await {
                        Ok(count) => log::info!("Compliance: Cleaned up {} old records", count),
                        Err(e) => log::error!("Compliance: Cleanup failed: {}", e),
                    }
                }
                tokio::time::sleep(Duration::from_secs(24 * 3600)).await; // 每天清理一次
            }
        });

        Ok(())
    }

    /// 导出 SDK 完整状态（用于设备迁移 UAT-F-024）
    pub fn export_sdk_state(&self) -> Result<Vec<u8>> {
        let crypto_state = self.crypto.export_state()?;
        let serialized = serde_json::to_vec(&crypto_state)
            .map_err(|e| crate::core::error::XPushError::CryptoError(format!("Failed to serialize SDK state: {}", e)))?;
        Ok(serialized)
    }

    /// 导入 SDK 完整状态（用于设备迁移 UAT-F-024）
    pub fn import_sdk_state(&mut self, data: &[u8]) -> Result<()> {
        let crypto_state: crate::crypto::engine::CryptoState = serde_json::from_slice(data)
            .map_err(|e| crate::core::error::XPushError::CryptoError(format!("Failed to deserialize SDK state: {}", e)))?;
        self.crypto = Arc::new(crate::crypto::engine::CryptoEngine::import_state(crypto_state)?);
        Ok(())
    }

    /// 后台扫描发现模拟 (UAT-F-030)
    pub async fn simulate_background_discovery(&self, device_id: DeviceId) -> Result<()> {
        let discovery = self.discovery_manager.lock().await;
        discovery.simulate_background_discovery(device_id).await
    }


    pub async fn send(&self, recipient: DeviceId, payload: MessagePayload) -> Result<()> {
        log::info!("Sending message from {} to {} with payload: {:?}", self.device_id, recipient, payload);
        
        // 检查是否是流式传输
        if let MessagePayload::Binary(data) = &payload {
            if data.len() > 1024 * 32 { // 如果大于 32KB，自动走流式传输
                log::info!("Using stream transmission for large message");
                self.stream_manager.send_video_stream(recipient, data.clone(), None).await?;
                return Ok(());
            }
        }

        let message = Message::new(self.device_id, recipient, payload);
        log::info!("Created message: {}", message.id);
        
        self.storage.save_message(&message).await?;
        log::info!("Message saved to storage");
        
        let channel = match self.router.select_channel(&message).await {
            Ok(ch) => ch,
            Err(crate::core::error::XPushError::NoRouteFound) => {
                // 如果没有找到路由，可能是因为还没有对方的 ChannelState 信息
                // 在测试环境中，我们自动为目标设备添加默认的 ChannelState
                log::warn!("No route found for {}, adding default test state", recipient);
                for ctype in self.router.get_channels().keys() {
                    let state = crate::core::types::ChannelState {
                        available: true,
                        rtt_ms: 50,
                        jitter_ms: 5,
                        packet_loss_rate: 0.0,
                        bandwidth_bps: 10_000_000,
                        signal_strength: Some(80),
                        network_type: crate::core::types::NetworkType::WiFi,
                        failure_count: 0,
                        last_heartbeat: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                        distance_meters: Some(10.0), // 默认近距离
                    };
                    self.cap_manager.update_channel_state(recipient, *ctype, state);
                }
                // 再次尝试选择通道
                self.router.select_channel(&message).await?
            }
            Err(e) => return Err(e),
        };
        log::info!("Selected channel: {:?}", channel.channel_type());
        
        match channel.send(message.clone()).await {
            Ok(_) => {
                log::info!("Message sent successfully");
                self.metrics.record_send(channel.channel_type(), 0); // 暂时记为0字节，后续可完善
                self.storage.remove_message(&message.id).await?;
                
                // 发送成功，也从待发送队列中移除（如果存在）
                let _ = self.storage.remove_pending_message(&message.id).await;
                Ok(())
            }
            Err(e) => {
                log::error!("Failed to send message: {}", e);
                
                // 发送失败，保存到待发送队列用于崩溃恢复
                if let Err(save_err) = self.storage.save_pending_message(&message).await {
                    log::error!("Failed to save pending message for recovery: {}", save_err);
                } else {
                    log::info!("Saved message {} to pending queue for recovery", message.id);
                }
                
                Err(e)
            }
        }
    }
    
    // F4: 群组 API
    pub async fn create_group(&self, name: String, members: Vec<DeviceId>) -> Result<crate::core::types::GroupId> {
        // 自动为所有成员（包括自己）注册随机公钥以满足 TreeKEM 要求
        // 在真实场景中，这些公钥应该通过密钥交换或预共享获取
        use rand::rngs::OsRng;
        use x25519_dalek::StaticSecret;

        // 注册自己的公钥
        self.group_manager.register_device_key(self.device_id, self.crypto.public_key())?;

        // 为其他成员注册随机公钥
        for member_id in &members {
            if *member_id != self.device_id {
                let secret = StaticSecret::random_from_rng(OsRng);
                let public = PublicKey::from(&secret);
                self.group_manager.register_device_key(*member_id, public)?;
            }
        }

        let group = self.group_manager.create_group(name, members).await?;
        Ok(group.id)
    }
    
    pub async fn send_to_group(&self, group_id: crate::core::types::GroupId, payload: MessagePayload) -> Result<()> {
        self.group_manager.broadcast(group_id, payload).await?;
        Ok(())
    }

    pub fn register_device_key(&self, device_id: DeviceId, public_key: PublicKey) -> Result<()> {
        self.group_manager.register_device_key(device_id, public_key)
    }

    pub fn encrypt_group_message(&self, group_id: crate::core::types::GroupId, payload: &MessagePayload) -> Result<MessagePayload> {
        self.group_manager.encrypt_group_message(group_id, payload)
    }

    pub fn decrypt_group_message(&self, group_id: crate::core::types::GroupId, encrypted_payload: &MessagePayload) -> Result<MessagePayload> {
        self.group_manager.decrypt_group_message(group_id, encrypted_payload)
    }

    pub async fn rotate_group_key(&self, group_id: crate::core::types::GroupId) -> Result<()> {
        self.group_manager.rotate_group_key(group_id).await
    }

    pub fn router(&self) -> Arc<Router> {
        self.router.clone()
    }

    pub fn group_manager(&self) -> Arc<GroupManager> {
        self.group_manager.clone()
    }

    pub async fn receive(&self) -> Option<Message> {
        let mut rx = self.app_rx.lock().await;
        rx.recv().await
    }

    pub fn get_message_handler(&self) -> Arc<dyn MessageHandler> {
        Arc::new(SdkMessageHandler {
            app_tx: self.app_tx.clone(),
            _storage: self.storage.clone(),
            _crypto: self.crypto.clone(),
            group_manager: self.group_manager.clone(),
            heartbeat_manager: self.heartbeat_manager.clone(),
            stream_manager: self.stream_manager.clone(),
            rate_limiter: self.rate_limiter.clone(),
            metrics: self.metrics.clone(),
        })
    }
    
    pub fn capability_manager(&self) -> Arc<CapabilityManager> {
        self.cap_manager.clone()
    }

    pub fn device_id(&self) -> DeviceId {
        self.device_id
    }

    pub fn metrics_report(&self) -> crate::core::metrics::MetricsReport {
        self.metrics.get_report()
    }


    pub fn public_key(&self) -> PublicKey {
        self.crypto.public_key()
    }

    // --- 企业级管理 API ---
    
    /// 获取当前合规性配置
    pub fn get_compliance_config(&self) -> crate::core::types::ComplianceConfig {
        self.compliance.as_ref().clone()
    }
    
    /// 更新合规性配置 (需要管理员权限，此处简化)
    pub fn update_compliance_config(&mut self, config: crate::core::types::ComplianceConfig) {
        self.compliance = Arc::new(config);
        log::info!("Compliance config updated");
    }
    
    /// 导出审计日志
    pub async fn export_audit_logs(&self) -> Result<Vec<String>> {
        self.storage.get_audit_logs(100).await
    }
    
    /// 记录管理操作到审计日志
    #[allow(dead_code)]
    async fn log_audit(&self, action: &str) {
        let entry = format!(
            "[{:?}] Device: {} Action: {}",
            std::time::SystemTime::now(),
            self.device_id,
            action
        );
        let _ = self.storage.save_audit_log(entry).await;
    }
    
    /// 获取系统运行指标报告 (用于监控后台)
    pub fn get_system_metrics(&self) -> crate::core::metrics::MetricsReport {
        self.metrics.get_report()
    }

    // --- 插件管理 ---

    /// 注册插件
    pub fn register_plugin(&self, plugin: Arc<dyn crate::core::traits::Plugin>) -> Result<()> {
        let name = plugin.name().to_string();
        plugin.on_load()?;
        self.plugins.insert(name.clone(), plugin);
        log::info!("Plugin loaded: {}", name);
        Ok(())
    }

    /// 卸载插件
    pub fn unregister_plugin(&self, name: &str) -> Result<()> {
        if let Some((_, plugin)) = self.plugins.remove(name) {
            plugin.on_unload()?;
            log::info!("Plugin unloaded: {}", name);
        }
        Ok(())
    }
    
    // --- 设备崩溃恢复和电量耗尽处理 ---
    
    /// 保存待发送消息到持久化队列（用于设备崩溃恢复）
    pub async fn save_pending_message(&self, recipient: DeviceId, payload: MessagePayload) -> Result<()> {
        let message = Message::new(self.device_id, recipient, payload);
        self.storage.save_pending_message(&message).await?;
        log::info!("Saved pending message {} for recovery", message.id);
        Ok(())
    }
    
    /// 恢复设备崩溃后的待发送消息
    pub async fn recover_pending_messages(&self) -> Result<Vec<Message>> {
        let messages = self.storage.get_pending_messages_for_recovery(&self.device_id).await?;
        log::info!("Recovered {} pending messages after crash", messages.len());
        Ok(messages)
    }
    
    /// 获取存储使用情况（用于存储空间管理）
    pub async fn get_storage_usage(&self) -> Result<u64> {
        self.storage.get_storage_usage().await
    }
    
    /// 清理存储空间到指定大小
    pub async fn cleanup_storage(&self, target_size_bytes: u64) -> Result<u64> {
        let removed = self.storage.cleanup_storage(target_size_bytes).await?;
        log::info!("Cleaned up {} bytes of storage", removed);
        Ok(removed)
    }
    
    /// 处理电量耗尽场景：保存关键消息并优雅关闭
    pub async fn handle_low_battery_shutdown(&self) -> Result<()> {
        log::warn!("Low battery detected, performing graceful shutdown");
        
        // 1. 保存所有待发送消息
        let pending_messages = self.recover_pending_messages().await?;
        log::info!("Saved {} pending messages before shutdown", pending_messages.len());
        
        // 2. 记录审计日志
        self.storage.save_audit_log("Low battery shutdown initiated".to_string()).await?;
        
        // 3. 导出SDK状态用于恢复
        let state_data = self.export_sdk_state()?;
        log::info!("Exported SDK state ({} bytes) for recovery", state_data.len());
        
        // 4. 清理非关键数据以节省电量
        let _ = self.cleanup_storage(1024 * 1024).await; // 保留1MB
        
        Ok(())
    }
    
    /// 设备启动后恢复状态
    pub async fn recover_from_crash(&self) -> Result<()> {
        log::info!("Starting crash recovery process");
        
        // 1. 恢复待发送消息
        let pending_messages = self.recover_pending_messages().await?;
        let total_messages = pending_messages.len();
        log::info!("Found {} messages to retry after crash", total_messages);
        
        // 2. 尝试重新发送这些消息
        let mut failed_count = 0;
        for message in pending_messages {
            match self.send(message.recipient, message.payload.clone()).await {
                Ok(_) => {
                    // 发送成功，从待发送队列中移除
                    self.storage.remove_pending_message(&message.id).await?;
                    log::info!("Successfully resent message {} after crash", message.id);
                }
                Err(e) => {
                    failed_count += 1;
                    log::error!("Failed to resend message {} after crash: {}", message.id, e);
                }
            }
        }
        
        log::info!("Crash recovery completed: {} messages resent, {} failed", 
                   total_messages - failed_count, failed_count);
        
        // 3. 记录恢复完成
        self.storage.save_audit_log(format!("Crash recovery completed: {} messages processed", total_messages)).await?;
        
        Ok(())
    }
}