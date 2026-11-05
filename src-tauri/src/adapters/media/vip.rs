//! VIP 图片正文的获取与识别。
//!
//! 在线阅读与下载任务共用同一条兜底链路：把上游返回的图片正文落到本地，
//! 再交给 OCR 还原文字。两处调用方只在「存在哪个目录」和「是否保留识别中间图」
//! 上不同，其余行为必须一致——否则同一章会因为入口不同而得到不同结果。

use crate::adapters::ocr::recognize_image;
use crate::adapters::sfacg::source::SfSource;
use crate::adapters::storage::replace_file;
use crate::domain::error::{AppError, AppResult};
use std::fs;
use std::path::{Path, PathBuf};

/// 一次 VIP 正文识别的结果。
pub(crate) struct RecognizedVipText {
    /// 识别出的正文。
    pub(crate) content: String,
    /// 保存到本地的原图绝对路径，供调用方决定是否记录或清理。
    pub(crate) image_path: PathBuf,
}

/// 下载 VIP 章节原图并识别为正文。
///
/// # 参数
/// - `app`：应用句柄，OCR 需要在平台侧拉起识别能力。
/// - `source`：已连接的内容源，用于读取网页 VIP 图片正文。
/// - `directory`：存放原图的目录，不存在时创建。
/// - `novel_id`：小说编号。
/// - `chapter_id`：章节编号。
/// - `keep_process_files`：是否保留 OCR 的中间切片，供人工核对识别质量。
///
/// # 错误
/// 图片读取、落盘或识别失败时返回错误；错误分类沿用调用方的语义
/// （上游取图失败为 `Upstream`，落盘失败为 `Storage`）。
pub(crate) async fn recognize_vip_chapter(
    app: &tauri::AppHandle,
    source: &SfSource,
    directory: &Path,
    novel_id: i64,
    chapter_id: i64,
    keep_process_files: bool,
) -> AppResult<RecognizedVipText> {
    let image = source.read_vip_image_chapter(novel_id, chapter_id).await?;
    fs::create_dir_all(directory)
        .map_err(|error| AppError::storage(format!("无法创建 VIP 正文目录：{error}")))?;
    let image_path = directory.join(format!("chapter-{chapter_id}.{}", image.extension));
    replace_file(&image_path, &image.bytes, "VIP 章节原图")?;

    let segments = keep_process_files.then(|| {
        directory
            .join("segments")
            .join(format!("chapter-{chapter_id}"))
    });
    let content = recognize_image(app, image_path.clone(), segments).await?;
    Ok(RecognizedVipText {
        content,
        image_path,
    })
}
