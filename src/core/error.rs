use crate::core::types::GroupId;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum XPushError {
    #[error("Channel error: {0}")]
    ChannelError(String),

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Crypto error: {0}")]
    CryptoError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("No suitable channel found for routing")]
    NoRouteFound,

    #[error("Timeout waiting for operation")]
    Timeout,

    #[error("Capability mismatch")]
    CapabilityMismatch,
    
    // 新增错误类型
    #[error("Group not found: {0}")]
    GroupNotFound(String),
    
    #[error("Not a member of group")]
    NotGroupMember,
    
    #[error("Stream error: {0}")]
    StreamError(String),
    
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    #[error("Group already exists: {0}")]
    GroupAlreadyExists(GroupId),
}

pub type Result<T> = std::result::Result<T, XPushError>;