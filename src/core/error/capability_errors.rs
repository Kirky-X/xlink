//! 能力匹配相关错误 (09xx)

use crate::core::error::{ErrorCategory, ErrorCode, XPushError};

impl XPushError {
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
            ErrorCode(901),
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
}
