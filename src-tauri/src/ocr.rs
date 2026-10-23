//! 桌面端本地 OCR worker 桥接。
//!
//! 图片获取、会话和文件落盘始终留在 Rust 原生层。本模块只把已保存的本地图片路径
//! 交给独立 Python 进程，避免向 worker、日志或渲染进程传递 Web Cookie。

#[cfg(not(target_os = "android"))]
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
#[cfg(not(target_os = "android"))]
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
#[cfg(not(target_os = "android"))]
use std::sync::{Arc, Mutex};
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

/// 桌面端整个应用共享的常驻 OCR worker。
///
/// worker 内部只创建一个识别引擎，所有请求在这里串行排队。引擎空闲三分钟后由
/// Python worker 释放；worker 进程本身继续保留，以便下一次识别无需重新启动 Python。
#[cfg(not(target_os = "android"))]
pub(crate) struct DesktopOcrState {
    worker: Arc<Mutex<Option<PersistentOcrWorker>>>,
}

#[cfg(not(target_os = "android"))]
impl Default for DesktopOcrState {
    fn default() -> Self {
        Self {
            worker: Arc::new(Mutex::new(None)),
        }
    }
}

#[cfg(not(target_os = "android"))]
impl Drop for DesktopOcrState {
    fn drop(&mut self) {
        if let Ok(mut worker) = self.worker.lock() {
            if let Some(worker) = worker.take() {
                worker.stop();
            }
        }
    }
}

#[cfg(not(target_os = "android"))]
struct PersistentOcrWorker {
    worker: PathBuf,
    model: PathBuf,
    bundled: bool,
    child: Child,
    input: BufWriter<ChildStdin>,
    output: BufReader<ChildStdout>,
}

#[cfg(not(target_os = "android"))]
enum WorkerRequestError {
    Ocr(String),
    Transport(String),
}

#[cfg(not(target_os = "android"))]
impl PersistentOcrWorker {
    fn start(worker: PathBuf, model: PathBuf, bundled: bool) -> Result<Self, String> {
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
        let child = command
            .arg("--model-path")
            .arg(&model)
            .arg("--server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                if bundled {
                    format!("无法启动随包 OCR worker：{error}")
                } else {
                    format!(
                        "无法启动 uv OCR 环境：{error}。请先运行 uv sync --project src-tauri/ocr-worker"
                    )
                }
            })?;
        let mut child = child;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| "OCR worker 未提供输入通道".to_string())?;
        let output = child
            .stdout
            .take()
            .ok_or_else(|| "OCR worker 未提供输出通道".to_string())?;
        Ok(Self {
            worker,
            model,
            bundled,
            child,
            input: BufWriter::new(input),
            output: BufReader::new(output),
        })
    }

    fn matches(&self, worker: &Path, model: &Path, bundled: bool) -> bool {
        self.bundled == bundled && self.worker == worker && self.model == model
    }

    fn is_running(&mut self) -> bool {
        self.child
            .try_wait()
            .map(|status| status.is_none())
            .unwrap_or(false)
    }

    fn request(
        &mut self,
        source: &Path,
        segments_dir: Option<&Path>,
    ) -> Result<String, WorkerRequestError> {
        let request = serde_json::json!({
            "input": source.to_string_lossy(),
            "segments_dir": segments_dir.map(|path| path.to_string_lossy()),
        });
        let payload = serde_json::to_vec(&request).map_err(|error| {
            WorkerRequestError::Transport(format!("无法编码 OCR 请求：{error}"))
        })?;
        self.input.write_all(&payload).map_err(|error| {
            WorkerRequestError::Transport(format!("无法写入 OCR worker：{error}"))
        })?;
        self.input.write_all(b"\n").map_err(|error| {
            WorkerRequestError::Transport(format!("无法写入 OCR worker：{error}"))
        })?;
        self.input.flush().map_err(|error| {
            WorkerRequestError::Transport(format!("无法刷新 OCR worker 请求：{error}"))
        })?;

        let mut line = String::new();
        let bytes = self.output.read_line(&mut line).map_err(|error| {
            WorkerRequestError::Transport(format!("无法读取 OCR worker：{error}"))
        })?;
        if bytes == 0 {
            return Err(WorkerRequestError::Transport(
                "OCR worker 意外退出".to_string(),
            ));
        }
        let response: serde_json::Value = serde_json::from_str(&line).map_err(|error| {
            WorkerRequestError::Transport(format!("OCR worker 返回了无效响应：{error}"))
        })?;
        if response.get("ok").and_then(serde_json::Value::as_bool) != Some(true) {
            let detail = response
                .get("error")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("worker 未返回有效错误信息");
            return Err(WorkerRequestError::Ocr(format!("本地 OCR 失败：{detail}")));
        }
        let text = response
            .get("text")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| WorkerRequestError::Transport("OCR worker 响应缺少文本".to_string()))?;
        if text.trim().is_empty() {
            return Err(WorkerRequestError::Ocr(
                "OCR 未识别到可用文字；原始图片已保留，可稍后重试".to_string(),
            ));
        }
        Ok(text.to_string())
    }

    fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
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
        let worker_state = app.state::<DesktopOcrState>().worker.clone();
        let result = run_worker(
            worker_state,
            resources.worker,
            resources.model,
            resources.bundled,
            source,
            segments_dir,
        )
        .await?;
        Ok(format_ocr_novel_text(&result))
    }
}

