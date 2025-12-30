//! 增强的错误类型定义模块
//!
//! 提供完整的错误处理解决方案，包括：
//! - 结构化错误码体系
//! - 错误分类和上下文追踪
//! - 链式错误支持
//! - 重试建议机制
//! - 错误统计和监控
//!
//! # 错误码格式
//!
//! 错误码采用 `XX-YYYY` 格式：
//! - `XX`: 模块分类 (01=系统, 02=通道, 03=加密, 04=群组, 05=设备, 06=流媒体, 07=存储, 08=协议)
//! - `YYYY`: 具体错误序号
//!
//! # 示例
//!
//! ```rust
//! use xpush::core::error::{XPushError, Result};
//!
//! fn example() -> Result<()> {
//!     Err(XPushError::device_not_found("device-001", file!()))
//! }
//! ```

use thiserror::Error;
use serde::{Deserialize, Serialize};
use std::fmt;

/// 错误码类型 - 用于唯一标识每种错误
///
/// # 格式
/// `XX-YYYY` 格式，其中：
/// - `XX`: 模块分类 (01-08)
/// - `YYYY`: 具体错误序号
///
/// # 示例
/// ```
/// use xpush::core::error::ErrorCode;
///
/// let code = ErrorCode(0203);
/// assert_eq!(format!("{}", code), "XP-0203");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorCode(pub u16);

impl ErrorCode {
    /// 从错误码字符串解析
    ///
    /// # Errors
    /// 如果字符串格式不正确，返回 None
    ///
    /// # Example
    /// ```
    /// use xpush::core::error::ErrorCode;
    ///
    /// let code = ErrorCode::from_str("0203");
    /// assert_eq!(code, Some(ErrorCode(0203)));
    /// ```
    pub fn from_str(s: &str) -> Option<Self> {
        let value = s.parse().ok()?;
        if value < 10000 {
            Some(Self(value))
        } else {
            None
        }
    }

    /// 获取模块分类
    ///
    /// 返回错误码的高两位数字，表示错误所属的模块
    ///
    /// # Example
    /// ```
    /// use xpush::core::error::ErrorCode;
    ///
    /// let code = ErrorCode(0203);
    /// assert_eq!(code.module(), 02);
    /// ```
    pub fn module(&self) -> u16 {
        self.0 / 100
    }

    /// 获取序号
    ///
    /// 返回错误码的低两位数字，表示具体错误
    ///
    /// # Example
    /// ```
    /// use xpush::core::error::ErrorCode;
    ///
    /// let code = ErrorCode(0203);
    /// assert_eq!(code.sequence(), 3);
    /// ```
    pub fn sequence(&self) -> u16 {
        self.0 % 100
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "XP-{:04}", self.0)
    }
}

impl fmt::LowerHex for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:04x}", self.0)
    }
}

/// 错误分类 - 用于分组管理错误
///
/// 每个分类对应一个模块，便于快速定位问题范围
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCategory {
    /// 通用系统错误 (01xx)
    /// 包括超时、输入验证、序列化等基础错误
    System,

    /// 通道通信错误 (02xx)
    /// 包括连接、发送、接收等通道相关错误
    Channel,

    /// 加密安全错误 (03xx)
    /// 包括加密、解密、签名、密钥管理等安全相关错误
    Crypto,

    /// 群组管理错误 (04xx)
    /// 包括群组创建、成员管理、邀请等群组相关错误
    Group,

    /// 设备发现错误 (05xx)
    /// 包括设备查找、在线状态、设备能力等设备相关错误
    Device,

    /// 流媒体传输错误 (06xx)
    /// 包括流初始化、媒体传输、编解码等流媒体相关错误
    Stream,

    /// 数据存储错误 (07xx)
    /// 包括存储读写、持久化等存储相关错误
    Storage,

    /// 协议解析错误 (08xx)
    /// 包括协议版本、消息格式等协议相关错误
    Protocol,

    /// 能力匹配错误 (09xx)
    /// 包括设备能力协商、功能兼容性等能力相关错误
    Capability,
}

impl ErrorCategory {
    /// 获取分类名称
    pub fn name(&self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Channel => "Channel",
            Self::Crypto => "Crypto",
            Self::Group => "Group",
            Self::Device => "Device",
            Self::Stream => "Stream",
            Self::Storage => "Storage",
            Self::Protocol => "Protocol",
            Self::Capability => "Capability",
        }
    }

    /// 获取错误码范围
    pub fn code_range(&self) -> (u16, u16) {
        match self {
            Self::System => (0100, 0199),
            Self::Channel => (0200, 0299),
            Self::Crypto => (0300, 0399),
            Self::Group => (0400, 0499),
            Self::Device => (0500, 0599),
            Self::Stream => (0600, 0699),
            Self::Storage => (0700, 0799),
            Self::Protocol => (0800, 0899),
            Self::Capability => (0900, 0999),
        }
    }
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// 重试建议 - 指导客户端如何处理错误
///
/// 错误处理策略的一个重要组成部分，帮助调用者决定是否重试操作
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetrySuggestion {
    /// 不需要重试
    /// 错误是确定性的，重试也不会改变结果
    NoRetry,

    /// 可重试
    /// 错误可能是暂时性的，建议在延迟后重试
    Retryable {
        /// 最大重试次数
        max_attempts: u32,
        /// 基础延迟时间（毫秒）
        base_delay_ms: u64,
    },

    /// 可重试，但需要用户干预
    /// 例如网络切换、权限确认等
    ManualIntervention,

    /// 致命错误，不应重试
    /// 重试可能导致更严重的问题
    Fatal,
}

