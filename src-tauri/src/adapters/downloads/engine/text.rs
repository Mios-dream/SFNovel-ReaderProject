//! 文字小说下载流程。
//!
//! 一次任务分成四步：准备目录与作品信息、拉取目录筛出章节、逐章读取并落盘、汇总结果。
//! 每一步都是 `TextDownload` 上的独立方法，失败语义写在方法文档里。

use super::assets::enrich_novel_metadata;
use super::prepare::{announce_started, format_skipped, prepare_book, BookSetup};
use super::{finish, JobContext};
use crate::adapters::downloads::registry;
use crate::adapters::downloads::DownloadState;
use crate::adapters::media::{recognize_vip_chapter, split_inline_images, InlineSegment};
use crate::adapters::ocr::relative_book_path;
use crate::adapters::sfacg::source::SfSource;
use crate::adapters::storage::{
    downloaded_text_chapter_ids, read_chapter_store, replace_file,
    write_book_manifest, write_chapter_store, NativeStorage,
};
use crate::application::ports::RemoteGateway;
use crate::domain::catalog::ChapterVolume;
use crate::domain::content::TextOrigin;
use crate::domain::download::JobStatus;
use crate::domain::error::{AppError, AppResult};
use crate::domain::ids::NovelId;
use crate::domain::library::{BookManifest, ChapterIndex, StoredChapter};
use crate::domain::settings::DownloadSettings;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Manager;

/// 下载正文中的内嵌图片并改写为本地图片协议地址。
///
/// 标记解析统一由 `media::split_inline_images` 完成，这里只负责取图、决定扩展名
/// 与写回 Markdown。任一张图失败都会中止整章，避免落盘一份缺图的正文。
async fn materialize_chapter_images(
    source: &SfSource,
    directory: &Path,
    novel_id: i64,
    chapter_id: i64,
    content: &str,
) -> AppResult<String> {
    if !content.contains("[img=") {
        return Ok(content.to_string());
    }

    let image_directory = directory.join("imgs");
    fs::create_dir_all(&image_directory)
        .map_err(|error| AppError::storage(format!("无法创建章节图片目录：{error}")))?;
    let referer = format!("https://book.sfacg.com/Novel/{novel_id}/");
    let mut output = String::with_capacity(content.len());
    let mut image_index = 0;

    for segment in split_inline_images(content) {
        let url = match segment {
            InlineSegment::Text(text) => {
                output.push_str(text);
                continue;
            }
            InlineSegment::Image(url) => url,
        };

        image_index += 1;
        let response = source
            .asset_request(url, &referer, "image/avif,image/webp,image/*,*/*;q=0.8")
            .send()
            .await
            .map_err(|error| AppError::upstream(format!("无法下载章节图片 {image_index}：{error}")))?
            .error_for_status()
            .map_err(|error| AppError::upstream(format!("章节图片 {image_index} 下载被拒绝：{error}")))?;
        let extension = match response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(str::trim)
        {
            Some("image/png") => "png",
            Some("image/gif") => "gif",
            Some("image/webp") => "webp",
            Some("image/avif") => "avif",
            _ => "jpeg",
        };
        let file_name = format!("chapter-{chapter_id}-{image_index}.{extension}");
        let target = image_directory.join(&file_name);
        let payload = response
            .bytes()
            .await
            .map_err(|error| AppError::upstream(format!("无法读取章节图片 {image_index}：{error}")))?;
        replace_file(&target, &payload, "章节图片")?;
        output.push_str(&format!("![章节插图](imgs/{file_name})"));
    }

    Ok(output)
}

