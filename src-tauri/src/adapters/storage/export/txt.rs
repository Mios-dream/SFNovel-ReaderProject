//! 纯文本（TXT）导出。

use super::{storage_error, ExportData};
use crate::domain::error::AppResult;
use std::fs;
use std::path::Path;

/// 写出纯文本导出，章节标题保留分卷信息。
///
/// # 参数
/// - `target`：TXT 目标路径。
/// - `data`：已收集的书籍内容。
///
/// # 错误
/// 文件写入失败时返回存储类错误。
pub(super) fn write(target: &Path, data: &ExportData) -> AppResult<()> {
    let mut text = format!(
        "{}\n{}\n\n{}\n\n",
        data.title, data.author, data.description
    );
    for chapter in &data.chapters {
        text.push_str(&format!(
            "{} - {}\n\n{}\n\n",
            chapter.volume,
            chapter.title,
            text_without_markdown(&chapter.content)
        ));
    }
    // 前置 BOM，避免 Windows 记事本把 UTF-8 中文识别为乱码。
    fs::write(target, format!("\u{feff}{text}"))
        .map_err(|error| storage_error("无法写入 TXT 导出", error))
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

#[cfg(test)]
mod tests {
    use super::super::testing::{sample_data, TempDir};
    use super::*;

    #[test]
    fn txt_export_writes_metadata_and_every_chapter() {
        let dir = TempDir::new("txt");
        let target = dir.join("book.txt");
        let data = sample_data(dir.join("audio"));

        write(&target, &data).expect("导出 TXT");

        let content = fs::read_to_string(&target).expect("读取导出文件");
        assert!(content.starts_with('\u{feff}'), "TXT 应以 BOM 开头");
        assert!(content.contains("示例作品"), "应包含标题");
        assert!(content.contains("示例作者"), "应包含作者");
        assert!(content.contains("这是示例简介。"), "应包含简介");
        assert!(content.contains("第一卷 - 第一章 开端"), "章节标题应带分卷");
        assert!(content.contains("第二卷 - 第三章 终局"), "应包含第二卷章节");
        assert!(content.contains("这是第一章的正文。"), "应包含正文");
    }

    #[test]
    fn txt_export_drops_markdown_headings_but_keeps_image_alt() {
        let dir = TempDir::new("txt-markdown");
        let target = dir.join("book.txt");
        let data = sample_data(dir.join("audio"));

        write(&target, &data).expect("导出 TXT");

        let content = fs::read_to_string(&target).expect("读取导出文件");
        assert!(
            !content.contains("## 第一章 开端"),
            "正文内的 Markdown 标题应被移除"
        );
        assert!(!content.contains("!["), "图片语法应被替换为说明文字");
        assert!(content.contains("插图说明"), "应保留图片说明文字");
        assert!(content.contains("收尾段落。"), "图片后的正文应保留");
    }
}