impl Default for RetrySuggestion {
    fn default() -> Self {
        Self::NoRetry
    }
}

/// 错误的详细上下文信息
///
/// 用于记录错误发生的环境信息，便于调试和问题追踪
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ErrorContext {
    /// 发生错误的位置（格式：文件名:行号）
    pub location: &'static str,

    /// 相关的设备ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,

    /// 相关的群组ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,

    /// 相关的会话ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    /// 相关的请求ID（用于分布式追踪）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,

    /// 原始错误消息
    pub original_message: String,

    /// 附加的调试信息（序列化为 JSON）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug_info: Option<serde_json::Value>,

    /// 发生时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// 重试建议
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_suggestion: Option<RetrySuggestion>,

    /// 影响范围估计
    #[serde(skip_serializing_if = "Option::is_none")]
    pub impact_scope: Option<ImpactScope>,
}

/// 影响范围 - 描述错误影响的范围
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImpactScope {
    /// 仅影响当前操作
    Operation,
    /// 影响当前会话
    Session,
    /// 影响整个设备
    Device,
    /// 影响整个群组
    Group,
    /// 影响整个系统
    System,
}

impl ErrorContext {
    /// 创建新的错误上下文
    #[inline]
    pub fn new(location: &'static str, original_message: String) -> Self {
        Self {
            location,
            original_message,
            timestamp: chrono::Utc::now(),
            ..Default::default()
        }
    }

    /// 添加设备ID上下文
    #[inline]
    pub fn with_device_id(mut self, device_id: impl Into<String>) -> Self {
        self.device_id = Some(device_id.into());
        self
    }

    /// 添加群组ID上下文
    #[inline]
    pub fn with_group_id(mut self, group_id: impl Into<String>) -> Self {
        self.group_id = Some(group_id.into());
        self
    }

    /// 添加会话ID上下文
    #[inline]
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// 添加请求ID上下文
    #[inline]
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// 添加调试信息
    #[inline]
    pub fn with_debug_info(mut self, info: serde_json::Value) -> Self {
        self.debug_info = Some(info);
        self
    }

    /// 设置重试建议
    #[inline]
    pub fn with_retry_suggestion(mut self, suggestion: RetrySuggestion) -> Self {
        self.retry_suggestion = Some(suggestion);
        self
    }

    /// 设置影响范围
    #[inline]
    pub fn with_impact_scope(mut self, scope: ImpactScope) -> Self {
        self.impact_scope = Some(scope);
        self
    }
}

/// 详细的错误信息结构
///
/// 提供完整的错误信息，包括根因分析和调试上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedError {
    /// 错误码
    pub code: ErrorCode,

    /// 错误分类
    pub category: ErrorCategory,

    /// 用户友好的简短描述
    pub message: String,

    /// 技术细节（供开发者查看）
    pub technical_details: String,

    /// 错误上下文
    pub context: ErrorContext,

    /// 根本原因（如果有链式错误）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_cause: Option<Box<DetailedError>>,

    /// 文档链接（用于自助排查）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation_url: Option<&'static str>,
}

impl DetailedError {
    /// 创建一个新的详细错误
    #[inline]
    pub fn new(
        code: ErrorCode,
        category: ErrorCategory,
        message: String,
        technical_details: String,
        location: &'static str,
    ) -> Self {
        let context = ErrorContext::new(location, technical_details.clone());
        Self {
            code,
            category,
            message,
            technical_details,
            context,
            root_cause: None,
            documentation_url: None,
        }
    }

    /// 添加设备ID上下文
    #[inline]
    pub fn with_device_id(mut self, device_id: impl Into<String>) -> Self {
        self.context.device_id = Some(device_id.into());
        self
    }

    /// 添加群组ID上下文
    #[inline]
    pub fn with_group_id(mut self, group_id: impl Into<String>) -> Self {
        self.context.group_id = Some(group_id.into());
        self
    }

    /// 添加调试信息
    #[inline]
    pub fn with_debug_info(mut self, info: serde_json::Value) -> Self {
        self.context.debug_info = Some(info);
        self
    }

    /// 设置重试建议
    #[inline]
    pub fn with_retry_suggestion(mut self, suggestion: RetrySuggestion) -> Self {
        self.context.retry_suggestion = Some(suggestion);
        self
    }

    /// 添加根因错误
    #[inline]
    pub fn with_root_cause(mut self, cause: DetailedError) -> Self {
        self.root_cause = Some(Box::new(cause));
        self
    }

    /// 设置文档链接
    #[inline]
    pub fn with_docs(mut self, url: &'static str) -> Self {
        self.documentation_url = Some(url);
        self
    }

    /// 转换为 JSON 字符串
    #[inline]
    pub fn to_json(&self) -> std::result::Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// 从 JSON 字符串解析
    #[inline]
    pub fn from_json(s: &'static str) -> std::result::Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

impl fmt::Display for DetailedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} - {} (Location: {})",
            self.code, self.message, self.technical_details, self.context.location
        )
    }
}

