use crate::core::types::{DeviceId, Group, Message, MessagePriority};
use crate::router::selector::Router;
use futures::stream::{FuturesUnordered, StreamExt};
use log::{debug, info, warn};
use std::collections::HashSet;
use std::sync::Arc;

/// 广播分发策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BroadcastStrategy {
    /// 直接发送给所有成员 (默认)
    Direct,
    /// 扇出发送 (由超级节点或指定中继节点协助)
    FanOut,
    /// 最小功耗策略
    PowerEfficient,
}

/// 广播分发结果
pub struct DispatchResult {
    pub successful_devices: HashSet<DeviceId>,
    pub failed_devices: HashSet<DeviceId>,
}

/// 广播策略执行器
pub struct BroadcastExecutor {
    router: Arc<Router>,
}

impl BroadcastExecutor {
    pub fn new(router: Arc<Router>) -> Self {
        Self { router }
    }

    /// 执行广播分发
    pub async fn execute_broadcast(
        &self,
        group: &Group,
        message: Message,
        strategy: BroadcastStrategy,
    ) -> DispatchResult {
        match strategy {
            BroadcastStrategy::Direct => self.execute_direct(group, message).await,
            BroadcastStrategy::FanOut => self.execute_fan_out(group, message).await,
            BroadcastStrategy::PowerEfficient => self.execute_power_efficient(group, message).await,
        }
    }

    /// 直接发送给所有成员
    async fn execute_direct(&self, group: &Group, message: Message) -> DispatchResult {
        let mut successful_devices = HashSet::new();
        let mut failed_devices = HashSet::new();
        let mut futures = FuturesUnordered::new();

        for &member_id in group.members.keys() {
            if member_id == message.sender {
                continue;
            }

            let router = self.router.clone();
            let mut msg_to_send = message.clone();
            msg_to_send.recipient = member_id; // 设置具体的接收者

            futures.push(async move {
                match router.select_channel(&msg_to_send).await {
                    Ok(channel) => {
                        if let Err(e) = channel.send(msg_to_send).await {
                            warn!("Failed to send message to member {}: {}", member_id, e);
                            (member_id, false)
                        } else {
                            debug!("Successfully sent broadcast chunk to {}", member_id);
                            (member_id, true)
                        }
                    }
                    Err(e) => {
                        warn!("No channel available for member {}: {}", member_id, e);
                        (member_id, false)
                    }
                }
            });
        }

        while let Some((member_id, success)) = futures.next().await {
            if success {
                successful_devices.insert(member_id);
            } else {
                failed_devices.insert(member_id);
            }
        }

        DispatchResult {
            successful_devices,
            failed_devices,
        }
    }

    /// 扇出发送 (TODO: 识别超级节点并利用中继)
    async fn execute_fan_out(&self, group: &Group, message: Message) -> DispatchResult {
        // 简化实现：目前回退到直接发送
        info!(
            "Fan-out strategy requested for group {}, falling back to Direct",
            group.id
        );
        self.execute_direct(group, message).await
    }

    /// 最小功耗策略
    async fn execute_power_efficient(&self, group: &Group, mut message: Message) -> DispatchResult {
        // 降低优先级以倾向于低功耗通道
        message.priority = MessagePriority::Low;
        self.execute_direct(group, message).await
    }
}