/// 下载 VIP 图片正文并执行 OCR，返回正文和 OCR 源图相对路径。
///
/// 取图与识别由 `media::recognize_vip_chapter` 统一实现，这里只负责把它放到
/// 书籍目录下的 `ocr/` 并换算成相对路径，供章节索引记录。
async fn resolve_vip_image_content(
    app: &tauri::AppHandle,
    source: &SfSource,
    directory: &Path,
    novel_id: i64,
    chapter_id: i64,
    save_ocr_process_files: bool,
) -> AppResult<(String, String)> {
    let recognized = recognize_vip_chapter(
        app,
        source,
        &directory.join("ocr"),
        novel_id,
        chapter_id,
        save_ocr_process_files,
    )
    .await?;
    let source_relative = relative_book_path(&recognized.image_path, directory)?;
    Ok((recognized.content, source_relative))
}

/// 待下载章节的目录信息，替代原先的七元组。
struct ChapterPlan {
    chapter_id: i64,
    volume_id: i64,
    volume_title: String,
    volume_index: i64,
    chapter_index: usize,
    chapter_title: String,
    is_vip: bool,
}

/// 按请求的章节编号从目录中挑出下载计划，保持目录原有顺序。
///
/// # 参数
/// - `catalog`：上游目录，按分卷与章节顺序排列。
/// - `requested`：用户勾选的章节编号集合。
fn select_chapters(catalog: &[ChapterVolume], requested: &HashSet<i64>) -> Vec<ChapterPlan> {
    let mut plans = Vec::new();
    for (volume_index, volume) in catalog.iter().enumerate() {
        for (chapter_index, chapter) in volume.chapters.iter().enumerate() {
            if !requested.contains(&chapter.chap_id) {
                continue;
            }
            plans.push(ChapterPlan {
                chapter_id: chapter.chap_id,
                volume_id: volume.volume_id,
                volume_title: volume.title.clone(),
                volume_index: volume_index as i64,
                chapter_index,
                chapter_title: chapter.title.clone(),
                is_vip: chapter.is_vip,
            });
        }
    }
    plans
}

/// 判断该章是否需要重新拉取。
///
/// 从未下载过的章节一律拉取；VIP 章节只有已存有 App 正文或 OCR 正文时才跳过，
/// 其余情况需要重新尝试，以便补齐之前失败的内容。
///
/// # 参数
/// - `plan`：待下载的章节计划。
/// - `existing`：本地已存的同编号章节。
fn needs_fetch(plan: &ChapterPlan, existing: Option<&StoredChapter>) -> bool {
    let Some(existing) = existing else {
        return true;
    };
    if !plan.is_vip {
        return false;
    }
    let stored_origin = existing
        .content_source
        .as_deref()
        .and_then(TextOrigin::parse);
    !(matches!(
        stored_origin,
        Some(TextOrigin::App) | Some(TextOrigin::WebVipOcr)
    ) && !existing.content.trim().is_empty())
}

/// 单章正文及其来源标识。
struct ChapterContent {
    content: String,
    content_source: Option<String>,
    ocr_source_path: Option<String>,
}

/// 单章读取的两种结局：拿到正文，或因 VIP 不可读而跳过。
enum ChapterRead {
    Ready(ChapterContent),
    Skipped { reason: String, notice: String },
}

/// 一次文字下载任务的运行时状态。
struct TextDownload<'a> {
    context: JobContext<'a>,
    source: SfSource,
    storage: NativeStorage,
    directory: PathBuf,
    novel_id: i64,
    settings: DownloadSettings,
    manifest: BookManifest,
    store: ChapterIndex,
    /// 被跳过的章节及原因，任务结束时汇总成提示。
    skipped: Vec<(String, String)>,
}

impl<'a> TextDownload<'a> {
    /// 阶段一：连接内容源、准备书籍目录并写入作品基本信息。
    ///
    /// # 错误
    /// 客户端创建、目录创建或清单写入失败时返回错误；风控同样直接中止。
    async fn prepare(context: JobContext<'a>, novel_id: i64, title: &str) -> AppResult<Self> {
        let BookSetup {
            source,
            storage,
            settings,
            directory,
            manifest,
        } = prepare_book(context, title).await?;

        let mut manifest = manifest;
        initialize_metadata(&mut manifest, novel_id, title);
        write_book_manifest(&directory, &manifest)?;

        let mut store = read_chapter_store(&directory)?;
        store.novel_id = novel_id;

        let mut task = Self {
            context,
            source,
            storage,
            directory,
            novel_id,
            settings,
            manifest,
            store,
            skipped: Vec::new(),
        };
        task.enrich_metadata().await?;
        announce_started(&context);
        Ok(task)
    }