/// 在阻塞线程中取得并复用已验证的常驻 OCR worker。
///
/// 已打包的可执行文件直接运行；开发资源中的 Python 脚本固定通过其同目录的 `uv`
/// 项目运行。所有调用共享一个 worker 和一个 OCR 引擎，输入图片和可选的分段目录均来自
/// 原生下载路径，不接受渲染进程路径。
///
/// # 错误
/// worker 不存在、开发项目目录无效、进程不能启动、协议异常或 OCR 失败时返回错误。
#[cfg(not(target_os = "android"))]
async fn run_worker(
    worker_state: Arc<Mutex<Option<PersistentOcrWorker>>>,
    worker: PathBuf,
    model: PathBuf,
    bundled: bool,
    source: PathBuf,
    segments_dir: Option<PathBuf>,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let mut current = worker_state
            .lock()
            .map_err(|_| "OCR worker 状态不可用".to_string())?;
        let needs_restart = current
            .as_mut()
            .map(|worker_process| {
                !worker_process.matches(&worker, &model, bundled) || !worker_process.is_running()
            })
            .unwrap_or(true);
        if needs_restart {
            if let Some(worker_process) = current.take() {
                worker_process.stop();
            }
            *current = Some(PersistentOcrWorker::start(
                worker.clone(),
                model.clone(),
                bundled,
            )?);
        }

        let request_result = current
            .as_mut()
            .ok_or_else(|| "OCR worker 未初始化".to_string())?
            .request(&source, segments_dir.as_deref());
        match request_result {
            Ok(text) => Ok(text),
            Err(WorkerRequestError::Ocr(error)) => Err(error),
            Err(WorkerRequestError::Transport(error)) => {
                if let Some(worker_process) = current.take() {
                    worker_process.stop();
                }
                Err(error)
            }
        }
    })
    .await
    .map_err(|error| format!("OCR worker 意外退出：{error}"))?
}

/// 将逐行 OCR 结果整理成按句号分段的小说正文。
///
/// 识别器按图片条带输出，每个条带的字数不固定，直接保留其换行会让单字或短语单独成行。
/// 因此先移除条带边界的换行和空白，再仅在中文句号后保留段落换行；末尾不额外增加空行。
pub(crate) fn format_ocr_novel_text(text: &str) -> String {
    text.split(['\r', '\n'])
        .map(str::trim)
        .collect::<String>()
        .replace('。', "。\n")
        .trim_end()
        .to_string()
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