/// 增强的主流错误类型
///
/// 提供丰富的错误信息和便利的工厂方法
///
/// # 使用示例
///
/// ```
/// use xpush::core::error::{XPushError, Result};
///
/// fn example() -> Result<()> {
///     Err(XPushError::device_not_found("device-001", file!()))
/// }
/// ```
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
#[error("{message}")]
pub struct XPushError {
    /// 错误码
    pub code: ErrorCode,

    /// 错误分类
    pub category: ErrorCategory,

    /// 用户友好的错误消息
    pub message: String,

    /// 错误上下文
    #[serde(default)]
    pub context: ErrorContext,

    /// 链式错误
    #[serde(default)]
    pub source: Option<Box<XPushError>>,

    /// 文档链接
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation_url: Option<&'static str>,
}

impl XPushError {
    /// 创建新错误（内部方法）
    fn new_internal(
        code: ErrorCode,
        category: ErrorCategory,
        message: String,
        technical_details: &str,
        location: &'static str,
    ) -> Self {
        Self {
            code,
            category,
            message,
            context: ErrorContext::new(location, technical_details.to_string()),
            source: None,
            documentation_url: None,
        }
    }

    // ============ 通道错误 (02xx) ============

    /// 通道初始化失败 (0201)
    ///
    /// 当通道模块初始化失败时返回此错误
    ///
    /// # Arguments
    ///
    /// * `details` - 失败的详细信息
    /// * `location` - 错误发生位置（使用 `file!()` 宏）
    ///
    /// # Example
    ///
    /// ```
    /// use xpush::core::error::XPushError;
    ///
    /// let error = XPushError::channel_init_failed("Bluetooth not available", file!());
    /// ```
    #[inline]
    pub fn channel_init_failed<S: Into<String>>(details: S, location: &'static str) -> Self {
        Self::new_internal(
            ErrorCode(0201),
            ErrorCategory::Channel,
            "通道初始化失败".to_string(),
            &format!("Failed to initialize channel: {}", details.into()),
            location,
        )
    }

    /// 通道连接断开 (0202)
    ///
    /// 当已建立的连接意外断开时返回此错误
    #[inline]
    pub fn channel_disconnected<S: Into<String>>(reason: S, location: &'static str) -> Self {
        Self::new_internal(
            ErrorCode(0202),
            ErrorCategory::Channel,
            "通道连接断开".to_string(),
            &format!("Channel disconnected: {}", reason.into()),
            location,
        )
        .with_retry_suggestion(RetrySuggestion::Retryable {
            max_attempts: 3,
            base_delay_ms: 1000,
        })
    }

    /// 通道消息发送失败 (0203)
    ///
    /// 当发送消息失败时返回此错误
    #[inline]
    pub fn channel_send_failed<S: Into<String>>(
        target: S,
        error: S,
        location: &'static str,
    ) -> Self {
        Self::new_internal(
            ErrorCode(0203),
            ErrorCategory::Channel,
            "消息发送失败".to_string(),
            &format!("Failed to send message to {}: {}", target.into(), error.into()),
            location,
        )
        .with_retry_suggestion(RetrySuggestion::Retryable {
            max_attempts: 3,
            base_delay_ms: 1000,
        })
    }

    /// 通道消息接收超时 (0204)
    ///
    /// 当等待消息超时时返回此错误
    #[inline]
    pub fn channel_receive_timeout<S: Into<String>>(channel: S, location: &'static str) -> Self {
        Self::new_internal(
            ErrorCode(0204),
            ErrorCategory::Channel,
            "消息接收超时".to_string(),
            &format!("Timeout waiting for message on channel: {}", channel.into()),
            location,
        )
        .with_retry_suggestion(RetrySuggestion::Retryable {
            max_attempts: 2,
            base_delay_ms: 2000,
        })
    }

    // ============ 加密错误 (03xx) ============

    /// 加密初始化失败 (0301)
    ///
    /// 当加密模块初始化失败时返回此错误
    #[inline]
    pub fn crypto_init_failed<S: Into<String>>(details: S, location: &'static str) -> Self {
        Self::new_internal(
            ErrorCode(0301),
            ErrorCategory::Crypto,
            "加密模块初始化失败".to_string(),
            &format!("Failed to initialize crypto module: {}", details.into()),
            location,
        )
    }

    /// 密钥派生失败 (0302)
    ///
    /// 当密钥派生操作失败时返回此错误
    #[inline]
    pub fn key_derivation_failed<S: Into<String>>(
        algorithm: S,
        reason: S,
        location: &'static str,
    ) -> Self {
        Self::new_internal(
            ErrorCode(0302),
            ErrorCategory::Crypto,
            "密钥派生失败".to_string(),
            &format!(
                "Key derivation failed for {}: {}",
                algorithm.into(),
                reason.into()
            ),
            location,
        )
    }

    /// 加密操作失败 (0303)
    ///
    /// 当加密或解密操作失败时返回此错误
    #[inline]
    pub fn encryption_failed<S: Into<String>>(
        operation: S,
        reason: S,
        location: &'static str,
    ) -> Self {
        Self::new_internal(
            ErrorCode(0303),
            ErrorCategory::Crypto,
            "加密操作失败".to_string(),
            &format!(
                "Encryption/decryption failed during {}: {}",
                operation.into(),
                reason.into()
            ),
            location,
        )
        .with_docs("https://docs.xpush.io/errors/0303")
    }

