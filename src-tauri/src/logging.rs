//! 轻量日志门面。
//!
//! release 构建默认只输出 `warn` 及以上级别，调试时用环境变量 `SF_LOG=debug`
//! （或 `info`/`warn`/`error`/`off`）临时放开。宏在 `lib.rs` 中以 `#[macro_use]`
//! 引入，模块里直接写 `log_debug!` 即可。
//!
//! 这里刻意不引入日志框架：目前只需要级别过滤，多一个依赖不值当。

use std::fmt;
use std::sync::atomic::{AtomicU8, Ordering};

/// 日志级别，数值越大越详细。
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
#[repr(u8)]
pub(crate) enum Level {
    /// 只记录影响功能可用性的失败。
    Error = 1,
    /// 记录可降级但仍需关注的情况。
    Warn = 2,
    /// 记录关键流程节点。
    Info = 3,
    /// 记录解析统计与请求细节。
    Debug = 4,
    /// 关闭全部输出。
    Off = 5,
}

impl Level {
    /// 返回日志行前缀。
    fn label(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Off => "off",
        }
    }

    /// 解析环境变量取值，无法识别时返回 `None`。
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "error" => Some(Self::Error),
            "warn" | "warning" => Some(Self::Warn),
            "info" => Some(Self::Info),
            "debug" | "trace" => Some(Self::Debug),
            "off" | "none" => Some(Self::Off),
            _ => None,
        }
    }
}

/// 当前生效的最低输出级别。
static THRESHOLD: AtomicU8 = AtomicU8::new(Level::Warn as u8);

/// 按环境变量 `SF_LOG` 初始化级别；未设置时 debug 构建输出到 `info`。
pub(crate) fn init_from_env() {
    let level = std::env::var("SF_LOG")
        .ok()
        .and_then(|value| Level::parse(&value))
        .unwrap_or(if cfg!(debug_assertions) {
            Level::Info
        } else {
            Level::Warn
        });
    THRESHOLD.store(level as u8, Ordering::Relaxed);
}

/// 判断某个级别是否会被输出。
pub(crate) fn enabled(level: Level) -> bool {
    (level as u8) <= THRESHOLD.load(Ordering::Relaxed)
}

/// 输出一行日志。由宏调用，业务代码不要直接用。
pub(crate) fn emit(level: Level, args: fmt::Arguments<'_>) {
    if enabled(level) {
        eprintln!("[{}] {args}", level.label());
    }
}

/// 输出错误级日志。
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::logging::emit($crate::logging::Level::Error, format_args!($($arg)*))
    };
}

/// 输出警告级日志。
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::logging::emit($crate::logging::Level::Warn, format_args!($($arg)*))
    };
}

/// 输出信息级日志。
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::logging::emit($crate::logging::Level::Info, format_args!($($arg)*))
    };
}

/// 输出调试级日志。
macro_rules! log_debug {
    ($($arg:tt)*) => {
        $crate::logging::emit($crate::logging::Level::Debug, format_args!($($arg)*))
    };
}
