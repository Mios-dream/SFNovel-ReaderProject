use super::error::AppApiError;
use super::AppClient;
use crate::domain::ids::validate_novel_id;
use serde_json::Value;

impl AppClient {
    /// 查找小说对应的有声专辑编号。
    ///
    /// 优先使用小说编号反查；失败时尝试标题提示，最后回退到公开小说详情中的
    /// 标题搜索，以兼容只登记了标题关系的专辑。
    ///
    /// # 参数
    /// - `novel_id`：来源小说编号。
    /// - `title_hint`：调用方已知的书名，可为空。
    ///
    /// # 返回
    /// 返回与来源小说匹配的有声专辑编号。
    ///
    /// # 错误
    /// 小说编号无效、上游请求失败或找不到可用专辑时返回错误。
    pub(crate) async fn find_audio_album_id(
        &self,
        novel_id: i64,
        title_hint: Option<&str>,
    ) -> Result<i64, AppApiError> {
        validate_novel_id(novel_id)?;
        if let Ok(album_id) = self.find_audio_album_id_by_novel_id(novel_id).await {
            return Ok(album_id);
        }
        if let Some(title) = title_hint.map(str::trim).filter(|title| !title.is_empty()) {
            if let Some(album_id) = self.search_audio_album_by_title(novel_id, title).await? {
                return Ok(album_id);
            }
        }
        let detail = self
            .get_public_data(&format!("/novels/{novel_id}"), &[])
            .await?;
        let title = detail
            .get("novelName")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .ok_or_else(|| AppApiError::local("SF 未返回有效小说标题".to_string()))?;
        if let Some(album_id) = self.search_audio_album_by_title(novel_id, title).await? {
            return Ok(album_id);
        }
        Err(AppApiError::local("该作品没有可用的有声专辑".to_string()))
    }

    /// 通过 App 的专辑反查接口读取来源小说对应的有声专辑编号。
    async fn find_audio_album_id_by_novel_id(&self, novel_id: i64) -> Result<i64, AppApiError> {
        let response = self
            .get_public_data(&format!("/albums/novelid/{novel_id}"), &[])
            .await?;
        Self::audio_album_id_from_payload(&response)
            .ok_or_else(|| AppApiError::local("SF 未返回对应的有声专辑".to_string()))
    }

    /// 按标题搜索并返回来源小说匹配的最新有声专辑编号。
    async fn search_audio_album_by_title(
        &self,
        novel_id: i64,
        title: &str,
    ) -> Result<Option<i64>, AppApiError> {
        let response = self
            .get_public_data(
                "/search/novels/result/new",
                &[
                    ("page", "0".to_string()),
                    ("q", title.to_string()),
                    ("size", "30".to_string()),
                    ("sort", "hot".to_string()),
                    ("searchType", "0".to_string()),
                ],
            )
            .await?;
        Ok(response
            .get("albums")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|album| album.get("novelId").and_then(Value::as_i64) == Some(novel_id))
            .max_by_key(|album| {
                album
                    .get("lastUpdateTime")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            })
            .and_then(|album| album.get("albumId").and_then(Value::as_i64))
            .filter(|album_id| *album_id > 0))
    }

    /// 从专辑反查响应中读取正数专辑编号。
    fn audio_album_id_from_payload(value: &Value) -> Option<i64> {
        value
            .get("albumId")
            .and_then(Value::as_i64)
            .or_else(|| {
                value
                    .as_array()
                    .and_then(|items| items.first())
                    .and_then(|item| item.get("albumId"))
                    .and_then(Value::as_i64)
            })
            .filter(|album_id| *album_id > 0)
    }
}
