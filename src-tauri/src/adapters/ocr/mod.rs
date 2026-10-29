//! 本地图片文字识别适配。

#[cfg(not(target_os = "android"))]
mod worker;

use std::path::{Path, PathBuf};

#[cfg(not(target_os = "android"))]
pub(crate) use worker::DesktopOcrState;

/// 对已持久化的来源图片执行文字识别。
pub(crate) async fn recognize_image(
    app: &tauri::AppHandle,
    source: PathBuf,
    segments_dir: Option<PathBuf>,
) -> Result<String, String> {
    #[cfg(target_os = "android")]
    {
        let text =
            crate::adapters::platform::android::recognize_image(app, source, segments_dir).await?;
        return Ok(format_ocr_novel_text(&text));
    }
    #[cfg(not(target_os = "android"))]
    {
        worker::recognize(app, source, segments_dir).await
    }
}

/// 将逐行 OCR 结果整理为按句号分段的小说正文。
pub(crate) fn format_ocr_novel_text(text: &str) -> String {
    text.split(['\r', '\n'])
        .map(str::trim)
        .collect::<String>()
        .replace('。', "。\n")
        .trim_end()
        .to_string()
}

/// 将书籍内路径转换为 Markdown 使用的相对路径。
pub(crate) fn relative_book_path(path: &Path, directory: &Path) -> Result<String, String> {
    path.strip_prefix(directory)
        .map_err(|_| "路径不属于当前书籍目录".to_string())
        .map(|value| value.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::format_ocr_novel_text;

    #[test]
    fn ocr_text_is_grouped_by_sentence() {
        assert_eq!(
            format_ocr_novel_text("第一句。\n第二\n句。 "),
            "第一句。\n第二句。"
        );
    }
}