    /// 无效的密文 (0304)
    ///
    /// 当验证密文失败时返回此错误
    #[inline]
    pub fn invalid_ciphertext<S: Into<String>>(reason: S, location: &'static str) -> Self {
        Self::new_internal(
            ErrorCode(0304),
            ErrorCategory::Crypto,
            "无效的密文数据".to_string(),
            &format!("Invalid ciphertext: {}", reason.into()),
            location,
        )
        .with_docs("https://docs.xpush.io/errors/0304")
    }

    /// 签名验证失败 (0305)
    ///
    /// 当数字签名验证失败时返回此错误
    #[inline]
    pub fn signature_verification_failed<S: Into<String>>(
        key_id: S,
        reason: S,
        location: &'static str,
    ) -> Self {
        Self::new_internal(
            ErrorCode(0305),
            ErrorCategory::Crypto,
            "签名验证失败".to_string(),
            &format!(
                "Signature verification failed for key {}: {}",
                key_id.into(),
                reason.into()
            ),
            location,
        )
        .with_retry_suggestion(RetrySuggestion::ManualIntervention)
    }

    // ============ 群组错误 (04xx) ============

    /// 群组不存在 (0401)
    ///
    /// 当尝试操作不存在的群组时返回此错误
    #[inline]
    pub fn group_not_found<S: Into<String>>(group_id: S, location: &'static str) -> Self {
        let group_id_str = group_id.into();
        Self::new_internal(
            ErrorCode(0401),
            ErrorCategory::Group,
            "群组不存在".to_string(),
            &format!("Group not found: {}", group_id_str),
            location,
        )
        .with_group_id(group_id_str)
    }

    /// 群组已存在 (0402)
    ///
    /// 当尝试创建已存在的群组时返回此错误
    #[inline]
    pub fn group_already_exists<S: Into<String>>(group_id: S, location: &'static str) -> Self {
        let group_id_str = group_id.into();
        Self::new_internal(
            ErrorCode(0402),
            ErrorCategory::Group,
            "群组已存在".to_string(),
            &format!("Group already exists: {}", group_id_str),
            location,
        )
        .with_group_id(group_id_str)
    }

    /// 非群组成员 (0403)
    ///
    /// 当非群组成员尝试执行需要成员资格的操作时返回此错误
    #[inline]
    pub fn not_group_member<S: Into<String>>(
        group_id: S,
        user_id: S,
        location: &'static str,
    ) -> Self {
        let group_id_str = group_id.into();
        let user_id_str = user_id.into();
        Self::new_internal(
            ErrorCode(0403),
            ErrorCategory::Group,
            "非群组成员无权操作".to_string(),
            &format!(
                "User {} is not a member of group {}",
                user_id_str, group_id_str
            ),
            location,
        )
        .with_group_id(group_id_str)
    }

    /// 群组成员已满 (0404)
    ///
    /// 当群组已达到最大成员数时返回此错误
    #[inline]
    pub fn group_full<S: Into<String>>(group_id: S, max_members: u32, location: &'static str) -> Self {
        let group_id_str = group_id.into();
        Self::new_internal(
            ErrorCode(0404),
            ErrorCategory::Group,
            "群组成员数量已达上限".to_string(),
            &format!("Group {} is full (max {} members)", group_id_str, max_members),
            location,
        )
        .with_group_id(group_id_str)
    }

    /// 群组创建失败 (0405)
    ///
    /// 当创建群组失败时返回此错误
    #[inline]
    pub fn group_creation_failed<S: Into<String>>(
        group_id: S,
        reason: S,
        location: &'static str,
    ) -> Self {
        let group_id_str = group_id.into();
        let reason_str = reason.into();
        Self::new_internal(
            ErrorCode(0405),
            ErrorCategory::Group,
            "创建群组失败".to_string(),
            &format!("Failed to create group {}: {}", group_id_str, reason_str),
            location,
        )
        .with_group_id(group_id_str)
    }

    /// 群组邀请失败 (0406)
    ///
    /// 当发送群组邀请失败时返回此错误
    #[inline]
    pub fn group_invite_failed<S: Into<String>>(
        group_id: S,
        user_id: S,
        reason: S,
        location: &'static str,
    ) -> Self {
        let group_id_str = group_id.into();
        let user_id_str = user_id.into();
        let reason_str = reason.into();
        Self::new_internal(
            ErrorCode(0406),
            ErrorCategory::Group,
            "发送群组邀请失败".to_string(),
            &format!(
                "Failed to invite user {} to group {}: {}",
                user_id_str, group_id_str, reason_str
            ),
            location,
        )
        .with_group_id(group_id_str)
        .with_retry_suggestion(RetrySuggestion::Retryable {
            max_attempts: 2,
            base_delay_ms: 500,
        })
    }

    // ============ 设备错误 (05xx) ============

