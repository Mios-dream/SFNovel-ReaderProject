//! 已下载书籍的 EPUB、Markdown、TXT、有声和漫画导出。

use super::book::{read_book_manifest, read_chapter_store, read_comic_chapters};
use super::paths::local_book_directory;
use super::settings;
use crate::domain::library::{ExportFile, StoredChapter};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// 将本地书籍导出为 EPUB、Markdown、TXT、音频包或漫画包。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `name`：本地书籍目录名。
/// - `format`：导出格式。
/// - `output_path`：绝对文件路径或 Android `content://` URI。
///
/// # 返回
/// 渲染进程可引用的导出地址及建议文件名。
///
/// # 错误
/// 书籍不存在、格式不受支持、扩展名不匹配、内容为空或写入失败时返回错误消息。
pub(crate) async fn export(
    app: &tauri::AppHandle,
    name: &str,
    format: &str,
    output_path: &str,
) -> Result<ExportFile, String> {
    let directory = local_book_directory(app, name)?;
    let metadata = read_book_manifest(&directory)?;
    let store = read_chapter_store(&directory)?;
    let mut chapters: Vec<_> = store.chapters.into_values().collect();
    chapters.sort_by_key(|chapter| (chapter.volume_index, chapter.chapter_index, chapter.id));
    let audio_directory = directory.join("audio");
    let comic_chapters = if format == "comic" {
        read_comic_chapters(&directory)?
    } else {
        Vec::new()
    };
    if format == "audio" {
        if !audio_directory.join("有声目录.m3u8").is_file() {
            return Err("这本书没有可打包的有声章节".to_string());
        }
    } else if format == "comic" {
        if comic_chapters.is_empty() {
            return Err("这本书没有可导出的已下载漫画章节".to_string());
        }
    } else if chapters.is_empty() {
        return Err("这本书没有可导出的已下载文字章节".to_string());
    }
    let export_metadata = if format == "audio" {
        metadata.audio.as_ref()
    } else if format == "comic" {
        metadata.comic.as_ref()
    } else {
        metadata.novel.as_ref()
    };
    let title = export_metadata
        .and_then(|work| work.title.as_deref())
        .unwrap_or(name);
    let author = export_metadata
        .and_then(|work| work.author.as_deref())
        .unwrap_or("未知作者");
    let description = export_metadata
        .and_then(|work| work.description.as_deref())
        .unwrap_or("");
    let file_name = match format {
        "epub" => format!("{name}.epub"),
        "audio" => format!("{name}-有声.zip"),
        "comic" => format!("{name}-漫画.zip"),
        _ => format!("{name}.txt"),
    };
    let expected_extension = match format {
        "epub" => "epub",
        "markdown" | "audio" | "comic" => "zip",
        _ => "txt",
    };
    let requested_path = output_path.trim();
    let is_document_uri = requested_path.starts_with("content://");
    let target = if is_document_uri {
        settings::export_temp_path(app, expected_extension)?
    } else {
        PathBuf::from(requested_path)
    };
    if !is_document_uri
        && (!target.is_absolute()
            || target.file_name().is_none()
            || !target
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case(expected_extension)))
    {
        return Err(format!("导出文件必须使用 .{expected_extension} 扩展名"));
    }
    match format {
        "epub" => write_epub_export(&target, title, author, description, &chapters)?,
        "markdown" => write_markdown_zip(&target, name, title, author, description, &chapters)?,
        "audio" => write_audio_zip(&target, &audio_directory)?,
        "comic" => write_comic_zip(&target, title, author, description, &comic_chapters)?,
        "txt" => {
            let mut text = format!("{title}\n{author}\n\n{description}\n\n");
            for chapter in &chapters {
                text.push_str(&format!(
                    "{} - {}\n\n{}\n\n",
                    chapter.volume,
                    chapter.title,
                    text_without_markdown(&chapter.content)
                ));
            }
            fs::write(&target, format!("\u{feff}{text}"))
                .map_err(|error| format!("无法写入 TXT 导出：{error}"))?;
        }
        _ => return Err("不支持的导出格式".to_string()),
    }
    if is_document_uri {
        let result = settings::android_write_export(app, &target, requested_path).await;
        let _ = fs::remove_file(&target);
        result?;
    }
    Ok(ExportFile {
        href: requested_path.to_string(),
        file_name,
    })
}

/// 去除正文中的 Markdown 标题和图片语法。
///
/// # 参数
/// - `value`：持久化章节正文。
fn text_without_markdown(value: &str) -> String {
    let mut output = String::new();
    for line in value.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("## ") {
            continue;
        }
        if let Some(stripped) = trimmed.strip_prefix("![") {
            if let Some((alt, _)) = stripped.split_once("](") {
                output.push_str(alt);
                output.push('\n');
            }
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }
    output.trim().to_string()
}