    /// 补全作者与简介；除风控外的失败只降级为默认文案。
    async fn enrich_metadata(&mut self) -> AppResult<()> {
        {
            let Self {
                source,
                directory,
                manifest,
                novel_id,
                ..
            } = self;
            let metadata = manifest.novel.get_or_insert_with(Default::default);
            if let Err(error) = enrich_novel_metadata(source, directory, metadata, *novel_id).await
            {
                if error.is_risk_control() {
                    return Err(error);
                }
            }
        }
        write_book_manifest(&self.directory, &self.manifest)
    }

    /// 阶段二：拉取目录，按请求的章节编号生成下载计划。
    ///
    /// # 错误
    /// 目录请求失败、目录里没有任何匹配章节或索引写入失败时返回错误。
    async fn plan(&mut self, chapter_ids: &[i64]) -> AppResult<Vec<ChapterPlan>> {
        let catalog = self
            .source
            .text_volumes(NovelId::new(self.novel_id)?)
            .await?;
        let requested = chapter_ids.iter().copied().collect::<HashSet<_>>();
        let plans = select_chapters(&catalog, &requested);
        if plans.is_empty() {
            return Err(AppError::invalid_input("没有可下载的章节"));
        }
        write_chapter_store(&self.directory, &self.store)?;
        self.persist_downloaded_ids()?;
        Ok(plans)
    }

    /// 阶段三：逐章读取并落盘，VIP 章节失败时记录并继续。
    ///
    /// # 错误
    /// 任务被暂停、触发风控或存储失败时中止；单章可跳过的情况不报错。
    async fn download(&mut self, plans: Vec<ChapterPlan>) -> AppResult<()> {
        let total = plans.len();
        for (index, plan) in plans.into_iter().enumerate() {
            if self.context.cancelled.load(Ordering::Relaxed) {
                return Err(AppError::cancelled("下载已暂停"));
            }
            self.announce_reading(index, total, &plan.chapter_title);
            if !self.process_chapter(&plan).await? {
                continue;
            }
            self.announce_finished(index, total, &plan.chapter_title);
            self.wait_between_requests().await;
        }
        Ok(())
    }

    /// 处理单章，返回是否已计入进度。
    ///
    /// 返回 `false` 表示该章被跳过：既不推进进度，也不做节流等待，
    /// 与改造前的行为保持一致。
    async fn process_chapter(&mut self, plan: &ChapterPlan) -> AppResult<bool> {
        if !self.needs_fetch(plan) {
            return Ok(true);
        }
        let content = match self.read_chapter(plan).await? {
            ChapterRead::Ready(content) => content,
            ChapterRead::Skipped { reason, notice } => {
                self.skipped.push((plan.chapter_title.clone(), reason));
                self.notify(&notice);
                return Ok(false);
            }
        };
        self.store_chapter(plan, content)?;
        Ok(true)
    }

    /// 判断该章是否必须重新读取。
    ///
    /// 已存在的普通章节直接复用；VIP 章节只有在来源可信且正文非空时才复用，
    /// 因为历史版本可能只写入了授权提示或占位图。
    fn needs_fetch(&self, plan: &ChapterPlan) -> bool {
        needs_fetch(plan, self.store.chapters.get(&plan.chapter_id.to_string()))
    }

