//! 桌面端本地 OCR worker 桥接。
//!
//! 图片获取、会话和文件落盘始终留在 Rust 原生层。本模块只把已保存的本地图片路径
//! 交给独立 Python 进程，避免向 worker、日志或渲染进程传递 Web Cookie。

use std::path::{Path, PathBuf};
#[cfg(not(target_os = "android"))]
use std::process::Command;
#[cfg(not(target_os = "android"))]
use tauri::{path::BaseDirectory, Manager};

#[cfg(target_os = "android")]
use crate::sfacg::recognize_android_image;

#[cfg(not(target_os = "android"))]
const RECOGNITION_MODEL_RESOURCE: &str = "resources/ocr-models/PP-OCRv6_rec_small.onnx";

#[cfg(not(target_os = "android"))]
struct DesktopOcrResources {
    worker: PathBuf,
    model: PathBuf,
    bundled: bool,
}

/// 定位随应用资源安装的 OCR worker，且不接受渲染进程传入的路径。
///
/// # Returns
/// 已验证的 worker、共享识别模型路径和 worker 的打包状态。
#[cfg(not(target_os = "android"))]
fn desktop_ocr_resources(app: &tauri::AppHandle) -> Result<DesktopOcrResources, String> {
    // `tauri dev` compiles with debug assertions. It must exercise the editable Python source
    // and current uv environment rather than a stale PyInstaller executable left in the
    // resource directory by a previous release build.
    #[cfg(debug_assertions)]
    {
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("ocr-worker")
            .join("ocr_worker.py");
        let model = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(RECOGNITION_MODEL_RESOURCE);
        if script.is_file() && model.is_file() {
            return Ok(DesktopOcrResources {
                worker: script,
                model,
                bundled: false,
            });
        }
    }

    let executable = if cfg!(target_os = "windows") {
        "ocr-worker/ocr_worker.exe"
    } else {
        "ocr-worker/ocr_worker"
    };
    let worker = app
        .path()
        .resolve(executable, BaseDirectory::Resource)
        .map_err(|error| format!("无法定位 OCR worker：{error}"))?;
    let model = app
        .path()
        .resolve(RECOGNITION_MODEL_RESOURCE, BaseDirectory::Resource)
        .map_err(|error| format!("无法定位 OCR 识别模型：{error}"))?;
    if worker.is_file() && model.is_file() {
        return Ok(DesktopOcrResources {
            worker,
            model,
            bundled: true,
        });
    }
    Err("OCR worker 或共享识别模型未随应用安装。请重新安装桌面应用".to_string())
}

/// 对已持久化的来源图片执行图片识别，返回识别结果。
///
/// # 错误
/// 当 Python、ONNX Runtime 或 worker 不可用时返回可操作的安装提示。此操作失败时不删除或覆盖
/// 已保存的来源图片。
pub(crate) async fn recognize_image(
    app: &tauri::AppHandle,
    source: PathBuf,
    segments_dir: Option<PathBuf>,
) -> Result<String, String> {
    #[cfg(target_os = "android")]
    {
        return recognize_android_image(app, source, segments_dir).await;
    }

    #[cfg(not(target_os = "android"))]
    {
        let resources = desktop_ocr_resources(app)?;
        let result = run_worker(
            resources.worker,
            resources.model,
            resources.bundled,
            source,
            segments_dir,
        )
        .await?;
        finish_worker_result(result)
    }
}

/// 在阻塞线程中启动已验证的 OCR worker。
///
/// 已打包的可执行文件直接运行；开发资源中的 Python 脚本固定通过其同目录的 `uv`
/// 项目运行。输入图片和可选的分段目录均来自原生下载路径，不接受渲染进程路径；
/// 识别文本通过 worker 标准输出返回。
///
/// # 错误
/// worker 不存在、开发项目目录无效、进程不能启动或等待任务异常时返回错误。
#[cfg(not(target_os = "android"))]
async fn run_worker(
    worker: PathBuf,
    model: PathBuf,
    bundled: bool,
    source: PathBuf,
    segments_dir: Option<PathBuf>,
) -> Result<std::process::Output, String> {
    tokio::task::spawn_blocking(move || {
        let mut command = if bundled {
            Command::new(&worker)
        } else {
            let project_directory = worker
                .parent()
                .ok_or_else(|| "OCR worker 项目目录无效".to_string())?;
            let mut command = Command::new("uv");
            command
                .arg("run")
                .arg("--project")
                .arg(project_directory)
                .arg("python")
                .arg(&worker);
            command
        };
        command
            .arg("--input")
            .arg(source)
            .arg("--model-path")
            .arg(model);
        if let Some(segments_dir) = segments_dir {
            command.arg("--segments-dir").arg(segments_dir);
        }
        command
            .arg("--workers")
            .arg("1")
            .output()
            .map_err(|error| {
                if bundled {
                    format!("无法启动随包 OCR worker：{error}")
                } else {
                    format!(
                        "无法启动 uv OCR 环境：{error}。请先运行 uv sync --project src-tauri/ocr-worker"
                    )
                }
            })
    })
    .await
    .map_err(|error| format!("OCR worker 意外退出：{error}"))?
}

/// 将 worker 标准输出转换为 OCR 文本，并保留来源图片。
///
/// # 错误
/// worker 失败、未返回 UTF-8 文本或输出仅含空白时返回可重试错误。
#[cfg(not(target_os = "android"))]
fn finish_worker_result(result: std::process::Output) -> Result<String, String> {
    if !result.status.success() {
        let detail = worker_error_detail(&result.stderr);
        let status = result
            .status
            .code()
            .map(|code| format!("退出码 {code}"))
            .unwrap_or_else(|| "被系统终止".to_string());
        return Err(format!(
            "本地 OCR 失败（{status}）：{detail}。原始图片已保留，可稍后重试"
        ));
    }
    let text = String::from_utf8(result.stdout)
        .map_err(|error| format!("OCR 返回的文本不是 UTF-8：{error}"))?;
    if text.trim().is_empty() {
        return Err("OCR 未识别到可用文字；原始图片已保留，可稍后重试".to_string());
    }
    Ok(text)
}

/// 从 OCR worker 的标准错误中提取一条可展示的诊断信息。
///
/// worker 可能在异常输出后追加空行；不能直接使用最后一行，否则会把真实错误截断为
/// 空字符串。诊断同时限制长度，避免 Python traceback 或第三方库输出撑满下载任务提示。
#[cfg(not(target_os = "android"))]
fn worker_error_detail(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .unwrap_or("worker 未返回有效错误信息")
        .chars()
        .take(300)
        .collect()
}

/// 将书籍内路径转换为可写入 Markdown 的相对路径。
///
/// # 错误
/// `path` 不属于 `directory` 时拒绝生成可能越界的书籍内相对路径。
pub(crate) fn relative_book_path(path: &Path, directory: &Path) -> Result<String, String> {
    path.strip_prefix(directory)
        .map_err(|_| "路径不属于当前书籍目录".to_string())
        .map(|value| value.to_string_lossy().replace('\\', "/"))
}
