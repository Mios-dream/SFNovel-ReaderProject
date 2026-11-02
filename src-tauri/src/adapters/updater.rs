//! GitHub Release 版本检测。

use crate::domain::error::{AppError, AppResult};
use crate::domain::update::UpdateInfo;
use serde::Deserialize;
use std::time::Duration;
use tauri::AppHandle;

const LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/Mios-dream/SFNovel-ReaderProject/releases/latest";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    body: Option<String>,
    html_url: String,
    draft: bool,
    prerelease: bool,
}

/// 读取 GitHub 最新 Release，并判断是否高于当前版本。
pub(crate) async fn check(app: &AppHandle) -> AppResult<Option<UpdateInfo>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| AppError::internal(format!("更新客户端初始化失败：{error}")))?;
    let release = client
        .get(LATEST_RELEASE_API)
        .header(reqwest::header::USER_AGENT, "sf-novel-flow")
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| AppError::upstream(format!("检查更新失败：{error}")))?
        .error_for_status()
        .map_err(|error| AppError::upstream(format!("检查更新失败：{error}")))?
        .json::<Release>()
        .await
        .map_err(|error| AppError::upstream(format!("更新信息格式无效：{error}")))?;

    if release.draft || release.prerelease {
        return Ok(None);
    }

    let current_version = app.package_info().version.to_string();
    let tag = release.tag_name.trim();
    let version = tag
        .strip_prefix('v')
        .or_else(|| tag.strip_prefix('V'))
        .unwrap_or(tag)
        .to_string();
    if !is_newer(&version, &current_version) {
        return Ok(None);
    }

    Ok(Some(UpdateInfo {
        current_version,
        version,
        notes: release.body,
        release_url: release.html_url,
    }))
}

fn is_newer(latest: &str, current: &str) -> bool {
    let latest = version_parts(latest);
    let current = version_parts(current);
    for index in 0..latest.len().max(current.len()) {
        let latest_part = latest.get(index).copied().unwrap_or(0);
        let current_part = current.get(index).copied().unwrap_or(0);
        if latest_part != current_part {
            return latest_part > current_part;
        }
    }
    false
}

fn version_parts(version: &str) -> Vec<u64> {
    version
        .split('.')
        .map(|part| {
            part.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}
