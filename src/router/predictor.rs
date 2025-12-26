use crate::core::types::{ChannelType, DeviceId};
use chrono::{DateTime, Timelike, Utc};
use dashmap::DashMap;

/// 路由历史记录，用于趋势分析
#[derive(Debug, Clone)]
struct RouteHistory {
    success_count: u32,
    failure_count: u32,
    avg_latency: f64,
    last_used: DateTime<Utc>,
}

/// 预测性路由引擎
/// 基于历史数据和时间模式预测最优通道
pub struct RoutePredictor {
    // Map: (DeviceId, ChannelType) -> RouteHistory
    history: DashMap<(DeviceId, ChannelType), RouteHistory>,
    // 存储时间模式，例如：某个设备在特定时间段（如办公时间）在特定通道（如 LAN）更可靠
    time_patterns: DashMap<(DeviceId, u32), ChannelType>, // u32 是小时 (0-23)
}

impl RoutePredictor {
    pub fn new() -> Self {
        Self {
            history: DashMap::new(),
            time_patterns: DashMap::new(),
        }
    }

    /// 记录一次路由尝试的结果
    pub fn record_result(
        &self,
        device_id: DeviceId,
        channel: ChannelType,
        success: bool,
        latency_ms: Option<u32>,
    ) {
        let key = (device_id, channel);
        let mut entry = self.history.entry(key).or_insert(RouteHistory {
            success_count: 0,
            failure_count: 0,
            avg_latency: 0.0,
            last_used: Utc::now(),
        });

        if success {
            entry.success_count += 1;
            if let Some(lat) = latency_ms {
                // 指数移动平均 (EMA) 更新延迟
                if entry.avg_latency == 0.0 {
                    entry.avg_latency = lat as f64;
                } else {
                    entry.avg_latency = entry.avg_latency * 0.7 + (lat as f64) * 0.3;
                }
            }
        } else {
            entry.failure_count += 1;
        }
        entry.last_used = Utc::now();

        // 如果成功率极高，更新时间模式
        let total = entry.success_count + entry.failure_count;
        if total > 10 && (entry.success_count as f64 / total as f64) > 0.9 {
            let hour = Utc::now().hour();
            self.time_patterns.insert((device_id, hour), channel);
        }
    }

    /// 预测给定设备在当前时刻的最优通道
    pub fn predict_best_channel(
        &self,
        device_id: DeviceId,
        available_channels: &[ChannelType],
    ) -> Option<ChannelType> {
        if available_channels.is_empty() {
            return None;
        }

        // 1. 首先检查时间模式
        let hour = Utc::now().hour();
        if let Some(pattern_channel) = self.time_patterns.get(&(device_id, hour)) {
            if available_channels.contains(&*pattern_channel) {
                return Some(*pattern_channel);
            }
        }

        // 2. 如果没有时间模式，基于成功率和延迟预测
        let mut best_channel = None;
        let mut max_score = -1.0;

        for &channel in available_channels {
            let score = if let Some(h) = self.history.get(&(device_id, channel)) {
                let total = h.success_count + h.failure_count;
                if total == 0 {
                    0.5 // 无数据，中等分数
                } else {
                    let success_rate = h.success_count as f64 / total as f64;
                    // 简单的预测得分：成功率 * (1 / (1 + 延迟))
                    let latency_factor = 1.0 / (1.0 + h.avg_latency / 100.0);
                    success_rate * 0.7 + latency_factor * 0.3
                }
            } else {
                0.5 // 无历史记录
            };

            if score > max_score {
                max_score = score;
                best_channel = Some(channel);
            }
        }

        best_channel
    }
}

impl Default for RoutePredictor {
    fn default() -> Self {
        Self::new()
    }
}
