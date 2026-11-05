use super::error::AppApiError;
use crate::domain::catalog::{ChapterSummary, ChapterVolume, TextAccessState};
use serde_json::Value;
use std::cell::Cell;

/// 将 App 小说目录响应转换为领域分卷列表。
///
/// 保留章节编号、标题、VIP 标记和火券价格；当前会话已购的 VIP 章节价格为 0。
/// 缺少编号的分卷或章节无法拼出上游地址，会被丢弃并汇总成一条告警，
/// 便于在上游字段变化时从日志发现，而不是让用户以为章节本来就少。
///
/// # 参数
/// - `response`：包含 `volumeList` 或 `volumes` 的 App 目录 JSON。
///
/// # 返回
/// 返回仅含有效编号分卷的目录列表。
///
/// # 错误
/// 响应缺少分卷数组时返回错误。
pub(super) fn parse_chapter_catalog(response: &Value) -> Result<Vec<ChapterVolume>, AppApiError> {
    let volumes = response
        .get("volumeList")
        .or_else(|| response.get("volumes"))
        .and_then(Value::as_array)
        .ok_or_else(|| AppApiError::local("SF App 目录未返回分卷列表".to_string()))?;
    let dropped_volumes = Cell::new(0usize);
    let dropped_chapters = Cell::new(0usize);
    let parsed = volumes
        .iter()
        .filter_map(|volume| {
            let volume_id = volume.get("volumeId").and_then(Value::as_i64).unwrap_or(0);
            if volume_id <= 0 {
                dropped_volumes.set(dropped_volumes.get() + 1);
                return None;
            }
            Some(ChapterVolume {
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
                        let Some(chap_id) = chapter.get("chapId").and_then(Value::as_i64) else {
                            dropped_chapters.set(dropped_chapters.get() + 1);
                            return None;
                        };
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
                            access_state,
                            downloaded: false,
                        })
                    })
                    .collect(),
            })
        })
        .collect();
    let (volumes_dropped, chapters_dropped) = (dropped_volumes.get(), dropped_chapters.get());
    if volumes_dropped > 0 || chapters_dropped > 0 {
        log_warn!(
            "SF App 目录存在缺少编号的条目，已丢弃：分卷 {volumes_dropped} 个、章节 {chapters_dropped} 章"
        );
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::parse_chapter_catalog;
    use crate::domain::catalog::TextAccessState;
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

    #[test]
    fn drops_entries_without_ids_instead_of_failing_whole_catalog() {
        let volumes = parse_chapter_catalog(&json!({
            "volumeList": [
                { "title": "没有编号的分卷", "chapterList": [] },
                {
                    "volumeId": 5,
                    "title": "正常分卷",
                    "chapterList": [
                        { "chapId": 10, "title": "正常章节" },
                        { "title": "没有编号的章节" }
                    ]
                }
            ]
        }))
        .unwrap();

        assert_eq!(volumes.len(), 1, "缺少编号的分卷应被丢弃");
        assert_eq!(volumes[0].volume_id, 5);
        assert_eq!(volumes[0].chapters.len(), 1, "缺少编号的章节应被丢弃");
        assert_eq!(volumes[0].chapters[0].chap_id, 10);
    }

    #[test]
    fn falls_back_to_default_titles_when_missing() {
        let volumes = parse_chapter_catalog(&json!({
            "volumeList": [{
                "volumeId": 1,
                "chapterList": [{ "chapId": 2 }]
            }]
        }))
        .unwrap();

        assert_eq!(volumes[0].title, "未命名分卷");
        assert_eq!(volumes[0].chapters[0].title, "未命名章节");
        assert!(!volumes[0].chapters[0].is_vip, "缺少标记时按免费章节处理");
    }
}
