use crate::capability::manager::CapabilityManager;
use crate::core::types::{ChannelType, DeviceId, Message, MessagePayload, NetworkType};
use crate::router::selector::Router;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::task::JoinHandle;

// F6: 近场设备心跳间隔 1-5秒
const NEAR_MIN_INTERVAL: Duration = Duration::from_secs(1);
const NEAR_MAX_INTERVAL: Duration = Duration::from_secs(5);

// F6: 远程设备心跳间隔 30-60秒，动态调整
const REMOTE_MIN_INTERVAL: Duration = Duration::from_secs(30);
const REMOTE_MAX_INTERVAL: Duration = Duration::from_secs(60);

// F6: 3次心跳失败后标记为不可达
const FAILURE_THRESHOLD: u32 = 3;

// F6: 信号强度阈值（dBm)
const SIGNAL_STRENGTH_NEAR_THRESHOLD: i8 = -60; // -60dBm 以上认为是近场

pub struct HeartbeatManager {
    local_device_id: DeviceId,
    router: Arc<Router>,
    cap_manager: Arc<CapabilityManager>,
    running_task: Option<JoinHandle<()>>,
}

impl HeartbeatManager {
    pub fn new(
        local_device_id: DeviceId,
        router: Arc<Router>,
        cap_manager: Arc<CapabilityManager>,
    ) -> Self {
        Self {
            local_device_id,
            router,
            cap_manager,
            running_task: None,
        }
    }

    pub fn start(&mut self) -> Option<JoinHandle<()>> {
        if self.running_task.is_some() { return None; }

        let router = self.router.clone();
        let cap_manager = self.cap_manager.clone();
        let local_id = self.local_device_id;

        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1)); // 基础 Tick
            
            loop {
                interval.tick().await;
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_else(|_| Duration::from_secs(0))
                    .as_millis() as u64;

                // 遍历所有已知设备
                let devices = cap_manager.get_all_remote_devices();
                
                for device_id in devices {
                    // 获取该设备最优通道状态
                    // 这里简化逻辑：检查任意一个通道的状态
                    // 实际应遍历该设备所有通道
                    let channel_type = ChannelType::Internet; // 默认检查 Internet，实际应动态获取
                    
                    if let Some(mut state) = cap_manager.get_channel_state(&device_id, &channel_type) {
                        // 1. 动态间隔判断 - 基于距离、信号强度和RTT
                        let distance = state.distance_meters.unwrap_or(20.0);
                        let signal_strength = state.signal_strength.unwrap_or(-100);
                        let network_type = state.network_type;
                        
                        let required_interval = if distance <= 10.0 || signal_strength >= SIGNAL_STRENGTH_NEAR_THRESHOLD || state.rtt_ms < 100 {
                            // 近场设备或低延迟：1-5秒动态调整
                            let base_interval = NEAR_MIN_INTERVAL;
                            let max_interval = NEAR_MAX_INTERVAL;
                            
                            // 根据距离调整间隔：1米以内 1秒，10米 5秒
                            let distance_factor = (distance / 10.0).clamp(0.0, 1.0);
                            
                            // 综合考虑信号强度和RTT作为微调
                            let signal_factor = ((signal_strength + 100) as f32 / 40.0).clamp(0.0, 1.0);
                            let rtt_factor = (state.rtt_ms as f32 / 200.0).clamp(0.0, 1.0);
                            
                            let factor = distance_factor * 0.7 + signal_factor * 0.15 + rtt_factor * 0.15;
                            let interval_ms = base_interval.as_millis() as f32 + 
                                (max_interval.as_millis() as f32 - base_interval.as_millis() as f32) * factor;
                            
                            Duration::from_millis(interval_ms as u64)
                        } else {
                            // 远程设备：30-60秒动态调整
                            let base_interval = REMOTE_MIN_INTERVAL;
                            let max_interval = REMOTE_MAX_INTERVAL;
                            
                            // 根据网络类型和RTT调整间隔
                            let network_factor = match network_type {
                                NetworkType::Bluetooth => 0.3, // BLE设备更频繁
                                NetworkType::WiFi => 0.5,
                                NetworkType::Ethernet => 1.0,
                                _ => 0.8, // 其他网络类型使用默认值
                            };
                            
                            let rtt_factor = (state.rtt_ms as f32 / 1000.0).clamp(0.0, 1.0);
                            let factor = (network_factor + rtt_factor) / 2.0;
                            
                            let interval_ms = base_interval.as_millis() as f32 + 
                                (max_interval.as_millis() as f32 - base_interval.as_millis() as f32) * factor;
                            
                            Duration::from_millis(interval_ms as u64)
                        };

                        let elapsed = now.saturating_sub(state.last_heartbeat);
                        if elapsed < required_interval.as_millis() as u64 {
                            continue; // 还没到时间
                        }

                        // 2. 发送 Ping
                        let payload = MessagePayload::Ping(now);
                        let msg = Message::new(local_id, device_id, payload);
                        
                        // 乐观更新：增加失败计数，如果 Pong 回来会重置
                        state.failure_count += 1;
                        if state.failure_count >= FAILURE_THRESHOLD {
                            state.available = false;
                            log::warn!("Device {} marked unavailable ({} failures)", device_id, state.failure_count);
                        }
                        cap_manager.update_channel_state(device_id, channel_type, state);

                        let r_clone = router.clone();
                        tokio::spawn(async move {
                            if let Ok(ch) = r_clone.select_channel(&msg).await {
                                let _ = ch.send(msg).await;
                            }
                        });
                    }
                }
            }
        });

        self.running_task = None;
        Some(task)
    }

    pub fn stop(&mut self) {
        // 由于所有权已移交给 SDK 的 background_tasks，这里不再直接 abort
        // SDK 会统一处理。保留此方法用于兼容性。
        log::info!("HeartbeatManager stop called (task managed by SDK)");
    }

    pub async fn handle_heartbeat(&self, message: &Message) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;

        match message.payload {
            MessagePayload::Ping(ts) => {
                // 回复 Pong
                let response = Message::new(self.local_device_id, message.sender, MessagePayload::Pong(ts));
                if let Ok(ch) = self.router.select_channel(&response).await {
                    let _ = ch.send(response).await;
                }
            },
            MessagePayload::Pong(ts) => {
                // 计算 RTT
                let rtt = (now.saturating_sub(ts)) as u32;
                // 假设通过 Internet 收到，实际应从 Message 元数据获取接收通道
                let channel_type = ChannelType::Internet;

                if let Some(mut state) = self.cap_manager.get_channel_state(&message.sender, &channel_type) {
                    state.available = true;
                    state.failure_count = 0;
                    state.last_heartbeat = now;
                    // 平滑 RTT 计算 (EWMA)
                    state.rtt_ms = (state.rtt_ms * 7 + rtt * 3) / 10;

                    self.cap_manager.update_channel_state(message.sender, channel_type, state);
                    log::debug!("Heartbeat success: {} RTT={}ms", message.sender, rtt);
                }
            },
            _ => {}
        }
    }
}