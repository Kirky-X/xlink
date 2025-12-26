use crate::core::types::{
    ChannelState, ChannelType, DeviceCapabilities, MessagePriority, NetworkType,
};

pub struct Scorer;

impl Scorer {
    /// Calculate a score (0.0 - 1.0) for a channel based on multiple factors.
    /// Higher is better.
    pub fn score(
        channel: ChannelType,
        state: &ChannelState,
        device_caps: &DeviceCapabilities,
        priority: MessagePriority,
    ) -> f64 {
        if !state.available {
            return 0.0;
        }

        // 1. Latency Score (Logarithmic decay)
        // Ideal is 10ms.
        let latency_score = 1.0 / (1.0 + (state.rtt_ms as f64 / 10.0).ln().max(0.0));

        // 2. Reliability Score
        let reliability_score = 1.0 - state.packet_loss_rate as f64;

        // 3. Power Score
        let power_cost = channel.power_cost();
        let power_score = if device_caps.is_charging {
            1.0
        } else {
            // F10: 增强功耗感知 - 结合电池电量调整得分
            let battery_factor = device_caps.battery_level.unwrap_or(100) as f64 / 100.0;
            match power_cost {
                1 => 1.0,                                // BLE
                2 => 0.8 * (0.5 + 0.5 * battery_factor), // Mesh / Lan
                3 => 0.6 * battery_factor,               // WiFi Direct
                _ => 0.4 * battery_factor,               // Internet
            }
        };

        // 4. Cost Score (F9: Cost Aware Routing)
        let cost_score = match state.network_type {
            NetworkType::WiFi
            | NetworkType::Ethernet
            | NetworkType::Loopback
            | NetworkType::Bluetooth => 1.0, // 免费/本地
            NetworkType::Cellular4G | NetworkType::Cellular5G => {
                if device_caps.data_cost_sensitive {
                    0.1 // 敏感模式下，尽量避免使用蜂窝网络
                } else {
                    0.6 // 正常模式下，蜂窝网络成本较高
                }
            }
            NetworkType::Unknown => 0.5,
        };

        // Weights based on priority
        let (w_lat, w_rel, w_pow, w_cost) = match priority {
            MessagePriority::Critical => (0.5, 0.4, 0.05, 0.05), // 紧急消息不惜成本
            MessagePriority::High => (0.4, 0.3, 0.1, 0.2),
            MessagePriority::Normal => (0.2, 0.3, 0.2, 0.3),
            MessagePriority::Low => (0.1, 0.2, 0.3, 0.4), // 低优先级消息看重成本和功耗
        };

        let final_score = (latency_score * w_lat)
            + (reliability_score * w_rel)
            + (power_score * w_pow)
            + (cost_score * w_cost);

        // Clamp to 0.0 - 1.0
        final_score.clamp(0.0, 1.0)
    }
}
