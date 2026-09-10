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
if ($LASTEXITCODE -ne 0) {
  throw "Unable to prepare the OCR worker Python environment"
}
$rapidOcrRoot = uv run --project (Split-Path -Parent $workerProject) python -c "from pathlib import Path; import rapidocr; print(Path(rapidocr.__file__).parent)"
if ($LASTEXITCODE -ne 0) {
  throw "Unable to locate RapidOCR in the OCR worker Python environment"
}
if ([string]::IsNullOrWhiteSpace($rapidOcrRoot)) {
  throw "RapidOCR did not report its installation directory"
}
if (-not (Test-Path -LiteralPath (Join-Path $rapidOcrRoot "config.yaml"))) {
  throw "RapidOCR configuration is missing from the worker environment"
}
$pythonArgs = @(
  "run", "--project", (Split-Path -Parent $workerProject), "pyinstaller",
  "--noconfirm", "--clean", "--onefile", "--name", "ocr_worker",
  "--distpath", $output, "--workpath", (Join-Path $env:TEMP "sf-novel-flow-ocr-build"),
  "--specpath", (Join-Path $env:TEMP "sf-novel-flow-ocr-spec"),
  "--collect-binaries", "onnxruntime",
  "--hidden-import", "rapidocr.inference_engine.onnxruntime",
  "--add-data", "$(Join-Path $rapidOcrRoot 'config.yaml');rapidocr",
  "--add-data", "$(Join-Path $rapidOcrRoot 'default_models.yaml');rapidocr",
  $worker
)
uv @pythonArgs
if ($LASTEXITCODE -ne 0) {
  throw "PyInstaller failed to build the OCR worker"
}
if (-not (Test-Path -LiteralPath (Join-Path $output "ocr_worker.exe"))) {
  throw "PyInstaller did not create src-tauri/ocr-worker/ocr_worker.exe"
}
$archiveContents = uv run --project (Split-Path -Parent $workerProject) pyi-archive_viewer -l (Join-Path $output "ocr_worker.exe")
if ($LASTEXITCODE -ne 0) {
  throw "Unable to inspect the packaged OCR worker"
}
$embeddedModels = $archiveContents | Select-String -Pattern '\.onnx$|\.onnx\s'
if ($embeddedModels) {
  throw "The worker must not embed OCR models: $embeddedModels"
}
