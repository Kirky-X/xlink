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
//! use xlink::core::error::{XPushError, Result};
//!
//! fn example() -> Result<()> {
//!     Err(XPushError::device_not_found("device-001", file!()))
//! }
//! ```

pub mod capability_errors;
pub mod channel_errors;
pub mod common_errors;
pub mod crypto_errors;
pub mod device_errors;
pub mod group_errors;
pub mod protocol_errors;
pub mod storage_errors;
pub mod stream_errors;

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// 错误码类型 - 用于唯一标识每种错误
///
/// # 格式
/// `XX-YYYY` 格式，其中：
/// - `XX`: 模块分类 (01-08)
/// - `YYYY`: 具体错误序号
///
/// # Example
/// ```
/// use xlink::core::error::ErrorCode;
///
/// let code = ErrorCode(203);
/// assert_eq!(format!("{}", code), "XP-0203");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorCode(pub u16);

impl ErrorCode {
    /// 获取模块分类
    ///
    /// 返回错误码的高两位数字，表示错误所属的模块
    ///
    /// # Example
    /// ```
    /// use xlink::core::error::ErrorCode;
    ///
    /// let code = ErrorCode(203);
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
    /// use xlink::core::error::ErrorCode;
    ///
    /// let code = ErrorCode(203);
    /// assert_eq!(code.sequence(), 3);
    /// ```
    pub fn sequence(&self) -> u16 {
        self.0 % 100
    }
}

impl std::str::FromStr for ErrorCode {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let value = s.parse::<u16>().map_err(|e| e.to_string())?;
        if value < 10000 {
            Ok(Self(value))
        } else {
            Err("Error code must be less than 10000".to_string())
        }
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
            Self::System => (100, 199),
            Self::Channel => (200, 299),
            Self::Crypto => (300, 399),
            Self::Group => (400, 499),
            Self::Device => (500, 599),
            Self::Stream => (600, 699),
            Self::Storage => (700, 799),
            Self::Protocol => (800, 899),
            Self::Capability => (900, 999),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RetrySuggestion {
    /// 不需要重试
    /// 错误是确定性的，重试也不会改变结果
    #[default]
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

    /// 原始错误消息（使用 Box 减少内存占用）
    pub original_message: Box<str>,

    /// 附加的调试信息（序列化为 JSON，使用 Box 减少内存占用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug_info: Option<Box<serde_json::Value>>,

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
            original_message: original_message.into_boxed_str(),
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
        self.debug_info = Some(Box::new(info));
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
        self.context.debug_info = Some(Box::new(info));
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
/// use xlink::core::error::{XPushError, Result};
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

    /// 错误上下文（使用 Box 减少内存占用）
    #[serde(default)]
    pub context: Box<ErrorContext>,

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
            context: Box::new(ErrorContext::new(location, technical_details.to_string())),
            source: None,
            documentation_url: None,
        }
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
        self.context.debug_info = Some(Box::new(info));
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
        let technical_details = self.context.original_message.to_string();
        let context = *self.context;
        let source = self.source.map(|s| {
            let xlink_error = *s;
            Box::new(xlink_error.to_detailed())
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
            self.code, self.message, self.category, self.context.location, self.context.timestamp
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
            ErrorCode(101),
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
            ErrorCode(103),
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
            context: Box::new(detailed.context),
            source: detailed
                .root_cause
                .map(|rc| Box::new(XPushError::from(*rc))),
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
        self.error_timeline.push((chrono::Utc::now(), code));
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
        self.error_timeline.iter().rev().take(n).cloned().collect()
    }

    /// 获取特定错误码的计数
    #[inline]
    pub fn get_count(&self, code: u16) -> u64 {
        *self.counts.get(&code).unwrap_or(&0)
    }

    /// 获取特定类别的错误计数
    #[inline]
    pub fn get_category_count(&self, category: ErrorCategory) -> u64 {
        *self.category_counts.get(category.name()).unwrap_or(&0)
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
/// use xlink::{xlink_error, core::error::XPushError};
///
/// fn example() -> Result<(), XPushError> {
///     Err(xlink_error!(0201, Channel, "通道初始化失败", "Bluetooth not available"))
/// }
/// ```
#[macro_export]
macro_rules! xlink_error {
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
/// use xlink::{with_context, core::error::XPushError};
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
        $crate::core::error::XPushError::storage_read_failed($key, format!("{}", $error), file!())
            .with_device_id($device_id)
    };
}

// Re-export all error creation methods from submodules
// Note: Error creation methods are implemented on XPushError directly in submodules
