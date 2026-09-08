param(
  [string]$Python = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$worker = Join-Path $root "src-tauri\ocr-worker\ocr_worker.py"
$output = Join-Path $root "src-tauri\ocr-worker"
$workerProject = Join-Path $root "src-tauri\ocr-worker\pyproject.toml"
if (-not (Test-Path -LiteralPath $workerProject)) {
  throw "OCR worker uv project is missing: $workerProject"
}
if ($Python) {
  $env:UV_PYTHON = $Python
}
uv sync --project (Split-Path -Parent $workerProject) --group build
$pythonArgs = @(
  "run", "--project", (Split-Path -Parent $workerProject), "pyinstaller",
  "--noconfirm", "--clean", "--onefile", "--name", "ocr_worker",
  "--distpath", $output, "--workpath", (Join-Path $env:TEMP "sf-novel-flow-ocr-build"),
  "--specpath", (Join-Path $env:TEMP "sf-novel-flow-ocr-spec"),
  "--collect-all", "rapidocr_onnxruntime", "--collect-all", "onnxruntime",
  $worker
)
uv @pythonArgs
if (-not (Test-Path -LiteralPath (Join-Path $output "ocr_worker.exe"))) {
  throw "PyInstaller did not create src-tauri/ocr-worker/ocr_worker.exe"
}
