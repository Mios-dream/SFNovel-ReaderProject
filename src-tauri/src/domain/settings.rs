//! 持久化用户设置与字典结果。

use serde::{Deserialize, Serialize};

/// 应用设置根结构，按功能分类持久化。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppSettings {
    /// 下载请求与过程文件选项。
    #[serde(default)]
    pub(crate) download: DownloadSettings,
    /// 本地书库存储选项。
    #[serde(default)]
    pub(crate) storage: StorageSettings,
    /// 桌面端窗口行为。
    #[serde(default)]
    pub(crate) desktop: DesktopSettings,
    /// 签到自动化选项。
    #[serde(default)]
    pub(crate) signin: SigninSettings,
}

/// 下载请求限流与过程文件选项。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadSettings {
    /// 相邻网络请求之间的最小间隔，单位毫秒。
    pub(crate) request_interval_ms: u64,
    /// 同时运行的下载任务上限。
    pub(crate) max_concurrent_downloads: u8,
    /// 是否对下载的图片执行 OCR 恢复并保存过程文件。
    #[serde(default)]
    pub(crate) ocr_process_files_enabled: bool,
}

impl Default for DownloadSettings {
    fn default() -> Self {
        Self {
            request_interval_ms: 500,
            max_concurrent_downloads: 1,
            ocr_process_files_enabled: false,
        }
    }
}

/// 本地书库存储选项。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StorageSettings {
    /// 是否把书库写入外部存储目录。
    #[serde(default)]
    pub(crate) use_external_storage: bool,
}

/// 桌面端窗口行为。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopSettings {
    /// 关闭主窗口时是否保留托盘后台运行。
    #[serde(default = "default_close_to_tray")]
    pub(crate) close_to_tray: bool,
    /// 是否在登录系统后自动启动应用。
    #[serde(default)]
    pub(crate) autostart: bool,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            autostart: false,
        }
    }
}

/// 保持托盘后台运行是关闭窗口的默认行为。
fn default_close_to_tray() -> bool {
    true
}

/// 签到自动化选项。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SigninSettings {
    /// 是否在后台自动完成每日签到。
    #[serde(default)]
    pub(crate) auto_signin_enabled: bool,
}

/// 读取或更新正文恢复字典后返回的结果。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContentDictionaryResult {
    /// 当前字典条目总数。
    pub(crate) size: usize,
    /// 本次更新新增的条目数；读取操作不返回该字段。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) added: Option<usize>,
}
