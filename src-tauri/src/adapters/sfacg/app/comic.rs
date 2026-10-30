use super::constants::COMIC_CHAPTER_EXPAND;
use super::AppClient;
use crate::domain::catalog::{ComicChapter, ComicUnlockState, NovelDetail};
use crate::domain::ids::validate_novel_id;
use serde_json::Value;

impl AppClient {
    /// 读取漫画标题和来源目录标识。
    ///
    /// # 参数
    /// - `comic_id`：漫画编号。
    ///
    /// # 返回
    /// 返回 `(漫画标题, 来源目录标识)`。
    ///
    /// # 错误
    /// 编号无效、上游请求失败，或响应缺少标题、目录标识时返回错误。
    pub(crate) async fn comic_identity(&self, comic_id: i64) -> Result<(String, String), String> {
        validate_novel_id(comic_id)?;
        let detail = self
            .get_public_data(&format!("/comics/{comic_id}"), &[])
            .await?;
        // 漫画名称与来源目录标识用于作品识别和本地记录。
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

    /// 获取公开漫画详情，并保留来源目录标识供本地记录使用。
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

    /// 读取漫画 App 目录，并按阅读顺序恢复章节。
    pub(crate) async fn get_comic_catalog(
        &self,
        comic_id: i64,
    ) -> Result<Vec<ComicChapter>, String> {
        validate_novel_id(comic_id)?;
        let payload = self
            .get_comic_data(
                &format!("/comics/{comic_id}/chaps"),
                &[("expand", COMIC_CHAPTER_EXPAND.to_string())],
            )
            .await?;
        let items = payload
            .as_array()
            .ok_or_else(|| "SF 漫画目录格式无效".to_string())?;
        let mut chapters = Vec::with_capacity(items.len());
        let mut seen = std::collections::HashSet::new();
        for item in items {
            let Some(id) = item.get("chapterId").and_then(Value::as_i64) else {
                continue;
            };
            let Some(path_name) = item
                .get("pathName")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            if !seen.insert(id) {
                continue;
            }
            let need_fire_money = item
                .get("expand")
                .and_then(|value| value.get("needFireMoney"))
                .and_then(Value::as_i64)
                .unwrap_or(0);
            let is_vip =
                item.get("isVip").and_then(Value::as_bool).unwrap_or(false) || need_fire_money > 0;
            chapters.push(ComicChapter {
                id,
                title: item
                    .get("chapterTitle")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("未命名章节")
                    .to_string(),
                path_name: path_name.to_string(),
                is_vip,
                is_unlocked: if is_vip {
                    ComicUnlockState::Unknown
                } else {
                    ComicUnlockState::Unlocked
                },
            });
        }
        if chapters.is_empty() {
            return Err("SF 漫画目录没有返回章节".to_string());
        }
        chapters.reverse();
        Ok(chapters)
    }

    /// 通过 App 接口读取漫画章节图片地址。
    pub(crate) async fn get_comic_chapter_images(
        &self,
        comic_id: i64,
        path_name: &str,
    ) -> Result<Vec<String>, String> {
        validate_novel_id(comic_id)?;
        if path_name.trim().is_empty() {
            return Err("漫画章节缺少图片路径".to_string());
        }
        let payload = self
            .get_comic_data(
                &format!(
                    "/comics/{comic_id}/chappics/{}",
                    urlencoding::encode(path_name)
                ),
                &[("autoOrder", "false".to_string())],
            )
            .await?;
        let images = payload
            .as_array()
            .ok_or_else(|| "SF 漫画图片接口未返回图片".to_string())?
            .iter()
            .filter_map(Value::as_str)
            .filter(|url| url.starts_with("https://"))
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        if images.is_empty() {
            return Err("该漫画章节没有可读取的图片".to_string());
        }
        Ok(images)
    }
}