    /// 设备未找到 (0501)
    ///
    /// 当设备不存在或已被移除时返回此错误
    #[inline]
    pub fn device_not_found<S: Into<String>>(device_id: S, location: &'static str) -> Self {
        let device_id_str = device_id.into();
        Self::new_internal(
            ErrorCode(0501),
            ErrorCategory::Device,
            "设备未找到".to_string(),
            &format!("Device not found: {}", device_id_str),
            location,
        )
        .with_device_id(device_id_str)
    }

    /// 设备不在线 (0502)
    ///
    /// 当设备离线不可达时返回此错误
    #[inline]
    pub fn device_offline<S: Into<String>>(device_id: S, location: &'static str) -> Self {
        let device_id_str = device_id.into();
        Self::new_internal(
            ErrorCode(0502),
            ErrorCategory::Device,
            "设备离线".to_string(),
            &format!("Device {} is offline", device_id_str),
            location,
        )
        .with_device_id(device_id_str)
        .with_retry_suggestion(RetrySuggestion::Retryable {
            max_attempts: 5,
            base_delay_ms: 2000,
        })
    }

    // ============ 流媒体错误 (06xx) ============

    /// 流初始化失败 (0601)
    ///
    /// 当媒体流初始化失败时返回此错误
    #[inline]
    pub fn stream_init_failed<S: Into<String>>(
        stream_type: S,
        codec: S,
        location: &'static str,
    ) -> Self {
        let stream_type_str = stream_type.into();
        let codec_str = codec.into();
        Self::new_internal(
            ErrorCode(0601),
            ErrorCategory::Stream,
            "流初始化失败".to_string(),
            &format!(
                "Failed to initialize {} stream with codec {}",
                stream_type_str, codec_str
            ),
            location,
        )
        .with_docs("https://docs.xpush.io/errors/0601")
    }

    /// 流连接断开 (0602)
    ///
    /// 当媒体流连接意外断开时返回此错误
    #[inline]
    pub fn stream_disconnected<S: Into<String>>(
        stream_id: S,
        reason: S,
        location: &'static str,
    ) -> Self {
        let stream_id_str = stream_id.into();
        let reason_str = reason.into();
        Self::new_internal(
            ErrorCode(0602),
            ErrorCategory::Stream,
            "流连接断开".to_string(),
            &format!("Stream {} disconnected: {}", stream_id_str, reason_str),
            location,
        )
        .with_retry_suggestion(RetrySuggestion::Retryable {
            max_attempts: 3,
            base_delay_ms: 1000,
        })
    }

    /// 无效的负载类型 (0603)
    ///
    /// 当收到无效的媒体负载类型时返回此错误
    #[inline]
    pub fn invalid_payload_type<S: Into<String>>(
        payload_type: S,
        expected: &[&str],
        location: &'static str,
    ) -> Self {
        let payload_type_str = payload_type.into();
        Self::new_internal(
            ErrorCode(0603),
            ErrorCategory::Stream,
            "无效的负载类型".to_string(),
            &format!(
                "Invalid payload type: {} (expected one of: {:?})",
                payload_type_str, expected
            ),
            location,
        )
    }

    /// 带宽不足 (0604)
    ///
    /// 当网络带宽不足以支持所需传输时返回此错误
    #[inline]
    pub fn insufficient_bandwidth<S: Into<String>>(
        required: u64,
        available: u64,
        location: &'static str,
    ) -> Self {
        Self::new_internal(
            ErrorCode(0604),
            ErrorCategory::Stream,
            "网络带宽不足".to_string(),
            &format!(
                "Insufficient bandwidth: required {} kbps, available {} kbps",
                required, available
            ),
            location,
        )
        .with_retry_suggestion(RetrySuggestion::ManualIntervention)
    }

    // ============ 存储错误 (07xx) ============

    /// 存储初始化失败 (0701)
    ///
    /// 当存储模块初始化失败时返回此错误
    #[inline]
    pub fn storage_init_failed<S: Into<String>>(
        storage_type: S,
        reason: S,
        location: &'static str,
    ) -> Self {
        let storage_type_str = storage_type.into();
        let reason_str = reason.into();
        Self::new_internal(
            ErrorCode(0701),
            ErrorCategory::Storage,
            "存储初始化失败".to_string(),
            &format!(
                "Failed to initialize {} storage: {}",
                storage_type_str, reason_str
            ),
            location,
        )
    }

    /// 数据写入失败 (0702)
    ///
    /// 当写入数据失败时返回此错误
    #[inline]
    pub fn storage_write_failed<S: Into<String>>(
        key: S,
        reason: S,
        location: &'static str,
    ) -> Self {
        let key_str = key.into();
        let reason_str = reason.into();
        Self::new_internal(
            ErrorCode(0702),
            ErrorCategory::Storage,
            "数据写入失败".to_string(),
            &format!("Failed to write data for key {}: {}", key_str, reason_str),
            location,
        )
        .with_retry_suggestion(RetrySuggestion::Retryable {
            max_attempts: 3,
            base_delay_ms: 100,
        })
    }

    /// 数据读取失败 (0703)
    ///
    /// 当读取数据失败时返回此错误
    #[inline]
    pub fn storage_read_failed<S: Into<String>>(key: S, reason: S, location: &'static str) -> Self {
        let key_str = key.into();
        let reason_str = reason.into();
        Self::new_internal(
            ErrorCode(0703),
            ErrorCategory::Storage,
            "数据读取失败".to_string(),
            &format!("Failed to read data for key {}: {}", key_str, reason_str),
            location,
        )
    }

