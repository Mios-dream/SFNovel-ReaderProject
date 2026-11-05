//! 已下载书籍的 EPUB、Markdown、TXT、有声和漫画导出。
//!
//! 入口 [`export`] 只负责定位书籍、收集内容与解析目标路径；每种格式的写出由同目录下的
//! 子模块独立实现，互不共享状态。打包与错误归类的公共动作收敛在本模块的
//! [`create_archive`] / [`write_entry`] / [`finish_archive`] / [`storage_error`]。

mod audio;
mod comic;
mod epub;
mod markdown;
mod txt;

use super::book::{read_book_manifest, read_chapter_store, read_comic_chapters};
use super::paths::local_book_directory;
use super::settings;
use crate::domain::error::{AppError, AppResult};
use crate::domain::library::{ExportFile, LibraryComicChapter, StoredChapter};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// 支持的导出格式。
#[derive(Clone, Copy)]
enum ExportFormat {
    Epub,
    Markdown,
    Txt,
    Audio,
    Comic,
}

impl ExportFormat {
    /// 解析前端传入的格式标识。
    fn parse(value: &str) -> Option<Self> {
        match value {
            "epub" => Some(Self::Epub),
            "markdown" => Some(Self::Markdown),
            "txt" => Some(Self::Txt),
            "audio" => Some(Self::Audio),
            "comic" => Some(Self::Comic),
            _ => None,
        }
    }

    /// 导出文件的扩展名。
    fn extension(self) -> &'static str {
        match self {
            Self::Epub => "epub",
            Self::Markdown | Self::Audio | Self::Comic => "zip",
            Self::Txt => "txt",
        }
    }

    /// 建议的导出文件名。
    fn file_name(self, name: &str) -> String {
        match self {
            Self::Epub => format!("{name}.epub"),
            Self::Audio => format!("{name}-有声.zip"),
            Self::Comic => format!("{name}-漫画.zip"),
            Self::Markdown | Self::Txt => format!("{name}.{}", self.extension()),
        }
    }

    /// 按格式写出文件。
    fn write(self, target: &Path, data: &ExportData) -> AppResult<()> {
        match self {
            Self::Epub => epub::write(target, data),
            Self::Markdown => markdown::write(target, data),
            Self::Txt => txt::write(target, data),
            Self::Audio => audio::write(target, &data.audio_directory),
            Self::Comic => comic::write(target, data),
        }
    }
}

/// 一次导出所需的全部内容。
struct ExportData {
    name: String,
    title: String,
    author: String,
    description: String,
    chapters: Vec<StoredChapter>,
    audio_directory: PathBuf,
    comic_chapters: Vec<LibraryComicChapter>,
}

impl ExportData {
    /// 确认所选格式确实有可导出的内容。
    fn ensure_not_empty(&self, format: ExportFormat) -> AppResult<()> {
        match format {
            ExportFormat::Audio => audio::ensure_available(&self.audio_directory)?,
            ExportFormat::Comic => comic::ensure_available(&self.comic_chapters)?,
            _ => {
                if self.chapters.is_empty() {
                    return Err(AppError::storage("这本书没有可导出的已下载文字章节"));
                }
            }
        }
        Ok(())
    }
}

/// 将本地书籍导出为 EPUB、Markdown、TXT、音频包或漫画包。
///
/// `output_path` 可以是绝对路径，也可以是 Android 的 `content://` URI。
/// 导出成功后返回渲染进程可引用的地址与建议文件名。
pub(crate) async fn export(
    app: &tauri::AppHandle,
    name: &str,
    format: &str,
    output_path: &str,
) -> AppResult<ExportFile> {
    let format =
        ExportFormat::parse(format).ok_or_else(|| AppError::storage("不支持的导出格式"))?;
    let data = load_export_data(app, name, format)?;
    let requested_path = output_path.trim();
    let is_document_uri = requested_path.starts_with("content://");
    let target = resolve_target(app, requested_path, format, is_document_uri)?;
    format.write(&target, &data)?;
    if is_document_uri {
        let result = settings::android_write_export(app, &target, requested_path).await;
        let _ = fs::remove_file(&target);
        result?;
    }
    Ok(ExportFile {
        href: requested_path.to_string(),
        file_name: format.file_name(name),
    })
}