/// 生成包含作品信息和全部章节的 Markdown 文本。
///
/// # 参数
/// - `title`：作品标题。
/// - `author`：作者名称。
/// - `description`：作品简介。
/// - `chapters`：按阅读顺序排列的章节。
fn local_markdown(
    title: &str,
    author: &str,
    description: &str,
    chapters: &[StoredChapter],
) -> String {
    let description = description
        .lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut output = format!(
        "---\ntitle: '{}'\nauthor: '{}'\nlang: 'zh-Hans'\ndescription: |-\n{}\n...\n",
        title.replace('\'', "\\'"),
        author.replace('\'', "\\'"),
        description,
    );
    let mut current_volume = None;
    for chapter in chapters {
        if current_volume != Some(chapter.volume.as_str()) {
            output.push_str(&format!("\n# {}\n", chapter.volume));
            current_volume = Some(&chapter.volume);
        }
        output.push('\n');
        output.push_str(&chapter.content);
        output.push('\n');
    }
    output
}

/// 将 Markdown 导出内容写入 ZIP 包。
///
/// # 参数
/// - `target`：ZIP 目标路径。
/// - `name`：书籍目录名，用作包内文件名。
/// - `title`：作品标题。
/// - `author`：作者名称。
/// - `description`：作品简介。
/// - `chapters`：按阅读顺序排列的章节。
///
/// # 错误
/// 文件创建、ZIP 条目写入或完成写入失败时返回错误消息。
fn write_markdown_zip(
    target: &Path,
    name: &str,
    title: &str,
    author: &str,
    description: &str,
    chapters: &[StoredChapter],
) -> Result<(), String> {
    let file = File::create(target).map_err(|error| format!("无法创建 Markdown 导出：{error}"))?;
    let mut archive = ZipWriter::new(file);
    archive
        .start_file(
            format!("{name}.md"),
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        )
        .map_err(|error| format!("无法创建 Markdown 条目：{error}"))?;
    archive
        .write_all(local_markdown(title, author, description, chapters).as_bytes())
        .map_err(|error| format!("无法写入 Markdown 导出：{error}"))?;
    archive
        .finish()
        .map(|_| ())
        .map_err(|error| format!("无法完成 Markdown 导出：{error}"))
}

