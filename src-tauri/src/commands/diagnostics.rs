//! 非敏感运行时诊断命令。

#[tauri::command]
/// 返回运行时连通性检查文本。
///
/// # 参数
/// - `name`：要写入响应主体的名称；空白时使用 `Novel Flow`。
pub(crate) fn greet(name: &str) -> String {
    let subject = if name.trim().is_empty() {
        "Novel Flow"
    } else {
        name.trim()
    };
    format!("Novel Flow native runtime ready for {subject}.")
}

#[tauri::command]
/// 返回应用包版本。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
pub(crate) fn app_runtime_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}
