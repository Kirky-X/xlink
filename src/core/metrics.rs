use crate::core::types::{ChannelType, DeviceId};
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub struct MetricsCollector {
    messages_sent: AtomicU64,
    messages_received: AtomicU64,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,

    // 按通道统计
    channel_usage: DashMap<ChannelType, AtomicU64>,

    // 延迟统计 (ms)
    last_rtt: DashMap<DeviceId, u32>,

    start_time: Instant,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self {
            messages_sent: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            channel_usage: DashMap::new(),
            last_rtt: DashMap::new(),
            start_time: Instant::now(),
        }
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_send(&self, channel: ChannelType, bytes: u64) {
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);

        self.channel_usage
            .entry(channel)
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_receive(&self, bytes: u64) {
        self.messages_received.fetch_add(1, Ordering::Relaxed);
        self.bytes_received.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn update_rtt(&self, device: DeviceId, rtt_ms: u32) {
        self.last_rtt.insert(device, rtt_ms);
    }

    pub fn get_report(&self) -> MetricsReport {
        MetricsReport {
            uptime_secs: self.start_time.elapsed().as_secs(),
            total_sent: self.messages_sent.load(Ordering::Relaxed),
            total_received: self.messages_received.load(Ordering::Relaxed),
            total_bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            total_bytes_received: self.bytes_received.load(Ordering::Relaxed),
        }
    }
}

pub struct MetricsReport {
    pub uptime_secs: u64,
    pub total_sent: u64,
    pub total_received: u64,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct AnalyticsEvent {
    pub timestamp: u64,
    pub event_type: String,
    pub device_id: String,
    pub channel: Option<String>,
    pub metadata: std::collections::HashMap<String, String>,
}

impl MetricsCollector {
    // ... 原有方法保持不变 ...

    /// 导出为 Prometheus 格式
    pub fn export_prometheus(&self) -> String {
        let mut report = String::new();
        report.push_str("# HELP xlink_messages_sent_total Total number of messages sent\n");
        report.push_str("# TYPE xlink_messages_sent_total counter\n");
        report.push_str(&format!(
            "xlink_messages_sent_total {}\n",
            self.messages_sent.load(Ordering::Relaxed)
        ));

        report.push_str("# HELP xlink_bytes_sent_total Total number of bytes sent\n");
        report.push_str("# TYPE xlink_bytes_sent_total counter\n");
        report.push_str(&format!(
            "xlink_bytes_sent_total {}\n",
            self.bytes_sent.load(Ordering::Relaxed)
        ));

        for entry in self.channel_usage.iter() {
            report.push_str(&format!(
                "xlink_channel_usage_total{{channel=\"{:?}\"}} {}\n",
                entry.key(),
                entry.value().load(Ordering::Relaxed)
            ));
        }
        report
    }

    /// 记录高级分析事件
    pub fn record_event(&self, event: AnalyticsEvent) {
        // 在实际生产中，这里可以异步发送到分析服务器或存入本地高性能缓冲区
        log::info!("Analytics Event: {:?}", event);
    }

    /// 清理所有指标数据 - use proper entry removal to avoid DashMap fragmentation
    pub fn clear(&self) {
        // Remove channel_usage entries one by one to avoid fragmentation
        let channel_keys: Vec<_> = self
            .channel_usage
            .iter()
            .map(|entry| *entry.key())
            .collect();
        for channel_type in channel_keys {
            self.channel_usage.remove(&channel_type);
        }

        // Remove last_rtt entries one by one to avoid fragmentation
        let device_keys: Vec<_> = self.last_rtt.iter().map(|entry| *entry.key()).collect();
        for device_id in device_keys {
            self.last_rtt.remove(&device_id);
        }

        log::debug!("MetricsCollector: Cleared all metrics data using entry removal");
    }
}