/// 读取书籍清单与章节内容，按格式补齐所需的元数据。
fn load_export_data(
    app: &tauri::AppHandle,
    name: &str,
    format: ExportFormat,
) -> AppResult<ExportData> {
    let directory = local_book_directory(app, name)?;
    let metadata = read_book_manifest(&directory)?;
    let store = read_chapter_store(&directory)?;
    let mut chapters: Vec<_> = store.chapters.into_values().collect();
    chapters.sort_by_key(|chapter| (chapter.volume_index, chapter.chapter_index, chapter.id));
    let comic_chapters = match format {
        ExportFormat::Comic => read_comic_chapters(&directory)?,
        _ => Vec::new(),
    };
    let work = match format {
        ExportFormat::Audio => metadata.audio.as_ref(),
        ExportFormat::Comic => metadata.comic.as_ref(),
        _ => metadata.novel.as_ref(),
    };
    let data = ExportData {
        name: name.to_string(),
        title: work
            .and_then(|work| work.title.clone())
            .unwrap_or_else(|| name.to_string()),
        author: work
            .and_then(|work| work.author.clone())
            .unwrap_or_else(|| "未知作者".to_string()),
        description: work
            .and_then(|work| work.description.clone())
            .unwrap_or_default(),
        chapters,
        audio_directory: directory.join("audio"),
        comic_chapters,
    };
    data.ensure_not_empty(format)?;
    Ok(data)
}

/// 解析导出目标路径，并校验扩展名。
fn resolve_target(
    app: &tauri::AppHandle,
    requested_path: &str,
    format: ExportFormat,
    is_document_uri: bool,
) -> AppResult<PathBuf> {
    if is_document_uri {
        return settings::export_temp_path(app, format.extension());
    }
    let target = PathBuf::from(requested_path);
    if !has_expected_extension(&target, format.extension()) {
        return Err(AppError::storage(format!(
            "导出文件必须使用 .{} 扩展名",
            format.extension()
        )));
    }
    Ok(target)
}

/// 判断路径是否为绝对路径且扩展名符合要求。
fn has_expected_extension(path: &Path, expected: &str) -> bool {
    path.is_absolute()
        && path.file_name().is_some()
        && path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case(expected))
}

/// 把文件系统与打包错误统一归类为存储类错误。
fn storage_error(action: impl std::fmt::Display, error: impl std::fmt::Display) -> AppError {
    AppError::storage(format!("{action}：{error}"))
}

/// ZIP 条目默认使用的压缩方式。
fn deflated() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)
}

/// 创建导出 ZIP 并返回写入器。
///
/// # 参数
/// - `target`：ZIP 目标路径。
/// - `label`：错误消息中使用的导出名称。
fn create_archive(target: &Path, label: &str) -> AppResult<ZipWriter<File>> {
    let file =
        File::create(target).map_err(|error| storage_error(format!("无法创建{label}"), error))?;
    Ok(ZipWriter::new(file))
}

/// 写入一个完整的 ZIP 条目。
///
/// # 参数
/// - `archive`：目标 ZIP 写入器。
/// - `name`：包内路径。
/// - `options`：压缩方式。
/// - `payload`：条目内容。
/// - `label`：错误消息中使用的条目名称。
fn write_entry(
    archive: &mut ZipWriter<File>,
    name: &str,
    options: SimpleFileOptions,
    payload: &[u8],
    label: &str,
) -> AppResult<()> {
    archive
        .start_file(name, options)
        .map_err(|error| storage_error(format!("无法创建{label}条目"), error))?;
    archive
        .write_all(payload)
        .map_err(|error| storage_error(format!("无法写入{label}"), error))
}

/// 结束打包并落盘全部条目。
fn finish_archive(archive: ZipWriter<File>, label: &str) -> AppResult<()> {
    archive
        .finish()
        .map(|_| ())
        .map_err(|error| storage_error(format!("无法完成{label}"), error))
}

#[cfg(test)]
pub(super) mod testing {
    //! 各格式测试共用的临时目录、示例内容与 ZIP 读取工具。

    use super::*;

    pub(super) use crate::adapters::storage::test_support::TempDir;

    /// 构造两卷三章的示例书籍，正文带 Markdown 标题与图片语法。
    pub(super) fn sample_chapters() -> Vec<StoredChapter> {
        vec![
            StoredChapter {
                id: 1,
                title: "第一章 开端".to_string(),
                volume: "第一卷".to_string(),
                content:
                    "## 第一章 开端\n这是第一章的正文。\n![插图说明](/media/1.jpg)\n收尾段落。"
                        .to_string(),
                volume_index: 0,
                chapter_index: 0,
                content_source: None,
                ocr_source_path: None,
            },
            StoredChapter {
                id: 2,
                title: "第二章 转折".to_string(),
                volume: "第一卷".to_string(),
                content: "## 第二章 转折\n这是第二章的正文。".to_string(),
                volume_index: 0,
                chapter_index: 1,
                content_source: None,
                ocr_source_path: None,
            },
            StoredChapter {
                id: 3,
                title: "第三章 终局".to_string(),
                volume: "第二卷".to_string(),
                content: "## 第三章 终局\n这是第三章的正文。".to_string(),
                volume_index: 1,
                chapter_index: 0,
                content_source: None,
                ocr_source_path: None,
            },
        ]
    }

