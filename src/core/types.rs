use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;
use uuid::Uuid;

// --- 基础 ID 定义 ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(pub Uuid);

impl DeviceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for DeviceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GroupId(pub Uuid);

impl GroupId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for GroupId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for GroupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for DeviceId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl std::str::FromStr for GroupId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

// --- 枚举定义 ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceType {
    Smartphone,
    Tablet,
    Laptop,
    Desktop,
    Server,
    IoTDevice,
    DevelopmentBoard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChannelType {
    BluetoothLE,
    BluetoothMesh,
    WiFiDirect,
    Internet,
    Lan,
}

impl ChannelType {
    pub fn power_cost(&self) -> u8 {
        match self {
            ChannelType::BluetoothLE => 1,
            ChannelType::BluetoothMesh => 2,
            ChannelType::WiFiDirect => 3,
            ChannelType::Internet => 5,
            ChannelType::Lan => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NetworkType {
    Unknown,
    WiFi,
    Ethernet,
    Cellular4G,
    Cellular5G,
    Bluetooth,
    Loopback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessagePriority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MemberRole {
    Admin,
    Member,
}

// --- 结构体定义 ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    pub device_id: DeviceId,
    pub device_type: DeviceType,
    pub device_name: String,
    pub supported_channels: HashSet<ChannelType>,
    pub battery_level: Option<u8>,
    pub is_charging: bool,
    pub data_cost_sensitive: bool,
}

// F9: 流量统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrafficStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub last_updated: u64,
}

/// 合规性控制配置 (GDPR, HIPAA)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ComplianceConfig {
    /// 是否启用隐私保护模式 (数据脱敏)
    pub privacy_mode: bool,
    /// 消息存储有效期 (天)，0表示永久
    pub retention_days: u32,
    /// 是否对设备ID进行匿名化
    pub anonymize_device_id: bool,
    /// 数据加密等级
    pub encryption_level: String,
}

impl Default for ComplianceConfig {
    fn default() -> Self {
        Self {
            privacy_mode: false,
            retention_days: 30,
            anonymize_device_id: true,
            encryption_level: "AES-256-GCM".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelState {
    pub available: bool,
    // F6: 详细健康指标
    pub rtt_ms: u32,           // Round Trip Time
    pub jitter_ms: u32,        // 抖动
    pub packet_loss_rate: f32, // 0.0 - 1.0
    pub bandwidth_bps: u64,
    pub signal_strength: Option<i8>,
    pub distance_meters: Option<f32>, // F6: 估算距离
    pub network_type: NetworkType,
    pub failure_count: u32,  // 连续失败次数
    pub last_heartbeat: u64, // 最后一次心跳时间戳
}

impl Default for ChannelState {
    fn default() -> Self {
        Self {
            available: false,
            rtt_ms: 9999,
            jitter_ms: 0,
            packet_loss_rate: 1.0,
            bandwidth_bps: 0,
            signal_strength: None,
            distance_meters: None,
            network_type: NetworkType::Unknown,
            failure_count: 0,
            last_heartbeat: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMember {
    pub device_id: DeviceId,
    pub role: MemberRole,
    pub joined_at: u64,
    pub last_seen: u64,       // 最后活跃时间
    pub status: MemberStatus, // 成员状态
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MemberStatus {
    #[default]
    Online,
    Offline,
    Away,
    Busy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberState {
    pub device_id: DeviceId,
    pub status: MemberStatus,
    pub last_seen: u64,
    pub rtt_ms: u32,
    pub packet_loss_rate: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: GroupId,
    pub name: String,
    pub members: HashMap<DeviceId, GroupMember>,
    pub created_at: u64,
}

// --- 消息定义 ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessagePayload {
    Text(String),
    Binary(Vec<u8>),
    // F4: 群组 ACK
    GroupAck {
        original_msg_id: Uuid,
        responder: DeviceId,
    },
    Ack(Uuid),

    // F6: 心跳包含发送时间戳用于计算 RTT
    Ping(u64),
    Pong(u64),

    GroupInvite {
        group_id: GroupId,
        name: String,
    },

    // F8: 增加时间戳用于流控
    StreamChunk {
        stream_id: Uuid,
        total_chunks: u32,
        chunk_index: u32,
        data: Vec<u8>,
        sent_at: u64,
    },

    // F8: 媒体帧定义，用于音视频帧重组
    StreamFrame {
        stream_id: Uuid,
        frame_index: u64,
        data: Vec<u8>,
        timestamp: u64,
    },

    // F8: 流控反馈
    StreamControl {
        stream_id: Uuid,
        suggested_window_size: u32, // 建议窗口大小
        pause: bool,
    },

    // F5: TreeKEM 群组密钥更新
    GroupKeyUpdate {
        group_id: GroupId,
        epoch: u64,
        update_path: Vec<u8>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub sender: DeviceId,
    pub recipient: DeviceId,
    pub group_id: Option<GroupId>,
    pub payload: MessagePayload,
    pub priority: MessagePriority,
    pub timestamp: u64,
    pub require_ack: bool,
}

impl Message {
    pub fn new(sender: DeviceId, recipient: DeviceId, payload: MessagePayload) -> Self {
        Self {
            id: Uuid::new_v4(),
            sender,
            recipient,
            group_id: None,
            payload,
            priority: MessagePriority::Normal,
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            require_ack: true,
        }
    }

    pub fn new_group(sender: DeviceId, group_id: GroupId, payload: MessagePayload) -> Self {
        Self {
            id: Uuid::new_v4(),
            sender,
            recipient: DeviceId(Uuid::nil()),
            group_id: Some(group_id),
            payload,
            priority: MessagePriority::Normal,
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            require_ack: true, // F4: 群组消息现在默认需要 ACK 处理
        }
    }
}
