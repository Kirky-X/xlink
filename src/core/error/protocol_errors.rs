//! 协议相关错误 (08xx)

use crate::core::error::{ErrorCategory, ErrorCode, XLinkError};

impl XLinkError {
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
            ErrorCode(801),
            ErrorCategory::Protocol,
            "协议版本不兼容".to_string(),
            &format!(
                "Protocol version mismatch: local={}, remote={}",
                local_str, remote_str
            ),
            location,
        )
        .with_docs("https://docs.xlink.io/errors/0801")
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
            ErrorCode(802),
            ErrorCategory::Protocol,
            "无效的协议消息".to_string(),
            &format!("Invalid {} message: {}", message_type_str, reason_str),
            location,
        )
    }
}
