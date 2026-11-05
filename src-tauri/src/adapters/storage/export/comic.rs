//! 漫画导出（图片 + 目录 ZIP）。

use super::{create_archive, deflated, finish_archive, storage_error, write_entry, ExportData};
use crate::domain::error::{AppError, AppResult};
use std::fs;
use std::path::Path;

/// 包内漫画目录的文件名。
const CATALOG: &str = "目录.txt";

/// 将漫画章节和目录信息打包为 ZIP。
///
/// # 参数
/// - `target`：ZIP 目标路径。
/// - `data`：已收集的书籍内容，使用其中的标题、作者、简介与漫画章节。
///
/// # 错误
/// 文件创建、图片读取或 ZIP 写入失败时返回存储类错误。
pub(super) fn write(target: &Path, data: &ExportData) -> AppResult<()> {
    let mut archive = create_archive(target, "漫画导出")?;
    let options = deflated();
    let mut catalog = format!(
        "{}\n{}\n\n{}\n\n",
        data.title, data.author, data.description
    );
    for (chapter_index, chapter) in data.comic_chapters.iter().enumerate() {
        catalog.push_str(&format!("{:03}. {}\n", chapter_index + 1, chapter.title));
        for (page_index, page) in chapter.pages.iter().enumerate() {
            let bytes = fs::read(page).map_err(|error| storage_error("无法读取漫画页面", error))?;
            write_entry(
                &mut archive,
                &format!(
                    "comic/{:03}/{:03}.{}",
                    chapter_index + 1,
                    page_index + 1,
                    page_extension(page)
                ),
                options,
                &bytes,
                "漫画归档",
            )?;
        }
    }
    write_entry(
        &mut archive,
        CATALOG,
        options,
        catalog.as_bytes(),
        "漫画目录",
    )?;
    finish_archive(archive, "漫画导出")
}

/// 确认存在可导出的已下载漫画章节。
///
/// # 参数
/// - `chapters`：已下载的漫画章节。
///
/// # 错误
/// 章节列表为空时返回存储类错误。
pub(super) fn ensure_available(
    chapters: &[crate::domain::library::LibraryComicChapter],
) -> AppResult<()> {
    if chapters.is_empty() {
        return Err(AppError::storage("这本书没有可导出的已下载漫画章节"));
    }
    Ok(())
}

/// 取页面文件的扩展名，未知时回退为 jpg。
///
/// # 参数
/// - `page`：本地页面文件路径。
fn page_extension(page: &str) -> String {
    Path::new(page)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("jpg")
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::super::testing::{
        archive_entry, archive_names, sample_comic_chapters, sample_data, TempDir,
    };
    use super::*;

    #[test]
    fn comic_export_numbers_chapters_and_pages() {
        let dir = TempDir::new("comic");
        let mut data = sample_data(dir.join("audio"));
        data.comic_chapters = sample_comic_chapters(&dir.join("pages"));
        let target = dir.join("comic.zip");

        write(&target, &data).expect("导出漫画");

        let mut names = archive_names(&target);
        names.sort();
        assert_eq!(
            names,
            vec![
                "comic/001/001.jpg".to_string(),
                "comic/001/002.jpg".to_string(),
                "目录.txt".to_string(),
            ],
            "页面应带章节与页码前缀"
        );
        let catalog = archive_entry(&target, CATALOG);
        assert!(
            catalog.starts_with("示例作品\n示例作者\n"),
            "目录以作品信息开头"
        );
        assert!(catalog.contains("001. 第一章 登场"), "目录应列出章节");
        assert_eq!(
            archive_entry(&target, "comic/001/001.jpg"),
            "page-1",
            "页面内容应原样写入"
        );
    }

    #[test]
    fn comic_export_fails_when_page_is_missing() {
        let dir = TempDir::new("comic-missing");
        let mut data = sample_data(dir.join("audio"));
        data.comic_chapters = sample_comic_chapters(&dir.join("pages"));
        data.comic_chapters[0].pages[0] = dir.join("absent.jpg").to_string_lossy().into_owned();

        let error = write(&dir.join("comic.zip"), &data).expect_err("页面缺失应失败");

        assert_eq!(error.kind(), crate::domain::error::ErrorKind::Storage);
        assert!(
            error.to_string().contains("漫画页面"),
            "错误消息应指向漫画页面"
        );
    }

    #[test]
    fn comic_availability_requires_downloaded_chapters() {
        let dir = TempDir::new("comic-availability");
        let chapters = sample_comic_chapters(&dir.join("pages"));

        let error = ensure_available(&[]).expect_err("空列表应失败");
        assert!(error.to_string().contains("漫画章节"));
        assert!(ensure_available(&chapters).is_ok(), "有章节时应通过检查");
    }

    #[test]
    fn page_extension_falls_back_to_jpg() {
        assert_eq!(page_extension("/tmp/a.PNG"), "png");
        assert_eq!(page_extension("/tmp/a"), "jpg");
    }
}
