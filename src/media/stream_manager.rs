use crate::core::error::{Result, XPushError};
use crate::core::types::{DeviceId, Message, MessagePayload, NetworkType};
use crate::router::selector::Router;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use uuid::Uuid;

const CHUNK_SIZE: usize = 1024 * 32;

// F8: 音频/视频流处理常量
const AUDIO_SAMPLE_RATE: u32 = 48000; // 48kHz 音频采样率
const AUDIO_CHANNELS: u8 = 2; // 立体声
const AUDIO_FRAME_SIZE: usize = 960; // 20ms 帧大小 (48000 * 0.02)
const MAX_AUDIO_LATENCY_MS: u32 = 200; // 最大音频延迟 200ms

const VIDEO_WIDTH: u32 = 640; // 480p 宽度
const VIDEO_HEIGHT: u32 = 480; // 480p 高度

// F9: 流量预警阈值 (1GB)
const TRAFFIC_ALERT_THRESHOLD_BYTES: u64 = 1024 * 1024 * 1024;
const VIDEO_FPS: u32 = 30; // 30fps
const VIDEO_BITRATE_INITIAL: u32 = 500_000; // 初始视频码率 500kbps
const VIDEO_BITRATE_MIN: u32 = 100_000; // 最小视频码率 100kbps
const VIDEO_BITRATE_MAX: u32 = 2_000_000; // 最大视频码率 2Mbps

// 自适应码率调整参数
const BITRATE_ADJUSTMENT_INTERVAL_MS: u64 = 1000; // 每秒调整一次
const RTT_THRESHOLD_HIGH_MS: u32 = 300; // 高延迟阈值
const PACKET_LOSS_THRESHOLD_HIGH: f32 = 0.05; // 高丢包率阈值

// F8: 流类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamType {
    Audio,
    Video,
    Data,
}

// F8: 音频流配置
#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u8,
    pub frame_size: usize,
    pub bitrate: u32,
    pub codec: AudioCodec,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: AUDIO_SAMPLE_RATE,
            channels: AUDIO_CHANNELS,
            frame_size: AUDIO_FRAME_SIZE,
            bitrate: 128_000, // 128kbps
            codec: AudioCodec::Opus,
        }
    }
}

// F8: 音频编解码器类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCodec {
    Opus,  // 推荐用于实时通信
    Pcm16, // 原始 PCM 数据
    Aac,   // AAC 编解码器
}

// F8: 视频流配置
#[derive(Debug, Clone)]
pub struct VideoConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u32,
    pub codec: VideoCodec,
    pub keyframe_interval: u32, // 关键帧间隔（帧数）
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            width: VIDEO_WIDTH,
            height: VIDEO_HEIGHT,
            fps: VIDEO_FPS,
            bitrate: VIDEO_BITRATE_INITIAL,
            codec: VideoCodec::H264,
            keyframe_interval: 30, // 1秒一个关键帧
        }
    }
}

// F8: 视频编解码器类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    H264, // H.264/AVC
    H265, // H.265/HEVC
    Vp8,  // VP8
    Vp9,  // VP9
}

// F8: 流元数据
#[derive(Debug, Clone)]
pub struct StreamMetadata {
    pub stream_type: StreamType,
    pub audio_config: Option<AudioConfig>,
    pub video_config: Option<VideoConfig>,
    pub estimated_bandwidth_bps: u32,
    pub target_latency_ms: u32,
}

// F8: 媒体帧定义，用于重组和同步
#[derive(Debug, Clone)]
pub struct MediaFrame {
    pub stream_id: Uuid,
    pub frame_index: u64,
    pub timestamp: u64,
    pub frame_type: FrameType,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    Audio,
    VideoIFrame,
    VideoPFrame,
}

