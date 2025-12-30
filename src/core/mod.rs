//! Core module - 核心功能模块
//!
//! 提供错误处理、指标收集、类型定义和 trait 接口
//!
//! # 模块结构
//!
//! - [`error`] - 增强的错误类型定义
//! - [`metrics`] - 性能指标收集
//! - [`traits`] - 核心 trait 接口
//! - [`types`] - 核心数据类型

pub mod error;
pub mod metrics;
pub mod traits;
pub mod types;

// 重新导出常用类型，便于使用
pub use error::{
    ErrorCategory, ErrorCode, ErrorContext, ErrorStatistics, ImpactScope, Result, RetrySuggestion,
    XPushError,
};
