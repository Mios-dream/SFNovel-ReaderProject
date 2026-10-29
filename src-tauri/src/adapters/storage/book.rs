//! 书库目录、书籍清单与本地媒体读取。

use super::atomic::{read_or_default, write_atomically};
use super::paths::{library_directory, local_book_directory, safe_library_name};
use crate::domain::library::{
    AudioTrackView, AvailableFormats, BookDetail, BookManifest, BookWorkDetail, ChapterIndex,
    LibraryChapterSummary, LibraryChapterVolume, LibraryComicChapter, LibraryEntry, StoredChapter,
    WorkMetadata,
};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// 书籍元数据清单文件名。
pub(crate) const MANIFEST_FILE: &str = ".novel-flow.json";
/// 文字章节索引文件名。
pub(crate) const CHAPTERS_FILE: &str = ".novel-flow-chapters.json";

/// 读取书籍元数据清单。
///
/// # 错误
/// 清单无法读取或格式无效时返回错误消息。
pub(crate) fn read_book_manifest(directory: &Path) -> Result<BookManifest, String> {
    read_or_default(&directory.join(MANIFEST_FILE), "本地数据")
}

/// 原子写入书籍元数据清单。
///
/// # 错误
/// 序列化或文件写入失败时返回错误消息。
pub(crate) fn write_book_manifest(directory: &Path, manifest: &BookManifest) -> Result<(), String> {
    write_atomically(&directory.join(MANIFEST_FILE), manifest, "本地数据")
}

/// 读取文字章节索引。
///
/// # 错误
/// 索引无法读取或格式无效时返回错误消息。
pub(crate) fn read_chapter_store(directory: &Path) -> Result<ChapterIndex, String> {
    read_or_default(&directory.join(CHAPTERS_FILE), "本地数据")
}

/// 原子写入文字章节索引。
///
/// # 错误
/// 序列化或文件写入失败时返回错误消息。
pub(crate) fn write_chapter_store(directory: &Path, store: &ChapterIndex) -> Result<(), String> {
    write_atomically(&directory.join(CHAPTERS_FILE), store, "章节索引")
}

/// 返回章节索引中的数字章节编号，并按升序排列。
///
/// # 参数
/// - `store`：文字章节索引。
pub(crate) fn downloaded_text_chapter_ids(store: &ChapterIndex) -> Vec<i64> {
    let mut ids: Vec<_> = store
        .chapters
        .keys()
        .filter_map(|id| id.parse().ok())
        .collect();
    ids.sort_unstable();
    ids
}

/// 扫描书库并返回书籍摘要。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 书库目录或条目元数据无法读取时返回错误消息。
pub(crate) fn list(app: &tauri::AppHandle) -> Result<Vec<LibraryEntry>, String> {
    let directory = library_directory(app)?;
    let entries = fs::read_dir(&directory).map_err(|error| format!("无法读取本地书库：{error}"))?;
    let mut books = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("读取书库条目失败：{error}"))?;
        let metadata = entry
            .metadata()
            .map_err(|error| format!("读取书库条目元数据失败：{error}"))?;
        if !metadata.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let text =
            entry.path().join(CHAPTERS_FILE).exists() || entry.path().join("book.md").exists();
        let audio = entry.path().join("audio").is_dir();
        let comic = entry.path().join("comic").is_dir();
        books.push(LibraryEntry {
            name,
            updated_at: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs().to_string())
                .unwrap_or_else(|| "0".to_string()),
            cover: ["novel-cover.jpeg", "audio-cover.jpeg", "comic-cover.jpeg"]
                .into_iter()
                .map(|file_name| entry.path().join("imgs").join(file_name))
                .find(|cover| cover.is_file())
                .map(|cover| cover.to_string_lossy().into_owned()),
            formats: AvailableFormats { text, audio, comic },
        });
    }
    books.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(books)
}