    /// 读取单章正文，必要时回退到图片 OCR。
    async fn read_chapter(&mut self, plan: &ChapterPlan) -> AppResult<ChapterRead> {
        if plan.is_vip {
            return self.read_vip_chapter(plan).await;
        }
        let storage = &self.storage;
        let result = self
            .source
            .read_chapter_text(
                self.novel_id,
                plan.volume_id,
                plan.chapter_id,
                |raw, encrypted| storage.decode_api_content(raw, encrypted),
            )
            .await?;
        let content = materialize_chapter_images(
            &self.source,
            &self.directory,
            self.novel_id,
            plan.chapter_id,
            &result.content,
        )
        .await?;
        Ok(ChapterRead::Ready(ChapterContent {
            content,
            content_source: Some(result.origin.as_str().to_string()),
            ocr_source_path: None,
        }))
    }

    /// 读取 VIP 章节：先试 App 直连正文，不行再下图片做 OCR。
    async fn read_vip_chapter(&mut self, plan: &ChapterPlan) -> AppResult<ChapterRead> {
        let storage = &self.storage;
        let direct = self
            .source
            .try_direct_text_chapter(plan.chapter_id, |raw, encrypted| {
                storage.decode_api_content(raw, encrypted)
            })
            .await;
        match direct {
            Some(Ok(content)) => Ok(ChapterRead::Ready(ChapterContent {
                content,
                content_source: Some(TextOrigin::App.as_str().to_string()),
                ocr_source_path: None,
            })),
            Some(Err(error)) if error.is_risk_control() => Err(error),
            Some(Err(error)) => Ok(ChapterRead::Skipped {
                notice: format!("App 正文不可用，已跳过：{}（{error}）", plan.chapter_title),
                reason: error.to_string(),
            }),
            None => {
                self.notify(&format!("正在保存并识别图片正文：{}", plan.chapter_title));
                let recognized = resolve_vip_image_content(
                    self.context.app,
                    &self.source,
                    &self.directory,
                    self.novel_id,
                    plan.chapter_id,
                    self.settings.ocr_process_files_enabled,
                )
                .await;
                match recognized {
                    Ok((content, source_path)) => Ok(ChapterRead::Ready(ChapterContent {
                        content,
                        content_source: Some(TextOrigin::WebVipOcr.as_str().to_string()),
                        ocr_source_path: Some(source_path),
                    })),
                    Err(error) if error.is_risk_control() => Err(error),
                    Err(error) => Ok(ChapterRead::Skipped {
                        notice: format!(
                            "VIP 章节处理失败，已跳过：{}（{error}）",
                            plan.chapter_title
                        ),
                        reason: error.to_string(),
                    }),
                }
            }
        }
    }

    /// 写入章节索引与清单，保证重启后可续传。
    fn store_chapter(&mut self, plan: &ChapterPlan, content: ChapterContent) -> AppResult<()> {
        self.store.chapters.insert(
            plan.chapter_id.to_string(),
            StoredChapter {
                id: plan.chapter_id,
                title: plan.chapter_title.clone(),
                volume: plan.volume_title.clone(),
                content: content.content,
                volume_index: plan.volume_index,
                chapter_index: plan.chapter_index as i64,
                content_source: content.content_source,
                ocr_source_path: content.ocr_source_path,
            },
        );
        write_chapter_store(&self.directory, &self.store)?;
        self.persist_downloaded_ids()
    }

    /// 同步清单里的已下载章节集合。
    fn persist_downloaded_ids(&mut self) -> AppResult<()> {
        self.manifest.downloaded_text_chapter_ids = Some(downloaded_text_chapter_ids(&self.store));
        write_book_manifest(&self.directory, &self.manifest)
    }

    /// 阶段四：收尾，把跳过的 VIP 章节汇总成一条可读的错误。
    fn into_result(self) -> AppResult<String> {
        if self.skipped.is_empty() {
            return Ok(self.directory.to_string_lossy().into_owned());
        }
        Err(AppError::upstream(format_skipped(&self.skipped, "VIP 章节")))
    }

