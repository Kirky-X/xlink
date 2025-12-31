//! 设备相关错误 (05xx)

use crate::core::error::{ErrorCategory, ErrorCode, RetrySuggestion, XLinkError};

impl XLinkError {
    /// 设备未找到 (0501)
    ///
    /// 当设备不存在或已被移除时返回此错误
    #[inline]
    pub fn device_not_found<S: Into<String>>(device_id: S, location: &'static str) -> Self {
        let device_id_str = device_id.into();
        Self::new_internal(
            ErrorCode(501),
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
            ErrorCode(502),
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
}
