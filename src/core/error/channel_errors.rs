//! 通道相关错误 (02xx)

use crate::core::error::{ErrorCategory, ErrorCode, RetrySuggestion, XPushError};

impl XPushError {
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
    /// use xlink::core::error::XPushError;
    ///
    /// let error = XPushError::channel_init_failed("Bluetooth not available", file!());
    /// ```
    #[inline]
    pub fn channel_init_failed<S: Into<String>>(details: S, location: &'static str) -> Self {
        Self::new_internal(
            ErrorCode(201),
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
        let reason_str = reason.into();
        Self::new_internal(
            ErrorCode(202),
            ErrorCategory::Channel,
            "通道连接断开".to_string(),
            &format!("Channel disconnected: {}", reason_str),
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
            ErrorCode(203),
            ErrorCategory::Channel,
            "消息发送失败".to_string(),
            &format!(
                "Failed to send message to {}: {}",
                target.into(),
                error.into()
            ),
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
            ErrorCode(204),
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
}