    /// 以给定有声目录构造示例导出内容；漫画章节默认为空。
    pub(super) fn sample_data(audio_directory: PathBuf) -> ExportData {
        ExportData {
            name: "示例作品".to_string(),
            title: "示例作品".to_string(),
            author: "示例作者".to_string(),
            description: "这是示例简介。".to_string(),
            chapters: sample_chapters(),
            audio_directory,
            comic_chapters: Vec::new(),
        }
    }

    /// 在目录内生成两页示例图片，并返回对应的漫画章节结构。
    pub(super) fn sample_comic_chapters(directory: &Path) -> Vec<LibraryComicChapter> {
        fs::create_dir_all(directory).expect("创建漫画目录");
        let mut pages = Vec::new();
        for index in 1..=2 {
            let page = directory.join(format!("{index:03}.jpg"));
            fs::write(&page, format!("page-{index}")).expect("写入示例页面");
            pages.push(page.to_string_lossy().into_owned());
        }
        vec![LibraryComicChapter {
            id: 1,
            title: "第一章 登场".to_string(),
            cover: None,
            pages,
        }]
    }

    /// 列出 ZIP 包内的条目名，按包内顺序返回。
    pub(super) fn archive_names(path: &Path) -> Vec<String> {
        let file = fs::File::open(path).expect("打开 ZIP");
        let archive = zip::ZipArchive::new(file).expect("解析 ZIP");
        archive
            .file_names()
            .map(|name| name.expect("读取条目名").into_owned())
            .collect()
    }

    /// 读取 ZIP 包内单个条目的文本内容。
    pub(super) fn archive_entry(path: &Path, name: &str) -> String {
        use std::io::Read;

        let file = fs::File::open(path).expect("打开 ZIP");
        let mut archive = zip::ZipArchive::new(file).expect("解析 ZIP");
        let mut entry = archive.by_name(name).expect("查找 ZIP 条目");
        let mut text = String::new();
        entry.read_to_string(&mut text).expect("读取 ZIP 条目");
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_known_formats_and_rejects_unknown() {
        for (value, expected) in [
            ("epub", "epub"),
            ("markdown", "zip"),
            ("txt", "txt"),
            ("audio", "zip"),
            ("comic", "zip"),
        ] {
            let format = ExportFormat::parse(value).expect("应识别已知格式");
            assert_eq!(format.extension(), expected, "{value} 的扩展名");
        }
        assert!(ExportFormat::parse("pdf").is_none(), "未知格式应被拒绝");
    }

    #[test]
    fn file_name_marks_audio_and_comic_archives() {
        let name = "示例作品";
        assert_eq!(ExportFormat::Epub.file_name(name), "示例作品.epub");
        assert_eq!(ExportFormat::Markdown.file_name(name), "示例作品.zip");
        assert_eq!(ExportFormat::Txt.file_name(name), "示例作品.txt");
        assert_eq!(ExportFormat::Audio.file_name(name), "示例作品-有声.zip");
        assert_eq!(ExportFormat::Comic.file_name(name), "示例作品-漫画.zip");
    }

    #[test]
    fn has_expected_extension_requires_absolute_matching_path() {
        let absolute = std::env::temp_dir().join("book.ZIP");
        assert!(
            has_expected_extension(&absolute, "zip"),
            "扩展名比较应忽略大小写"
        );
        assert!(
            !has_expected_extension(&absolute, "txt"),
            "扩展名不符应拒绝"
        );
        assert!(
            !has_expected_extension(Path::new("book.zip"), "zip"),
            "相对路径不应通过校验"
        );
    }

    #[test]
    fn ensure_not_empty_reports_missing_media() {
        let directory = std::env::temp_dir().join("sf-export-empty-audio");
        let data = testing::sample_data(directory);

        let error = data
            .ensure_not_empty(ExportFormat::Audio)
            .expect_err("缺少播放清单应失败");
        assert_eq!(error.kind(), crate::domain::error::ErrorKind::Storage);
        assert!(
            error.to_string().contains("有声"),
            "错误消息应说明有声内容缺失"
        );

        let error = data
            .ensure_not_empty(ExportFormat::Comic)
            .expect_err("缺少漫画章节应失败");
        assert!(
            error.to_string().contains("漫画"),
            "错误消息应说明漫画内容缺失"
        );

        assert!(
            data.ensure_not_empty(ExportFormat::Txt).is_ok(),
            "文字章节齐备"
        );
    }
}
