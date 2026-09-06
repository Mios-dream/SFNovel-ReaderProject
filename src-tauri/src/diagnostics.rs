//! 面向渲染进程的非敏感运行时诊断命令。

/// 返回原生运行时的非敏感诊断描述。
///
/// 此命令仅用于验证 Vue 到 Rust 的 IPC 链路，不暴露文件路径、凭据或网络状态。
///
/// # 参数
/// * `name` - 渲染进程提供的可选显示名称。
///
/// # 返回值
/// 适合在开发诊断视图展示的问候文本。
#[tauri::command]
pub(crate) fn greet(name: &str) -> String {
    let subject = if name.trim().is_empty() {
        "Novel Flow"
    } else {
        name.trim()
    };
    format!("Novel Flow native runtime ready for {subject}.")
}

/// 返回供渲染进程诊断界面使用的应用运行时版本。
///
/// 版本来自 Tauri 应用包元数据，调用不产生副作用。
///
/// # 返回值
/// 应用包声明的语义化版本号。
#[tauri::command]
pub(crate) fn app_runtime_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}
