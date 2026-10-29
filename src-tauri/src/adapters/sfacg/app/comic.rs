use super::AppClient;
use crate::domain::catalog::NovelDetail;
use crate::domain::ids::validate_novel_id;
use serde_json::Value;

impl AppClient {
    /// 读取漫画标题和网页目录标识。
    ///
    /// # 参数
    /// - `comic_id`：漫画编号。
    ///
    /// # 返回
    /// 返回 `(漫画标题, 网页目录标识)`。
    ///
    /// # 错误
    /// 编号无效、上游请求失败，或响应缺少标题、目录标识时返回错误。
    pub(crate) async fn comic_identity(&self, comic_id: i64) -> Result<(String, String), String> {
        validate_novel_id(comic_id)?;
        let detail = self
            .get_public_data(&format!("/comics/{comic_id}"), &[])
            .await?;
        // 漫画名称与目录标识是网页目录客户端的必要信息，必须返回。
        let title = detail
            .get("comicName")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "SF 漫画接口未返回作品名称".to_string())?
            .to_string();
        let folder = detail
            .get("folderName")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "SF 漫画接口未返回目录标识".to_string())?
            .to_string();
        Ok((title, folder))
    }

    /// 获取公开漫画详情，并保留网页目录标识供网页目录客户端使用。
    ///
    /// # 错误
    /// 漫画编号无效、上游请求失败或响应缺少有效作品信息时返回错误。
    pub(crate) async fn get_comic_details(&self, comic_id: i64) -> Result<NovelDetail, String> {
        validate_novel_id(comic_id)?;
        let detail = self
            .get_public_data(
                &format!("/comics/{comic_id}"),
                &[(
                    "expand",
                    "intro,typeName,sysTags,chapterCount,latestchapter,fav,ticket,pointCount"
                        .to_string(),
                )],
            )
            .await?;
        let text = |keys: &[&str]| {
            keys.iter().find_map(|key| {
                detail
                    .get(*key)
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .map(ToString::to_string)
                    .or_else(|| {
                        detail
                            .get("expand")
                            .and_then(|expand| expand.get(*key))
                            .and_then(Value::as_str)
                            .filter(|value| !value.trim().is_empty())
                            .map(ToString::to_string)
                    })
            })
        };
        let tags = detail
            .get("sysTags")
            .or_else(|| detail.get("tags"))
            .or_else(|| {
                detail
                    .get("expand")
                    .and_then(|expand| expand.get("sysTags").or_else(|| expand.get("tags")))
            })
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|tag| {
                        tag.as_str()
                            .or_else(|| tag.get("tagName").and_then(Value::as_str))
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(ToString::to_string)
                    })
                    .take(8)
                    .collect()
            });
        let expand = detail.get("expand");
        let latest = expand.and_then(|value| {
            value
                .get("latestChapter")
                .or_else(|| value.get("latestchapter"))
        });
        Ok(NovelDetail {
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
            type_name: Some(
                text(&["typeName", "categoryName"]).unwrap_or_else(|| "漫画".to_string()),
            ),
            tags,
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
        })
    }
}
