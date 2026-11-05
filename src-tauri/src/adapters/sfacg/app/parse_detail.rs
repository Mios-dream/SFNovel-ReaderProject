use super::parsing::{parse_tags, payload_text};
use crate::domain::catalog::NovelDetail;
use serde_json::Value;

/// 将 App 小说详情载荷转换为领域详情 DTO。
///
/// 字段缺失时使用稳定的展示回退值；章节与统计扩展字段只在上游实际返回时保留。
///
/// # 参数
/// - `detail`：App 小说详情 JSON。
/// - `novel_id`：调用方请求的小说编号。
///
/// # 返回
/// 返回带默认文案和可选扩展元数据的作品详情。
pub(super) fn detail_from_novel_payload(detail: &Value, novel_id: i64) -> NovelDetail {
    let expand = detail.get("expand");
    let latest = expand.and_then(|value| {
        value
            .get("latestChapter")
            .or_else(|| value.get("latestchapter"))
    });
    NovelDetail {
        novel_id,
        novel_name: detail
            .get("novelName")
            .and_then(Value::as_str)
            .unwrap_or("未命名作品")
            .to_string(),
        author_name: detail
            .get("authorName")
            .and_then(Value::as_str)
            .unwrap_or("未知作者")
            .to_string(),
        novel_cover: detail
            .get("novelCover")
            .and_then(Value::as_str)
            .or_else(|| {
                expand
                    .and_then(|value| value.get("bigNovelCover"))
                    .and_then(Value::as_str)
            })
            .unwrap_or_default()
            .to_string(),
        last_update_time: detail
            .get("lastUpdateTime")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        description: expand
            .and_then(|value| value.get("intro"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("暂无简介")
            .to_string(),
        is_finish: detail
            .get("isFinish")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        type_name: expand
            .and_then(|value| value.get("typeName"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string),
        tags: parse_tags(expand.and_then(|value| value.get("sysTags"))),
        score: detail.get("point").and_then(Value::as_f64),
        chapter_count: expand
            .and_then(|value| value.get("chapterCount"))
            .and_then(Value::as_i64),
        character_count: detail.get("charCount").and_then(Value::as_i64),
        view_count: detail.get("viewTimes").and_then(Value::as_i64),
        mark_count: detail.get("markCount").and_then(Value::as_i64),
        point_count: expand
            .and_then(|value| value.get("pointCount"))
            .and_then(Value::as_i64),
        favorite_count: expand
            .and_then(|value| value.get("fav"))
            .and_then(Value::as_i64),
        ticket_count: expand
            .and_then(|value| value.get("ticket"))
            .and_then(Value::as_i64),
        latest_chapter_title: latest
            .and_then(|value| value.get("title"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        latest_chapter_time: latest
            .and_then(|value| value.get("addTime"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        allow_download: detail.get("allowDown").and_then(Value::as_bool),
        source_path: None,
    }
}

/// 将 App 有声详情载荷转换为兼容小说详情 DTO。
pub(super) fn detail_from_audio_payload(detail: &Value, novel_id: i64) -> NovelDetail {
    let text = |keys: &[&str]| payload_text(detail, keys);
    let expand = detail.get("expand");
    let latest = expand.and_then(|value| {
        value
            .get("latestChapter")
            .or_else(|| value.get("latestchapter"))
    });
    NovelDetail {
        novel_id,
        novel_name: text(&["name", "albumName", "novelName"])
            .unwrap_or_else(|| "未命名有声作品".to_string()),
        author_name: text(&["authorName", "author", "anchorName"])
            .unwrap_or_else(|| "未知作者".to_string()),
        novel_cover: text(&[
            "coverBig",
            "coverMedium",
            "coverSmall",
            "albumCover",
            "cover",
        ])
        .unwrap_or_default(),
        last_update_time: text(&["lastUpdateTime", "updateTime"]).unwrap_or_default(),
        description: text(&["intro", "description", "content"])
            .unwrap_or_else(|| "暂无简介".to_string()),
        is_finish: detail
            .get("isFinished")
            .or_else(|| detail.get("isFinish"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        type_name: text(&["typeName", "categoryName"]),
        tags: parse_tags(
            detail
                .get("sysTags")
                .or_else(|| detail.get("tags"))
                .or_else(|| expand.and_then(|value| value.get("sysTags"))),
        ),
        score: detail.get("point").and_then(Value::as_f64),
        chapter_count: detail
            .get("chapterCount")
            .or_else(|| expand.and_then(|value| value.get("chapterCount")))
            .and_then(Value::as_i64),
        character_count: detail.get("charCount").and_then(Value::as_i64),
        view_count: detail
            .get("visitTimes")
            .or_else(|| detail.get("viewTimes"))
            .and_then(Value::as_i64),
        mark_count: detail.get("markCount").and_then(Value::as_i64),
        point_count: detail.get("pointCount").and_then(Value::as_i64),
        favorite_count: detail.get("fav").and_then(Value::as_i64),
        ticket_count: detail.get("ticket").and_then(Value::as_i64),
        latest_chapter_title: latest
            .and_then(|value| value.get("title"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        latest_chapter_time: latest
            .and_then(|value| value.get("addTime"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        allow_download: None,
        source_path: None,
    }
}

/// 将漫画详情响应转换为统一的详情结构。
///
/// 漫画的字段名与小说、有声都不同（`comicName`、`comicCover`、`folderName`），
/// 但结构与兜底规则一致，因此与另外两种载荷放在同一模块，便于对照维护。
///
/// # 参数
/// - `detail`：`/comics/{id}` 响应体。
/// - `comic_id`：漫画编号，作为详情的主编号返回。
pub(super) fn detail_from_comic_payload(detail: &Value, comic_id: i64) -> NovelDetail {
    let text = |keys: &[&str]| payload_text(detail, keys);
    let expand = detail.get("expand");
    let latest = expand.and_then(|value| {
        value
            .get("latestChapter")
            .or_else(|| value.get("latestchapter"))
    });
    NovelDetail {
        novel_id: comic_id,
        novel_name: text(&["comicName", "novelName", "name"])
            .unwrap_or_else(|| "未命名漫画".to_string()),
        author_name: text(&["authorName", "author"]).unwrap_or_else(|| "未知作者".to_string()),
        novel_cover: text(&[
            "coverBig",
            "comicCover",
            "coverMedium",
            "coverSmall",
            "novelCover",
            "cover",
        ])
        .unwrap_or_default(),
        last_update_time: text(&["lastUpdateTime", "updateTime"]).unwrap_or_default(),
        description: text(&["intro", "description", "content"])
            .unwrap_or_else(|| "暂无简介".to_string()),
        is_finish: detail
            .get("isFinished")
            .or_else(|| detail.get("isFinish"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        type_name: Some(text(&["typeName", "categoryName"]).unwrap_or_else(|| "漫画".to_string())),
        tags: parse_tags(
            detail
                .get("sysTags")
                .or_else(|| detail.get("tags"))
                .or_else(|| expand.and_then(|value| value.get("sysTags"))),
        ),
        score: detail.get("point").and_then(Value::as_f64),
        chapter_count: detail
            .get("chapterCount")
            .or_else(|| expand.and_then(|value| value.get("chapterCount")))
            .and_then(Value::as_i64),
        character_count: detail.get("charCount").and_then(Value::as_i64),
        view_count: detail
            .get("visitTimes")
            .or_else(|| detail.get("viewTimes"))
            .and_then(Value::as_i64),
        mark_count: detail.get("markCount").and_then(Value::as_i64),
        point_count: detail
            .get("pointCount")
            .or_else(|| expand.and_then(|value| value.get("pointCount")))
            .and_then(Value::as_i64),
        favorite_count: detail
            .get("favoriteCount")
            .or_else(|| detail.get("fav"))
            .or_else(|| expand.and_then(|value| value.get("fav")))
            .and_then(Value::as_i64),
        ticket_count: detail
            .get("ticket")
            .or_else(|| expand.and_then(|value| value.get("ticket")))
            .and_then(Value::as_i64),
        latest_chapter_title: latest
            .and_then(|value| value.get("title"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        latest_chapter_time: latest
            .and_then(|value| value.get("addTime"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        allow_download: None,
        source_path: text(&["folderName"]).filter(|value| {
            !value.is_empty()
                && value.len() <= 100
                && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{detail_from_comic_payload, detail_from_novel_payload};
    use serde_json::{json, Map, Value};

    fn novel_payload() -> Value {
        let mut payload: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../TESTDATA/novelInfo.json"
        )))
        .expect("novel fixture should be valid JSON");
        let object = payload.as_object_mut().unwrap();
        let mut expand = Map::new();
        for (source, target) in [("intro", "intro"), ("tags", "sysTags")] {
            if let Some(value) = object.remove(source) {
                expand.insert(target.to_string(), value);
            }
        }
        expand.insert("chapterCount".to_string(), json!(88));
        object.insert("expand".to_string(), Value::Object(expand));
        payload
    }

    #[test]
    fn parses_real_novel_fixture_with_expanded_metadata() {
        let detail = detail_from_novel_payload(&novel_payload(), 216187);

        assert_eq!(detail.novel_id, 216187);
        assert_eq!(detail.novel_name, "异界医生的我娶了个龙傲天美少女");
        assert_eq!(detail.author_name, "Nova酱");
        assert_eq!(detail.character_count, Some(2_368_638));
        assert_eq!(detail.chapter_count, Some(88));
        assert_eq!(
            detail.tags,
            Some(vec![
                "异世界".to_string(),
                "无敌".to_string(),
                "倒追".to_string()
            ])
        );
        assert!(detail.description.starts_with("这个大陆，医者为尊"));
        assert!(!detail.is_finish);
    }

    #[test]
    fn comic_payload_prefers_comic_fields() {
        let detail = detail_from_comic_payload(
            &json!({
                "comicName": "漫画名",
                "authorName": "作者",
                "comicCover": "https://img/cover.jpg",
                "folderName": "comic2024",
                "sysTags": ["热血", { "tagName": "校园" }]
            }),
            77,
        );

        assert_eq!(detail.novel_id, 77);
        assert_eq!(detail.novel_name, "漫画名");
        assert_eq!(detail.novel_cover, "https://img/cover.jpg");
        assert_eq!(detail.source_path.as_deref(), Some("comic2024"));
        assert_eq!(
            detail.type_name.as_deref(),
            Some("漫画"),
            "缺少分类时回退到漫画"
        );
        assert_eq!(
            detail.tags,
            Some(vec!["热血".to_string(), "校园".to_string()]),
            "字符串标签与对象标签都应解析"
        );
    }

    #[test]
    fn comic_payload_drops_unsafe_folder_name() {
        let detail = detail_from_comic_payload(&json!({ "folderName": "../escape" }), 1);

        assert!(
            detail.source_path.is_none(),
            "含路径分隔符的来源标识不能透传给下游"
        );
        assert_eq!(detail.novel_name, "未命名漫画");
        assert_eq!(detail.description, "暂无简介");
        assert_eq!(detail.author_name, "未知作者");
    }

    #[test]
    fn comic_payload_reads_counters_from_expand() {
        let detail = detail_from_comic_payload(
            &json!({
                "comicName": "漫画",
                "isFinished": true,
                "expand": {
                    "chapterCount": 120,
                    "fav": 9,
                    "ticket": 4,
                    "pointCount": 3,
                    "latestChapter": { "title": "最新话", "addTime": "2026-01-01" }
                }
            }),
            5,
        );

        assert!(detail.is_finish);
        assert_eq!(detail.chapter_count, Some(120));
        assert_eq!(detail.favorite_count, Some(9));
        assert_eq!(detail.ticket_count, Some(4));
        assert_eq!(detail.point_count, Some(3));
        assert_eq!(detail.latest_chapter_title.as_deref(), Some("最新话"));
        assert_eq!(detail.latest_chapter_time.as_deref(), Some("2026-01-01"));
    }
}