/// 将本地有声目录打包为 ZIP。
///
/// # 参数
/// - `target`：ZIP 目标路径。
/// - `audio_directory`：本地音频目录。
///
/// # 错误
/// 目录读取、文件写入或 ZIP 完成失败时返回错误消息。
fn write_audio_zip(target: &Path, audio_directory: &Path) -> Result<(), String> {
    let file = File::create(target).map_err(|error| format!("无法创建有声导出：{error}"))?;
    let mut archive = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for entry in
        fs::read_dir(audio_directory).map_err(|error| format!("无法读取有声目录：{error}"))?
    {
        let entry = entry.map_err(|error| format!("读取有声文件失败：{error}"))?;
        if !entry
            .file_type()
            .map_err(|error| format!("读取有声文件类型失败：{error}"))?
            .is_file()
        {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if file_name != "有声目录.m3u8" && !file_name.to_ascii_lowercase().ends_with(".mp3") {
            continue;
        }
        archive
            .start_file(format!("audio/{file_name}"), options)
            .map_err(|error| format!("无法创建有声归档条目：{error}"))?;
        let bytes = fs::read(entry.path()).map_err(|error| format!("无法读取有声文件：{error}"))?;
        archive
            .write_all(&bytes)
            .map_err(|error| format!("无法写入有声导出：{error}"))?;
    }
    archive
        .finish()
        .map(|_| ())
        .map_err(|error| format!("无法完成有声导出：{error}"))
}

/// 将漫画章节和目录信息打包为 ZIP。
///
/// # 参数
/// - `target`：ZIP 目标路径。
/// - `title`：作品标题。
/// - `author`：作者名称。
/// - `description`：作品简介。
/// - `comic_chapters`：按顺序排列的本地漫画章节。
///
/// # 错误
/// 文件创建、图片读取或 ZIP 写入失败时返回错误消息。
fn write_comic_zip(
    target: &Path,
    title: &str,
    author: &str,
    description: &str,
    comic_chapters: &[crate::domain::library::LibraryComicChapter],
) -> Result<(), String> {
    let file = File::create(target).map_err(|error| format!("无法创建漫画导出：{error}"))?;
    let mut archive = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut catalog = format!("{title}\n{author}\n\n{description}\n\n");
    for (chapter_index, chapter) in comic_chapters.iter().enumerate() {
        catalog.push_str(&format!("{:03}. {}\n", chapter_index + 1, chapter.title));
        for (page_index, page) in chapter.pages.iter().enumerate() {
            let extension = Path::new(page)
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("jpg")
                .to_ascii_lowercase();
            archive
                .start_file(
                    format!(
                        "comic/{:03}/{:03}.{extension}",
                        chapter_index + 1,
                        page_index + 1
                    ),
                    options,
                )
                .map_err(|error| format!("无法创建漫画归档条目：{error}"))?;
            let bytes = fs::read(page).map_err(|error| format!("无法读取漫画页面：{error}"))?;
            archive
                .write_all(&bytes)
                .map_err(|error| format!("无法写入漫画导出：{error}"))?;
        }
    }
    archive
        .start_file("目录.txt", options)
        .map_err(|error| format!("无法创建漫画目录：{error}"))?;
    archive
        .write_all(catalog.as_bytes())
        .map_err(|error| format!("无法写入漫画目录：{error}"))?;
    archive
        .finish()
        .map(|_| ())
        .map_err(|error| format!("无法完成漫画导出：{error}"))
}

/// 生成 EPUB 3 文件。
///
/// # 参数
/// - `target`：EPUB 目标路径。
/// - `title`：作品标题。
/// - `author`：作者名称。
/// - `description`：作品简介。
/// - `chapters`：按阅读顺序排列的章节。
///
/// # 错误
/// 文件创建、XML 生成、条目写入或 ZIP 完成失败时返回错误消息。
fn write_epub_export(
    target: &Path,
    title: &str,
    author: &str,
    description: &str,
    chapters: &[StoredChapter],
) -> Result<(), String> {
    fn xml(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }
    let file = File::create(target).map_err(|error| format!("无法创建 EPUB：{error}"))?;
    let mut archive = ZipWriter::new(file);
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let compressed =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    archive
        .start_file("mimetype", stored)
        .map_err(|error| format!("无法创建 EPUB 条目：{error}"))?;
    archive
        .write_all(b"application/epub+zip")
        .map_err(|error| format!("无法写入 EPUB：{error}"))?;
    let container = "<?xml version=\"1.0\"?><container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\"><rootfiles><rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/></rootfiles></container>";
    archive
        .start_file("META-INF/container.xml", compressed)
        .map_err(|error| format!("无法创建 EPUB 条目：{error}"))?;
    archive
        .write_all(container.as_bytes())
        .map_err(|error| format!("无法写入 EPUB：{error}"))?;
    let mut manifest = String::from("<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>");
    let mut spine = String::new();
    let mut nav = String::new();
    for (index, chapter) in chapters.iter().enumerate() {
        let id = format!("chapter-{}", index + 1);
        let file_name = format!("chapter-{:04}.xhtml", index + 1);
        manifest.push_str(&format!(
            "<item id=\"{id}\" href=\"{file_name}\" media-type=\"application/xhtml+xml\"/>"
        ));
        spine.push_str(&format!("<itemref idref=\"{id}\"/>"));
        nav.push_str(&format!(
            "<li><a href=\"{file_name}\">{}</a></li>",
            xml(&chapter.title)
        ));
        let body = chapter
            .content
            .lines()
            .filter(|line| !line.trim_start().starts_with("## "))
            .map(xml)
            .collect::<Vec<_>>()
            .join("<br/>");
        let page = format!("<?xml version=\"1.0\" encoding=\"utf-8\"?><!DOCTYPE html><html xmlns=\"http://www.w3.org/1999/xhtml\" xml:lang=\"zh-Hans\"><head><meta charset=\"utf-8\"/><title>{}</title></head><body><h1>{}</h1><p>{}</p></body></html>", xml(&chapter.title), xml(&chapter.title), body);
        archive
            .start_file(format!("OEBPS/{file_name}"), compressed)
            .map_err(|error| format!("无法创建 EPUB 条目：{error}"))?;
        archive
            .write_all(page.as_bytes())
            .map_err(|error| format!("无法写入 EPUB：{error}"))?;
    }
    let nav_document = format!("<?xml version=\"1.0\" encoding=\"utf-8\"?><!DOCTYPE html><html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\"><head><meta charset=\"utf-8\"/><title>目录</title></head><body><nav epub:type=\"toc\"><h1>目录</h1><ol>{nav}</ol></nav></body></html>");
    archive
        .start_file("OEBPS/nav.xhtml", compressed)
        .map_err(|error| format!("无法创建 EPUB 条目：{error}"))?;
    archive
        .write_all(nav_document.as_bytes())
        .map_err(|error| format!("无法写入 EPUB：{error}"))?;
    let package = format!("<?xml version=\"1.0\" encoding=\"utf-8\"?><package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"book-id\" xml:lang=\"zh-Hans\"><metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:identifier id=\"book-id\">urn:uuid:local</dc:identifier><dc:title>{}</dc:title><dc:creator>{}</dc:creator><dc:language>zh-Hans</dc:language><dc:description>{}</dc:description></metadata><manifest>{manifest}</manifest><spine>{spine}</spine></package>", xml(title), xml(author), xml(description));
    archive
        .start_file("OEBPS/content.opf", compressed)
        .map_err(|error| format!("无法创建 EPUB 条目：{error}"))?;
    archive
        .write_all(package.as_bytes())
        .map_err(|error| format!("无法写入 EPUB：{error}"))?;
    archive
        .finish()
        .map(|_| ())
        .map_err(|error| format!("无法完成 EPUB：{error}"))
}
