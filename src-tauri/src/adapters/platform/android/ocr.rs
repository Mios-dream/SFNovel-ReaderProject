//! Android 正文图片 OCR。

use super::call_ocr;
use crate::domain::error::{AppError, AppResult};
use serde::Deserialize;
use std::path::PathBuf;

/// OCR 插件返回的识别结果。
#[derive(Deserialize)]
struct OcrResponse {
    text: String,
}

/// 识别一章正文图片中的文字。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `source`：原始图片路径。
/// - `segments_dir`：可选的分段图片目录。
///
/// # 错误
/// 插件调用失败或未识别到文字时返回错误。
pub(crate) async fn recognize_image(
    app: &tauri::AppHandle,
    source: PathBuf,
    segments_dir: Option<PathBuf>,
) -> AppResult<String> {
    let result = call_ocr::<OcrResponse, _>(
        app,
        "recognizeChapter",
        serde_json::json!({
            "sourcePath": source.to_string_lossy(),
            "segmentsDir": segments_dir.map(|directory| directory.to_string_lossy().into_owned()),
        }),
        "Android OCR 失败",
    )
    .await?;
    if result.text.trim().is_empty() {
        return Err(AppError::storage(
            "Android OCR 未识别到可用文字；原始图片已保留，可稍后重试",
        ));
    }
    Ok(result.text)
}
