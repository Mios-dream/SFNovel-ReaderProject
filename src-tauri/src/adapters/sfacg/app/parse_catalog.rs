use crate::domain::catalog::{ChapterSummary, ChapterVolume, TextAccessState, TextContentKind};
use serde_json::Value;

/// 将 App 小说目录响应转换为领域分卷列表。
///
/// 保留章节编号、标题、VIP 标记和火券价格；当前会话已购的 VIP 章节价格为 0。
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
                        let need_fire_money = chapter
                            .get("needFireMoney")
                            .and_then(Value::as_i64)
                            .unwrap_or(0);
                        let access_state = if !is_vip || need_fire_money == 0 {
                            TextAccessState::Available
                        } else {
                            TextAccessState::Unavailable
                        };
                        Some(ChapterSummary {
                            chap_id,
                            title: chapter
                                .get("ntitle")
                                .and_then(Value::as_str)
                                .or_else(|| chapter.get("title").and_then(Value::as_str))
                                .unwrap_or("未命名章节")
                                .to_string(),
                            is_vip,
                            need_fire_money,
                            content_kind: if is_vip {
                                TextContentKind::ImageVip
                            } else {
                                TextContentKind::Text
                            },
                            access_state,
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
        assert!(matches!(
            volumes[0].chapters[0].access_state,
            TextAccessState::Available
        ));
        assert!(volumes[0].chapters[0].need_fire_money == 0);
        assert!(!volumes[0].chapters[0].downloaded);

        let vip_chapter = volumes
            .iter()
            .flat_map(|volume| &volume.chapters)
            .find(|chapter| chapter.is_vip)
            .expect("fixture should contain VIP chapters");
        assert!(matches!(
            vip_chapter.content_kind,
            TextContentKind::ImageVip
        ));
        assert!(matches!(
            vip_chapter.access_state,
            TextAccessState::Unavailable
        ));
        assert!(vip_chapter.need_fire_money > 0);
    }

    #[test]
    fn treats_zero_price_vip_chapter_as_available() {
        let volumes = parse_chapter_catalog(&json!({
            "volumeList": [{
                "volumeId": 1,
                "title": "正文",
                "chapterList": [{
                    "chapId": 2,
                    "title": "已购章节",
                    "isVip": true,
                    "needFireMoney": 0
                }]
            }]
        }))
        .unwrap();
        let chapter = &volumes[0].chapters[0];
        assert_eq!(chapter.need_fire_money, 0);
        assert!(matches!(chapter.access_state, TextAccessState::Available));
    }

    #[test]
    fn rejects_catalog_without_volume_list() {
        assert!(parse_chapter_catalog(&json!({ "data": [] })).is_err());
    }
}