// F9: 流量统计
#[derive(Debug, Clone)]
pub struct TrafficStatistics {
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub total_packets_sent: u64,
    pub total_packets_received: u64,
    pub network_type: NetworkType,
    pub app_breakdown: HashMap<String, AppTraffic>,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct AppTraffic {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
}

// F9: 用户流量偏好设置
#[derive(Debug, Clone)]
pub struct UserTrafficPreferences {
    pub wifi_cost_per_mb: f32,
    pub cellular_cost_per_mb: f32,
    pub roaming_cost_multiplier: f32,
    pub monthly_data_limit_mb: u64,
    pub enable_cost_alerts: bool,
    pub enable_data_saver: bool,
}

impl Default for UserTrafficPreferences {
    fn default() -> Self {
        Self {
            wifi_cost_per_mb: 0.0,         // WiFi 通常免费
            cellular_cost_per_mb: 0.1,     // 蜂窝网络默认 0.1 元/MB
            roaming_cost_multiplier: 10.0, // 漫游费用倍数
            monthly_data_limit_mb: 1024,   // 默认 1GB 月流量限制
            enable_cost_alerts: true,
            enable_data_saver: false,
        }
    }
}

// F9: 网络统计信息
#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub rtt_ms: u32,
    pub packet_loss_rate: f32,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub bandwidth_bps: u32,
}

// F9: 网络监控器
pub struct NetworkMonitor {
    current_network: NetworkType,
    network_change_handlers: Vec<Box<dyn Fn(NetworkType) + Send + Sync>>,
}

impl NetworkMonitor {
    pub fn new() -> Self {
        Self {
            current_network: NetworkType::Unknown,
            network_change_handlers: Vec::new(),
        }
    }

    pub fn detect_network_type(&self) -> NetworkType {
        self.current_network
    }

    pub fn register_network_change_handler(
        &mut self,
        handler: Box<dyn Fn(NetworkType) + Send + Sync>,
    ) {
        self.network_change_handlers.push(handler);
    }

    pub fn update_network_type(&mut self, new_network: NetworkType) {
        if self.current_network != new_network {
            self.current_network = new_network;
            for handler in &self.network_change_handlers {
                handler(new_network);
            }
        }
    }
}

impl Default for NetworkMonitor {
    fn default() -> Self {
        Self::new()
    }
}

// F8: 自适应码率控制器
struct BitrateController {
    current_bitrate: u32,
    target_bitrate: u32,
    _network_type: NetworkType,
    rtt_ms: u32,
    packet_loss_rate: f32,
    last_adjustment_time: u64,
}

