//! 原生应用装配入口。
//!
//! 本模块只负责运行时、共享状态、平台插件、媒体协议和 IPC 命令的注册；
//! 业务规则位于 `application`，外部系统访问位于 `adapters`。

mod adapters;
mod application;
mod commands;
mod domain;

use adapters::downloads::{DownloadState, NativeDownloadGateway};
use adapters::media::OnlineMediaState;
use adapters::platform::initialize_device_identity;
use application::ports::DownloadGateway;
use application::state::AuthSessionState;
#[cfg(not(target_os = "android"))]
use tauri::Manager;

#[cfg(target_os = "android")]
use adapters::platform::{android_auth_plugin, android_ocr_plugin};

#[cfg(target_os = "windows")]
use adapters::platform::{restore_app_session, restore_web_session};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// 构建并运行 Tauri 应用。
///
/// 该函数注册共享状态、平台插件、媒体协议和 IPC 命令，并在启动时恢复
/// 设备身份、下载任务及桌面端会话。函数返回时应用已经退出或启动失败。
///
/// # Panics
/// Tauri 上下文创建失败或事件循环无法启动时会 panic。
pub fn run() {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .manage(AuthSessionState::default())
        .manage(DownloadState::default())
        .manage(OnlineMediaState::default())
        .setup(|app| {
            let handle = app.handle().clone();
            #[cfg(target_os = "android")]
            tauri::async_runtime::block_on(initialize_device_identity(&handle))?;
            #[cfg(not(target_os = "android"))]
            initialize_device_identity(&handle)?;

            if let Ok(directory) = adapters::storage::paths::app_data_directory(&handle) {
                application::state::install_profile_cache_path(
                    directory.join("profile-cache.json"),
                );
            }

            let policy = adapters::storage::NativeStorage::new(handle.clone())
                .policy()
                .unwrap_or_default();
            adapters::downloads::configure_concurrency(policy.max_concurrent_downloads);

            let _ = NativeDownloadGateway::new(handle.clone()).restore();

            #[cfg(target_os = "windows")]
            if let Ok(Some(session)) = restore_app_session(&handle) {
                let _ = app.state::<AuthSessionState>().replace(Some(session));
            }
            #[cfg(target_os = "windows")]
            if let Ok(Some(web_session)) = restore_web_session(&handle) {
                let _ = app.state::<AuthSessionState>().merge_web(web_session);
            }
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .register_asynchronous_uri_scheme_protocol(
            adapters::media::ONLINE_MEDIA_SCHEME,
            adapters::media::handle_online_media_request,
        );

    #[cfg(not(target_os = "android"))]
    {
        builder = builder.manage(adapters::ocr::DesktopOcrState::default());
    }
    #[cfg(target_os = "android")]
    {
        builder = builder.plugin(android_auth_plugin());
        builder = builder.plugin(android_ocr_plugin());
    }

    builder
        .invoke_handler(tauri::generate_handler![
            commands::diagnostics::greet,
            commands::diagnostics::app_runtime_version,
            commands::library::list_local_library,
            commands::library::open_download_directory,
            commands::settings::get_request_policy,
            commands::settings::save_request_policy,
            commands::library::get_content_dictionary,
            commands::library::update_content_dictionary,
            commands::catalog::search_novels,
            commands::catalog::get_novel_details,
            commands::catalog::get_audio_details,
            commands::catalog::get_chapter_volumes,
            commands::catalog::get_audio_chapters,
            commands::catalog::get_comic_chapters,
            commands::catalog::get_comic_details,
            commands::catalog::get_bookshelf,
            commands::reading::load_online_text_chapter,
            commands::reading::load_online_comic_chapter,
            commands::reading::load_online_audio_track,
            commands::library::get_local_book,
            commands::library::get_local_chapter,
            commands::library::get_local_comic_chapter,
            commands::library::delete_local_book,
            commands::library::ensure_external_storage_access,
            commands::library::export_local_book,
            commands::downloads::create_text_download,
            commands::downloads::create_audio_download,
            commands::downloads::create_comic_download,
            commands::downloads::list_download_jobs,
            commands::downloads::pause_download_job,
            commands::downloads::resume_download_job,
            commands::downloads::delete_download_job,
            commands::auth::auth_status,
            commands::auth::login_with_password,
            commands::auth::start_official_login,
            commands::auth::logout,
            commands::auth::logout_app_session,
            commands::auth::logout_web_session,
            commands::auth::verify_authenticated_request,
            commands::auth::get_account_profile
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
