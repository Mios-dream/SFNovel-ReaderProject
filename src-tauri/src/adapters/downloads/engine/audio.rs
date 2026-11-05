//! 有声章节下载流程。
//!
//! 与文字下载保持同一形状：准备目录与作品信息、拉取目录筛出章节、
//! 逐章下载并刷新播放列表、汇总结果。

use super::assets::enrich_audio_metadata;
use super::prepare::{
    announce_started, format_skipped, prepare_book, BookSetup, DownloadedField,
};
use super::{finish, JobContext};
use crate::adapters::downloads::registry;
use crate::adapters::downloads::DownloadState;
use crate::adapters::sfacg::source::SfSource;
use crate::adapters::storage::{replace_file, safe_library_name};
use crate::application::ports::RemoteGateway;
use crate::domain::catalog::AudioChapter;
use crate::domain::download::JobStatus;
use crate::domain::error::{AppError, AppResult};
use crate::domain::ids::{validate_album_id, NovelId};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Manager;

/// 播放列表文件名。
const PLAYLIST_NAME: &str = "有声目录.m3u8";

/// 清理章节标题并限制为可安全用于文件名的长度。
///
/// # 参数
/// - `value`：上游章节标题。
fn safe_audio_name(value: &str) -> String {
    safe_library_name(value).chars().take(100).collect()
}

/// 使用 App 凭证下载有声章节。
async fn fetch_audio(source: &SfSource, url: &str, referer: &str) -> AppResult<Vec<u8>> {
    source
        .audio_asset_request(url, referer, "audio/mpeg,*/*;q=0.8")
        .send()
        .await
        .map_err(|error| AppError::upstream(format!("无法下载有声章节：{error}")))?
        .error_for_status()
        .map_err(|error| AppError::upstream(format!("有声章节下载被拒绝：{error}")))?
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|error| AppError::storage(format!("无法读取有声章节：{error}")))
}

/// 根据已存在的音频文件生成 M3U 播放列表。
///
/// # 参数
/// - `audio_directory`：有声文件目录。
/// - `catalog`：有声目录，用于保持播放顺序。
///
/// # 错误
/// 播放列表写入失败时返回错误消息。
fn persist_audio_playlist(audio_directory: &Path, catalog: &[AudioChapter]) -> AppResult<()> {
    let mut playlist = vec!["#EXTM3U".to_string()];
    for (index, chapter) in catalog.iter().enumerate() {
        let file_name = format!("{:03} - {}.mp3", index + 1, safe_audio_name(&chapter.title));
        if audio_directory.join(&file_name).is_file() {
            playlist.push(format!("#EXTINF:-1,{} - {}", chapter.volume, chapter.title));
            playlist.push(file_name);
        }
    }
    fs::write(
        audio_directory.join(PLAYLIST_NAME),
        format!("{}\n", playlist.join("\n")),
    )
    .map_err(|error| AppError::storage(format!("无法写入有声播放列表：{error}")))
}

/// 挑出选中章节在完整目录中的下标，保持播放顺序。
///
/// # 参数
/// - `catalog`：上游返回的完整有声目录。
/// - `selected`：用户勾选的章节编号集合。
fn select_indices(catalog: &[AudioChapter], selected: &HashSet<i64>) -> Vec<usize> {
    catalog
        .iter()
        .enumerate()
        .filter(|(_, chapter)| selected.contains(&chapter.id))
        .map(|(index, _)| index)
        .collect()
}

/// 一次有声下载任务的运行时状态。
struct AudioDownload<'a> {
    context: JobContext<'a>,
    book: BookSetup,
    album_id: i64,
    /// 本地目录与展示名称。
    title: String,
    /// 专辑完整目录，用于保持播放顺序与文件名编号。
    catalog: Vec<AudioChapter>,
    /// 被跳过的章节及原因，任务结束时汇总成提示。
    skipped: Vec<(String, String)>,
}