    /// 推送"正在读取"进度。
    fn announce_reading(&self, index: usize, total: usize, chapter_title: &str) {
        let JobContext {
            app, state, job_id, ..
        } = self.context;
        registry::update(state, job_id, |job| {
            job.status = JobStatus::Downloading;
            job.progress = ((index * 96) / total) as u8;
            job.message = format!("正在读取：{chapter_title}");
        });
        registry::emit_job(app, state, job_id);
    }

    /// 推送单章完成进度。
    fn announce_finished(&self, index: usize, total: usize, chapter_title: &str) {
        let JobContext {
            app, state, job_id, ..
        } = self.context;
        registry::update(state, job_id, |job| {
            job.progress = 2 + (((index + 1) * 94) / total) as u8;
            job.message = format!("已完成：{chapter_title}");
        });
        registry::emit_job(app, state, job_id);
        registry::emit_library(app, state, job_id);
    }

    /// 只更新提示文案，不改状态与进度。
    fn notify(&self, message: &str) {
        let JobContext {
            app, state, job_id, ..
        } = self.context;
        registry::update(state, job_id, |job| job.message = message.to_string());
        registry::emit_job(app, state, job_id);
    }

    /// 按设置的最小间隔等待，避免触发上游限流。
    async fn wait_between_requests(&self) {
        tokio::time::sleep(std::time::Duration::from_millis(
            self.settings.request_interval_ms,
        ))
        .await;
    }
}

/// 写入作品基本信息，避免目录页缺失时书库条目没有标题。
fn initialize_metadata(manifest: &mut BookManifest, novel_id: i64, title: &str) {
    let metadata = manifest.novel.get_or_insert_with(Default::default);
    metadata.id = Some(novel_id);
    metadata.title = Some(title.to_string());
    metadata
        .author
        .get_or_insert_with(|| "未知作者".to_string());
    metadata
        .description
        .get_or_insert_with(|| "暂无简介".to_string());
}

/// 执行文字下载任务并发布最终状态。
pub(super) async fn run(
    app: tauri::AppHandle,
    job_id: String,
    novel_id: i64,
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
    let result = run_inner(&context, novel_id, &title, &chapter_ids).await;
    finish(&context, result, "文本章节下载完成", false);
}

