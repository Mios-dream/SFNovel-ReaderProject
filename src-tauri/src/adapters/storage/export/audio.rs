//! 有声导出（音频 ZIP）。

use super::{create_archive, deflated, finish_archive, storage_error, write_entry};
use crate::domain::error::{AppError, AppResult};
use std::fs;
use std::path::Path;

/// 有声目录内的播放清单文件名。
pub(super) const PLAYLIST: &str = "有声目录.m3u8";

/// 将本地有声目录打包为 ZIP。
///
/// # 参数
/// - `target`：ZIP 目标路径。
/// - `audio_directory`：本地音频目录。
///
/// # 错误
/// 目录读取、文件写入或 ZIP 完成失败时返回存储类错误。
pub(super) fn write(target: &Path, audio_directory: &Path) -> AppResult<()> {
    let mut archive = create_archive(target, "有声导出")?;
    let options = deflated();
    let entries =
        fs::read_dir(audio_directory).map_err(|error| storage_error("无法读取有声目录", error))?;
    for entry in entries {
        let entry = entry.map_err(|error| storage_error("读取有声文件失败", error))?;
        let file_type = entry
            .file_type()
            .map_err(|error| storage_error("读取有声文件类型失败", error))?;
        if !file_type.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if !is_packable(&file_name) {
            continue;
        }
        let bytes =
            fs::read(entry.path()).map_err(|error| storage_error("无法读取有声文件", error))?;
        write_entry(
            &mut archive,
            &format!("audio/{file_name}"),
            options,
            &bytes,
            "有声归档",
        )?;
    }
    finish_archive(archive, "有声导出")
}

/// 播放清单与 mp3 才是需要打包的内容。
///
/// # 参数
/// - `file_name`：有声目录内的文件名。
fn is_packable(file_name: &str) -> bool {
    file_name == PLAYLIST || file_name.to_ascii_lowercase().ends_with(".mp3")
}

/// 确认有声目录内存在播放清单。
///
/// # 参数
/// - `audio_directory`：本地音频目录。
///
/// # 错误
/// 缺少播放清单时返回存储类错误。
pub(super) fn ensure_available(audio_directory: &Path) -> AppResult<()> {
    if audio_directory.join(PLAYLIST).is_file() {
        return Ok(());
    }
    Err(AppError::storage("这本书没有可打包的有声章节"))
}

#[cfg(test)]
mod tests {
    use super::super::testing::{archive_names, TempDir};
    use super::*;

    #[test]
    fn audio_export_keeps_playlist_and_mp3_only() {
        let dir = TempDir::new("audio");
        let audio = dir.join("audio");
        fs::create_dir_all(audio.join("子目录")).expect("创建嵌套目录");
        for name in [PLAYLIST, "001.mp3", "002.MP3", "cover.jpg", "notes.txt"] {
            fs::write(audio.join(name), name.as_bytes()).expect("写入示例文件");
        }
        let target = dir.join("audio.zip");

        write(&target, &audio).expect("导出有声");

        let mut names = archive_names(&target);
        names.sort();
        assert_eq!(
            names,
            vec![
                "audio/001.mp3".to_string(),
                "audio/002.MP3".to_string(),
                "audio/有声目录.m3u8".to_string(),
            ],
            "只应打包播放清单与 mp3"
        );
    }

    #[test]
    fn audio_export_fails_when_directory_is_missing() {
        let dir = TempDir::new("audio-missing");

        let error =
            write(&dir.join("audio.zip"), &dir.join("absent")).expect_err("目录不存在应失败");

        assert_eq!(error.kind(), crate::domain::error::ErrorKind::Storage);
        assert!(
            error.to_string().contains("有声"),
            "错误消息应说明有声目录问题"
        );
    }

    #[test]
    fn ensure_available_detects_absent_manifest() {
        let dir = TempDir::new("playlist");
        let audio = dir.join("audio");
        fs::create_dir_all(&audio).expect("创建音频目录");

        let error = ensure_available(&audio).expect_err("空目录应被判定为没有有声章节");
        assert_eq!(error.kind(), crate::domain::error::ErrorKind::Storage);

        fs::write(audio.join(PLAYLIST), b"#EXTM3U\n").expect("写入播放清单");
        assert!(ensure_available(&audio).is_ok(), "存在清单时不应报错");
    }

    #[test]
    fn is_packable_accepts_playlist_and_mp3_case_insensitively() {
        assert!(is_packable(PLAYLIST));
        assert!(is_packable("001.mp3"));
        assert!(is_packable("002.MP3"));
        assert!(!is_packable("cover.jpg"));
        assert!(!is_packable("001.mp3.tmp"));
    }
}