/// 读取一本书的文字、音频、漫画和封面信息。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `name`：本地书籍目录名。
///
/// # 错误
/// 书籍目录、清单、章节索引或媒体目录读取失败时返回错误消息。
pub(crate) fn detail(app: &tauri::AppHandle, name: &str) -> Result<BookDetail, String> {
    let directory = local_book_directory(app, name)?;
    let metadata = read_book_manifest(&directory)?;
    let store = read_chapter_store(&directory)?;
    let mut chapters: Vec<_> = store.chapters.into_values().collect();
    chapters.sort_by_key(|chapter| (chapter.volume_index, chapter.chapter_index, chapter.id));
    let mut chapter_volumes: Vec<LibraryChapterVolume> = Vec::new();
    for chapter in chapters {
        if chapter_volumes.last().map(|volume| volume.volume.as_str())
            != Some(chapter.volume.as_str())
        {
            chapter_volumes.push(LibraryChapterVolume {
                volume: chapter.volume.clone(),
                chapters: Vec::new(),
            });
        }
        chapter_volumes
            .last_mut()
            .expect("chapter volume was just inserted")
            .chapters
            .push(LibraryChapterSummary {
                id: chapter.id,
                title: chapter.title,
            });
    }
    let epub_name = format!("{}.epub", name);
    let epub_path = directory.join(&epub_name);
    let epub_href = epub_path
        .is_file()
        .then(|| epub_path.to_string_lossy().into_owned());
    Ok(BookDetail {
        name: name.to_string(),
        novel: work_detail(metadata.novel, &directory, "novel-cover.jpeg"),
        audio: work_detail(metadata.audio, &directory, "audio-cover.jpeg"),
        comic: work_detail(metadata.comic, &directory, "comic-cover.jpeg"),
        image_directory: directory.join("imgs").to_string_lossy().into_owned(),
        audio_tracks: read_audio_tracks(&directory)?,
        epub_href,
        chapter_volumes,
        comic_chapters: read_comic_chapters(&directory)?,
    })
}

/// 将持久化元数据转换为渲染进程使用的媒体详情。
///
/// # 参数
/// - `metadata`：可选持久化元数据。
/// - `directory`：书籍目录。
/// - `cover_file`：封面文件名。
fn work_detail(
    metadata: Option<WorkMetadata>,
    directory: &Path,
    cover_file: &str,
) -> Option<BookWorkDetail> {
    let metadata = metadata?;
    let id = metadata.id?;
    let cover_path = directory.join("imgs").join(cover_file);
    let cover = cover_path
        .is_file()
        .then(|| cover_path.to_string_lossy().into_owned());
    Some(BookWorkDetail {
        id,
        catalog_id: metadata.catalog_id,
        online_path: metadata.online_path,
        title: metadata.title.unwrap_or_else(|| "未命名作品".to_string()),
        author: metadata.author.unwrap_or_else(|| "未知作者".to_string()),
        description: metadata.description.unwrap_or_default(),
        type_name: metadata.type_name,
        tags: metadata.tags,
        is_finished: metadata.is_finished,
        score: metadata.score,
        chapter_count: metadata.chapter_count,
        character_count: metadata.character_count,
        view_count: metadata.view_count,
        mark_count: metadata.mark_count,
        point_count: metadata.point_count,
        favorite_count: metadata.favorite_count,
        ticket_count: metadata.ticket_count,
        allow_download: metadata.allow_download,
        latest_chapter_title: metadata.latest_chapter_title,
        latest_chapter_time: metadata.latest_chapter_time,
        last_update_time: metadata.last_update_time,
        cover,
    })
}

/// 解析本地 M3U 播放列表并返回可播放轨道。
///
/// # 错误
/// 播放列表无法读取时返回错误消息；无效条目会被跳过。
fn read_audio_tracks(directory: &Path) -> Result<Vec<AudioTrackView>, String> {
    let audio_directory = directory.join("audio");
    let playlist_path = audio_directory.join("有声目录.m3u8");
    if !playlist_path.is_file() {
        return Ok(Vec::new());
    }
    let playlist = fs::read_to_string(&playlist_path)
        .map_err(|error| format!("无法读取有声播放列表：{error}"))?;
    let mut tracks = Vec::new();
    let mut title = None;
    for line in playlist.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("#EXTINF:") {
            title = value
                .split_once(',')
                .map(|(_, track_title)| track_title.trim().to_string())
                .filter(|value| !value.is_empty());
            continue;
        }
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let file_name = PathBuf::from(line);
        if file_name.components().count() != 1
            || file_name
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.eq_ignore_ascii_case("mp3"))
                != Some(true)
        {
            title = None;
            continue;
        }
        let Some(file_name) = file_name.file_name().and_then(|value| value.to_str()) else {
            title = None;
            continue;
        };
        let file = audio_directory.join(file_name);
        if file.is_file() {
            let fallback = file
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("未命名音频");
            tracks.push(AudioTrackView {
                title: title.take().unwrap_or_else(|| fallback.to_string()),
                href: file.to_string_lossy().into_owned(),
            });
        }
        title = None;
    }
    Ok(tracks)
}

