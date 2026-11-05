//! Markdown 导出（单文件 ZIP）。

use super::{create_archive, deflated, finish_archive, write_entry, ExportData};
use crate::domain::error::AppResult;
use crate::domain::library::StoredChapter;
use std::path::Path;

/// 将 Markdown 导出内容写入 ZIP 包。
///
/// # 参数
/// - `target`：ZIP 目标路径。
/// - `data`：已收集的书籍内容，包内文件名取自 `data.name`。
///
/// # 错误
/// 文件创建、条目写入或完成写入失败时返回存储类错误。
pub(super) fn write(target: &Path, data: &ExportData) -> AppResult<()> {
    let mut archive = create_archive(target, "Markdown 导出")?;
    let document = document(&data.title, &data.author, &data.description, &data.chapters);
    write_entry(
        &mut archive,
        &format!("{}.md", data.name),
        deflated(),
        document.as_bytes(),
        "Markdown 导出",
    )?;
    finish_archive(archive, "Markdown 导出")
}

/// 生成包含作品信息和全部章节的 Markdown 文本。
///
/// # 参数
/// - `title`：作品标题。
/// - `author`：作者名称。
/// - `description`：作品简介。
/// - `chapters`：按阅读顺序排列的章节。
fn document(title: &str, author: &str, description: &str, chapters: &[StoredChapter]) -> String {
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

#[cfg(test)]
mod tests {
    use super::super::testing::{archive_entry, archive_names, sample_data, TempDir};
    use super::*;

    #[test]
    fn markdown_export_packs_single_document_with_front_matter() {
        let dir = TempDir::new("markdown");
        let target = dir.join("book.zip");
        let data = sample_data(dir.join("audio"));

        write(&target, &data).expect("导出 Markdown");

        assert_eq!(archive_names(&target), vec!["示例作品.md".to_string()]);
        let document = archive_entry(&target, "示例作品.md");
        assert!(
            document.starts_with("---\ntitle: '示例作品'\nauthor: '示例作者'"),
            "应写入 YAML 元信息，实际开头：{}",
            &document[..document.len().min(60)]
        );
        assert!(document.contains("lang: 'zh-Hans'"), "应声明语言");
        assert!(
            document.contains("description: |-\n  这是示例简介。"),
            "应写入简介"
        );
        assert!(document.contains("\n# 第一卷\n"), "应按分卷生成标题");
        assert!(document.contains("\n# 第二卷\n"), "第二卷应单独成段");
        assert!(
            document.contains("## 第一章 开端"),
            "Markdown 导出应保留正文原有标记"
        );
    }

    #[test]
    fn markdown_front_matter_escapes_single_quotes() {
        let dir = TempDir::new("markdown-quote");
        let target = dir.join("book.zip");
        let mut data = sample_data(dir.join("audio"));
        data.title = "作者's 作品".to_string();

        write(&target, &data).expect("导出 Markdown");

        let document = archive_entry(&target, "示例作品.md");
        assert!(
            document.contains("title: '作者\\'s 作品'"),
            "YAML 单引号应转义，实际：{}",
            &document[..document.len().min(60)]
        );
    }
}
