//! 群组相关错误 (04xx)

use crate::core::error::{ErrorCategory, ErrorCode, RetrySuggestion, XPushError};

impl XPushError {
    /// 群组不存在 (0401)
    ///
    /// 当尝试操作不存在的群组时返回此错误
    #[inline]
    pub fn group_not_found<S: Into<String>>(group_id: S, location: &'static str) -> Self {
        let group_id_str = group_id.into();
        Self::new_internal(
            ErrorCode(401),
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
            ErrorCode(402),
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
            ErrorCode(403),
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
    pub fn group_full<S: Into<String>>(
        group_id: S,
        max_members: u32,
        location: &'static str,
    ) -> Self {
        let group_id_str = group_id.into();
        Self::new_internal(
            ErrorCode(404),
            ErrorCategory::Group,
            "群组成员数量已达上限".to_string(),
            &format!(
                "Group {} is full (max {} members)",
                group_id_str, max_members
            ),
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
            ErrorCode(405),
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
            ErrorCode(406),
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
}