    // ============ 协议错误 (08xx) ============

    /// 协议版本不兼容 (0801)
    ///
    /// 当本地和远程协议版本不兼容时返回此错误
    #[inline]
    pub fn protocol_version_mismatch<S: Into<String>>(
        local: S,
        remote: S,
        location: &'static str,
    ) -> Self {
        let local_str = local.into();
        let remote_str = remote.into();
        Self::new_internal(
            ErrorCode(0801),
            ErrorCategory::Protocol,
            "协议版本不兼容".to_string(),
            &format!(
                "Protocol version mismatch: local={}, remote={}",
                local_str, remote_str
            ),
            location,
        )
        .with_docs("https://docs.xpush.io/errors/0801")
    }

    /// 无效的协议消息 (0802)
    ///
    /// 当收到格式错误的协议消息时返回此错误
    #[inline]
    pub fn invalid_protocol_message<S: Into<String>>(
        message_type: S,
        reason: S,
        location: &'static str,
    ) -> Self {
        let message_type_str = message_type.into();
        let reason_str = reason.into();
        Self::new_internal(
            ErrorCode(0802),
            ErrorCategory::Protocol,
            "无效的协议消息".to_string(),
            &format!(
                "Invalid {} message: {}",
                message_type_str, reason_str
            ),
            location,
        )
    }

    // ============ 能力匹配错误 (09xx) ============

    /// 能力不匹配 (0901)
    ///
    /// 当设备能力不满足要求时返回此错误
    #[inline]
    pub fn capability_mismatch<S: Into<String>>(
        device_id: S,
        required: S,
        actual: S,
        location: &'static str,
    ) -> Self {
        let device_id_str = device_id.into();
        let required_str = required.into();
        let actual_str = actual.into();
        Self::new_internal(
            ErrorCode(0901),
            ErrorCategory::Capability,
            "设备能力不满足要求".to_string(),
            &format!(
                "Device {} capability mismatch: required {} but got {}",
                device_id_str, required_str, actual_str
            ),
            location,
        )
        .with_device_id(device_id_str)
    }

    // ============ 通用系统错误 (01xx) ============

    /// 超时错误 (0101)
    ///
    /// 当操作超时时返回此错误
    #[inline]
    pub fn timeout<S: Into<String>>(operation: S, duration_ms: u64, location: &'static str) -> Self {
        let operation_str = operation.into();
        Self::new_internal(
            ErrorCode(0101),
            ErrorCategory::System,
            "操作超时".to_string(),
            &format!(
                "Operation '{}' timed out after {}ms",
                operation_str, duration_ms
            ),
            location,
        )
        .with_retry_suggestion(RetrySuggestion::Retryable {
            max_attempts: 2,
            base_delay_ms: 1000,
        })
    }

    /// 无效输入 (0102)
    ///
    /// 当输入参数无效时返回此错误
    #[inline]
    pub fn invalid_input<S: Into<String>>(field: S, reason: S, location: &'static str) -> Self {
        let field_str = field.into();
        let reason_str = reason.into();
        Self::new_internal(
            ErrorCode(0102),
            ErrorCategory::System,
            "输入参数无效".to_string(),
            &format!("Invalid input for {}: {}", field_str, reason_str),
            location,
        )
    }

    /// 序列化错误 (0103)
    ///
    /// 当数据序列化或反序列化失败时返回此错误
    #[inline]
    pub fn serialization_failed<S: Into<String>>(
        operation: S,
        reason: S,
        location: &'static str,
    ) -> Self {
        let operation_str = operation.into();
        let reason_str = reason.into();
        Self::new_internal(
            ErrorCode(0103),
            ErrorCategory::System,
            "数据序列化失败".to_string(),
            &format!(
                "Serialization failed during {}: {}",
                operation_str, reason_str
            ),
            location,
        )
    }

    /// 资源耗尽 (0104)
    ///
    /// 当系统资源耗尽时返回此错误
    #[inline]
    pub fn resource_exhausted<S: Into<String>>(
        resource: S,
        current: u64,
        limit: u64,
        location: &'static str,
    ) -> Self {
        let resource_str = resource.into();
        Self::new_internal(
            ErrorCode(0104),
            ErrorCategory::System,
            "资源耗尽".to_string(),
            &format!(
                "Resource exhausted: {} (current={}, limit={})",
                resource_str, current, limit
            ),
            location,
        )
    }

    /// 没有找到合适的路由 (0105)
    ///
    /// 当没有找到合适的路由时返回此错误
    #[inline]
    pub fn no_route_found<S: Into<String>>(
        destination: S,
        reason: S,
        location: &'static str,
    ) -> Self {
        let destination_str = destination.into();
        let reason_str = reason.into();
        Self::new_internal(
            ErrorCode(0105),
            ErrorCategory::System,
            "未找到可用路由".to_string(),
            &format!(
                "No suitable route found for {}: {}",
                destination_str, reason_str
            ),
            location,
        )
        .with_retry_suggestion(RetrySuggestion::Retryable {
            max_attempts: 3,
            base_delay_ms: 500,
        })
    }

    // ============ 便利方法 ============

