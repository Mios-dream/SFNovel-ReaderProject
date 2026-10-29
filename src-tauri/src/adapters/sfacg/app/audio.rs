use super::constants::AUDIO_CHAPTER_EXPAND;
use super::parse_audio::{
    audio_catalog_title, audio_source_from_payload, audio_volume_entries, walk_audio_catalog,
};
use super::AppClient;
use crate::domain::catalog::AudioChapter;
use crate::domain::ids::validate_novel_id;

impl AppClient {
    /// 读取 App 有声专辑的分卷和完整章节目录。
    ///
    /// # 参数
    /// - `album_id`：有声专辑编号。
    /// - `title_hint`：专辑详情不可用时用于展示的标题回退值。
    ///
    /// # 返回
    /// 返回专辑标题，以及按上游分卷顺序展开并去重后的章节。
    ///
    /// # 错误
    /// 编号无效、上游没有分卷或章节，或任一目录请求失败时返回错误。
    pub(crate) async fn get_audio_catalog(
        &self,
        album_id: i64,
        title_hint: Option<&str>,
    ) -> Result<(String, Vec<AudioChapter>), String> {
        validate_novel_id(album_id)?;
        let volume_payload = self
            .get_data(&format!("/albums/{album_id}/volumes"), &[])
            .await?;
        let volumes = audio_volume_entries(&volume_payload);
        if volumes.is_empty() {
            return Err("SF App 有声目录没有返回分卷".to_string());
        }
        let volume_titles = volumes
            .iter()
            .cloned()
            .collect::<std::collections::HashMap<_, _>>();
        let mut chapters = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (volume_id, volume_title) in &volumes {
            let payload = self
                .get_data(
                    &format!("/albumvolumes/{volume_id}/chaps"),
                    &[("expand", AUDIO_CHAPTER_EXPAND.to_string())],
                )
                .await
                .map_err(|error| format!("SF App 有声分卷 {volume_id} 读取失败：{error}"))?;
            walk_audio_catalog(
                &payload,
                Some(*volume_id),
                Some(volume_title.as_str()),
                &volume_titles,
                &mut seen,
                &mut chapters,
            );
        }
        if chapters.is_empty() {
            return Err("SF App 有声目录没有返回章节".to_string());
        }
        eprintln!(
            "[sfacg] App audio catalog loaded: album_id={album_id}, chapters={}",
            chapters.len()
        );
        let fallback_title = title_hint
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .unwrap_or("未命名有声作品");
        let title = match self
            .get_public_data(
                &format!("/albums/{album_id}"),
                &[("expand", "intro".to_string())],
            )
            .await
        {
            Ok(detail) => {
                audio_catalog_title(&detail).unwrap_or_else(|| fallback_title.to_string())
            }
            Err(error) => {
                eprintln!(
                    "[sfacg] App audio title unavailable: album_id={album_id}, error={error}"
                );
                fallback_title.to_string()
            }
        };
        Ok((title, chapters))
    }

    /// 从 App 有声章节详情查找音频地址。
    ///
    /// `autoOrder=false` 禁止上游自动购买。不同客户端版本可能使用不同字段名，
    /// 因此只接受明确指向 SF 音频文件的 URL；找不到时由调用方回退到手机版页面。
    pub(crate) async fn find_audio_chapter_source(
        &self,
        chapter_id: i64,
    ) -> Result<Option<String>, String> {
        validate_novel_id(chapter_id)?;
        let payload = self
            .get_data(
                &format!("/albumchaps/{chapter_id}"),
                &[
                    ("expand", AUDIO_CHAPTER_EXPAND.to_string()),
                    ("autoOrder", "false".to_string()),
                ],
            )
            .await?;
        Ok(audio_source_from_payload(&payload))
    }
}
