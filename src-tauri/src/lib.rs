//! 原生应用装配入口。
//!
//! 本模块负责组装 Tauri 运行时、共享状态、平台插件与前端可调用命令；具体
//! 业务实现按 SFACG 通信、本地书库、下载任务和诊断功能拆分在独立源文件中。

mod app_client;
mod web_client;

use app_client::AppClient;
use web_client::WebClient;

include!("sfacg.rs");
include!("library.rs");
include!("downloads.rs");
include!("diagnostics.rs");

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// 配置并在当前平台运行 Tauri 应用。
///
/// # 副作用
/// 注册原生状态、IPC 命令和打开器插件；在 Android 注册认证插件；并将已持久化
/// 的未完成下载恢复为暂停状态。函数会阻塞至应用退出。
pub fn run() {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .manage(AuthSessionState::default())
        .manage(NativeJobState::default())
        .setup(|app| {
            #[cfg(target_os = "android")]
            tauri::async_runtime::block_on(initialize_device_identity(&app.handle()))?;
            #[cfg(not(target_os = "android"))]
            initialize_device_identity(&app.handle())?;
            // A malformed recovery snapshot must not block application startup.
            let _ = restore_native_jobs(&app.handle());
            #[cfg(target_os = "windows")]
            if let Ok(Some(session)) = restore_desktop_auth_session(&app.handle()) {
                if let Ok(mut current) = app.state::<AuthSessionState>().session.lock() {
                    *current = Some(session);
                }
            }
            #[cfg(target_os = "windows")]
            if let Ok(Some(web_session)) = restore_desktop_web_session(&app.handle()) {
                if let Ok(mut current) = app.state::<AuthSessionState>().session.lock() {
                    if let Some(existing) = current.as_mut() {
                        existing.web_cookie = web_session.web_cookie;
                    } else {
                        *current = Some(web_session);
                    }
                }
            }
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init());
    #[cfg(target_os = "android")]
    {
        builder = builder.plugin(android_sfacg_auth_plugin());
    }
    builder
        .invoke_handler(tauri::generate_handler![
            greet,
            app_runtime_version,
            list_local_library,
            get_request_policy,
            save_request_policy,
            get_content_dictionary,
            update_content_dictionary,
            search_novels,
            get_novel_details,
            get_chapter_volumes,
            get_audio_chapters,
            get_comic_chapters,
            get_bookshelf,
            get_local_book,
            get_local_chapter,
            get_local_comic_chapter,
            delete_local_book,
            ensure_external_storage_access,
            export_local_book,
            create_text_download,
            create_audio_download,
            create_comic_download,
            list_download_jobs,
            pause_download_job,
            resume_download_job,
            delete_download_job,
            auth_status,
            login_with_password,
            start_official_login,
            logout,
            verify_authenticated_request,
            get_user_profile
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