impl<'a> AudioDownload<'a> {
    /// 阶段一：准备目录、写入专辑信息并补全元数据。
    ///
    /// # 错误
    /// 客户端创建、专辑定位、清单写入失败时返回错误；风控直接中止。
    async fn prepare(
        context: JobContext<'a>,
        novel_id: i64,
        album_id: Option<i64>,
        title: &str,
    ) -> AppResult<Self> {
        let mut book = prepare_book(context, title).await?;
        let novel_id = NovelId::new(novel_id)?;
        let album_id = match album_id {
            Some(album_id) => {
                validate_album_id(album_id)?;
                album_id
            }
            None => book.source.audio_album_id(novel_id, Some(title)).await?,
        };
        book.write_identity(|manifest| {
            let audio = manifest.audio.get_or_insert_with(Default::default);
            audio.id = Some(album_id);
            audio.catalog_id = Some(novel_id.get());
            audio.title = Some(title.to_string());
            audio
                .author
                .get_or_insert_with(|| "未知作者".to_string());
            audio
                .description
                .get_or_insert_with(|| "暂无简介".to_string());
        })?;
        let enriched = {
            let BookSetup {
                source,
                directory,
                manifest,
                ..
            } = &mut book;
            let audio = manifest
                .audio
                .as_mut()
                .ok_or_else(|| AppError::internal("有声元数据初始化失败"))?;
            enrich_audio_metadata(source, directory, audio, novel_id.get(), album_id).await
        };
        book.absorb_enrich_failure(enriched)?;
        book.persist()?;
        announce_started(&context);
        Ok(Self {
            context,
            book,
            album_id,
            title: title.to_string(),
            catalog: Vec::new(),
            skipped: Vec::new(),
        })
    }

    /// 阶段二：拉取目录，返回选中章节在完整目录中的下标。
    ///
    /// # 错误
    /// 目录请求失败或没有任何匹配章节时返回错误。
    async fn plan(&mut self, chapter_ids: &[i64]) -> AppResult<Vec<usize>> {
        let (_catalog_title, catalog) = self
            .book
            .source
            .audio_catalog(self.album_id, Some(self.title.as_str()))
            .await?;
        let selected = chapter_ids.iter().copied().collect::<HashSet<_>>();
        let indices = select_indices(&catalog, &selected);
        if indices.is_empty() {
            return Err(AppError::invalid_input("没有可下载的有声章节"));
        }
        self.catalog = catalog;
        Ok(indices)
    }

    /// 阶段三：逐章下载，成功后立即刷新播放列表。
    ///
    /// # 返回
    /// 本轮实际尝试的章节数，供收尾判断是否全部失败。
    ///
    /// # 错误
    /// 任务被暂停、触发风控或存储失败时中止；单章可跳过的情况不报错。
    async fn download(&mut self, indices: Vec<usize>) -> AppResult<usize> {
        let audio_directory = self.book.media_directory("audio", "有声")?;
        let mut downloaded = self.book.take_downloaded_ids(DownloadedField::Audio);
        let total = indices.len();
        for (selected_index, catalog_index) in indices.into_iter().enumerate() {
            if self.context.cancelled.load(Ordering::Relaxed) {
                return Err(AppError::cancelled("下载已暂停"));
            }
            let chapter = self.catalog[catalog_index].clone();
            let file_name = format!(
                "{:03} - {}.mp3",
                catalog_index + 1,
                safe_audio_name(&chapter.title)
            );
            let target = audio_directory.join(&file_name);
            self.announce_downloading(selected_index, total, &chapter.title);
            if !self.save_chapter(&chapter, &target).await? {
                continue;
            }
            downloaded.insert(chapter.id);
            self.book
                .save_downloaded_ids(DownloadedField::Audio, &downloaded)?;
            persist_audio_playlist(&audio_directory, &self.catalog)?;
            self.announce_finished(selected_index, total, &chapter.title);
            self.wait_between_requests().await;
        }
        self.book
            .save_downloaded_ids(DownloadedField::Audio, &downloaded)?;
        persist_audio_playlist(&audio_directory, &self.catalog)?;
        Ok(total)
    }

    /// 下载单章音频，返回是否计入进度。
    ///
    /// 返回 `false` 表示该章被跳过：既不推进进度，也不做节流等待。
    ///
    /// # 错误
    /// 风控、文件写入失败时返回错误。
    async fn save_chapter(&mut self, chapter: &AudioChapter, target: &Path) -> AppResult<bool> {
        if target.is_file() {
            return Ok(true);
        }
        let audio_url = match self.book.source.audio_source(chapter.id).await {
            Ok(url) => url,
            Err(error) if error.is_risk_control() => return Err(error),
            Err(error) => {
                self.skip(chapter, error, "章节不可下载，已跳过");
                return Ok(false);
            }
        };
        let payload = fetch_audio(
            &self.book.source,
            &audio_url,
            &format!("https://m.sfacg.com/a/{}/", chapter.id),
        )
        .await;
        let payload = match payload {
            Ok(payload) => payload,
            Err(error) if error.is_risk_control() => return Err(error),
            Err(error) => {
                self.skip(chapter, error, "章节音频下载失败，已跳过");
                return Ok(false);
            }
        };
        replace_file(target, &payload, "有声章节")?;
        Ok(true)
    }

    /// 记录被跳过的章节并更新提示文案。
    fn skip(&mut self, chapter: &AudioChapter, error: AppError, notice: &str) {
        log_warn!(
            "[sfacg] audio chapter skipped: chapter_id={}, title={}, error={error}",
            chapter.id,
            chapter.title
        );
        self.skipped.push((chapter.title.clone(), error.to_string()));
        self.notify(&format!("{notice}：{}", chapter.title));
    }

