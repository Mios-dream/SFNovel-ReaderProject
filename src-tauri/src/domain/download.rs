//! 下载任务模型。

use serde::{Deserialize, Serialize};

/// 与渲染进程契约兼容的下载任务状态。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadJob {
    /// 任务唯一标识。
    pub(crate) id: String,
    /// 展示给用户的下载标题。
    pub(crate) title: String,
    /// 下载类型，例如 `text`、`audio` 或 `comic`。
    pub(crate) kind: String,
    /// 任务状态，例如 `pending`、`running`、`done`、`paused` 或 `error`。
    pub(crate) status: String,
    /// 百分比进度，范围为 0 到 100。
    pub(crate) progress: u8,
    /// 当前阶段或错误信息。
    pub(crate) message: String,
    /// 完成后的本地文件或目录地址。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) file: Option<String>,
    /// 关联的作品编号。
    pub(crate) novel_id: Option<i64>,
}

/// 恢复下载任务所需的私有输入。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct DownloadSpec {
    /// 下载类型。
    pub(crate) kind: String,
    /// 作品编号。
    pub(crate) novel_id: i64,
    /// App 媒体或专辑编号。
    #[serde(default)]
    pub(crate) source_id: Option<i64>,
    /// 网页来源路径。
    #[serde(default)]
    pub(crate) source_path: Option<String>,
    /// 任务展示标题。
    pub(crate) title: String,
    /// 待下载的章节编号。
    pub(crate) chapter_ids: Vec<i64>,
}

/// 应用重启后可手动恢复的任务快照。
#[derive(Default, Deserialize, Serialize)]
pub(crate) struct JobSnapshot {
    /// 以任务编号为键的公开任务状态。
    pub(crate) jobs: std::collections::HashMap<String, DownloadJob>,
    /// 以任务编号为键的恢复参数。
    pub(crate) specs: std::collections::HashMap<String, DownloadSpec>,
}