    /// 添加设备ID上下文
    #[inline]
    pub fn with_device_id<S: Into<String>>(mut self, device_id: S) -> Self {
        self.context.device_id = Some(device_id.into());
        self
    }

    /// 添加群组ID上下文
    #[inline]
    pub fn with_group_id<S: Into<String>>(mut self, group_id: S) -> Self {
        self.context.group_id = Some(group_id.into());
        self
    }

    /// 添加会话ID上下文
    #[inline]
    pub fn with_session_id<S: Into<String>>(mut self, session_id: S) -> Self {
        self.context.session_id = Some(session_id.into());
        self
    }

    /// 添加请求ID上下文
    #[inline]
    pub fn with_request_id<S: Into<String>>(mut self, request_id: S) -> Self {
        self.context.request_id = Some(request_id.into());
        self
    }

    /// 添加调试信息
    #[inline]
    pub fn with_debug_info(mut self, info: serde_json::Value) -> Self {
        self.context.debug_info = Some(info);
        self
    }

    /// 设置重试建议
    #[inline]
    pub fn with_retry_suggestion(mut self, suggestion: RetrySuggestion) -> Self {
        self.context.retry_suggestion = Some(suggestion);
        self
    }

    /// 设置文档链接
    #[inline]
    pub fn with_docs(mut self, url: &'static str) -> Self {
        self.documentation_url = Some(url);
        self
    }

    /// 设置链式错误源
    #[inline]
    pub fn with_source(mut self, source: XPushError) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    /// 获取错误码
    #[inline]
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// 获取错误分类
    #[inline]
    pub fn category(&self) -> ErrorCategory {
        self.category
    }

    /// 获取错误消息
    #[inline]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// 获取原始消息
    #[inline]
    pub fn original_message(&self) -> &str {
        &self.context.original_message
    }

    /// 获取错误位置
    #[inline]
    pub fn location(&self) -> &'static str {
        self.context.location
    }

    /// 获取时间戳
    #[inline]
    pub fn timestamp(&self) -> chrono::DateTime<chrono::Utc> {
        self.context.timestamp
    }

    /// 检查是否可以重试
    #[inline]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self.context.retry_suggestion,
            Some(RetrySuggestion::Retryable { .. })
        )
    }

    /// 获取重试建议
    #[inline]
    pub fn retry_suggestion(&self) -> Option<RetrySuggestion> {
        self.context.retry_suggestion
    }

    /// 转换为 JSON 字符串
    #[inline]
    pub fn to_json(&self) -> std::result::Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// 转换为 DetailedError
    #[inline]
    pub fn to_detailed(self) -> DetailedError {
        let code = self.code;
        let category = self.category;
        let message = self.message.clone();
        let _location = self.context.location;
        let technical_details = self.context.original_message.clone();
        let context = self.context;
        let source = self.source.map(|s| {
            let xpush_error = *s;
            Box::new(xpush_error.to_detailed())
        });

        DetailedError {
            code,
            category,
            message,
            technical_details,
            context,
            root_cause: source,
            documentation_url: None,
        }
    }

    /// 获取链式错误的迭代器
    #[inline]
    pub fn source_iter(&self) -> SourceIter<'_> {
        SourceIter(Some(self))
    }

    /// 转换为日志格式
    ///
    /// 生成适合日志系统的格式化字符串
    #[inline]
    pub fn to_log_string(&self) -> String {
        let mut result = format!(
            "[{}] {} | Category: {} | Location: {} | Time: {}",
            self.code,
            self.message,
            self.category,
            self.context.location,
            self.context.timestamp
        );

        if let Some(ref device_id) = self.context.device_id {
            result.push_str(&format!(" | Device: {}", device_id));
        }
        if let Some(ref group_id) = self.context.group_id {
            result.push_str(&format!(" | Group: {}", group_id));
        }
        if let Some(ref suggestion) = self.context.retry_suggestion {
            result.push_str(&format!(" | Retry: {:?}", suggestion));
        }

        result
    }
}

/// 链式错误迭代器
pub struct SourceIter<'a>(Option<&'a XPushError>);

impl<'a> Iterator for SourceIter<'a> {
    type Item = &'a XPushError;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.0.take()?;
        self.0 = current.source.as_deref();
        Some(current)
    }
}

/// 从标准错误类型转换实现

impl From<std::io::Error> for XPushError {
    #[inline]
    fn from(error: std::io::Error) -> Self {
        Self::new_internal(
            ErrorCode(0101),
            ErrorCategory::System,
            "IO操作失败".to_string(),
            &format!("IO error: {}", error),
            file!(),
        )
        .with_retry_suggestion(RetrySuggestion::Retryable {
            max_attempts: 3,
            base_delay_ms: 100,
        })
    }
}

impl From<serde_json::Error> for XPushError {
    #[inline]
    fn from(error: serde_json::Error) -> Self {
        Self::new_internal(
            ErrorCode(0103),
            ErrorCategory::System,
            "JSON序列化失败".to_string(),
            &format!("JSON error: {}", error),
            file!(),
        )
    }
}

impl From<DetailedError> for XPushError {
    #[inline]
    fn from(detailed: DetailedError) -> Self {
        Self {
            code: detailed.code,
            category: detailed.category,
            message: detailed.message,
            context: detailed.context,
            source: detailed.root_cause.map(|rc| Box::new(XPushError::from(*rc))),
            documentation_url: None,
        }
    }
}

