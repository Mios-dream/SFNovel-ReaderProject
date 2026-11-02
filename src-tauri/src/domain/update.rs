//! GitHub Release 更新信息。

use serde::Serialize;

/// 更新清单中供界面展示的字段。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateInfo {
    pub(crate) current_version: String,
    pub(crate) version: String,
    pub(crate) notes: Option<String>,
    pub(crate) release_url: String,
}
