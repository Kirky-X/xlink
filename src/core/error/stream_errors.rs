//! 流媒体相关错误 (06xx)

use crate::core::error::{ErrorCategory, ErrorCode, RetrySuggestion, XLinkError};

impl XLinkError {
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
            ErrorCode(601),
            ErrorCategory::Stream,
            "流初始化失败".to_string(),
            &format!(
                "Failed to initialize {} stream with codec {}",
                stream_type_str, codec_str
            ),
            location,
        )
        .with_docs("https://docs.xlink.io/errors/0601")
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
            ErrorCode(602),
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
            ErrorCode(603),
            ErrorCategory::Stream,
            "无效的负载类型".to_string(),
            &format!(
                "Invalid payload type: {} (expected one of: {:?})",
                payload_type_str, expected
            ),
            location,
        )
        .with_docs("https://docs.xlink.io/errors/0603")
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
            ErrorCode(604),
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
}