/// 便捷类型别名
pub type Result<T> = std::result::Result<T, XPushError>;

/// 错误统计信息
///
/// 用于收集和报告错误统计数据
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ErrorStatistics {
    /// 按错误码统计的计数
    counts: std::collections::HashMap<u16, u64>,

    /// 按类别统计的计数
    category_counts: std::collections::HashMap<String, u64>,

    /// 最后一次错误发生时间
    last_error_time: Option<chrono::DateTime<chrono::Utc>>,

    /// 错误发生的总时间线
    error_timeline: Vec<(chrono::DateTime<chrono::Utc>, u16)>,
}

impl ErrorStatistics {
    /// 创建一个新的统计实例
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// 记录一个错误
    pub fn record(&mut self, error: &XPushError) {
        let code = error.code.0;
        *self.counts.entry(code).or_insert(0) += 1;

        let category_name = error.category.name().to_string();
        *self.category_counts.entry(category_name).or_insert(0) += 1;

        self.last_error_time = Some(chrono::Utc::now());
        self.error_timeline
            .push((chrono::Utc::now(), code));
    }

    /// 获取错误总数
    #[inline]
    pub fn total_count(&self) -> u64 {
        self.counts.values().sum()
    }

    /// 获取最常见的错误
    ///
    /// # Arguments
    /// * `n` - 返回前 N 个常见错误
    ///
    /// # Returns
    /// 错误码和对应计数的列表，按出现频率降序排列
    pub fn get_most_common(&self, n: usize) -> Vec<(u16, u64)> {
        let mut errors: Vec<_> = self.counts.iter().collect();
        errors.sort_by(|a, b| b.1.cmp(a.1));
        errors
            .into_iter()
            .take(n)
            .map(|(&code, &count)| (code, count))
            .collect()
    }

    /// 获取按类别分组的错误统计
    #[inline]
    pub fn get_by_category(&self) -> &std::collections::HashMap<String, u64> {
        &self.category_counts
    }

    /// 获取最近 N 个错误
    #[inline]
    pub fn get_recent(&self, n: usize) -> Vec<(chrono::DateTime<chrono::Utc>, u16)> {
        self.error_timeline
            .iter()
            .rev()
            .take(n)
            .cloned()
            .collect()
    }

    /// 获取特定错误码的计数
    #[inline]
    pub fn get_count(&self, code: u16) -> u64 {
        *self.counts.get(&code).unwrap_or(&0)
    }

    /// 获取特定类别的错误计数
    #[inline]
    pub fn get_category_count(&self, category: ErrorCategory) -> u64 {
        *self
            .category_counts
            .get(category.name())
            .unwrap_or(&0)
    }

    /// 获取最后错误时间
    #[inline]
    pub fn last_error(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.last_error_time
    }

    /// 检查是否有特定类型的错误
    #[inline]
    pub fn has_errors(&self) -> bool {
        !self.counts.is_empty()
    }

    /// 清空统计
    #[inline]
    pub fn clear(&mut self) {
        self.counts.clear();
        self.category_counts.clear();
        self.last_error_time = None;
        self.error_timeline.clear();
    }
}

/// 错误日志格式化辅助函数
///
/// 生成适合日志系统的格式化错误字符串
#[inline]
pub fn format_error_for_log(error: &XPushError) -> String {
    error.to_log_string()
}

/// 将错误转换为用户友好的消息
///
/// 去除技术细节，提供用户可理解的消息
#[inline]
pub fn to_user_message(error: &XPushError) -> String {
    error.message.clone()
}

/// 错误处理宏
///
/// 提供更简洁的错误创建语法
///
/// # 使用示例
///
/// ```ignore
/// use xpush::{xpush_error, core::error::XPushError};
///
/// fn example() -> Result<(), XPushError> {
///     Err(xpush_error!(0201, Channel, "通道初始化失败", "Bluetooth not available"))
/// }
/// ```
#[macro_export]
macro_rules! xpush_error {
    ($code:expr, $category:ident, $message:expr, $details:expr) => {
        XPushError::new_internal(
            ErrorCode($code),
            ErrorCategory::$category,
            $message.to_string(),
            $details,
            file!(),
        )
    };
    ($code:expr, $category:ident, $message:expr, $details:expr, $($method:ident($value:expr)),+) => {
        {
            let mut error = XPushError::new_internal(
                ErrorCode($code),
                ErrorCategory::$category,
                $message.to_string(),
                $details,
                file!(),
            );
            $(
                error = error.$method($value);
            )+
            error
        }
    };
}

/// 创建带上下文的错误
///
/// # 使用示例
///
/// ```ignore
/// use xpush::{with_context, core::error::XPushError};
///
/// fn example() -> Result<(), XPushError> {
///     let result = std::fs::read_to_string("config.json")
///         .map_err(|e| with_context!(e, "config.json", "device-001"))?;
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! with_context {
    ($error:expr, $key:expr, $device_id:expr) => {
        $crate::core::error::XPushError::storage_read_failed(
            $key,
            format!("{}", $error),
            file!(),
        )
        .with_device_id($device_id)
    };
}