//! 正文内嵌图标记解析。
//!
//! 上游正文用 `[img=...]https://...[/img]` 表示插图。在线阅读与下载都要把它换成
//! 本地可用的形式，但"怎么写回"两边不同（阅读器用占位符 + 图片列表，下载写
//! Markdown 相对路径）。本模块只负责切分，写回策略留在各自调用点，避免两套解析
//! 规则各自演化。

/// 内嵌图的起始标记。
const START_MARKER: &str = "[img=";

/// 内嵌图的结束标记。
const END_MARKER: &str = "[/img]";

/// 正文被切分后的片段。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InlineSegment<'a> {
    /// 原样输出的文本，可能包含无法解析的标记。
    Text(&'a str),
    /// 已校验为 http(s) 的图片地址。
    Image(&'a str),
}

/// 按内嵌图标记切分正文。
///
/// 无法解析的标记一律保留为文本片段：缺少 `]`、缺少 `[/img]`、地址为空或不是
/// http(s) 时都不丢弃上游原文，否则正文会静默少内容。
pub(crate) fn split(content: &str) -> Vec<InlineSegment<'_>> {
    let mut segments = Vec::new();
    let mut cursor = 0;

    while let Some(offset) = content[cursor..].find(START_MARKER) {
        let start = cursor + offset;
        if start > cursor {
            segments.push(InlineSegment::Text(&content[cursor..start]));
        }
        match image_url(content, start) {
            Some((url, next)) => {
                segments.push(InlineSegment::Image(url));
                cursor = next;
            }
            None => {
                // 只吞掉标记本身，后续内容继续参与解析。
                segments.push(InlineSegment::Text(&content[start..start + START_MARKER.len()]));
                cursor = start + START_MARKER.len();
            }
        }
    }

    if cursor < content.len() {
        segments.push(InlineSegment::Text(&content[cursor..]));
    }
    segments
}

/// 解析起始位置处的完整标记，返回图片地址与标记之后的位置。
fn image_url(content: &str, start: usize) -> Option<(&str, usize)> {
    let metadata_end = content[start..].find(']')?;
    let url_start = start + metadata_end + 1;
    let url_end = url_start + content[url_start..].find(END_MARKER)?;
    let url = content[url_start..url_end].trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return None;
    }
    Some((url, url_end + END_MARKER.len()))
}

#[cfg(test)]
mod tests {
    use super::{split, InlineSegment};

    #[test]
    fn plain_text_stays_one_segment() {
        assert_eq!(split("没有任何插图"), vec![InlineSegment::Text("没有任何插图")]);
        assert!(split("").is_empty());
    }

    #[test]
    fn marker_is_replaced_by_image_segment() {
        let segments = split("前文[img=]https://a.test/1.png[/img]后文");

        assert_eq!(
            segments,
            vec![
                InlineSegment::Text("前文"),
                InlineSegment::Image("https://a.test/1.png"),
                InlineSegment::Text("后文"),
            ]
        );
    }

    #[test]
    fn metadata_inside_marker_is_ignored() {
        let segments = split("[img=600,800]http://a.test/2.jpg[/img]");

        assert_eq!(segments, vec![InlineSegment::Image("http://a.test/2.jpg")]);
    }

    #[test]
    fn multiple_images_keep_order() {
        let segments = split("[img=]https://a.test/1[/img]中[img=]https://a.test/2[/img]");

        assert_eq!(
            segments,
            vec![
                InlineSegment::Image("https://a.test/1"),
                InlineSegment::Text("中"),
                InlineSegment::Image("https://a.test/2"),
            ]
        );
    }

    #[test]
    fn missing_end_marker_keeps_original_text() {
        let content = "[img=]https://a.test/1.png";
        let segments = split(content);

        let rebuilt: String = segments
            .iter()
            .map(|segment| match segment {
                InlineSegment::Text(text) => *text,
                InlineSegment::Image(url) => url,
            })
            .collect();
        assert_eq!(rebuilt, content, "解析失败不能丢原文");
        assert!(!segments.iter().any(|s| matches!(s, InlineSegment::Image(_))));
    }

    #[test]
    fn missing_closing_bracket_keeps_original_text() {
        let content = "前[img=https://a.test/1.png[/img]后";
        let segments = split(content);

        assert!(
            !segments.iter().any(|s| matches!(s, InlineSegment::Image(_))),
            "畸形标记不应被当成图片"
        );
        let rebuilt: String = segments
            .iter()
            .map(|segment| match segment {
                InlineSegment::Text(text) => *text,
                InlineSegment::Image(url) => url,
            })
            .collect();
        assert_eq!(rebuilt, content);
    }

    #[test]
    fn empty_url_is_not_an_image() {
        let content = "[img=][/img]";
        let segments = split(content);

        assert!(!segments.iter().any(|s| matches!(s, InlineSegment::Image(_))));
    }

    #[test]
    fn non_http_url_is_not_an_image() {
        for content in [
            "[img=]//a.test/1.png[/img]",
            "[img=]data:image/png;base64,AAA[/img]",
            "[img=]file:///c:/1.png[/img]",
        ] {
            let segments = split(content);
            assert!(
                !segments.iter().any(|s| matches!(s, InlineSegment::Image(_))),
                "{content} 不应被当作可下载图片"
            );
        }
    }

    #[test]
    fn image_url_is_trimmed() {
        let segments = split("[img=]  https://a.test/1.png  [/img]");

        assert_eq!(segments, vec![InlineSegment::Image("https://a.test/1.png")]);
    }
}
