// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// 启动 Tauri 应用，并将控制权交给桌面端和移动端共用的库入口。
///
/// # 副作用
/// 初始化原生运行时、注册命令，并持续阻塞至应用退出。
fn main() {
    sf_novel_flow_lib::run()
}
