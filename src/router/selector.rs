use crate::capability::manager::CapabilityManager;
use crate::core::error::{Result, XPushError};
use crate::core::traits::Channel;
use crate::core::types::{ChannelType, DeviceId, Message, MessagePayload};
use crate::router::scoring::Scorer;
use std::collections::HashMap;
use std::sync::Arc;

use std::sync::Mutex;

pub struct Router {
    channels: HashMap<ChannelType, Arc<dyn Channel>>,
    cap_manager: Arc<CapabilityManager>,
    traffic_stats: Mutex<HashMap<ChannelType, u64>>,
    route_history: Mutex<HashMap<DeviceId, Vec<ChannelType>>>,
    traffic_thresholds: HashMap<ChannelType, u64>,
}

impl Router {
    pub fn new(
        channels: HashMap<ChannelType, Arc<dyn Channel>>,
        cap_manager: Arc<CapabilityManager>,
    ) -> Self {
        Self {
            channels,
            cap_manager,
            traffic_stats: Mutex::new(HashMap::new()),
            route_history: Mutex::new(HashMap::new()),
            traffic_thresholds: HashMap::new(),
        }
    }

    pub fn with_thresholds(mut self, thresholds: HashMap<ChannelType, u64>) -> Self {
        self.traffic_thresholds = thresholds;
        self
    }

    pub fn get_channels(&self) -> &HashMap<ChannelType, Arc<dyn Channel>> {
        &self.channels
    }

    /// 获取通道累计流量（字节）
    pub fn get_traffic_stats(&self) -> HashMap<ChannelType, u64> {
        self.traffic_stats.lock().unwrap().clone()
    }

    /// 记录流量
    fn record_traffic(&self, ctype: ChannelType, bytes: u64) {
        let mut stats = self.traffic_stats.lock().unwrap();
        let current = stats.entry(ctype).or_insert(0);
        *current += bytes;

        // F10: 流量预警 - 检查是否超过阈值
        if let Some(&threshold) = self.traffic_thresholds.get(&ctype) {
            if *current >= threshold {
                log::warn!(
                    "Traffic threshold exceeded for channel {:?}: current={}, threshold={}",
                    ctype,
                    current,
                    threshold
                );
                // 实际生产中这里可能会触发事件或回调
            }
        }
    }

    /// 记录路由历史
    fn record_history(&self, target: DeviceId, ctype: ChannelType) {
        let mut history = self.route_history.lock().unwrap();
        let entries = history.entry(target).or_default();
        entries.push(ctype);
        if entries.len() > 10 {
            entries.remove(0);
        }
    }

    /// 基于历史预测最佳通道
    fn predict_best_channel(&self, target: &DeviceId) -> Option<ChannelType> {
        let history = self.route_history.lock().unwrap();
        let entries = history.get(target)?;
        if entries.is_empty() {
            return None;
        }

        // 简单的频率统计预测
        let mut counts = HashMap::new();
        for &ctype in entries {
            *counts.entry(ctype).or_insert(0) += 1;
        }

        counts
            .into_iter()
            .max_by_key(|&(_, count)| count)
            .map(|(ctype, _)| ctype)
    }

    pub async fn select_channel(&self, message: &Message) -> Result<Arc<dyn Channel>> {
        let target = &message.recipient;
        let local_caps = self.cap_manager.get_local_caps();

        let mut best_score = -1.0;
        let mut best_channel_type = None;

        // F7: 预测性路由 - 检查历史记录
        if let Some(predicted_ctype) = self.predict_best_channel(target) {
            if let Some(state) = self.cap_manager.get_channel_state(target, &predicted_ctype) {
                if state.available {
                    // 如果预测的通道当前可用，则优先考虑
                    let score =
                        Scorer::score(predicted_ctype, &state, &local_caps, message.priority);
                    if score > 0.6 {
                        // 只要分数尚可，就直接使用，减少计算开销
                        best_score = score;
                        best_channel_type = Some(predicted_ctype);
                    }
                }
            }
        }

        if best_channel_type.is_none() {
            // Iterate over all registered channels
            for ctype in self.channels.keys() {
                // Check if we have state info for this target on this channel
                if let Some(state) = self.cap_manager.get_channel_state(target, ctype) {
                    let score = Scorer::score(*ctype, &state, &local_caps, message.priority);

                    log::debug!("Channel {:?} score: {:.4}", ctype, score);

                    if score > best_score && score > 0.0 {
                        best_score = score;
                        best_channel_type = Some(*ctype);
                    }
                }
            }
        }

        if let Some(ctype) = best_channel_type {
            let channel = self.channels.get(&ctype).unwrap().clone();

            // 记录消息预计流量
            let bytes = match &message.payload {
                MessagePayload::Text(t) => t.len() as u64,
                MessagePayload::Binary(b) => b.len() as u64,
                MessagePayload::StreamChunk { data, .. } => data.len() as u64,
                MessagePayload::StreamFrame { data, .. } => data.len() as u64,
                MessagePayload::GroupKeyUpdate { update_path, .. } => update_path.len() as u64,
                _ => 64,
            };
            self.record_traffic(ctype, bytes);

            // 记录历史
            self.record_history(*target, ctype);

            Ok(channel)
        } else {
            Err(XPushError::NoRouteFound)
        }
    }

    /// 清理路由器中的数据，防止内存泄漏
    pub async fn clear_channels(&self) {
        // 清理流量统计
        self.traffic_stats.lock().unwrap().clear();
        // 清理路由历史
        self.route_history.lock().unwrap().clear();
        log::debug!("Router: Cleared traffic stats and route history");
    }

    /// 同步清理路由器中的数据，防止内存泄漏（用于Drop）
    pub fn clear_channels_sync(&self) {
        // 清理流量统计
        self.traffic_stats.lock().unwrap().clear();
        // 清理路由历史
        self.route_history.lock().unwrap().clear();
        // 清理通道映射（需要获取可变引用，这在Drop中不可行，所以暂时跳过）
        // self.channels.clear(); // 无法在Drop中调用，因为需要&mut self
        log::debug!("Router: Synchronously cleared traffic stats and route history");
    }
}
