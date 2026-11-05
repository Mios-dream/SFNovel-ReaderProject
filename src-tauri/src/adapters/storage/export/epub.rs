//! EPUB 3 导出。

use super::{create_archive, deflated, finish_archive, write_entry, ExportData};
use crate::domain::error::AppResult;
use crate::domain::library::StoredChapter;
use std::fs::File;
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// EPUB 规定的容器描述文件。
const CONTAINER: &str = "<?xml version=\"1.0\"?><container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\"><rootfiles><rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/></rootfiles></container>";

/// 导航、清单与阅读顺序的累积结果。
#[derive(Default)]
struct Toc {
    manifest: String,
    spine: String,
    nav: String,
}

/// 生成 EPUB 3 文件。
///
/// # 参数
/// - `target`：EPUB 目标路径。
/// - `data`：已收集的书籍内容。
///
/// # 错误
/// 文件创建、XML 生成、条目写入或 ZIP 完成失败时返回存储类错误。
pub(super) fn write(target: &Path, data: &ExportData) -> AppResult<()> {
    let mut archive = create_archive(target, "EPUB")?;
    // `mimetype` 必须是包内第一个条目且不压缩，否则阅读器会拒绝识别。
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let compressed = deflated();
    write_entry(
        &mut archive,
        "mimetype",
        stored,
        b"application/epub+zip",
        "EPUB",
    )?;
    write_entry(
        &mut archive,
        "META-INF/container.xml",
        compressed,
        CONTAINER.as_bytes(),
        "EPUB",
    )?;
    let toc = write_chapters(&mut archive, compressed, &data.chapters)?;
    write_entry(
        &mut archive,
        "OEBPS/nav.xhtml",
        compressed,
        nav_document(&toc.nav).as_bytes(),
        "EPUB",
    )?;
    write_entry(
        &mut archive,
        "OEBPS/content.opf",
        compressed,
        package(data, &toc).as_bytes(),
        "EPUB",
    )?;
    finish_archive(archive, "EPUB")
}

/// 逐章写入 XHTML 页面，同时累积导航与阅读顺序。
///
/// # 参数
/// - `archive`：目标 ZIP 写入器。
/// - `options`：章节页面的压缩方式。
/// - `chapters`：按阅读顺序排列的章节。
fn write_chapters(
    archive: &mut ZipWriter<File>,
    options: SimpleFileOptions,
    chapters: &[StoredChapter],
) -> AppResult<Toc> {
    let mut toc = Toc {
        manifest: String::from(
            "<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>",
        ),
        ..Toc::default()
    };
    for (index, chapter) in chapters.iter().enumerate() {
        let id = format!("chapter-{}", index + 1);
        let file_name = format!("chapter-{:04}.xhtml", index + 1);
        toc.manifest.push_str(&format!(
            "<item id=\"{id}\" href=\"{file_name}\" media-type=\"application/xhtml+xml\"/>"
        ));
        toc.spine.push_str(&format!("<itemref idref=\"{id}\"/>"));
        toc.nav.push_str(&format!(
            "<li><a href=\"{file_name}\">{}</a></li>",
            xml(&chapter.title)
        ));
        write_entry(
            archive,
            &format!("OEBPS/{file_name}"),
            options,
            chapter_page(chapter).as_bytes(),
            "EPUB",
        )?;
    }
    Ok(toc)
}

/// 生成单章的 XHTML 页面。
///
/// # 参数
/// - `chapter`：待渲染的章节。
fn chapter_page(chapter: &StoredChapter) -> String {
    let body = chapter
        .content
        .lines()
        .filter(|line| !line.trim_start().starts_with("## "))
        .map(xml)
        .collect::<Vec<_>>()
        .join("<br/>");
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><!DOCTYPE html><html xmlns=\"http://www.w3.org/1999/xhtml\" xml:lang=\"zh-Hans\"><head><meta charset=\"utf-8\"/><title>{title}</title></head><body><h1>{title}</h1><p>{body}</p></body></html>",
        title = xml(&chapter.title),
    )
}

/// 生成导航文档。
///
/// # 参数
/// - `nav`：已累积的导航条目。
fn nav_document(nav: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><!DOCTYPE html><html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\"><head><meta charset=\"utf-8\"/><title>目录</title></head><body><nav epub:type=\"toc\"><h1>目录</h1><ol>{nav}</ol></nav></body></html>"
    )
}