/// 同一本小说的下载串行执行，避免两个任务同时写同一份章节索引。
async fn run_inner(
    context: &JobContext<'_>,
    novel_id: i64,
    title: &str,
    chapter_ids: &[i64],
) -> AppResult<String> {
    let context = *context;
    let lock = registry::text_lock(context.state, novel_id)?;
    let _guard = lock.lock().await;

    let mut task = TextDownload::prepare(context, novel_id, title).await?;
    let plans = task.plan(chapter_ids).await?;
    task.download(plans).await?;
    task.into_result()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::catalog::{ChapterSummary, TextAccessState};

    /// 构造一条目录章节。
    fn summary(id: i64, title: &str, is_vip: bool) -> ChapterSummary {
        ChapterSummary {
            chap_id: id,
            title: title.to_string(),
            is_vip,
            need_fire_money: 0,
            access_state: TextAccessState::Available,
            downloaded: false,
        }
    }

    /// 构造一个分卷。
    fn volume(id: i64, title: &str, chapters: Vec<ChapterSummary>) -> ChapterVolume {
        ChapterVolume {
            volume_id: id,
            title: title.to_string(),
            chapters,
        }
    }

    /// 构造一份下载计划。
    fn plan(id: i64, is_vip: bool) -> ChapterPlan {
        ChapterPlan {
            chapter_id: id,
            volume_id: 1,
            volume_title: "第一卷".to_string(),
            volume_index: 0,
            chapter_index: 0,
            chapter_title: "章节".to_string(),
            is_vip,
        }
    }

    /// 构造一条本地已存章节。
    fn stored(source: Option<&str>, content: &str) -> StoredChapter {
        StoredChapter {
            id: 1,
            title: "章节".to_string(),
            volume: "第一卷".to_string(),
            content: content.to_string(),
            volume_index: 0,
            chapter_index: 0,
            content_source: source.map(str::to_string),
            ocr_source_path: None,
        }
    }

    #[test]
    fn select_chapters_keeps_catalog_order_and_skips_unselected() {
        let catalog = vec![
            volume(
                10,
                "第一卷",
                vec![
                    summary(1, "第一章", false),
                    summary(2, "第二章", false),
                ],
            ),
            volume(
                20,
                "第二卷",
                vec![summary(3, "第三章", true)],
            ),
        ];
        let requested = [3, 1].into_iter().collect::<HashSet<_>>();

        let plans = select_chapters(&catalog, &requested);

        assert_eq!(
            plans.iter().map(|plan| plan.chapter_id).collect::<Vec<_>>(),
            vec![1, 3],
            "应按目录顺序而非勾选顺序排列，并跳过未勾选章节"
        );
        assert_eq!(plans[0].volume_title, "第一卷");
        assert_eq!(plans[1].volume_index, 1, "第二卷的序号应记录为 1");
        assert_eq!(plans[1].volume_id, 20);
    }

    #[test]
    fn select_chapters_records_position_within_volume() {
        let catalog = vec![volume(
            10,
            "第一卷",
            vec![
                summary(1, "第一章", false),
                summary(2, "第二章", false),
            ],
        )];
        let requested = [2].into_iter().collect::<HashSet<_>>();

        let plans = select_chapters(&catalog, &requested);

        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].chapter_index, 1, "章节序号应为卷内下标");
        assert_eq!(plans[0].chapter_title, "第二章");
    }

    #[test]
    fn select_chapters_returns_empty_when_nothing_matches() {
        let catalog = vec![volume(
            10,
            "第一卷",
            vec![summary(1, "第一章", false)],
        )];

        let plans = select_chapters(&catalog, &[99].into_iter().collect::<HashSet<_>>());

        assert!(plans.is_empty());
    }

    #[test]
    fn needs_fetch_always_true_for_new_chapter() {
        let plan = plan(1, false);

        assert!(needs_fetch(&plan, None), "从未下载过的章节必须拉取");
    }

    #[test]
    fn needs_fetch_skips_vip_chapter_with_app_content() {
        let plan = plan(1, true);
        let existing = stored(Some(TextOrigin::App.as_str()), "已下载正文");

        assert!(!needs_fetch(&plan, Some(&existing)), "已有 App 正文的 VIP 章节应跳过");
    }

    #[test]
    fn needs_fetch_skips_vip_chapter_with_ocr_content() {
        let plan = plan(1, true);
        let existing = stored(Some(TextOrigin::WebVipOcr.as_str()), "OCR 正文");

        assert!(!needs_fetch(&plan, Some(&existing)), "已有 OCR 正文的 VIP 章节应跳过");
    }

    #[test]
    fn needs_fetch_refetches_vip_chapter_with_empty_content() {
        let plan = plan(1, true);

        assert!(
            needs_fetch(&plan, Some(&stored(Some(TextOrigin::App.as_str()), "   "))),
            "正文为空时应重新拉取"
        );
        assert!(
            needs_fetch(&plan, Some(&stored(Some(TextOrigin::Web.as_str()), "网页正文"))),
            "非 App 来源的正文不足以跳过 VIP 章节"
        );
    }

    #[test]
    fn needs_fetch_refetches_vip_chapter_with_unknown_origin() {
        let plan = plan(1, true);

        assert!(
            needs_fetch(&plan, Some(&stored(Some("legacy"), "正文"))),
            "来源标识无法识别时必须重取，不能当成已完成章节"
        );
        assert!(
            needs_fetch(&plan, Some(&stored(None, "正文"))),
            "缺少来源标识时同样不能跳过"
        );
    }

    #[test]
    fn needs_fetch_skips_non_vip_chapter_already_stored() {
        let plan = plan(1, false);

        assert!(
            !needs_fetch(&plan, Some(&stored(Some(TextOrigin::App.as_str()), "正文"))),
            "非 VIP 章节已存在时不必重复拉取"
        );
    }
}
