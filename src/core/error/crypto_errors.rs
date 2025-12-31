//! 加密相关错误 (03xx)

use crate::core::error::{ErrorCategory, ErrorCode, RetrySuggestion, XLinkError};

impl XLinkError {
    /// 加密初始化失败 (0301)
    ///
    /// 当加密模块初始化失败时返回此错误
    #[inline]
    pub fn crypto_init_failed<S: Into<String>>(details: S, location: &'static str) -> Self {
        let details_str = details.into();
        Self::new_internal(
            ErrorCode(301),
            ErrorCategory::Crypto,
            "加密模块初始化失败".to_string(),
            &format!("Failed to initialize crypto module: {}", details_str),
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
            ErrorCode(302),
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
            ErrorCode(303),
            ErrorCategory::Crypto,
            "加密操作失败".to_string(),
            &format!(
                "Encryption/decryption failed during {}: {}",
                operation.into(),
                reason.into()
            ),
            location,
        )
        .with_docs("https://docs.xlink.io/errors/0303")
    }

    /// 无效的密文 (0304)
    ///
    /// 当验证密文失败时返回此错误
    #[inline]
    pub fn invalid_ciphertext<S: Into<String>>(reason: S, location: &'static str) -> Self {
        let reason_str = reason.into();
        Self::new_internal(
            ErrorCode(304),
            ErrorCategory::Crypto,
            "无效的密文数据".to_string(),
            &format!("Invalid ciphertext: {}", reason_str),
            location,
        )
        .with_docs("https://docs.xlink.io/errors/0304")
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
            ErrorCode(305),
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
}