    /// 阶段四：汇总结果。
    ///
    /// # 错误
    /// 全部章节都被跳过，或部分章节被跳过时返回汇总提示；
    /// 全部成功时返回播放列表相对地址。
    fn into_result(self, total: usize) -> AppResult<String> {
        if self.skipped.len() == total {
            let (chapter_title, error) = &self.skipped[0];
            return Err(AppError::upstream(format!(
                "所有有声章节都不可下载：{chapter_title}（{error}）"
            )));
        }
        if self.skipped.is_empty() {
            return Ok(format!("audio/{PLAYLIST_NAME}"));
        }
        Err(AppError::upstream(format_skipped(&self.skipped, "有声章节")))
    }

    /// 推送"正在下载"进度。
    fn announce_downloading(&self, index: usize, total: usize, chapter_title: &str) {
        let progress = 2 + ((index * 94) / total) as u8;
        self.announce(progress, &format!("正在下载：{chapter_title}"), false);
    }

    /// 推送单章完成进度。
    fn announce_finished(&self, index: usize, total: usize, chapter_title: &str) {
        let progress = 2 + (((index + 1) * 94) / total) as u8;
        self.announce(progress, &format!("已完成：{chapter_title}"), true);
    }

    /// 更新进度与提示，`with_library` 决定是否同时广播书库变更。
    fn announce(&self, progress: u8, message: &str, with_library: bool) {
        let JobContext {
            app,
            state,
            job_id,
            ..
        } = self.context;
        registry::update(state, job_id, |job| {
            job.status = JobStatus::Downloading;
            job.progress = progress;
            job.message = message.to_string();
        });
        registry::emit_job(app, state, job_id);
        if with_library {
            registry::emit_library(app, state, job_id);
        }
    }

    /// 只更新提示文案，不改状态与进度。
    fn notify(&self, message: &str) {
        let JobContext {
            app,
            state,
            job_id,
            ..
        } = self.context;
        registry::update(state, job_id, |job| job.message = message.to_string());
        registry::emit_job(app, state, job_id);
    }

    /// 按设置的最小间隔等待，避免触发上游限流。
    async fn wait_between_requests(&self) {
        tokio::time::sleep(std::time::Duration::from_millis(
            self.book.settings.request_interval_ms,
        ))
        .await;
    }
}

/// 执行有声下载任务并发布最终状态。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `job_id`：任务唯一标识。
/// - `novel_id`：小说编号。
/// - `album_id`：可选的有声专辑编号。
/// - `title`：本地目录与展示名称。
/// - `chapter_ids`：待下载章节编号。
/// - `cancelled`：任务取消标记。
pub(super) async fn run(
    app: tauri::AppHandle,
    job_id: String,
    novel_id: i64,
    album_id: Option<i64>,
    title: String,
    chapter_ids: Vec<i64>,
    cancelled: Arc<AtomicBool>,
) {
    let state = app.state::<DownloadState>();
    let context = JobContext {
        app: &app,
        state: state.inner(),
        job_id: &job_id,
        cancelled: cancelled.as_ref(),
    };
    let result = run_inner(&context, novel_id, album_id, &title, &chapter_ids).await;
    finish(&context, result, "有声章节下载完成", true);
}

/// 依次执行四个阶段。
async fn run_inner(
    context: &JobContext<'_>,
    novel_id: i64,
    album_id: Option<i64>,
    title: &str,
    chapter_ids: &[i64],
) -> AppResult<String> {
    let mut task = AudioDownload::prepare(*context, novel_id, album_id, title).await?;
    let indices = task.plan(chapter_ids).await?;
    let total = task.download(indices).await?;
    task.into_result(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一条有声目录章节。
    fn chapter(id: i64, title: &str) -> AudioChapter {
        AudioChapter {
            id,
            title: title.to_string(),
            volume: "第一卷".to_string(),
            is_vip: false,
            is_unlocked: true,
        }
    }

    #[test]
    fn select_indices_keeps_playlist_order() {
        let catalog = vec![chapter(11, "第一章"), chapter(12, "第二章"), chapter(13, "第三章")];

        let indices = select_indices(&catalog, &[13, 11].into_iter().collect::<HashSet<_>>());

        assert_eq!(indices, vec![0, 2], "下标应按目录顺序返回，供播放列表编号使用");
    }

    #[test]
    fn select_indices_returns_empty_when_nothing_selected() {
        let catalog = vec![chapter(11, "第一章")];

        let indices = select_indices(&catalog, &[99].into_iter().collect::<HashSet<_>>());

        assert!(indices.is_empty());
    }
}
