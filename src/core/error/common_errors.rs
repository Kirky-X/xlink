//! 通用系统错误 (01xx)

use crate::core::error::{ErrorCategory, ErrorCode, RetrySuggestion, XPushError};

impl XPushError {
    /// 超时错误 (0101)
    ///
    /// 当操作超时时返回此错误
    #[inline]
    pub fn timeout<S: Into<String>>(
        operation: S,
        duration_ms: u64,
        location: &'static str,
    ) -> Self {
        let operation_str = operation.into();
        Self::new_internal(
            ErrorCode(101),
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
            ErrorCode(102),
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
            ErrorCode(103),
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
            ErrorCode(104),
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
            ErrorCode(105),
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
}
