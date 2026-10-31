use super::parse_catalog::parse_chapter_catalog;
use super::parsing::json_bool_field;
use super::AppClient;
use crate::domain::catalog::ChapterVolume;
use crate::domain::content::ApiChapterContent;
use crate::domain::ids::validate_novel_id;
use serde_json::Value;

impl AppClient {
    /// 读取文字章节正文及其所属作品、分卷编号。
    ///
    /// # 参数
    /// - `chapter_id`：待读取的章节编号。
    ///
    /// # 返回
    /// 返回正文、所属小说编号和所属分卷编号。
    ///
    /// # 错误
    /// 章节编号无效、上游未返回正文或缺少作品归属信息时返回错误。
    pub(crate) async fn get_chapter_content_and_metadata(
        &self,
        chapter_id: i64,
    ) -> Result<ApiChapterContent, String> {
        if chapter_id <= 0 {
            return Err("章节编号无效".to_string());
        }
        let response = self
            .get_data(
                &format!("/Chaps/{chapter_id}"),
                &[("expand", "content,isContentEncrypted".to_string())],
            )
            .await?;
        let expand = response.get("expand");
        let content = expand
            .and_then(|value| value.get("content"))
            .and_then(Value::as_str)
            .or_else(|| response.get("content").and_then(Value::as_str))
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "SF App API 未返回章节正文".to_string())?;
        let is_content_encrypted = expand
            .and_then(|value| json_bool_field(value, &["isContentEncrypted"]))
            .unwrap_or(false);
        let novel_id = response
            .get("novelId")
            .and_then(Value::as_i64)
            .ok_or_else(|| "SF App API 未返回章节所属作品信息".to_string())?;
        let volume_id = response
            .get("volumeId")
            .and_then(Value::as_i64)
            .ok_or_else(|| "SF App API 未返回章节所属卷信息".to_string())?;
        Ok(ApiChapterContent {
            content: content.to_string(),
            is_content_encrypted,
            novel_id,
            volume_id,
        })
    }

    /// 获取文字小说的 App 章节目录。
    ///
    /// 目录请求用于章节元数据；正文下载由下载策略决定是否优先使用 App API。
    pub(crate) async fn get_chapter_catalog(
        &self,
        novel_id: i64,
    ) -> Result<Vec<ChapterVolume>, String> {
        validate_novel_id(novel_id)?;
        let response = self
            .get_public_data(&format!("/novels/{novel_id}/dirs"), &[])
            .await?;
        parse_chapter_catalog(&response)
    }
}
