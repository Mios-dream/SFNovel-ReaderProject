//! 桌面端常驻 Python OCR worker。

use crate::adapters::ocr::format_ocr_novel_text;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use tauri::path::BaseDirectory;
use tauri::Manager;

/// 随应用发布的 OCR 识别模型资源路径。
const RECOGNITION_MODEL_RESOURCE: &str = "resources/ocr-models/PP-OCRv6_rec_small.onnx";

/// 解析后的 OCR worker 与模型资源位置。
struct DesktopOcrResources {
    /// worker 可执行文件或开发环境脚本路径。
    worker: PathBuf,
    /// 识别模型路径。
    model: PathBuf,
    /// 是否使用随应用安装的二进制 worker。
    bundled: bool,
}

/// 桌面端整个应用共享的常驻 OCR worker。
pub(crate) struct DesktopOcrState {
    /// 可复用的常驻 worker；空闲时为 `None`。
    worker: Arc<Mutex<Option<PersistentOcrWorker>>>,
}

impl Default for DesktopOcrState {
    fn default() -> Self {
        Self {
            worker: Arc::new(Mutex::new(None)),
        }
    }
}

impl Drop for DesktopOcrState {
    /// 应用退出时终止常驻 OCR 子进程。
    fn drop(&mut self) {
        if let Ok(mut worker) = self.worker.lock() {
            if let Some(worker) = worker.take() {
                worker.stop();
            }
        }
    }
}

/// 常驻 OCR 子进程及其可复用的输入输出通道。
struct PersistentOcrWorker {
    /// worker 可执行文件或开发环境脚本路径。
    worker: PathBuf,
    /// 识别模型路径。
    model: PathBuf,
    /// 是否使用随应用安装的二进制 worker。
    bundled: bool,
    /// 子进程句柄。
    child: Child,
    /// 子进程标准输入。
    input: BufWriter<ChildStdin>,
    /// 子进程标准输出。
    output: BufReader<ChildStdout>,
}

/// OCR worker 请求失败的类型，用于决定是否需要重启进程。
enum WorkerRequestError {
    /// 图片内容识别失败，worker 本身仍可继续使用。
    Ocr(String),
    /// 输入输出或协议失效，通常需要重建 worker。
    Transport(String),
}

impl PersistentOcrWorker {
    /// 启动常驻 OCR worker 并接管其标准输入输出。
    ///
    /// # 参数
    /// - `worker`：worker 可执行文件或开发脚本路径。
    /// - `model`：识别模型路径。
    /// - `bundled`：为 `true` 时直接执行 worker，否则通过 `uv` 启动。
    ///
    /// # 错误
    /// 进程启动失败、标准通道缺失时返回错误消息。
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
        let mut child = command
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

    /// 判断当前 worker 是否匹配指定资源并可直接复用。
    ///
    /// # 参数
    /// - `worker`：期望的 worker 路径。
    /// - `model`：期望的模型路径。
    /// - `bundled`：期望的运行模式。
    fn matches(&self, worker: &Path, model: &Path, bundled: bool) -> bool {
        self.bundled == bundled && self.worker == worker && self.model == model
    }

    /// 检查子进程是否仍在运行。
    fn is_running(&mut self) -> bool {
        self.child
            .try_wait()
            .map(|status| status.is_none())
            .unwrap_or(false)
    }

    /// 发送一次 OCR 请求并读取单行 JSON 响应。
    ///
    /// # 参数
    /// - `source`：待识别图片路径。
    /// - `segments_dir`：可选的识别分片输出目录。
    ///
    /// # 错误
    /// 请求编码、进程通信或 OCR 识别失败时返回对应错误类型。
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

    /// 终止并等待 OCR 子进程退出。
    fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// 定位开发环境或安装包中的 OCR worker 与模型。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 资源路径无法解析或资源文件缺失时返回错误消息。
fn desktop_ocr_resources(app: &tauri::AppHandle) -> Result<DesktopOcrResources, String> {
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

/// 识别图片并把结果格式化为小说正文。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `source`：待识别图片路径。
/// - `segments_dir`：可选的识别分片输出目录。
///
/// # 错误
/// 资源定位、worker 启动或 OCR 失败时返回错误消息。
pub(super) async fn recognize(
    app: &tauri::AppHandle,
    source: PathBuf,
    segments_dir: Option<PathBuf>,
) -> Result<String, String> {
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

/// 在阻塞线程中复用或重建 worker，并执行 OCR 请求。
///
/// # 参数
/// - `worker_state`：应用共享的常驻 worker 状态。
/// - `worker`：worker 路径。
/// - `model`：模型路径。
/// - `bundled`：是否为随包二进制。
/// - `source`：待识别图片路径。
/// - `segments_dir`：可选的识别分片输出目录。
///
/// # 错误
/// worker 状态不可用、启动失败或请求失败时返回错误消息。
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