impl BitrateController {
    fn new(network_type: NetworkType) -> Self {
        let initial_bitrate = match network_type {
            NetworkType::Ethernet => 1_000_000, // 1Mbps
            NetworkType::WiFi => 500_000,       // 500kbps
            NetworkType::Cellular5G => 300_000, // 300kbps
            NetworkType::Cellular4G => 200_000, // 200kbps
            NetworkType::Bluetooth => 100_000,  // 100kbps
            _ => 100_000,                       // 默认 100kbps
        };

        Self {
            current_bitrate: initial_bitrate,
            target_bitrate: initial_bitrate,
            _network_type: network_type,
            rtt_ms: 0,
            packet_loss_rate: 0.0,
            last_adjustment_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    fn update_network_stats(&mut self, rtt_ms: u32, packet_loss_rate: f32) {
        self.rtt_ms = rtt_ms;
        self.packet_loss_rate = packet_loss_rate;

        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if current_time - self.last_adjustment_time >= BITRATE_ADJUSTMENT_INTERVAL_MS / 1000 {
            self.adjust_bitrate();
            self.last_adjustment_time = current_time;
        }
    }

    fn adjust_bitrate(&mut self) {
        // 基于网络状况调整码率
        if self.rtt_ms > RTT_THRESHOLD_HIGH_MS || self.packet_loss_rate > PACKET_LOSS_THRESHOLD_HIGH
        {
            // 网络状况差，降低码率
            self.target_bitrate = (self.current_bitrate as f32 * 0.8) as u32;
        } else if self.rtt_ms < RTT_THRESHOLD_HIGH_MS / 2
            && self.packet_loss_rate < PACKET_LOSS_THRESHOLD_HIGH / 2.0
        {
            // 网络状况好，提高码率
            self.target_bitrate = (self.current_bitrate as f32 * 1.2) as u32;
        }

        // 确保码率在合理范围内
        self.target_bitrate = self
            .target_bitrate
            .clamp(VIDEO_BITRATE_MIN, VIDEO_BITRATE_MAX);

        // 平滑调整当前码率
        self.current_bitrate = ((self.current_bitrate as f32 * 0.8
            + self.target_bitrate as f32 * 0.2) as u32)
            .clamp(VIDEO_BITRATE_MIN, VIDEO_BITRATE_MAX);
    }

    fn get_current_bitrate(&self) -> u32 {
        self.current_bitrate
    }
}

#[allow(dead_code)]
struct StreamSession {
    total_chunks: u32,
    received_chunks: HashMap<u32, Vec<u8>>,
    last_activity: u64,
    stream_type: StreamType,
    #[allow(dead_code)]
    metadata: Option<StreamMetadata>,
    // F8: 音频/视频特定处理状态
    #[allow(dead_code)]
    audio_buffer: Option<Vec<u8>>, // 音频帧缓冲区
    #[allow(dead_code)]
    video_frame_buffer: Option<Vec<u8>>, // 视频帧缓冲区
    #[allow(dead_code)]
    jitter_buffer: Vec<Vec<u8>>, // 抖动缓冲区
    // F8: 优先级队列，按时间戳排序
    priority_queue: Vec<MediaFrame>,
    // F9: 网络统计信息
    network_stats: Option<NetworkStats>,
}

impl StreamSession {
    fn new(stream_type: StreamType, total_chunks: u32) -> Self {
        Self {
            total_chunks,
            received_chunks: HashMap::new(),
            last_activity: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            stream_type,
            metadata: None,
            audio_buffer: None,
            video_frame_buffer: None,
            jitter_buffer: Vec::new(),
            priority_queue: Vec::new(),
            network_stats: Some(NetworkStats {
                rtt_ms: 0,
                packet_loss_rate: 0.0,
                bytes_sent: 0,
                bytes_received: 0,
                packets_sent: 0,
                packets_received: 0,
                bandwidth_bps: 0,
            }),
        }
    }

    fn is_complete(&self) -> bool {
        self.received_chunks.len() == self.total_chunks as usize
    }

    fn get_data(&self) -> Vec<u8> {
        let mut result = Vec::new();
        for i in 0..self.total_chunks {
            if let Some(chunk) = self.received_chunks.get(&i) {
                result.extend_from_slice(chunk);
            }
        }
        result
    }
}

#[derive(Debug)]
#[allow(dead_code)]
enum StreamControlMessage {
    Start,
    Pause,
    Resume,
    Stop,
    AdjustBitrate(u32),
}

#[allow(dead_code)]
pub struct StreamManager {
    local_device_id: DeviceId,
    router: Arc<Router>,
    sessions: Arc<Mutex<HashMap<Uuid, StreamSession>>>,
    #[allow(dead_code)]
    controllers: Arc<Mutex<HashMap<Uuid, mpsc::Sender<StreamControlMessage>>>>,
    bitrate_controllers: Arc<Mutex<HashMap<Uuid, BitrateController>>>,
    network_monitor: Arc<Mutex<NetworkMonitor>>,
    user_preferences: Arc<Mutex<UserTrafficPreferences>>,
}

impl StreamManager {
    // F8: 处理接收到的流分片
    pub async fn handle_chunk(
        &self,
        stream_id: Uuid,
        total_chunks: u32,
        chunk_index: u32,
        data: Vec<u8>,
    ) -> Result<Option<Vec<u8>>> {
        let is_complete;
        {
            let mut sessions = self.sessions.lock().unwrap();

            // 获取或创建会话
            let session = sessions
                .entry(stream_id)
                .or_insert_with(|| StreamSession::new(StreamType::Data, total_chunks));

            // 更新会话信息
            session.total_chunks = total_chunks;
            session.received_chunks.insert(chunk_index, data);
            session.last_activity = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            log::debug!(
                "Received chunk {}/{} for stream {}",
                chunk_index + 1,
                total_chunks,
                stream_id
            );

            is_complete = session.received_chunks.len() as u32 == session.total_chunks;
        }

        // 检查是否所有分片都已接收
        if is_complete {
            let session_opt;
            {
                let mut sessions = self.sessions.lock().unwrap();
                session_opt = sessions.remove(&stream_id);
            }

            if let Some(mut session) = session_opt {
                // 重组数据
                let mut full_data = Vec::with_capacity(session.total_chunks as usize * 1024 * 32);
                for i in 0..session.total_chunks {
                    if let Some(chunk) = session.received_chunks.remove(&i) {
                        full_data.extend_from_slice(&chunk);
                    } else {
                        return Err(XPushError::stream_init_failed(
                            "chunk_assembly".to_string(),
                            format!("Missing chunk {} for stream {}", i, stream_id),
                            file!(),
                        ));
                    }
                }

                log::info!(
                    "Stream {} reassembled successfully ({} chunks, {} bytes)",
                    stream_id,
                    total_chunks,
                    full_data.len()
                );
                return Ok(Some(full_data));
            }
        }

        Ok(None)
    }

    pub fn new(local_device_id: DeviceId, router: Arc<Router>) -> Self {
        let manager = Self {
            local_device_id,
            router,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            controllers: Arc::new(Mutex::new(HashMap::new())),
            bitrate_controllers: Arc::new(Mutex::new(HashMap::new())),
            network_monitor: Arc::new(Mutex::new(NetworkMonitor::new())),
            user_preferences: Arc::new(Mutex::new(UserTrafficPreferences::default())),
        };

        // 注册网络变更处理程序
        manager.register_network_change_handler();
        manager
    }

    // F8: 注册网络变更自动调整处理程序
    fn register_network_change_handler(&self) {
        let bitrate_controllers = Arc::clone(&self.bitrate_controllers);

        self.network_monitor
            .lock()
            .unwrap()
            .register_network_change_handler(Box::new(move |new_network| {
                log::info!(
                    "Network change detected: {:?}, adjusting stream parameters",
                    new_network
                );

                // 根据网络类型调整所有活跃的码率控制器
                let mut controllers = bitrate_controllers.lock().unwrap();
                for (_, controller) in controllers.iter_mut() {
                    // 重新初始化码率控制器以适应新的网络环境
                    *controller = BitrateController::new(new_network);
                }

                // 可以在这里添加更多的网络自适应逻辑
                // 例如：调整音频缓冲区大小、视频分辨率等
            }));
    }

    // F8: 发送音频流（专用接口）
    pub async fn send_audio_stream(
        &self,
        recipient: DeviceId,
        audio_data: Vec<u8>,
        config: Option<AudioConfig>,
    ) -> Result<Uuid> {
        let audio_config = config.unwrap_or_default();
        let metadata = StreamMetadata {
            stream_type: StreamType::Audio,
            audio_config: Some(audio_config.clone()),
            video_config: None,
            estimated_bandwidth_bps: 128_000,
            target_latency_ms: MAX_AUDIO_LATENCY_MS,
        };

        log::info!(
            "Starting audio stream to {} (sample_rate: {}Hz, channels: {}, bitrate: {}bps)",
            recipient,
            audio_config.sample_rate,
            audio_config.channels,
            audio_config.bitrate
        );

        // 创建音频特定的流会话
        let stream_id = Uuid::new_v4();
        {
            let mut sessions = self.sessions.lock().unwrap();
            sessions.insert(
                stream_id,
                StreamSession {
                    total_chunks: 0,
                    received_chunks: HashMap::new(),
                    last_activity: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    stream_type: StreamType::Audio,
                    metadata: Some(metadata),
                    audio_buffer: Some(Vec::with_capacity(AUDIO_FRAME_SIZE * 10)),
                    video_frame_buffer: None,
                    jitter_buffer: Vec::new(),
                    priority_queue: Vec::new(),
                    network_stats: Some(NetworkStats {
                        rtt_ms: 0,
                        packet_loss_rate: 0.0,
                        bytes_sent: 0,
                        bytes_received: 0,
                        packets_sent: 0,
                        packets_received: 0,
                        bandwidth_bps: 0,
                    }),
                },
            );
        }

        // 初始化音频码率控制器
        let bitrate_controller = BitrateController::new(NetworkType::Unknown);
        self.bitrate_controllers
            .lock()
            .unwrap()
            .insert(stream_id, bitrate_controller);

        // 将音频数据分帧处理
        let frames = self.split_audio_into_frames(audio_data, &audio_config);

        // 发送音频帧
        for (i, frame) in frames.iter().enumerate() {
            let mut frame_message = Message::new(
                self.local_device_id,
                recipient,
                MessagePayload::StreamFrame {
                    stream_id,
                    frame_index: i as u64,
                    data: frame.clone(),
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                },
            );
            frame_message.priority = crate::core::types::MessagePriority::High; // 音频流高优先级

            let channel_res = self.router.select_channel(&frame_message).await;
            if let Ok(channel) = channel_res {
                let msg = frame_message;
                tokio::spawn(async move {
                    let _ = channel.send(msg).await;
                });
            }
        }

        log::info!(
            "Audio stream {} sent successfully to {}",
            stream_id,
            recipient
        );
        Ok(stream_id)
    }

    // F8: 将音频数据分帧
    fn split_audio_into_frames(&self, audio_data: Vec<u8>, config: &AudioConfig) -> Vec<Vec<u8>> {
        let mut frames = Vec::new();
        let frame_size_bytes = config.frame_size * (config.channels as usize) * 2; // 16-bit PCM

        for chunk in audio_data.chunks(frame_size_bytes) {
            frames.push(chunk.to_vec());
        }

        frames
    }

    // F8: 发送音频帧
    pub async fn send_audio_frame(
        &self,
        recipient: DeviceId,
        stream_id: Uuid,
        frame_data: Vec<u8>,
        timestamp: u64,
    ) -> Result<()> {
        let frame_message = Message::new(
            self.local_device_id,
            recipient,
            MessagePayload::StreamChunk {
                stream_id,
                chunk_index: (timestamp / 20) as u32, // 简单估算帧索引
                total_chunks: 0,                      // 音频流是持续的
                data: frame_data,
                sent_at: timestamp,
            },
        );

        // 尝试发送，忽略可能的路由错误（音频流允许丢包）
        let channel_res = self.router.select_channel(&frame_message).await;
        if let Ok(channel) = channel_res {
            let msg = frame_message;
            tokio::spawn(async move {
                let _ = channel.send(msg).await;
            });
        }

        Ok(())
    }

    // F8: 发送视频流（专用接口）
    pub async fn send_video_stream(
        &self,
        recipient: DeviceId,
        video_data: Vec<u8>,
        config: Option<VideoConfig>,
    ) -> Result<Uuid> {
        let video_config = config.unwrap_or_default();
        let metadata = StreamMetadata {
            stream_type: StreamType::Video,
            audio_config: None,
            video_config: Some(video_config.clone()),
            estimated_bandwidth_bps: video_config.bitrate,
            target_latency_ms: 100, // 视频流目标延迟 100ms
        };

        log::info!(
            "Starting video stream to {} (resolution: {}x{}, fps: {}, bitrate: {}bps)",
            recipient,
            video_config.width,
            video_config.height,
            video_config.fps,
            video_config.bitrate
        );

        // 创建视频特定的流会话
        let stream_id = Uuid::new_v4();
        {
            let mut sessions = self.sessions.lock().unwrap();
            sessions.insert(
                stream_id,
                StreamSession {
                    total_chunks: 0,
                    received_chunks: HashMap::new(),
                    last_activity: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    stream_type: StreamType::Video,
                    metadata: Some(metadata),
                    audio_buffer: None,
                    video_frame_buffer: Some(Vec::new()),
                    jitter_buffer: Vec::new(),
                    priority_queue: Vec::new(),
                    network_stats: Some(NetworkStats {
                        rtt_ms: 0,
                        packet_loss_rate: 0.0,
                        bytes_sent: 0,
                        bytes_received: 0,
                        packets_sent: 0,
                        packets_received: 0,
                        bandwidth_bps: 0,
                    }),
                },
            );
        }

        // 初始化视频码率控制器
        let bitrate_controller = BitrateController::new(NetworkType::Unknown);
        {
            let mut controllers = self.bitrate_controllers.lock().unwrap();
            controllers.insert(stream_id, bitrate_controller);
        }

        // 将视频数据分片处理
        let chunks = self.split_video_into_chunks(video_data, &video_config);

        // 发送视频分片
        for (i, chunk) in chunks.iter().enumerate() {
            let chunk_message = Message::new(
                self.local_device_id,
                recipient,
                MessagePayload::StreamChunk {
                    stream_id,
                    chunk_index: i as u32,
                    total_chunks: chunks.len() as u32,
                    data: chunk.clone(),
                    sent_at: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                },
            );

            // 尝试发送
            let channel_res = self.router.select_channel(&chunk_message).await;
            if let Ok(channel) = channel_res {
                let msg = chunk_message;
                tokio::spawn(async move {
                    let _ = channel.send(msg).await;
                });
            }
        }

        log::info!(
            "Video stream {} sent successfully to {}",
            stream_id,
            recipient
        );
        Ok(stream_id)
    }

    // F8: 将视频数据分片
    fn split_video_into_chunks(&self, video_data: Vec<u8>, _config: &VideoConfig) -> Vec<Vec<u8>> {
        let mut chunks = Vec::new();
        let target_chunk_size = CHUNK_SIZE;

        for chunk in video_data.chunks(target_chunk_size) {
            chunks.push(chunk.to_vec());
        }

        chunks
    }

    // F8: 接收流数据
    pub async fn receive_stream_data(
        &self,
        stream_id: Uuid,
        chunk_index: u32,
        total_chunks: u32,
        data: Vec<u8>,
    ) -> Result<Vec<u8>> {
        let mut result_data = Vec::new();

        {
            let mut sessions = self.sessions.lock().unwrap();
            let session = sessions.entry(stream_id).or_insert(StreamSession {
                total_chunks,
                received_chunks: HashMap::new(),
                last_activity: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                stream_type: StreamType::Data, // 默认类型
                metadata: None,
                audio_buffer: None,
                video_frame_buffer: None,
                jitter_buffer: Vec::new(),
                priority_queue: Vec::new(),
                network_stats: Some(NetworkStats {
                    rtt_ms: 0,
                    packet_loss_rate: 0.0,
                    bytes_sent: 0,
                    bytes_received: 0,
                    packets_sent: 0,
                    packets_received: 0,
                    bandwidth_bps: 0,
                }),
            });

            session.received_chunks.insert(chunk_index, data);
            session.last_activity = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            if session.is_complete() {
                result_data = session.get_data();
                sessions.remove(&stream_id);
                log::info!(
                    "Stream {} completed, received {} chunks",
                    stream_id,
                    total_chunks
                );
            } else {
                log::debug!(
                    "Stream {} progress: {}/{}",
                    stream_id,
                    session.received_chunks.len(),
                    total_chunks
                );
            }
        }

        Ok(result_data)
    }

    // F8: 处理音频帧
    pub fn process_audio_frame(
        &self,
        stream_id: Uuid,
        frame_data: Vec<u8>,
        timestamp: u64,
    ) -> Result<()> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(&stream_id) {
            if session.stream_type == StreamType::Audio {
                // 将音频帧添加到缓冲区
                if let Some(ref mut buffer) = session.audio_buffer {
                    buffer.extend_from_slice(&frame_data);

                    // 检查缓冲区大小，避免过度累积
                    if buffer.len() > AUDIO_FRAME_SIZE * 100 {
                        // 最多缓存100帧
                        buffer.drain(0..AUDIO_FRAME_SIZE);
                    }

                    log::debug!(
                        "Audio frame processed for stream {}, buffer size: {} bytes",
                        stream_id,
                        buffer.len()
                    );
                }

                // 创建媒体帧并添加到优先级队列
                let media_frame = MediaFrame {
                    stream_id,
                    frame_index: timestamp / 20, // 假设20ms一帧
                    timestamp,
                    frame_type: FrameType::Audio,
                    data: frame_data,
                };

                session.priority_queue.push(media_frame);
                // 按时间戳排序
                session.priority_queue.sort_by_key(|f| f.timestamp);

                session.last_activity = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
            }
        }
        Ok(())
    }

    // F8: 处理视频帧
    pub fn process_video_frame(
        &self,
        stream_id: Uuid,
        frame_data: Vec<u8>,
        frame_type: FrameType,
        timestamp: u64,
    ) -> Result<()> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(&stream_id) {
            if session.stream_type == StreamType::Video {
                // 将视频帧添加到缓冲区
                if let Some(ref mut buffer) = session.video_frame_buffer {
                    buffer.extend_from_slice(&frame_data);

                    // 检查缓冲区大小，避免过度累积
                    if buffer.len() > 1024 * 1024 {
                        // 最多缓存1MB
                        buffer.clear();
                    }

                    log::debug!(
                        "Video frame processed for stream {}, type: {:?}, size: {} bytes",
                        stream_id,
                        frame_type,
                        frame_data.len()
                    );
                }

                // 创建媒体帧并添加到优先级队列
                let media_frame = MediaFrame {
                    stream_id,
                    frame_index: timestamp / 33, // 假设30fps，约33ms一帧
                    timestamp,
                    frame_type,
                    data: frame_data,
                };

                session.priority_queue.push(media_frame);
                // 按时间戳排序，确保关键帧优先
                session.priority_queue.sort_by(|a, b| {
                    // 关键帧优先级最高
                    let a_priority = match a.frame_type {
                        FrameType::VideoIFrame => 0,
                        FrameType::VideoPFrame => 1,
                        FrameType::Audio => 2,
                    };
                    let b_priority = match b.frame_type {
                        FrameType::VideoIFrame => 0,
                        FrameType::VideoPFrame => 1,
                        FrameType::Audio => 2,
                    };

                    a_priority
                        .cmp(&b_priority)
                        .then(a.timestamp.cmp(&b.timestamp))
                });

                session.last_activity = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
            }
        }
        Ok(())
    }

