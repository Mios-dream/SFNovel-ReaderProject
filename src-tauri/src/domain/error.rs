//! 应用层错误类型。

use std::fmt;

/// 稳定的错误分类，供应用层决定回退策略。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ErrorKind {
    /// 调用方提供的参数不合法。
    InvalidInput,
    /// 当前请求缺少有效登录会话。
    Unauthenticated,
    /// 请求的本地或远程资源不存在。
    NotFound,
    /// 当前账号或进程无权访问资源。
    PermissionDenied,
    /// 上游服务或网络请求失败。
    Upstream,
    /// 本地文件系统读写失败。
    Storage,
    /// 不应由调用方处理的内部错误。
    Internal,
}

/// 带语义分类和应用层可读信息的错误。
#[derive(Debug)]
pub(crate) struct AppError {
    /// 稳定的错误分类。
    #[allow(dead_code)]
    kind: ErrorKind,
    /// 面向应用层或用户的消息。
    message: String,
}

impl AppError {
    /// 创建带稳定分类和应用层消息的错误。
    ///
    /// # 参数
    /// - `kind`：供应用层选择处理策略的错误分类。
    /// - `message`：可直接展示或写日志的错误消息。
    pub(crate) fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// 创建参数校验错误。
    pub(crate) fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidInput, message)
    }

    /// 创建登录状态缺失或失效错误。
    pub(crate) fn unauthenticated(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Unauthenticated, message)
    }

    /// 创建上游请求或响应错误。
    pub(crate) fn upstream(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Upstream, message)
    }

    /// 创建资源不存在错误。
    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, message)
    }

    /// 创建权限不足错误。
    pub(crate) fn permission_denied(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::PermissionDenied, message)
    }

    /// 创建本地存储错误。
    pub(crate) fn storage(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Storage, message)
    }

    /// 创建内部错误。
    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, message)
    }

    /// 消费错误并返回适合跨 IPC 传递的消息。
    pub(crate) fn into_message(self) -> String {
        self.message
    }

    /// 判断错误是否表示当前登录会话已失效。
    pub(crate) fn is_unauthenticated(&self) -> bool {
        self.kind == ErrorKind::Unauthenticated
    }
}

impl fmt::Display for AppError {
    /// 输出面向用户或日志的纯文本消息。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AppError {}

impl From<String> for AppError {
    /// 把未分类字符串转换为内部错误。
    fn from(message: String) -> Self {
        Self::internal(message)
    }
}

impl From<&str> for AppError {
    /// 把未分类字符串切片转换为内部错误。
    fn from(message: &str) -> Self {
        Self::internal(message)
    }
}

/// 应用层结果别名。
pub(crate) type AppResult<T> = Result<T, AppError>;