/// 扫描具有完成标记的本地漫画章节及页面。
///
/// # 错误
/// 漫画目录或页面列表无法读取时返回错误消息。
pub(crate) fn read_comic_chapters(directory: &Path) -> Result<Vec<LibraryComicChapter>, String> {
    let root = directory.join("comic");
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut chapters = Vec::new();
    for entry in fs::read_dir(root).map_err(|error| format!("无法读取漫画目录：{error}"))?
    {
        let entry = entry.map_err(|error| format!("读取漫画章节失败：{error}"))?;
        let chapter_dir = entry.path();
        if !chapter_dir.is_dir() || !chapter_dir.join(".complete").is_file() {
            continue;
        }
        let Some(id) = entry
            .file_name()
            .to_str()
            .and_then(|value| value.parse::<i64>().ok())
        else {
            continue;
        };
        let title = fs::read_to_string(chapter_dir.join(".complete"))
            .unwrap_or_else(|_| format!("第 {id} 话"))
            .trim()
            .to_string();
        let mut pages = fs::read_dir(&chapter_dir)
            .map_err(|error| format!("无法读取漫画页：{error}"))?
            .filter_map(Result::ok)
            .filter(|item| item.path().is_file() && item.file_name() != ".complete")
            .map(|item| item.path())
            .filter(|path| {
                matches!(
                    path.extension()
                        .and_then(|value| value.to_str())
                        .map(str::to_ascii_lowercase)
                        .as_deref(),
                    Some("jpg" | "jpeg" | "png" | "webp" | "avif")
                )
            })
            .collect::<Vec<_>>();
        pages.sort();
        if !pages.is_empty() {
            let pages = pages
                .into_iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            chapters.push(LibraryComicChapter {
                id,
                title,
                cover: pages.first().cloned(),
                pages,
            });
        }
    }
    chapters.sort_by_key(|chapter| chapter.id);
    Ok(chapters)
}

/// 读取指定本地漫画章节。
///
/// # 错误
/// 书籍目录不存在、章节未下载或漫画目录读取失败时返回错误消息。
pub(crate) fn comic_chapter(
    app: &tauri::AppHandle,
    name: &str,
    chapter_id: i64,
) -> Result<LibraryComicChapter, String> {
    let directory = local_book_directory(app, name)?;
    read_comic_chapters(&directory)?
        .into_iter()
        .find(|chapter| chapter.id == chapter_id)
        .ok_or_else(|| "本地漫画章节不存在".to_string())
}

/// 从书籍清单读取文字作品编号。
///
/// # 错误
/// 清单无法读取时返回错误消息。
fn metadata_novel_id(directory: &Path) -> Result<Option<i64>, String> {
    Ok(read_book_manifest(directory)?
        .novel
        .and_then(|work| work.id))
}

/// 读取指定本地文字章节，并校验作品编号一致。
///
/// # 错误
/// 书籍目录、章节索引或元数据读取失败，或章节不存在时返回错误消息。
pub(crate) fn chapter(
    app: &tauri::AppHandle,
    name: &str,
    chapter_id: i64,
) -> Result<StoredChapter, String> {
    let directory = local_book_directory(app, name)?;
    let store = read_chapter_store(&directory)?;
    if store.novel_id != 0 && metadata_novel_id(&directory)? != Some(store.novel_id) {
        return Err("本地章节与书籍元数据不匹配".to_string());
    }
    store
        .chapters
        .into_values()
        .find(|chapter| chapter.id == chapter_id)
        .ok_or_else(|| "本地未找到该章节正文".to_string())
}

/// 汇总某类媒体已经下载的章节编号。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `kind`：`text`、`audio` 或 `comic`。
/// - `title`：可选的书籍标题；提供时只扫描对应目录。
///
/// # 错误
/// 书库或书籍清单无法读取时返回错误消息。
pub(crate) fn downloaded_ids(
    app: &tauri::AppHandle,
    kind: &str,
    title: Option<&str>,
) -> Result<HashSet<i64>, String> {
    let mut result = HashSet::new();
    if kind == "text" || title.is_none() {
        let directory = library_directory(app)?;
        for entry in fs::read_dir(&directory).into_iter().flatten().flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Ok(metadata) = read_book_manifest(&path) {
                let ids = match kind {
                    "audio" => metadata.downloaded_audio_chapter_ids,
                    "comic" => metadata.downloaded_comic_chapter_ids,
                    _ => metadata.downloaded_text_chapter_ids,
                };
                if let Some(ids) = ids {
                    result.extend(ids);
                }
            }
        }
        return Ok(result);
    }
    let directory = library_directory(app)?.join(safe_library_name(title.unwrap_or_default()));
    if directory.is_dir() {
        let metadata = read_book_manifest(&directory)?;
        let ids = match kind {
            "audio" => metadata.downloaded_audio_chapter_ids,
            "comic" => metadata.downloaded_comic_chapter_ids,
            _ => metadata.downloaded_text_chapter_ids,
        };
        if let Some(ids) = ids {
            result.extend(ids);
        }
    }
    Ok(result)
}

/// 删除一本本地书籍的全部文件。
///
/// # 错误
/// 目录非法、书籍不存在或删除失败时返回错误消息。
pub(crate) fn delete(app: &tauri::AppHandle, name: &str) -> Result<(), String> {
    let directory = local_book_directory(app, name)?;
    fs::remove_dir_all(directory).map_err(|error| format!("删除本地书籍失败：{error}"))
}