    // F8: 获取待处理的媒体帧
    pub fn get_pending_media_frames(&self, stream_id: Uuid) -> Vec<MediaFrame> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(&stream_id) {
            // 返回并清空优先级队列
            std::mem::take(&mut session.priority_queue)
        } else {
            Vec::new()
        }
    }

    // F8: 自适应码率调整
    pub fn adjust_stream_bitrate(
        &self,
        stream_id: Uuid,
        rtt_ms: u32,
        packet_loss_rate: f32,
    ) -> Result<u32> {
        let mut controllers = self.bitrate_controllers.lock().unwrap();
        if let Some(controller) = controllers.get_mut(&stream_id) {
            controller.update_network_stats(rtt_ms, packet_loss_rate);
            let new_bitrate = controller.get_current_bitrate();
            log::info!(
                "Adjusted bitrate for stream {} to {} bps (RTT: {}ms, Loss: {:.2}%)",
                stream_id,
                new_bitrate,
                rtt_ms,
                packet_loss_rate * 100.0
            );
            Ok(new_bitrate)
        } else {
            Err(XPushError::stream_disconnected(
                format!("stream_id={}", stream_id),
                format!("Stream not found: {}", stream_id),
                file!(),
            ))
        }
    }

    // F9: 获取流量统计信息
    pub fn get_traffic_statistics(&self) -> TrafficStatistics {
        let sessions = self.sessions.lock().unwrap();
        let mut total_sent = 0u64;
        let mut total_received = 0u64;
        let mut total_packets_sent = 0u64;
        let mut total_packets_received = 0u64;
        let mut app_breakdown = HashMap::new();

        for (_stream_id, session) in sessions.iter() {
            if let Some(stats) = &session.network_stats {
                total_sent += stats.bytes_sent;
                total_received += stats.bytes_received;
                total_packets_sent += stats.packets_sent;
                total_packets_received += stats.packets_received;

                // 按应用类型分类统计
                let app_name = match session.stream_type {
                    StreamType::Audio => "Audio",
                    StreamType::Video => "Video",
                    StreamType::Data => "Data",
                };

                let app_traffic = app_breakdown
                    .entry(app_name.to_string())
                    .or_insert(AppTraffic {
                        bytes_sent: 0,
                        bytes_received: 0,
                        packets_sent: 0,
                        packets_received: 0,
                    });

                app_traffic.bytes_sent += stats.bytes_sent;
                app_traffic.bytes_received += stats.bytes_received;
                app_traffic.packets_sent += stats.packets_sent;
                app_traffic.packets_received += stats.packets_received;
            }
        }

        let network_type = self.network_monitor.lock().unwrap().detect_network_type();

        TrafficStatistics {
            total_bytes_sent: total_sent,
            total_bytes_received: total_received,
            total_packets_sent,
            total_packets_received,
            network_type,
            app_breakdown,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    // F9: 更新用户流量偏好设置
    pub fn update_user_preferences(&self, preferences: UserTrafficPreferences) {
        *self.user_preferences.lock().unwrap() = preferences.clone();
        log::info!("Updated user traffic preferences: {:?}", preferences);
    }

    // F9: 获取用户流量偏好设置
    pub fn get_user_preferences(&self) -> UserTrafficPreferences {
        self.user_preferences.lock().unwrap().clone()
    }

    // F9: 估算流量成本
    pub fn estimate_traffic_cost(&self, bytes: u64, network_type: NetworkType) -> f32 {
        let preferences = self.user_preferences.lock().unwrap();
        let mb = bytes as f32 / (1024.0 * 1024.0);

        match network_type {
            NetworkType::WiFi => preferences.wifi_cost_per_mb * mb,
            NetworkType::Cellular5G | NetworkType::Cellular4G => {
                preferences.cellular_cost_per_mb * mb * preferences.roaming_cost_multiplier
            }
            _ => 0.0,
        }
    }

    // F9: 实时网络类型检测
    pub async fn detect_network_type(&self) -> NetworkType {
        // 1. 首先尝试通过系统接口检测
        let interfaces = pnet_datalink::interfaces();
        for interface in interfaces {
            if interface.name.contains("wlan") || interface.name.contains("wifi") {
                return NetworkType::WiFi;
            } else if interface.name.contains("eth") || interface.name.contains("en") {
                return NetworkType::Ethernet;
            }
        }

        // 2. 基于网络特征进行智能检测 (回退方案)
        let (total_rtt, total_loss, count) = {
            let sessions = self.sessions.lock().unwrap();
            let mut total_rtt = 0u32;
            let mut total_loss = 0.0f32;
            let mut count = 0u32;

            for session in sessions.values() {
                if let Some(stats) = &session.network_stats {
                    total_rtt += stats.rtt_ms;
                    total_loss += stats.packet_loss_rate;
                    count += 1;
                }
            }
            (total_rtt, total_loss, count)
        };

        if count == 0 {
            // 尝试通过 local-ip-address 获取默认接口信息
            if let Ok(ip) = local_ip_address::local_ip() {
                if ip.is_loopback() {
                    return NetworkType::Loopback;
                }
            }
            return NetworkType::Unknown;
        }

        let avg_rtt = total_rtt / count;
        let avg_loss = total_loss / count as f32;

        // 基于网络特征进行智能检测
        match (avg_rtt, avg_loss) {
            (0..=5, 0.0..=0.001) => NetworkType::Loopback, // 极低延迟和丢包 -> 回环
            (6..=20, 0.0..=0.005) => NetworkType::Ethernet, // 低延迟低丢包 -> 以太网
            (21..=100, 0.0..=0.02) => NetworkType::WiFi,   // 中等延迟 -> WiFi
            (101..=200, 0.0..=0.05) => NetworkType::Cellular5G, // 较高延迟 -> 5G
            (201..=300, 0.0..=0.1) => NetworkType::Cellular4G, // 高延迟 -> 4G
            (301..=500, 0.0..=0.2) => NetworkType::Bluetooth, // 最高延迟 -> 蓝牙
            _ => NetworkType::Unknown,                     // 无法识别的特征
        }
    }

    // F9: 检查并触发流量预警
    pub fn check_traffic_alerts(&self, stats: &TrafficStatistics) {
        let total_bytes = stats.total_bytes_sent + stats.total_bytes_received;
        if total_bytes > TRAFFIC_ALERT_THRESHOLD_BYTES {
            log::warn!(
                "Traffic Alert: Total data usage ({:.2} MB) exceeds threshold ({:.2} MB)",
                total_bytes as f64 / (1024.0 * 1024.0),
                TRAFFIC_ALERT_THRESHOLD_BYTES as f64 / (1024.0 * 1024.0)
            );
        }
    }

    // 清理超时的会话
    pub fn cleanup_timeout_sessions(&self) {
        let mut sessions = self.sessions.lock().unwrap();
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let timeout_duration = 300; // 5分钟超时

        sessions.retain(|stream_id, session| {
            if current_time - session.last_activity > timeout_duration {
                log::info!("Cleaning up timeout session: {}", stream_id);
                false
            } else {
                true
            }
        });
    }

    /// 清理所有活动流，防止内存泄漏
    pub fn clear_streams(&self) {
        let mut sessions = self.sessions.lock().unwrap();
        sessions.clear();
        let mut controllers = self.controllers.lock().unwrap();
        controllers.clear();
        let mut bitrate_controllers = self.bitrate_controllers.lock().unwrap();
        bitrate_controllers.clear();
    }
}
