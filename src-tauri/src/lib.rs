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
use adapters::sfacg::source::SfSource;
#[cfg(not(target_os = "android"))]
use application::signin::SigninService;
#[cfg(not(target_os = "android"))]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(not(target_os = "android"))]
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

#[cfg(target_os = "android")]
use adapters::platform::{android_auth_plugin, android_ocr_plugin};

#[cfg(target_os = "windows")]
use adapters::platform::{restore_app_session, restore_web_session};

#[cfg(not(target_os = "android"))]
/// 显示并聚焦主窗口。
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(not(target_os = "android"))]
/// 创建桌面系统托盘及菜单。
fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    let mut tray = TrayIconBuilder::new()
        .tooltip("SF Novel Flow")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

#[cfg(not(target_os = "android"))]
fn civil_date_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 { shifted } else { shifted - 146_096 } / 146_097;
    let day_of_era = (shifted - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = if month_part < 10 {
        month_part + 3
    } else {
        month_part - 9
    };
    (
        if month <= 2 { year + 1 } else { year },
        month as u32,
        day as u32,
    )
}

#[cfg(not(target_os = "android"))]
fn beijing_date_and_hour() -> (String, u32) {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        + 8 * 60 * 60;
    let days = (seconds / 86_400) as i64;
    let hour = ((seconds % 86_400) / 3_600) as u32;
    let (year, month, day) = civil_date_from_days(days);
    (format!("{year:04}-{month:02}-{day:02}"), hour)
}

#[cfg(not(target_os = "android"))]
fn spawn_auto_signin(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_attempt = String::new();
        loop {
            let (date, hour) = beijing_date_and_hour();
            if hour >= 12 && last_attempt != date {
                let enabled = adapters::storage::NativeStorage::new(app.clone())
                    .settings()
                    .map(|settings| settings.signin.auto_signin_enabled)
                    .unwrap_or(false);
                if enabled {
                    if let Ok(remote) = SfSource::connect(&app).await {
                        if remote.has_app_session() {
                            last_attempt = date.clone();
                            match SigninService::new(&remote).auto_sign_in(&date).await {
                                Ok(true) => eprintln!("[signin] {date} 自动签到成功"),
                                Ok(false) => eprintln!("[signin] {date} 已签到"),
                                Err(error) => eprintln!("[signin] {date} 自动签到失败：{error}"),
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(10 * 60)).await;
        }
    });
}

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
            #[cfg(not(target_os = "android"))]
            setup_tray(&handle)?;

            if let Ok(directory) = adapters::storage::paths::app_data_directory(&handle) {
                application::state::install_profile_cache_path(
                    directory.join("profile-cache.json"),
                );
            }

            let settings = adapters::storage::NativeStorage::new(handle.clone())
                .settings()
                .unwrap_or_default();
            adapters::downloads::configure_concurrency(settings.download.max_concurrent_downloads);
            #[cfg(target_os = "windows")]
            let _ = adapters::platform::set_autostart(settings.desktop.autostart);

            let _ = NativeDownloadGateway::new(handle.clone()).restore();

            #[cfg(target_os = "windows")]
            if let Ok(Some(session)) = restore_app_session(&handle) {
                let _ = app.state::<AuthSessionState>().replace(Some(session));
            }
            #[cfg(target_os = "windows")]
            if let Ok(Some(web_session)) = restore_web_session(&handle) {
                let _ = app.state::<AuthSessionState>().merge_web(web_session);
            }
            #[cfg(not(target_os = "android"))]
            spawn_auto_signin(handle.clone());
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
        builder = builder.on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let close_to_tray = adapters::storage::NativeStorage::new(app.clone())
                    .settings()
                    .map(|settings| settings.desktop.close_to_tray)
                    .unwrap_or(true);
                if close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        });
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
            commands::settings::get_settings,
            commands::settings::save_settings,
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
            commands::auth::get_account_profile,
            commands::signin::get_signin_month,
            commands::signin::get_signin_overview,
            commands::signin::submit_signin,
            commands::signin::claim_signin_reward
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(all(test, not(target_os = "android")))]
mod tests {
    use super::civil_date_from_days;

    #[test]
    fn converts_days_to_civil_dates() {
        assert_eq!(civil_date_from_days(0), (1970, 1, 1));
        assert_eq!(civil_date_from_days(20_759), (2026, 11, 2));
    }
}
