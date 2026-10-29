use crate::domain::catalog::{ChapterSummary, ChapterVolume, TextAccessState, TextContentKind};
use serde_json::Value;

/// 将 App 小说目录响应转换为领域分卷列表。
///
/// 保留章节编号、标题和 VIP 标记；访问状态初始化为未知，下载状态初始化为未下载。
///
/// # 参数
/// - `response`：包含 `volumeList` 或 `volumes` 的 App 目录 JSON。
///
/// # 返回
/// 返回仅含有效编号分卷的目录列表。
///
/// # 错误
/// 响应缺少分卷数组时返回错误。
pub(super) fn parse_chapter_catalog(response: &Value) -> Result<Vec<ChapterVolume>, String> {
    let volumes = response
        .get("volumeList")
        .or_else(|| response.get("volumes"))
        .and_then(Value::as_array)
        .ok_or_else(|| "SF App 目录未返回分卷列表".to_string())?;
    Ok(volumes
        .iter()
        .filter_map(|volume| {
            let volume_id = volume.get("volumeId").and_then(Value::as_i64).unwrap_or(0);
            (volume_id > 0).then(|| ChapterVolume {
                volume_id,
                title: volume
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("未命名分卷")
                    .to_string(),
                chapters: volume
                    .get("chapterList")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|chapter| {
                        let chap_id = chapter.get("chapId").and_then(Value::as_i64)?;
                        let is_vip = chapter
                            .get("isVip")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        Some(ChapterSummary {
                            chap_id,
                            title: chapter
                                .get("ntitle")
                                .and_then(Value::as_str)
                                .or_else(|| chapter.get("title").and_then(Value::as_str))
                                .unwrap_or("未命名章节")
                                .to_string(),
                            is_vip,
                            content_kind: if is_vip {
                                TextContentKind::ImageVip
                            } else {
                                TextContentKind::Text
                            },
                            access_state: TextAccessState::Unknown,
                            downloaded: false,
                        })
                    })
                    .collect(),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::parse_chapter_catalog;
    use crate::domain::catalog::{TextAccessState, TextContentKind};
    use serde_json::{json, Value};

    fn volume_fixture() -> Value {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../TESTDATA/volumeInfos.json"
        )))
        .expect("volume fixture should be valid JSON")
    }

    #[test]
    fn parses_real_volume_fixture_and_preserves_chapter_metadata() {
        let volumes = parse_chapter_catalog(&json!({ "volumeList": volume_fixture() })).unwrap();

        assert_eq!(volumes.len(), 7);
        assert_eq!(
            volumes
                .iter()
                .map(|volume| volume.chapters.len())
                .sum::<usize>(),
            1144
        );
        assert_eq!(volumes[0].volume_id, 823444);
        assert_eq!(volumes[0].title, "背投与插画");
        assert_eq!(volumes[0].chapters[0].chap_id, 7431226);
        assert_eq!(volumes[0].chapters[0].title, "背投");
        assert!(!volumes[0].chapters[0].is_vip);
        assert!(matches!(
            volumes[0].chapters[0].content_kind,
            TextContentKind::Text
        ));
        assert!(volumes
            .iter()
            .flat_map(|volume| &volume.chapters)
            .all(
                |chapter| matches!(chapter.access_state, TextAccessState::Unknown)
                    && !chapter.downloaded
            ));

        let vip_chapter = volumes
            .iter()
            .flat_map(|volume| &volume.chapters)
            .find(|chapter| chapter.is_vip)
            .expect("fixture should contain VIP chapters");
        assert!(matches!(
            vip_chapter.content_kind,
            TextContentKind::ImageVip
        ));
    }

    #[test]
    fn rejects_catalog_without_volume_list() {
        assert!(parse_chapter_catalog(&json!({ "data": [] })).is_err());
    }
}