/// 生成包文档（元数据、清单与阅读顺序）。
///
/// # 参数
/// - `data`：书籍元信息。
/// - `toc`：已累积的清单与阅读顺序。
fn package(data: &ExportData, toc: &Toc) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"book-id\" xml:lang=\"zh-Hans\"><metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:identifier id=\"book-id\">urn:uuid:local</dc:identifier><dc:title>{}</dc:title><dc:creator>{}</dc:creator><dc:language>zh-Hans</dc:language><dc:description>{}</dc:description></metadata><manifest>{}</manifest><spine>{}</spine></package>",
        xml(&data.title),
        xml(&data.author),
        xml(&data.description),
        toc.manifest,
        toc.spine,
    )
}

/// 转义 XML 特殊字符。
///
/// # 参数
/// - `value`：原始文本。
fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::super::testing::{archive_entry, archive_names, sample_data, TempDir};
    use super::*;

    #[test]
    fn epub_export_writes_required_structure() {
        let dir = TempDir::new("epub");
        let data = sample_data(dir.join("audio"));
        let target = dir.join("book.epub");

        write(&target, &data).expect("导出 EPUB");

        let names = archive_names(&target);
        assert_eq!(
            names.first().map(String::as_str),
            Some("mimetype"),
            "mimetype 必须是第一个条目"
        );
        assert!(names.contains(&"META-INF/container.xml".to_string()));
        assert!(names.contains(&"OEBPS/nav.xhtml".to_string()));
        assert!(names.contains(&"OEBPS/content.opf".to_string()));
        assert_eq!(
            names
                .iter()
                .filter(|name| name.starts_with("OEBPS/chapter-"))
                .count(),
            3
        );
        assert_eq!(archive_entry(&target, "mimetype"), "application/epub+zip");
    }

    #[test]
    fn epub_package_carries_metadata_and_spine() {
        let dir = TempDir::new("epub-package");
        let data = sample_data(dir.join("audio"));
        let target = dir.join("book.epub");

        write(&target, &data).expect("导出 EPUB");

        let package = archive_entry(&target, "OEBPS/content.opf");
        assert!(
            package.contains("<dc:title>示例作品</dc:title>"),
            "应写入标题"
        );
        assert!(
            package.contains("<dc:creator>示例作者</dc:creator>"),
            "应写入作者"
        );
        assert!(
            package.contains("<dc:description>这是示例简介。</dc:description>"),
            "应写入简介"
        );
        assert_eq!(
            package.matches("<itemref").count(),
            3,
            "每个章节都应有阅读顺序条目"
        );
        assert!(package.contains("href=\"nav.xhtml\""), "清单应包含导航文档");
    }

    #[test]
    fn epub_chapter_page_drops_markdown_heading() {
        let dir = TempDir::new("epub-page");
        let data = sample_data(dir.join("audio"));
        let target = dir.join("book.epub");

        write(&target, &data).expect("导出 EPUB");

        let page = archive_entry(&target, "OEBPS/chapter-0001.xhtml");
        assert!(page.contains("<h1>第一章 开端</h1>"), "章节标题应写入 h1");
        assert!(!page.contains("## "), "正文内的 Markdown 标题应被过滤");
        assert!(page.contains("这是第一章的正文。"), "应包含正文");
        assert!(page.contains("<br/>"), "段落之间应换行");
    }

    #[test]
    fn epub_escapes_xml_special_characters() {
        let dir = TempDir::new("epub-escape");
        let mut data = sample_data(dir.join("audio"));
        data.title = "A&B <C>".to_string();
        data.chapters[0].title = "D&E \"F\"".to_string();
        let target = dir.join("book.epub");

        write(&target, &data).expect("导出 EPUB");

        let nav = archive_entry(&target, "OEBPS/nav.xhtml");
        assert!(
            nav.contains("D&amp;E &quot;F&quot;"),
            "导航标题应转义，实际：{nav}"
        );
        let package = archive_entry(&target, "OEBPS/content.opf");
        assert!(
            package.contains("<dc:title>A&amp;B &lt;C&gt;</dc:title>"),
            "元数据不应出现裸 & 或 <"
        );
    }
}
