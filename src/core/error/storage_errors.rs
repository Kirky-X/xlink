//! 存储相关错误 (07xx)

use crate::core::error::{ErrorCategory, ErrorCode, RetrySuggestion, XPushError};

impl XPushError {
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
            ErrorCode(701),
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
            ErrorCode(702),
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
            ErrorCode(703),
            ErrorCategory::Storage,
            "数据读取失败".to_string(),
            &format!("Failed to read data for key {}: {}", key_str, reason_str),
            location,
        )
    }
}
